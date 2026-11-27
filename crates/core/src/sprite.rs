// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `sprite.c` / `sprite.h`.

use serde::{Deserialize, Serialize};

pub const FONT_SHAPES: usize = 0;
pub const SMALL_FONT_SHAPES: usize = 1;
pub const TINY_FONT: usize = 2;
pub const PLANET_SHAPES: usize = 3;
pub const FACE_SHAPES: usize = 4;
/// Also contains help shapes.
pub const OPTION_SHAPES: usize = 5;
pub const WEAPON_SHAPES: usize = 6;
/// Used for ending pics.
pub const EXTRA_SHAPES: usize = 7;

pub const SPRITE_TABLES_MAX: usize = 8;
pub const SPRITES_PER_TABLE_MAX: usize = 151;

/// `Sprite`. `data` is the heap block upstream mallocs with `size` bytes
/// (`None` is NULL).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub width: u16,
    pub height: u16,
    pub size: u16,
    pub data: Option<Vec<u8>>,
}

/// `Sprite_array`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteArray {
    pub count: usize,
    #[serde(with = "crate::serde_big")]
    pub sprite: [Sprite; SPRITES_PER_TABLE_MAX],
}

impl Default for SpriteArray {
    fn default() -> Self {
        SpriteArray {
            count: 0,
            sprite: std::array::from_fn(|_| Sprite::default()),
        }
    }
}

/// `Sprite2_array`. `data` is the heap block upstream mallocs with `size`
/// bytes (`None` is NULL).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite2Array {
    pub size: usize,
    pub data: Option<Vec<u8>>,
}

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// FKA shapearray, shapex, shapey, shapesize, shapexist, maxshape.
    pub sprite_table: Box<[SpriteArray; SPRITE_TABLES_MAX]>,
    /// Shop icons and arrows sprite sheet (FKA shapes6).
    pub shop_sprite_sheet: Sprite2Array,
    /// Explosions sprite sheet (FKA shapes6).
    pub explosion_sprite_sheet: Sprite2Array,
    /// Enemy sprite sheet banks (FKA eShapes1..4).
    pub enemy_sprite_sheets: [Sprite2Array; 4],
    /// FKA enemyShapeTables.
    pub enemy_sprite_sheet_ids: [u8; 4],
    /// Destruct sprite sheet (FKA shapes6).
    pub destruct_sprite_sheet: Sprite2Array,
    /// FKA shapesC1.
    pub sprite_sheet8: Sprite2Array,
    /// FKA shapes9.
    pub sprite_sheet9: Sprite2Array,
    /// FKA eShapes6.
    pub sprite_sheet10: Sprite2Array,
    /// FKA eShapes5.
    pub sprite_sheet11: Sprite2Array,
    /// FKA shapesW2.
    pub sprite_sheet12: Sprite2Array,
}

impl Default for State {
    fn default() -> Self {
        State {
            sprite_table: vec![SpriteArray::default(); SPRITE_TABLES_MAX]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("SPRITE_TABLES_MAX entries")),
            shop_sprite_sheet: Sprite2Array::default(),
            explosion_sprite_sheet: Sprite2Array::default(),
            enemy_sprite_sheets: Default::default(),
            enemy_sprite_sheet_ids: [0; 4],
            destruct_sprite_sheet: Sprite2Array::default(),
            sprite_sheet8: Sprite2Array::default(),
            sprite_sheet9: Sprite2Array::default(),
            sprite_sheet10: Sprite2Array::default(),
            sprite_sheet11: Sprite2Array::default(),
            sprite_sheet12: Sprite2Array::default(),
        }
    }
}
