//! Differential tests for `episodes.c` / `episodes.h`.

use rustyrian_core::episodes::State;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn name(i: usize) -> [u8; 31] {
    std::array::from_fn(|j| (i * 31 + j) as u8)
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, p) in s.weapon_port.iter_mut().enumerate() {
        p.name = name(i);
        p.opnum = i as u8;
        p.op = [[i as u16; 11], [!(i as u16); 11]];
        p.cost = 1000 + i as u16;
        p.itemgraphic = 7;
        p.poweruse = 3;
    }
    for (i, w) in s.weapons.iter_mut().enumerate() {
        w.drain = i as u16;
        w.shotrepeat = 1;
        w.multi = 2;
        w.weapani = 3;
        w.max = 4;
        w.tx = 5;
        w.ty = 6;
        w.aim = 7;
        w.attack = [i as u8; 8];
        w.del = [8; 8];
        w.sx = [-1; 8];
        w.sy = [-2; 8];
        w.bx = [-3; 8];
        w.by = [-4; 8];
        w.sg = [i as u16 ^ 0xaaaa; 8];
        w.acceleration = -5;
        w.accelerationx = 6;
        w.circlesize = 9;
        w.sound = 10;
        w.trail = 11;
        w.shipblastfilter = 12;
    }
    for (i, p) in s.power_sys.iter_mut().enumerate() {
        p.name = name(i + 1);
        p.itemgraphic = 1;
        p.power = 2;
        p.speed = -3;
        p.cost = 4;
    }
    for (i, x) in s.ships.iter_mut().enumerate() {
        x.name = name(i + 2);
        x.shipgraphic = 1;
        x.itemgraphic = 2;
        x.ani = 3;
        x.spd = -4;
        x.dmg = 5;
        x.cost = 6;
        x.bigshipgraphic = 7;
    }
    for (i, o) in s.options.iter_mut().enumerate() {
        o.name = name(i + 3);
        o.pwr = 1;
        o.itemgraphic = 2;
        o.cost = 3;
        o.tr = 4;
        o.option = 5;
        o.opspd = -6;
        o.ani = 7;
        o.gr = [i as u16; 20];
        o.wport = 8;
        o.wpnum = 9;
        o.ammo = 10;
        o.stop = true;
        o.icongr = 11;
    }
    for (i, x) in s.shields.iter_mut().enumerate() {
        x.name = name(i + 4);
        x.tpwr = 1;
        x.mpwr = 2;
        x.itemgraphic = 3;
        x.cost = 4;
    }
    for (i, x) in s.special.iter_mut().enumerate() {
        x.name = name(i + 5);
        x.itemgraphic = 1;
        x.pwr = 2;
        x.stype = 3;
        x.wpn = 4;
    }
    for (i, e) in s.enemy_dat.iter_mut().enumerate() {
        e.ani = 1;
        e.tur = [2, 3, 4];
        e.freq = [5, 6, 7];
        e.xmove = -1;
        e.ymove = -2;
        e.xaccel = -3;
        e.yaccel = -4;
        e.xcaccel = -5;
        e.ycaccel = -6;
        e.startx = -300;
        e.starty = 300;
        e.startxc = -7;
        e.startyc = 8;
        e.armor = 9;
        e.esize = 10;
        e.egraphic = [i as u16; 20];
        e.explosiontype = 11;
        e.animate = 2;
        e.shapebank = 12;
        e.xrev = -13;
        e.yrev = 14;
        e.dgr = 15;
        e.dlevel = -16;
        e.dani = 17;
        e.elaunchfreq = 18;
        e.elaunchtype = 19;
        e.value = -20;
        e.eenemydie = 21;
    }
    s.initial_episode_num = 2;
    s.episode_num = 3;
    s.episode_avail = [true, false, true, true, false];
    s.episode_filename[..10].copy_from_slice(b"tyrian.hdt");
    s.cube_filename[..9].copy_from_slice(b"cubetxt1.");
    s.bonus_level = true;
    s.jump_back_to_episode1 = true;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
