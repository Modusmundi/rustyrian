//! Mirror of `rustyrian_core::sprite::State` <-> the C globals of `sprite.c`.
//!
//! Sprite pixel data pointers are heap blocks upstream mallocs; they are
//! mirrored as their contents (the block's allocated size).

use super::lvllib::{heap_from_c, heap_to_c};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::sprite::{Sprite, Sprite2Array, SpriteArray, State};

fn sprite_array_from_c(c: &b::Sprite_array, r: &mut SpriteArray) {
    r.count = c.count;
    for (d, s) in r.sprite.iter_mut().zip(c.sprite.iter()) {
        *d = Sprite {
            width: s.width,
            height: s.height,
            size: s.size,
            // SAFETY: `data` is NULL or a block malloc'd by `load_sprites`.
            data: unsafe { heap_from_c(s.data, |&x| x) },
        };
    }
}

fn sprite_array_to_c(r: &SpriteArray, c: &mut b::Sprite_array) {
    c.count = r.count;
    for (d, s) in c.sprite.iter_mut().zip(r.sprite.iter()) {
        d.width = s.width;
        d.height = s.height;
        d.size = s.size;
        // SAFETY: `data` is NULL or a block from upstream's allocator owned by
        // this sprite.
        unsafe { heap_to_c(&mut d.data, s.data.as_deref(), |&x| x) };
    }
}

/// Converts a C `Sprite2_array`.
///
/// # Safety
/// `c.data` is NULL or a live block from upstream's allocator.
pub unsafe fn sprite2s_from_c(c: &b::Sprite2_array) -> Sprite2Array {
    Sprite2Array {
        size: c.size,
        // SAFETY: per the caller.
        data: unsafe { heap_from_c(c.data, |&x| x) },
    }
}

/// Writes a core `Sprite2Array` into a C one, replacing (and freeing) its
/// data block.
///
/// # Safety
/// `c.data` is NULL or a live block from upstream's allocator owned by `c`.
pub unsafe fn sprite2s_to_c(r: &Sprite2Array, c: &mut b::Sprite2_array) {
    c.size = r.size;
    // SAFETY: per the caller.
    unsafe { heap_to_c(&mut c.data, r.data.as_deref(), |&x| x) };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; every
        // sprite data pointer is NULL or malloc'd by upstream.
        unsafe {
            let t = global(&raw const b::sprite_table);
            for (c, r) in t.iter().zip(self.sprite_table.iter_mut()) {
                sprite_array_from_c(c, r);
            }
            self.shop_sprite_sheet = sprite2s_from_c(global(&raw const b::shopSpriteSheet));
            self.explosion_sprite_sheet =
                sprite2s_from_c(global(&raw const b::explosionSpriteSheet));
            let e = global(&raw const b::enemySpriteSheets);
            for (c, r) in e.iter().zip(self.enemy_sprite_sheets.iter_mut()) {
                *r = sprite2s_from_c(c);
            }
            self.enemy_sprite_sheet_ids = *global(&raw const b::enemySpriteSheetIds);
            self.destruct_sprite_sheet = sprite2s_from_c(global(&raw const b::destructSpriteSheet));
            self.sprite_sheet8 = sprite2s_from_c(global(&raw const b::spriteSheet8));
            self.sprite_sheet9 = sprite2s_from_c(global(&raw const b::spriteSheet9));
            self.sprite_sheet10 = sprite2s_from_c(global(&raw const b::spriteSheet10));
            self.sprite_sheet11 = sprite2s_from_c(global(&raw const b::spriteSheet11));
            self.sprite_sheet12 = sprite2s_from_c(global(&raw const b::spriteSheet12));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above; each data block is owned by its global.
        unsafe {
            let t = global_mut(&raw mut b::sprite_table);
            for (c, r) in t.iter_mut().zip(self.sprite_table.iter()) {
                sprite_array_to_c(r, c);
            }
            sprite2s_to_c(
                &self.shop_sprite_sheet,
                global_mut(&raw mut b::shopSpriteSheet),
            );
            sprite2s_to_c(
                &self.explosion_sprite_sheet,
                global_mut(&raw mut b::explosionSpriteSheet),
            );
            let e = global_mut(&raw mut b::enemySpriteSheets);
            for (c, r) in e.iter_mut().zip(self.enemy_sprite_sheets.iter()) {
                sprite2s_to_c(r, c);
            }
            *global_mut(&raw mut b::enemySpriteSheetIds) = self.enemy_sprite_sheet_ids;
            sprite2s_to_c(
                &self.destruct_sprite_sheet,
                global_mut(&raw mut b::destructSpriteSheet),
            );
            sprite2s_to_c(&self.sprite_sheet8, global_mut(&raw mut b::spriteSheet8));
            sprite2s_to_c(&self.sprite_sheet9, global_mut(&raw mut b::spriteSheet9));
            sprite2s_to_c(&self.sprite_sheet10, global_mut(&raw mut b::spriteSheet10));
            sprite2s_to_c(&self.sprite_sheet11, global_mut(&raw mut b::spriteSheet11));
            sprite2s_to_c(&self.sprite_sheet12, global_mut(&raw mut b::spriteSheet12));
        }
    }
}
