//! Mirror of `rustyrian_core::backgrnd::State` <-> the C globals of `backgrnd.c`.
//!
//! `mapYPos`/`mapY2Pos`/`mapY3Pos` map to element indices into the flattened
//! `mainmap` of `megaData1`/`megaData2`/`megaData3` (rule R8; see the core
//! type).

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::backgrnd::{StarfieldStar, State};

type Tile = *mut b::JE_byte;

/// Start of `megaData1.mainmap`, `megaData2.mainmap`, `megaData3.mainmap`.
fn map_bases() -> [*mut Tile; 3] {
    // SAFETY: only addresses are taken.
    unsafe {
        [
            (&raw mut b::megaData1.mainmap).cast::<Tile>(),
            (&raw mut b::megaData2.mainmap).cast::<Tile>(),
            (&raw mut b::megaData3.mainmap).cast::<Tile>(),
        ]
    }
}

fn map_pos_from_c(p: *mut Tile, base: *mut Tile) -> Option<isize> {
    if p.is_null() {
        return None;
    }
    let d = (p as isize).wrapping_sub(base as isize);
    debug_assert_eq!(d % size_of::<Tile>() as isize, 0);
    Some(d / size_of::<Tile>() as isize)
}

fn map_pos_to_c(r: Option<isize>, base: *mut Tile) -> *mut Tile {
    match r {
        None => std::ptr::null_mut(),
        Some(i) => base.wrapping_offset(i),
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        let [m1, m2, m3] = map_bases();
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.back_pos = *global(&raw const b::backPos);
            self.back_pos2 = *global(&raw const b::backPos2);
            self.back_pos3 = *global(&raw const b::backPos3);
            self.back_move = *global(&raw const b::backMove);
            self.back_move2 = *global(&raw const b::backMove2);
            self.back_move3 = *global(&raw const b::backMove3);
            self.map_x = *global(&raw const b::mapX);
            self.map_y = *global(&raw const b::mapY);
            self.map_x2 = *global(&raw const b::mapX2);
            self.map_x3 = *global(&raw const b::mapX3);
            self.map_y2 = *global(&raw const b::mapY2);
            self.map_y3 = *global(&raw const b::mapY3);
            self.map_y_pos = map_pos_from_c(*global(&raw const b::mapYPos), m1);
            self.map_y2_pos = map_pos_from_c(*global(&raw const b::mapY2Pos), m2);
            self.map_y3_pos = map_pos_from_c(*global(&raw const b::mapY3Pos), m3);
            self.map_x_pos = *global(&raw const b::mapXPos);
            self.old_map_x_ofs = *global(&raw const b::oldMapXOfs);
            self.map_x_ofs = *global(&raw const b::mapXOfs);
            self.map_x2_ofs = *global(&raw const b::mapX2Ofs);
            self.map_x2_pos = *global(&raw const b::mapX2Pos);
            self.map_x3_pos = *global(&raw const b::mapX3Pos);
            self.old_map_x3_ofs = *global(&raw const b::oldMapX3Ofs);
            self.map_x3_ofs = *global(&raw const b::mapX3Ofs);
            self.temp_map_x_ofs = *global(&raw const b::tempMapXOfs);
            self.map_xbp_pos = *global(&raw const b::mapXbpPos);
            self.map_x2bp_pos = *global(&raw const b::mapX2bpPos);
            self.map_x3bp_pos = *global(&raw const b::mapX3bpPos);
            self.map1_y_delay = *global(&raw const b::map1YDelay);
            self.map1_y_delay_max = *global(&raw const b::map1YDelayMax);
            self.map2_y_delay = *global(&raw const b::map2YDelay);
            self.map2_y_delay_max = *global(&raw const b::map2YDelayMax);
            self.any_smoothies = *global(&raw const b::anySmoothies);
            self.smoothie_data = *global(&raw const b::smoothie_data);
            self.starfield_stars = global(&raw const b::starfield_stars).map(|s| StarfieldStar {
                color: s.color,
                position: s.position,
                speed: s.speed,
            });
            self.starfield_speed = *global(&raw const b::starfield_speed);
        }
    }

    fn store_to_c(&self) {
        let [m1, m2, m3] = map_bases();
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::backPos) = self.back_pos;
            *global_mut(&raw mut b::backPos2) = self.back_pos2;
            *global_mut(&raw mut b::backPos3) = self.back_pos3;
            *global_mut(&raw mut b::backMove) = self.back_move;
            *global_mut(&raw mut b::backMove2) = self.back_move2;
            *global_mut(&raw mut b::backMove3) = self.back_move3;
            *global_mut(&raw mut b::mapX) = self.map_x;
            *global_mut(&raw mut b::mapY) = self.map_y;
            *global_mut(&raw mut b::mapX2) = self.map_x2;
            *global_mut(&raw mut b::mapX3) = self.map_x3;
            *global_mut(&raw mut b::mapY2) = self.map_y2;
            *global_mut(&raw mut b::mapY3) = self.map_y3;
            *global_mut(&raw mut b::mapYPos) = map_pos_to_c(self.map_y_pos, m1);
            *global_mut(&raw mut b::mapY2Pos) = map_pos_to_c(self.map_y2_pos, m2);
            *global_mut(&raw mut b::mapY3Pos) = map_pos_to_c(self.map_y3_pos, m3);
            *global_mut(&raw mut b::mapXPos) = self.map_x_pos;
            *global_mut(&raw mut b::oldMapXOfs) = self.old_map_x_ofs;
            *global_mut(&raw mut b::mapXOfs) = self.map_x_ofs;
            *global_mut(&raw mut b::mapX2Ofs) = self.map_x2_ofs;
            *global_mut(&raw mut b::mapX2Pos) = self.map_x2_pos;
            *global_mut(&raw mut b::mapX3Pos) = self.map_x3_pos;
            *global_mut(&raw mut b::oldMapX3Ofs) = self.old_map_x3_ofs;
            *global_mut(&raw mut b::mapX3Ofs) = self.map_x3_ofs;
            *global_mut(&raw mut b::tempMapXOfs) = self.temp_map_x_ofs;
            *global_mut(&raw mut b::mapXbpPos) = self.map_xbp_pos;
            *global_mut(&raw mut b::mapX2bpPos) = self.map_x2bp_pos;
            *global_mut(&raw mut b::mapX3bpPos) = self.map_x3bp_pos;
            *global_mut(&raw mut b::map1YDelay) = self.map1_y_delay;
            *global_mut(&raw mut b::map1YDelayMax) = self.map1_y_delay_max;
            *global_mut(&raw mut b::map2YDelay) = self.map2_y_delay;
            *global_mut(&raw mut b::map2YDelayMax) = self.map2_y_delay_max;
            *global_mut(&raw mut b::anySmoothies) = self.any_smoothies;
            *global_mut(&raw mut b::smoothie_data) = self.smoothie_data;
            *global_mut(&raw mut b::starfield_stars) =
                self.starfield_stars.map(|s| b::StarfieldStar {
                    color: s.color,
                    position: s.position,
                    speed: s.speed,
                });
            *global_mut(&raw mut b::starfield_speed) = self.starfield_speed;
        }
    }
}
