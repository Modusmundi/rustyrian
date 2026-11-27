//! Mirror of `rustyrian_core::episodes::State` <-> the C globals of `episodes.c`.

use super::lvllib::{chars_from_c, chars_to_c};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::episodes::{
    JeEnemyDat, JeOptionType, JePower, JeShield, JeShip, JeSpecial, JeWeaponPort, JeWeaponType,
    State,
};

pub fn weapon_from_c(c: &b::JE_WeaponType) -> JeWeaponType {
    JeWeaponType {
        drain: c.drain,
        shotrepeat: c.shotrepeat,
        multi: c.multi,
        weapani: c.weapani,
        max: c.max,
        tx: c.tx,
        ty: c.ty,
        aim: c.aim,
        attack: c.attack,
        del: c.del,
        sx: c.sx,
        sy: c.sy,
        bx: c.bx,
        by: c.by,
        sg: c.sg,
        acceleration: c.acceleration,
        accelerationx: c.accelerationx,
        circlesize: c.circlesize,
        sound: c.sound,
        trail: c.trail,
        shipblastfilter: c.shipblastfilter,
    }
}

pub fn weapon_to_c(r: &JeWeaponType) -> b::JE_WeaponType {
    b::JE_WeaponType {
        drain: r.drain,
        shotrepeat: r.shotrepeat,
        multi: r.multi,
        weapani: r.weapani,
        max: r.max,
        tx: r.tx,
        ty: r.ty,
        aim: r.aim,
        attack: r.attack,
        del: r.del,
        sx: r.sx,
        sy: r.sy,
        bx: r.bx,
        by: r.by,
        sg: r.sg,
        acceleration: r.acceleration,
        accelerationx: r.accelerationx,
        circlesize: r.circlesize,
        sound: r.sound,
        trail: r.trail,
        shipblastfilter: r.shipblastfilter,
    }
}

fn port_from_c(c: &b::_bindgen_ty_1) -> JeWeaponPort {
    JeWeaponPort {
        name: chars_from_c(&c.name),
        opnum: c.opnum,
        op: c.op,
        cost: c.cost,
        itemgraphic: c.itemgraphic,
        poweruse: c.poweruse,
    }
}

fn port_to_c(r: &JeWeaponPort) -> b::_bindgen_ty_1 {
    b::_bindgen_ty_1 {
        name: chars_to_c(&r.name),
        opnum: r.opnum,
        op: r.op,
        cost: r.cost,
        itemgraphic: r.itemgraphic,
        poweruse: r.poweruse,
    }
}

fn power_from_c(c: &b::_bindgen_ty_2) -> JePower {
    JePower {
        name: chars_from_c(&c.name),
        itemgraphic: c.itemgraphic,
        power: c.power,
        speed: c.speed,
        cost: c.cost,
    }
}

fn power_to_c(r: &JePower) -> b::_bindgen_ty_2 {
    b::_bindgen_ty_2 {
        name: chars_to_c(&r.name),
        itemgraphic: r.itemgraphic,
        power: r.power,
        speed: r.speed,
        cost: r.cost,
    }
}

fn special_from_c(c: &b::_bindgen_ty_3) -> JeSpecial {
    JeSpecial {
        name: chars_from_c(&c.name),
        itemgraphic: c.itemgraphic,
        pwr: c.pwr,
        stype: c.stype,
        wpn: c.wpn,
    }
}

fn special_to_c(r: &JeSpecial) -> b::_bindgen_ty_3 {
    b::_bindgen_ty_3 {
        name: chars_to_c(&r.name),
        itemgraphic: r.itemgraphic,
        pwr: r.pwr,
        stype: r.stype,
        wpn: r.wpn,
    }
}

fn option_from_c(c: &b::JE_OptionType) -> JeOptionType {
    JeOptionType {
        name: chars_from_c(&c.name),
        pwr: c.pwr,
        itemgraphic: c.itemgraphic,
        cost: c.cost,
        tr: c.tr,
        option: c.option,
        opspd: c.opspd,
        ani: c.ani,
        gr: c.gr,
        wport: c.wport,
        wpnum: c.wpnum,
        ammo: c.ammo,
        stop: c.stop,
        icongr: c.icongr,
    }
}

fn option_to_c(r: &JeOptionType) -> b::JE_OptionType {
    b::JE_OptionType {
        name: chars_to_c(&r.name),
        pwr: r.pwr,
        itemgraphic: r.itemgraphic,
        cost: r.cost,
        tr: r.tr,
        option: r.option,
        opspd: r.opspd,
        ani: r.ani,
        gr: r.gr,
        wport: r.wport,
        wpnum: r.wpnum,
        ammo: r.ammo,
        stop: r.stop,
        icongr: r.icongr,
    }
}

fn shield_from_c(c: &b::_bindgen_ty_4) -> JeShield {
    JeShield {
        name: chars_from_c(&c.name),
        tpwr: c.tpwr,
        mpwr: c.mpwr,
        itemgraphic: c.itemgraphic,
        cost: c.cost,
    }
}

fn shield_to_c(r: &JeShield) -> b::_bindgen_ty_4 {
    b::_bindgen_ty_4 {
        name: chars_to_c(&r.name),
        tpwr: r.tpwr,
        mpwr: r.mpwr,
        itemgraphic: r.itemgraphic,
        cost: r.cost,
    }
}

fn ship_from_c(c: &b::_bindgen_ty_5) -> JeShip {
    JeShip {
        name: chars_from_c(&c.name),
        shipgraphic: c.shipgraphic,
        itemgraphic: c.itemgraphic,
        ani: c.ani,
        spd: c.spd,
        dmg: c.dmg,
        cost: c.cost,
        bigshipgraphic: c.bigshipgraphic,
    }
}

