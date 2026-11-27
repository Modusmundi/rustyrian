//! Differential tests for `shots.c` / `shots.h`.

use rustyrian_core::shots::{PlayerShotDataType, State};
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
    let mut s = State::default();
    for (i, x) in s.player_shot_data.iter_mut().enumerate() {
        let n = i as i16;
        *x = PlayerShotDataType {
            shot_x: n,
            shot_y: -n,
            shot_xm: 2 * n,
            shot_ym: -2 * n,
            shot_xc: 3,
            shot_yc: -3,
            shot_complicated: i % 2 == 0,
            shot_dev_x: 4,
            shot_dir_x: -1,
            shot_dev_y: 5,
            shot_dir_y: 1,
            shot_cir_size_x: 6,
            shot_cir_size_y: 7,
            shot_trail: 8,
            shot_gr: 900 + i as u16,
            shot_ani: 10,
            shot_ani_max: 11,
            shot_dmg: 12,
            shot_blast_filter: 13,
            chain_reaction: 14,
            player_number: 2,
            aim_at_enemy: 15,
            aim_delay: 16,
            aim_delay_max: 17,
        };
    }
    for (i, a) in s.shot_avail.iter_mut().enumerate() {
        *a = i as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
