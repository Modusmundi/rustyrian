// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `helptext.c` / `helptext.h`: the encrypted text table
//! `tyrian.hdt` and the help-box text wrapper.

use crate::Game;
use crate::file::{self, File};
use crate::fonthand::{self, c_str};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::log_fatal;
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// `MENU_MAX`.
pub const MENU_MAX: usize = 14;
/// `DESTRUCT_MODES`.
pub const DESTRUCT_MODES: usize = 5;
/// `HELPTEXT_MISCTEXT_COUNT`.
pub const HELPTEXT_MISCTEXT_COUNT: usize = 68;
/// `HELPTEXT_MISCTEXTB_COUNT`.
pub const HELPTEXT_MISCTEXTB_COUNT: usize = 5;
/// `HELPTEXT_MISCTEXTB_SIZE`.
pub const HELPTEXT_MISCTEXTB_SIZE: usize = 11;
/// `HELPTEXT_MENUTEXT_SIZE`.
pub const HELPTEXT_MENUTEXT_SIZE: usize = 21;
/// `HELPTEXT_MAINMENUHELP_COUNT`.
pub const HELPTEXT_MAINMENUHELP_COUNT: usize = 34;
/// `HELPTEXT_NETWORKTEXT_COUNT`.
pub const HELPTEXT_NETWORKTEXT_COUNT: usize = 4;
/// `HELPTEXT_NETWORKTEXT_SIZE`.
pub const HELPTEXT_NETWORKTEXT_SIZE: usize = 22;
/// `HELPTEXT_SUPERSHIPS_COUNT`.
pub const HELPTEXT_SUPERSHIPS_COUNT: usize = 11;
/// `HELPTEXT_SPECIALNAME_COUNT`.
pub const HELPTEXT_SPECIALNAME_COUNT: usize = 9;
/// `HELPTEXT_SHIPINFO_COUNT`.
pub const HELPTEXT_SHIPINFO_COUNT: usize = 13;

/// `menuHelp`: `[1..maxmenu, 1..11]`.
pub const MENU_HELP: [[u8; 11]; MENU_MAX] = [
    [1, 34, 2, 3, 4, 5, 0, 0, 0, 0, 0],
    [6, 7, 8, 9, 10, 11, 11, 12, 0, 0, 0],
    [13, 14, 15, 15, 16, 17, 12, 0, 0, 0, 0],
    [0; 11],
    [0; 11],
    [0; 11],
    [0; 11],
    [0; 11],
    [0; 11],
    [4, 30, 30, 3, 5, 0, 0, 0, 0, 0, 0],
    [0; 11],
    [16, 17, 15, 15, 12, 0, 0, 0, 0, 0, 0],
    [31, 31, 31, 31, 32, 12, 0, 0, 0, 0, 0],
    [4, 34, 3, 5, 0, 0, 0, 0, 0, 0, 0],
];

/// `shipInfo`'s type, `char[13][2][256]`.
pub type ShipInfo = [[[u8; 256]; 2]; HELPTEXT_SHIPINFO_COUNT];

