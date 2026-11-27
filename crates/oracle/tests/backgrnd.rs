//! Differential tests for `backgrnd.c` / `backgrnd.h`.

use rustyrian_core::backgrnd::{StarfieldStar, State};
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
        back_pos: 1,
        back_pos2: 2,
        back_pos3: 3,
        back_move: 4,
        back_move2: 5,
        back_move3: 6,
        map_x: 7,
        map_y: 8,
        map_x2: 9,
        map_x3: 10,
        map_y2: 11,
        map_y3: 12,
        // One-before-begin, as `tyrian2.c:693-695` forms for row 0.
        map_y_pos: Some(-1),
        map_y2_pos: Some(14 * 599 + 13),
        map_y3_pos: Some(15 * 3 - 1),
        map_x_pos: 13,
        old_map_x_ofs: 14,
        map_x_ofs: 15,
        map_x2_ofs: 16,
        map_x2_pos: 17,
        map_x3_pos: 18,
        old_map_x3_ofs: 19,
        map_x3_ofs: 20,
        temp_map_x_ofs: 21,
        map_xbp_pos: -5,
        map_x2bp_pos: 1,
        map_x3bp_pos: -9,
        map1_y_delay: 22,
        map1_y_delay_max: 23,
        map2_y_delay: 24,
        map2_y_delay_max: 25,
        any_smoothies: true,
        smoothie_data: [1, 2, 3, 4, 5, 6, 7, 8, 9],
        starfield_speed: -2,
        ..State::default()
    };
    for (i, st) in s.starfield_stars.iter_mut().enumerate() {
        *st = StarfieldStar {
            color: 0x90 + i as u8 % 16,
            position: (i * 653) as u16,
            speed: i as i32 % 3 + 2,
        };
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
