// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `editship.c` / `editship.h`.

use crate::Game;
use crate::file;
use crate::sprite::{Sprite2Array, sprite2s_valid};
use serde::{Deserialize, Serialize};

/// `JE_ShipsType`.
pub type JeShipsType = [u8; 154];

/// `extraCryptKey`.
pub const EXTRA_CRYPT_KEY: [u8; 10] = [58, 23, 16, 192, 254, 82, 113, 147, 62, 99];

/// `ENOMEM` (12 on every platform the port supports).
const ENOMEM: i32 = 12;

/// `EXTRA_SHIPS_FILE_SIZE`: the ship data before its four checksums.
pub const EXTRA_SHIPS_FILE_SIZE: usize = 154 - 4;

/// Mutable globals of `editship.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub extra_avail: bool,
    #[serde(with = "crate::serde_big")]
    pub extra_ships: JeShipsType,
    pub extra_shapes: Sprite2Array,
}

impl Default for State {
    fn default() -> Self {
        State {
            extra_avail: false,
            extra_ships: [0; 154],
            extra_shapes: Sprite2Array::default(),
        }
    }
}

/// Loads the user's custom ships (`newsh$.shp`: sprite sheet, then the
/// encrypted ship table) if the file exists.
#[doc(alias = "loadExtraShapes")]
pub fn load_extra_shapes(g: &mut Game) {
    debug_assert!(g.editship.extra_shapes.data.is_none());

    let mut file = file::user_file_open(g, "newsh$.shp", "rb");
    if file.error {
        return;
    }

    let file_length = file::file_get_length(&mut file);
    // D-UB-EDITSHIP-1: a failed length (-1, an I/O error) counts as too
    // short; upstream casts it to a huge size and allocates it.
    let ships_size = g.editship.extra_ships.len();
    if let Ok(len) = usize::try_from(file_length)
        && len >= ships_size
    {
        let size = len - ships_size;
        let e = &mut g.editship;
        e.extra_shapes.size = size;
        // D-UB-EDITSHIP-1: upstream does not check `malloc` (a directory's
        // length can be huge); an allocation failure becomes a read error
        // with ENOMEM and the sheet stays unloaded.
        let mut data = Vec::new();
        if data.try_reserve_exact(size).is_ok() {
            data.resize(size, 0);
            file::file_read_exactly(&mut file, &mut data);
            e.extra_shapes.data = Some(data);
        } else {
            file.error = true;
            file.errnum = ENOMEM;
        }
        file::file_read_exactly(&mut file, &mut e.extra_ships);

        e.extra_avail = !file.error;

        e.extra_avail &= decrypt_extra_ships_data(&mut e.extra_ships);

        // D-UB-EDITSHIP-1: a sheet whose sprites would be drawn from outside
        // its data makes the custom ships unavailable.
        e.extra_avail &= sprite2s_valid(&e.extra_shapes);
    }

    file::file_close(&mut file);
}

/// Decrypts the custom ship table in place; returns whether its four
/// checksums match.
#[doc(alias = "decryptExtraShipsData")]
pub fn decrypt_extra_ships_data(data: &mut JeShipsType) -> bool {
    for i in (0..EXTRA_SHIPS_FILE_SIZE).rev() {
        data[i] ^= EXTRA_CRYPT_KEY[(i + 1) % 10];
        if i > 0 {
            data[i] ^= data[i - 1];
        }
    }

    let body = &data[..EXTRA_SHIPS_FILE_SIZE];

    let y = body.iter().fold(0u8, |y, &d| y.wrapping_add(d));
    if data[EXTRA_SHIPS_FILE_SIZE] != y {
        return false;
    }

    let y = body.iter().fold(0u8, |y, &d| y.wrapping_sub(d));
    if data[EXTRA_SHIPS_FILE_SIZE + 1] != y {
        return false;
    }

    let y = body
        .iter()
        .fold(1u8, |y, &d| y.wrapping_mul(d).wrapping_add(1));
    if data[EXTRA_SHIPS_FILE_SIZE + 2] != y {
        return false;
    }

    let y = body.iter().fold(0u8, |y, &d| y ^ d);
    if data[EXTRA_SHIPS_FILE_SIZE + 3] != y {
        return false;
    }

    true
}