/// Mutable globals of `helptext.c` (rule R3). Each is an array of
/// NUL-terminated C strings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `[1..39] of string [230]`.
    #[serde(with = "crate::serde_big::d2_boxed")]
    pub help_txt: Box<[[u8; 231]; 39]>,
    /// `[1..21] of string [15]`.
    pub p_name: [[u8; 16]; 21],
    /// `[1..68] of string [41]`.
    #[serde(with = "crate::serde_big::d2")]
    pub misc_text: [[u8; 42]; HELPTEXT_MISCTEXT_COUNT],
    /// `[1..5] of string [10]`.
    pub misc_text_b: [[u8; HELPTEXT_MISCTEXTB_SIZE]; HELPTEXT_MISCTEXTB_COUNT],
    /// `[1..8] of string [17]`.
    pub key_name: [[u8; 18]; 8],
    /// `[1..7] of string [20]`.
    pub menu_text: [[u8; HELPTEXT_MENUTEXT_SIZE]; 7],
    /// `[1..9] of string [30]`.
    pub outputs: [[u8; 31]; 9],
    /// `[1..6] of string [20]`.
    pub topic_name: [[u8; 21]; 6],
    /// `[1..34] of string [65]`.
    #[serde(with = "crate::serde_big::d2")]
    pub main_menu_help: [[u8; 66]; HELPTEXT_MAINMENUHELP_COUNT],
    /// `[1..6] of string [20]`.
    pub in_game_text: [[u8; 21]; 6],
    /// `[1..6] of string [12]`.
    pub detail_level: [[u8; 13]; 6],
    /// `[1..5] of string [12]`.
    pub game_speed_text: [[u8; 13]; 5],
    /// `[1..3] of string [12]`.
    pub input_devices: [[u8; 13]; 3],
    /// `[1..4] of string [20]`.
    pub network_text: [[u8; HELPTEXT_NETWORKTEXT_SIZE]; HELPTEXT_NETWORKTEXT_COUNT],
    /// `[0..9] of string [20]`.
    pub difficulty_name_b: [[u8; 21]; 11],
    /// `[1..5] of string [20]`.
    pub joy_button_names: [[u8; 21]; 5],
    /// `[0..10] of string [25]`.
    pub super_ships: [[u8; 26]; HELPTEXT_SUPERSHIPS_COUNT],
    /// `[1..9] of string [9]`.
    pub special_name: [[u8; 10]; HELPTEXT_SPECIALNAME_COUNT],
    /// `[1..25] of string [21]`.
    pub destruct_help: [[u8; 22]; 25],
    /// `[1..17] of string [16]`.
    pub weapon_names: [[u8; 17]; 17],
    /// `[1..destructmodes] of string [12]`.
    pub destruct_mode_name: [[u8; 13]; DESTRUCT_MODES],
    /// `[1..13, 1..2] of string`.
    #[serde(with = "ship_info_serde")]
    pub ship_info: Box<ShipInfo>,
    /// `[0..14, 1..11] of string [17]`.
    pub menu_int: [[[u8; 18]; 11]; MENU_MAX + 1],
}

impl Default for State {
    fn default() -> Self {
        State {
            help_txt: vec![[0; 231]; 39]
                .into_boxed_slice()
                .try_into()
                .expect("39 rows"),
            p_name: [[0; 16]; 21],
            misc_text: [[0; 42]; HELPTEXT_MISCTEXT_COUNT],
            misc_text_b: [[0; HELPTEXT_MISCTEXTB_SIZE]; HELPTEXT_MISCTEXTB_COUNT],
            key_name: [[0; 18]; 8],
            menu_text: [[0; HELPTEXT_MENUTEXT_SIZE]; 7],
            outputs: [[0; 31]; 9],
            topic_name: [[0; 21]; 6],
            main_menu_help: [[0; 66]; HELPTEXT_MAINMENUHELP_COUNT],
            in_game_text: [[0; 21]; 6],
            detail_level: [[0; 13]; 6],
            game_speed_text: [[0; 13]; 5],
            input_devices: [[0; 13]; 3],
            network_text: [[0; HELPTEXT_NETWORKTEXT_SIZE]; HELPTEXT_NETWORKTEXT_COUNT],
            difficulty_name_b: [[0; 21]; 11],
            joy_button_names: [[0; 21]; 5],
            super_ships: [[0; 26]; HELPTEXT_SUPERSHIPS_COUNT],
            special_name: [[0; 10]; HELPTEXT_SPECIALNAME_COUNT],
            destruct_help: [[0; 22]; 25],
            weapon_names: [[0; 17]; 17],
            destruct_mode_name: [[0; 13]; DESTRUCT_MODES],
            ship_info: vec![[[0; 256]; 2]; HELPTEXT_SHIPINFO_COUNT]
                .into_boxed_slice()
                .try_into()
                .expect("13 rows"),
            menu_int: [[[0; 18]; 11]; MENU_MAX + 1],
        }
    }
}

