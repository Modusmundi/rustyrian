//! Differential tests for `tyrian2.c` / `tyrian2.h`.

use rustyrian_core::tyrian2::{BossBarT, State};
use rustyrian_core::varz::JeEventRecType;
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
        boss_bar: [
            BossBarT {
                link_num: 1,
                armor: 2,
                color: 3,
            },
            BossBarT {
                link_num: 4,
                armor: 5,
                color: 6,
            },
        ],
        quit: true,
        load_level_ok: true,
        level_enemy_max: 40,
        level_enemy_frequency: 96,
        ..State::default()
    };
    for (i, e) in s.event_rec.iter_mut().enumerate() {
        *e = JeEventRecType {
            eventtime: i as u16,
            eventtype: (i % 256) as u8,
            eventdat: -(i as i16),
            eventdat2: i as i16 * 3,
            eventdat3: -1,
            eventdat5: 2,
            eventdat6: -3,
            eventdat4: 4,
        };
    }
    s.level_enemy[39] = 900;
    s.temp_str[..4].copy_from_slice(b"abc\0");
    s.temp_str[30] = 0xfe;
    s.item_avail[8][9] = 7;
    s.item_avail_max[8] = 10;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
