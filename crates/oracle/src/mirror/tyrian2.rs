//! Mirror of `rustyrian_core::tyrian2::State` <-> the C globals of
//! `tyrian2.c`.

use super::varz::{event_from_c, event_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::tyrian2::{BossBarT, State};

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            let bb = global(&raw const b::boss_bar);
            for (r, c) in self.boss_bar.iter_mut().zip(bb.iter()) {
                *r = BossBarT {
                    link_num: c.link_num,
                    armor: c.armor,
                    color: c.color,
                };
            }
            self.quit = *sv::<bool>(b::ot_sv_tyrian2_quit);
            self.load_level_ok = *global(&raw const b::loadLevelOk);
            let ev = global(&raw const b::eventRec);
            for (r, c) in self.event_rec.iter_mut().zip(ev.iter()) {
                *r = event_from_c(c);
            }
            self.level_enemy_max = *global(&raw const b::levelEnemyMax);
            self.level_enemy_frequency = *global(&raw const b::levelEnemyFrequency);
            self.level_enemy = *global(&raw const b::levelEnemy);
            self.temp_str = global(&raw const b::tempStr).map(|c| c as u8);
            self.item_avail = *global(&raw const b::itemAvail);
            self.item_avail_max = *global(&raw const b::itemAvailMax);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            let bb = global_mut(&raw mut b::boss_bar);
            for (r, c) in self.boss_bar.iter().zip(bb.iter_mut()) {
                *c = b::boss_bar_t {
                    link_num: r.link_num,
                    armor: r.armor,
                    color: r.color,
                };
            }
            *sv::<bool>(b::ot_sv_tyrian2_quit) = self.quit;
            *global_mut(&raw mut b::loadLevelOk) = self.load_level_ok;
            let ev = global_mut(&raw mut b::eventRec);
            for (r, c) in self.event_rec.iter().zip(ev.iter_mut()) {
                *c = event_to_c(r);
            }
            *global_mut(&raw mut b::levelEnemyMax) = self.level_enemy_max;
            *global_mut(&raw mut b::levelEnemyFrequency) = self.level_enemy_frequency;
            *global_mut(&raw mut b::levelEnemy) = self.level_enemy;
            *global_mut(&raw mut b::tempStr) = self.temp_str.map(|c| c as std::ffi::c_char);
            *global_mut(&raw mut b::itemAvail) = self.item_avail;
            *global_mut(&raw mut b::itemAvailMax) = self.item_avail_max;
        }
    }
}
