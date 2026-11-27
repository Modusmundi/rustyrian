//! Mirror of `rustyrian_core::shots::State` <-> the C globals of `shots.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::shots::{PlayerShotDataType, State};

fn shot_from_c(c: &b::PlayerShotDataType) -> PlayerShotDataType {
    PlayerShotDataType {
        shot_x: c.shotX,
        shot_y: c.shotY,
        shot_xm: c.shotXM,
        shot_ym: c.shotYM,
        shot_xc: c.shotXC,
        shot_yc: c.shotYC,
        shot_complicated: c.shotComplicated,
        shot_dev_x: c.shotDevX,
        shot_dir_x: c.shotDirX,
        shot_dev_y: c.shotDevY,
        shot_dir_y: c.shotDirY,
        shot_cir_size_x: c.shotCirSizeX,
        shot_cir_size_y: c.shotCirSizeY,
        shot_trail: c.shotTrail,
        shot_gr: c.shotGr,
        shot_ani: c.shotAni,
        shot_ani_max: c.shotAniMax,
        shot_dmg: c.shotDmg,
        shot_blast_filter: c.shotBlastFilter,
        chain_reaction: c.chainReaction,
        player_number: c.playerNumber,
        aim_at_enemy: c.aimAtEnemy,
        aim_delay: c.aimDelay,
        aim_delay_max: c.aimDelayMax,
    }
}

fn shot_to_c(r: &PlayerShotDataType) -> b::PlayerShotDataType {
    b::PlayerShotDataType {
        shotX: r.shot_x,
        shotY: r.shot_y,
        shotXM: r.shot_xm,
        shotYM: r.shot_ym,
        shotXC: r.shot_xc,
        shotYC: r.shot_yc,
        shotComplicated: r.shot_complicated,
        shotDevX: r.shot_dev_x,
        shotDirX: r.shot_dir_x,
        shotDevY: r.shot_dev_y,
        shotDirY: r.shot_dir_y,
        shotCirSizeX: r.shot_cir_size_x,
        shotCirSizeY: r.shot_cir_size_y,
        shotTrail: r.shot_trail,
        shotGr: r.shot_gr,
        shotAni: r.shot_ani,
        shotAniMax: r.shot_ani_max,
        shotDmg: r.shot_dmg,
        shotBlastFilter: r.shot_blast_filter,
        chainReaction: r.chain_reaction,
        playerNumber: r.player_number,
        aimAtEnemy: r.aim_at_enemy,
        aimDelay: r.aim_delay,
        aimDelayMax: r.aim_delay_max,
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            let c = global(&raw const b::playerShotData);
            for (d, s) in self.player_shot_data.iter_mut().zip(c.iter()) {
                *d = shot_from_c(s);
            }
            self.shot_avail = *global(&raw const b::shotAvail);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            let c = global_mut(&raw mut b::playerShotData);
            for (d, s) in c.iter_mut().zip(self.player_shot_data.iter()) {
                *d = shot_to_c(s);
            }
            *global_mut(&raw mut b::shotAvail) = self.shot_avail;
        }
    }
}
