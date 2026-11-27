//! Differential tests for `sprite.c` / `sprite.h`.

use rustyrian_core::sprite::{Sprite, Sprite2Array, State};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sheet(n: usize) -> Sprite2Array {
    Sprite2Array {
        size: n,
        data: Some((0..n).map(|i| (i * 13 + n) as u8).collect()),
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (t, table) in s.sprite_table.iter_mut().enumerate() {
        table.count = 10 + t;
        for (i, sp) in table.sprite.iter_mut().enumerate().take(table.count) {
            if i % 3 == 0 {
                continue;
            }
            let size = (i * 5 + t) as u16;
            *sp = Sprite {
                width: 12,
                height: 14,
                size,
                data: Some((0..size).map(|x| x as u8 ^ 0x5a).collect()),
            };
        }
    }
    s.shop_sprite_sheet = sheet(100);
    s.explosion_sprite_sheet = sheet(0);
    s.enemy_sprite_sheets = [sheet(1), Sprite2Array::default(), sheet(300), sheet(4)];
    s.enemy_sprite_sheet_ids = [1, 2, 3, 255];
    s.destruct_sprite_sheet = sheet(5);
    s.sprite_sheet8 = sheet(6);
    s.sprite_sheet9 = sheet(7);
    s.sprite_sheet10 = sheet(8);
    s.sprite_sheet11 = sheet(9);
    s.sprite_sheet12 = Sprite2Array {
        size: 77,
        data: None,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    // Storing again replaces (and frees) the blocks stored above.
    s.shop_sprite_sheet = sheet(3);
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