/// Serde for `ship_info` as a flat byte sequence (serde's array impls stop
/// at 32 elements).
mod ship_info_serde {
    use super::{HELPTEXT_SHIPINFO_COUNT, ShipInfo};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(a: &ShipInfo, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(a.iter().flatten().flatten())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Box<ShipInfo>, D::Error> {
        let v = Vec::<u8>::deserialize(d)?;
        if v.len() != HELPTEXT_SHIPINFO_COUNT * 2 * 256 {
            return Err(D::Error::invalid_length(v.len(), &"13*2*256 bytes"));
        }
        let mut r: Box<ShipInfo> = vec![[[0u8; 256]; 2]; HELPTEXT_SHIPINFO_COUNT]
            .into_boxed_slice()
            .try_into()
            .map_err(|_| D::Error::custom("length mismatch"))?;
        for (d, s) in r.iter_mut().flatten().zip(v.as_chunks::<256>().0) {
            *d = *s;
        }
        Ok(r)
    }
}

const CRYPT_KEY: [u8; 10] = [204, 129, 63, 255, 71, 19, 25, 62, 1, 99];

/// `decrypt_string(s, len)` with `len = s.len()`.
pub fn decrypt_string(s: &mut [u8]) {
    let len = s.len();
    if len == 0 {
        return;
    }

    let mut i = len - 1;
    loop {
        s[i] ^= CRYPT_KEY[i % CRYPT_KEY.len()];
        if i == 0 {
            break;
        }
        s[i] ^= s[i - 1];
        i -= 1;
    }
}

/// `readEncryptedString(file, dst, size)`; `dst = None` is C's `NULL, 0`
/// (skip a string), otherwise `size = dst.len()`.
#[doc(alias = "readEncryptedString")]
pub fn read_encrypted_string(file: &mut File, dst: Option<&mut [u8]>) {
    let mut buffer = [0u8; 255];

    let len = file::file_read_u8(file);
    file::file_read_exactly(file, &mut buffer[..usize::from(len)]);

    let Some(dst) = dst else {
        return;
    };
    let size = dst.len();
    if size == 0 {
        return;
    }

    decrypt_string(&mut buffer[..usize::from(len)]);

    // C's `assert(len < size)` is compiled out (NDEBUG); external data can
    // break it, so it is not a `debug_assert!` here (rule R10).
    let len = usize::from(len).min(size - 1);

    dst[..len].copy_from_slice(&buffer[..len]);
    dst[len] = 0;
}

/// `menuInt_entries`: strings per `menuInt` menu (`-1`, i.e. `UINT_MAX`, for
/// menus never loaded).
const MENU_INT_ENTRIES: [u32; MENU_MAX + 1] = [
    u32::MAX,
    7,
    9,
    8,
    u32::MAX,
    u32::MAX,
    11,
    u32::MAX,
    u32::MAX,
    u32::MAX,
    6,
    4,
    6,
    7,
    5,
];

/// Reads a block: a skipped header string, `rows` strings, and (unless
/// `trailer` is false) a skipped trailer string.
fn read_block<const N: usize>(file: &mut File, rows: &mut [[u8; N]], trailer: bool) {
    read_encrypted_string(file, None);
    for row in rows.iter_mut() {
        read_encrypted_string(file, Some(row));
    }
    if trailer {
        read_encrypted_string(file, None);
    }
}

#[doc(alias = "JE_loadHelpText")]
pub fn je_load_help_text(g: &mut Game) -> Flow {
    let filename = "tyrian.hdt";

    let mut file = file::data_file_open(g, filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let _ = file::file_read_u32(&mut file); // Episode 1-3 item data position

    let menu = |i: usize| MENU_INT_ENTRIES[i] as usize;
    let h = &mut g.helptext;
    let m = &mut g.menus;
    let f = &mut file;

    /*Online Help*/
    read_block(f, &mut h.help_txt[..], true);
    /*Planet names*/
    read_block(f, &mut h.p_name, true);
    /*Miscellaneous text*/
    read_block(f, &mut h.misc_text, true);
    /*Little Miscellaneous text*/
    read_block(f, &mut h.misc_text_b, true);
    /*Key names*/
    read_block(f, &mut h.menu_int[6][..menu(6)], true);
    /*Main Menu*/
    read_block(f, &mut h.menu_text, true);
    /*Event text*/
    read_block(f, &mut h.outputs, true);
    /*Help topics*/
    read_block(f, &mut h.topic_name, true);
    /*Main Menu Help*/
    read_block(f, &mut h.main_menu_help, true);
    /*Menu 1 - Main*/
    read_block(f, &mut h.menu_int[1][..menu(1)], true);
    /*Menu 2 - Items*/
    read_block(f, &mut h.menu_int[2][..menu(2)], true);
    /*Menu 3 - Options*/
    read_block(f, &mut h.menu_int[3][..menu(3)], true);
    /*InGame Menu*/
    read_block(f, &mut h.in_game_text, true);
    /*Detail Level*/
    read_block(f, &mut h.detail_level, true);
    /*Game speed text*/
    read_block(f, &mut h.game_speed_text, true);
    // episode names
    read_block(f, &mut m.episode_name, true);
    // difficulty names
    read_block(f, &mut m.difficulty_name, true);
    // gameplay mode names
    read_block(f, &mut m.gameplay_name, true);
    /*Menu 10 - 2Player Main*/
    read_block(f, &mut h.menu_int[10][..menu(10)], true);
    /*Input Devices*/
    read_block(f, &mut h.input_devices, true);
    /*Network text*/
    read_block(f, &mut h.network_text, true);
    /*Menu 11 - 2Player Network*/
    read_block(f, &mut h.menu_int[11][..menu(11)], true);
    /*HighScore Difficulty Names*/
    read_block(f, &mut h.difficulty_name_b, true);
    /*Menu 12 - Network Options*/
    read_block(f, &mut h.menu_int[12][..menu(12)], true);
    /*Menu 13 - Joystick*/
    read_block(f, &mut h.menu_int[13][..menu(13)], true);
    /*Joystick Button Assignments*/
    read_block(f, &mut h.joy_button_names, true);
    /*SuperShips - For Super Arcade Mode*/
    read_block(f, &mut h.super_ships, true);
    /*SuperShips - For Super Arcade Mode*/
    read_block(f, &mut h.special_name, true);
    /*Secret DESTRUCT game*/
    read_block(f, &mut h.destruct_help, true);
    /*Secret DESTRUCT weapons*/
    read_block(f, &mut h.weapon_names, true);
    /*Secret DESTRUCT modes*/
    read_block(f, &mut h.destruct_mode_name, true);
    /*NEW: Ship Info*/
    read_encrypted_string(f, None);
    for info in h.ship_info.iter_mut() {
        read_encrypted_string(f, Some(&mut info[0]));
        read_encrypted_string(f, Some(&mut info[1]));
    }
    read_encrypted_string(f, None);
    /*Menu 12 - Network Options*/
    read_block(f, &mut h.menu_int[14][..menu(14)], false);

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file::file_close(&mut file);
    Ok(())
}

/// `SDL_strlcpy(dst, src, maxlen)`: copies `src` up to its NUL (or its end),
/// at most `maxlen - 1` bytes, and terminates; `maxlen` 0 leaves `dst` as is.
fn strlcpy(dst: &mut [u8], src: &[u8], maxlen: usize) {
    if maxlen == 0 {
        return;
    }
    let n = c_str(src).len().min(maxlen - 1);
    dst[..n].copy_from_slice(&src[..n]);
    dst[n] = 0;
}

/// Draws `message` word-wrapped to `box_width` characters per line, lines
/// `vertical_height` pixels apart. `message` is the C buffer holding the
/// string: like upstream, a one-character message reads past its NUL.
///
/// Reads outside `message` (only reachable from a buffer too small for the
/// upstream scan, where C reads outside the string's array) yield NUL, and a
/// string of 256 or more bytes (on which upstream never terminates) draws
/// nothing (divergence `D-UB-HELP-1`); every caller's buffer is shorter.
#[doc(alias = "JE_helpBox")]
#[allow(clippy::too_many_arguments)]
pub fn je_help_box(
    g: &mut Game,
    screen: ScreenId,
    x: i32,
    mut y: i32,
    message: &[u8],
    box_width: u8,
    vertical_height: u8,
    color: u8,
    brightness: u8,
    shade_type: u8,
) {
    let len = c_str(message).len();
    if len == 0 || len > usize::from(u8::MAX) {
        return;
    }
    let at = |i: i32| -> u8 {
        usize::try_from(i)
            .ok()
            .and_then(|i| message.get(i))
            .copied()
            .unwrap_or(0)
    };
    let from = |i: i32| -> &[u8] {
        usize::try_from(i)
            .ok()
            .and_then(|i| message.get(i..))
            .unwrap_or(&[])
    };
    let (color, brightness, shade_type) = (
        u32::from(color),
        i32::from(brightness),
        u32::from(shade_type),
    );
    // `(unsigned)(pos - startpos)` with both promoted to int
    let span = |pos: u8, startpos: u8| (i32::from(pos) - i32::from(startpos)) as u32;

    let mut substring = [0u8; 256];

    let mut pos: u8 = 1;
    let mut endpos: u8 = 0;
    let mut endstring = false;

    loop {
        let startpos = endpos.wrapping_add(1);

        loop {
            endpos = pos;
            loop {
                pos = pos.wrapping_add(1);
                if usize::from(pos) == len {
                    endstring = true;
                    if span(pos, startpos) < u32::from(box_width) {
                        endpos = pos.wrapping_add(1);
                    }
                }

                if at(i32::from(pos) - 1) == b' ' || endstring {
                    break;
                }
            }

            if span(pos, startpos) > u32::from(box_width) || endstring {
                break;
            }
        }

        let maxlen = (i32::from(endpos) - i32::from(startpos) + 1) as i64 as u64;
        let maxlen = usize::try_from(maxlen)
            .unwrap_or(usize::MAX)
            .min(substring.len());
        strlcpy(&mut substring, from(i32::from(startpos) - 1), maxlen);
        fonthand::je_text_shade(g, screen, x, y, &substring, color, brightness, shade_type);

        y = y.wrapping_add(i32::from(vertical_height));

        if endstring {
            break;
        }
    }

    if i32::from(endpos) != i32::from(pos) + 1 {
        let tail = from(i32::from(endpos));
        fonthand::je_text_shade(g, screen, x, y, tail, color, brightness, shade_type);
    }
}

/// Help text `message_num` (1-based) in a full-shade help box. A number
/// outside the table (C reads outside `helpTxt`) draws nothing
/// (`D-UB-HELP-1`).
#[doc(alias = "JE_HBox")]
#[allow(clippy::too_many_arguments)]
pub fn je_h_box(
    g: &mut Game,
    screen: ScreenId,
    x: i32,
    y: i32,
    message_num: u8,
    box_width: u8,
    vertical_height: u8,
    color: u8,
    brightness: u8,
) {
    let i = i32::from(message_num) - 1;
    let Some(&message) = usize::try_from(i)
        .ok()
        .and_then(|i| g.helptext.help_txt.get(i))
    else {
        return;
    };
    je_help_box(
        g,
        screen,
        x,
        y,
        &message,
        box_width,
        vertical_height,
        color,
        brightness,
        fonthand::FULL_SHADE as u8,
    );
}
