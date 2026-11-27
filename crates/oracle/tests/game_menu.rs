//! Differential tests for `game_menu.c` / `game_menu.h`.

use rustyrian_core::game_menu::State;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        joystick_config: 1,
        y_loc: 2,
        y_chg: -3,
        new_pal: 4,
        cur_pal: 5,
        old_pal: 6,
        quik_save: true,
        old_menu: 7,
        back_from_help: true,
        last_direction: -8,
        first_menu9: true,
        palette_changed: true,
        col: -9,
        col_c: 10,
        last_cur_sel: 11,
        cur_menu: 12,
        cur_item_type: 13,
        cur_item: 14,
        cursor: 15,
        left_power: true,
        right_power: true,
        right_power_afford: true,
        current_cube: 3,
        planet_ani: 16,
        planet_ani_wait: 17,
        current_dot_num: 18,
        current_dot_wait: 19,
        nav_x: 1.5,
        nav_y: -2.25,
        new_nav_x: 3.0,
        new_nav_y: 4.5,
        temp_nav_x: -20,
        temp_nav_y: 21,
        ..State::default()
    };
    s.menu_choices[13] = 5;
    s.cur_sel[0] = 2;
    s.planet_dots[4] = 9;
    s.planet_dot_x[4][9] = -100;
    s.planet_dot_y[0][0] = 100;
    s.old_items[1].ship = 3;
    s.old_items[1].weapon[1].power = 5;
    s.old_items[0].super_arcade_mode = 1;
    for (i, c) in s.cube.iter_mut().enumerate() {
        c.title[..3].copy_from_slice(b"T\0x");
        c.header[12] = i as u8;
        c.face_sprite = -(i as i32);
        c.text[89][35] = 0x7f;
        c.text[0][0] = b'A' + i as u8;
        c.last_line = 40 + i as i32;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