fn ship_to_c(r: &JeShip) -> b::_bindgen_ty_5 {
    b::_bindgen_ty_5 {
        name: chars_to_c(&r.name),
        shipgraphic: r.shipgraphic,
        itemgraphic: r.itemgraphic,
        ani: r.ani,
        spd: r.spd,
        dmg: r.dmg,
        cost: r.cost,
        bigshipgraphic: r.bigshipgraphic,
    }
}

fn enemy_from_c(c: &b::_bindgen_ty_6) -> JeEnemyDat {
    JeEnemyDat {
        ani: c.ani,
        tur: c.tur,
        freq: c.freq,
        xmove: c.xmove,
        ymove: c.ymove,
        xaccel: c.xaccel,
        yaccel: c.yaccel,
        xcaccel: c.xcaccel,
        ycaccel: c.ycaccel,
        startx: c.startx,
        starty: c.starty,
        startxc: c.startxc,
        startyc: c.startyc,
        armor: c.armor,
        esize: c.esize,
        egraphic: c.egraphic,
        explosiontype: c.explosiontype,
        animate: c.animate,
        shapebank: c.shapebank,
        xrev: c.xrev,
        yrev: c.yrev,
        dgr: c.dgr,
        dlevel: c.dlevel,
        dani: c.dani,
        elaunchfreq: c.elaunchfreq,
        elaunchtype: c.elaunchtype,
        value: c.value,
        eenemydie: c.eenemydie,
    }
}

fn enemy_to_c(r: &JeEnemyDat) -> b::_bindgen_ty_6 {
    b::_bindgen_ty_6 {
        ani: r.ani,
        tur: r.tur,
        freq: r.freq,
        xmove: r.xmove,
        ymove: r.ymove,
        xaccel: r.xaccel,
        yaccel: r.yaccel,
        xcaccel: r.xcaccel,
        ycaccel: r.ycaccel,
        startx: r.startx,
        starty: r.starty,
        startxc: r.startxc,
        startyc: r.startyc,
        armor: r.armor,
        esize: r.esize,
        egraphic: r.egraphic,
        explosiontype: r.explosiontype,
        animate: r.animate,
        shapebank: r.shapebank,
        xrev: r.xrev,
        yrev: r.yrev,
        dgr: r.dgr,
        dlevel: r.dlevel,
        dani: r.dani,
        elaunchfreq: r.elaunchfreq,
        elaunchtype: r.elaunchtype,
        value: r.value,
        eenemydie: r.eenemydie,
    }
}

/// `dst[i] = f(&src[i])` for every element.
fn convert<S, D, const N: usize>(dst: &mut [D; N], src: &[S; N], f: impl Fn(&S) -> D) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = f(s);
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            convert(
                &mut self.weapon_port,
                global(&raw const b::weaponPort),
                port_from_c,
            );
            convert(
                &mut self.weapons,
                global(&raw const b::weapons),
                weapon_from_c,
            );
            convert(
                &mut self.power_sys,
                global(&raw const b::powerSys),
                power_from_c,
            );
            convert(&mut self.ships, global(&raw const b::ships), ship_from_c);
            convert(
                &mut self.options,
                global(&raw const b::options),
                option_from_c,
            );
            convert(
                &mut self.shields,
                global(&raw const b::shields),
                shield_from_c,
            );
            convert(
                &mut self.special,
                global(&raw const b::special),
                special_from_c,
            );
            convert(
                &mut self.enemy_dat,
                global(&raw const b::enemyDat),
                enemy_from_c,
            );
            self.initial_episode_num = *global(&raw const b::initial_episode_num);
            self.episode_num = *global(&raw const b::episodeNum);
            self.episode_avail = *global(&raw const b::episodeAvail);
            self.episode_filename = chars_from_c(global(&raw const b::episodeFilename));
            self.cube_filename = chars_from_c(global(&raw const b::cubeFilename));
            self.bonus_level = *global(&raw const b::bonusLevel);
            self.jump_back_to_episode1 = *global(&raw const b::jumpBackToEpisode1);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            convert(
                global_mut(&raw mut b::weaponPort),
                &self.weapon_port,
                port_to_c,
            );
            convert(global_mut(&raw mut b::weapons), &self.weapons, weapon_to_c);
            convert(
                global_mut(&raw mut b::powerSys),
                &self.power_sys,
                power_to_c,
            );
            convert(global_mut(&raw mut b::ships), &self.ships, ship_to_c);
            convert(global_mut(&raw mut b::options), &self.options, option_to_c);
            convert(global_mut(&raw mut b::shields), &self.shields, shield_to_c);
            convert(global_mut(&raw mut b::special), &self.special, special_to_c);
            convert(
                global_mut(&raw mut b::enemyDat),
                &self.enemy_dat,
                enemy_to_c,
            );
            *global_mut(&raw mut b::initial_episode_num) = self.initial_episode_num;
            *global_mut(&raw mut b::episodeNum) = self.episode_num;
            *global_mut(&raw mut b::episodeAvail) = self.episode_avail;
            *global_mut(&raw mut b::episodeFilename) = chars_to_c(&self.episode_filename);
            *global_mut(&raw mut b::cubeFilename) = chars_to_c(&self.cube_filename);
            *global_mut(&raw mut b::bonusLevel) = self.bonus_level;
            *global_mut(&raw mut b::jumpBackToEpisode1) = self.jump_back_to_episode1;
        }
    }
}
