//! Mirror of `rustyrian_core::varz::State` <-> the C globals of `varz.c`.
//!
//! Pointer globals map to the indices described in `rustyrian_core::varz`
//! (rule R8); each conversion panics on a pointer outside its target, which
//! upstream never forms (`BKwrapN` indices may leave the array bounds, as
//! upstream's pointer arithmetic may, and are mirrored as computed).

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::varz::{
    EnemyShotType, Explosion, JeEventRecType, JeMegaDataShape, JeSingleEnemyType, RepExplosionType,
    Sprite2Ref, State, SuperpixelType,
};
use std::ffi::c_void;

/// Maps a C `Sprite2_array *` to its target.
pub fn sprite2_ref_from_c(p: *const b::Sprite2_array) -> Option<Sprite2Ref> {
    if p.is_null() {
        return None;
    }
    for r in all_sprite2_refs() {
        if std::ptr::eq(sprite2_ref_to_c(Some(r)), p) {
            return Some(r);
        }
    }
    panic!("Sprite2_array pointer is not one of upstream's sheets");
}

fn all_sprite2_refs() -> [Sprite2Ref; 13] {
    use Sprite2Ref::*;
    [
        ShopSpriteSheet,
        ExplosionSpriteSheet,
        EnemySpriteSheets(0),
        EnemySpriteSheets(1),
        EnemySpriteSheets(2),
        EnemySpriteSheets(3),
        DestructSpriteSheet,
        SpriteSheet8,
        SpriteSheet9,
        SpriteSheet10,
        SpriteSheet11,
        SpriteSheet12,
        ExtraShapes,
    ]
}

/// The C `Sprite2_array *` for `r`.
pub fn sprite2_ref_to_c(r: Option<Sprite2Ref>) -> *mut b::Sprite2_array {
    use Sprite2Ref::*;
    let Some(r) = r else {
        return std::ptr::null_mut();
    };
    match r {
        ShopSpriteSheet => &raw mut b::shopSpriteSheet,
        ExplosionSpriteSheet => &raw mut b::explosionSpriteSheet,
        EnemySpriteSheets(i) => {
            assert!(i < 4, "enemySpriteSheets index out of range");
            // SAFETY: in-bounds element address; no reference is formed.
            unsafe {
                (&raw mut b::enemySpriteSheets)
                    .cast::<b::Sprite2_array>()
                    .add(usize::from(i))
            }
        }
        DestructSpriteSheet => &raw mut b::destructSpriteSheet,
        SpriteSheet8 => &raw mut b::spriteSheet8,
        SpriteSheet9 => &raw mut b::spriteSheet9,
        SpriteSheet10 => &raw mut b::spriteSheet10,
        SpriteSheet11 => &raw mut b::spriteSheet11,
        SpriteSheet12 => &raw mut b::spriteSheet12,
        ExtraShapes => &raw mut b::extraShapes,
    }
}

/// The C `enemyDat` element type.
type CEnemyDat = <b::JE_EnemyDatType as IntoIterator>::Item;

/// Maps `void *enemydatofs` to its `enemyDat` index.
pub fn enemydatofs_from_c(p: *const c_void) -> Option<usize> {
    if p.is_null() {
        return None;
    }
    let base = (&raw const b::enemyDat).cast::<CEnemyDat>() as usize;
    let size = std::mem::size_of::<CEnemyDat>();
    let off = (p as usize)
        .checked_sub(base)
        .expect("enemydatofs before enemyDat");
    assert_eq!(off % size, 0, "enemydatofs not at an element");
    Some(off / size)
}

/// The C `enemydatofs` for an `enemyDat` index.
pub fn enemydatofs_to_c(i: Option<usize>) -> *mut c_void {
    i.map_or(std::ptr::null_mut(), |i| {
        (&raw mut b::enemyDat)
            .cast::<CEnemyDat>()
            .wrapping_add(i)
            .cast()
    })
}

macro_rules! same {
    ($d:expr, $s:expr; $($f:ident),* $(,)?) => { $($d.$f = $s.$f;)* };
}

/// Converts a C enemy.
pub fn enemy_from_c(c: &b::JE_SingleEnemyType) -> JeSingleEnemyType {
    let mut r = JeSingleEnemyType::default();
    same!(r, c; fillbyte, ex, ey, exc, eyc, exca, eyca, excc, eycc, exccw, eyccw, armorleft,
        eshotwait, eshotmultipos, enemycycle, ani, egr, size, linknum, aniactive, animax,
        aniwhenfire, exrev, eyrev, exccadd, eyccadd, exccwmax, eyccwmax, edamaged, enemytype,
        animin, edgr, edlevel, edani, fill1, filter, evalue, fixedmovey, freq, launchwait,
        launchtype, launchfreq, xaccel, yaccel, tur, enemydie, enemyground, explonum, mapoffset,
        scoreitem, special, flagnum, setto, iced, launchspecial, xminbounce, xmaxbounce,
        yminbounce, ymaxbounce, fill);
    r.sprite2s = sprite2_ref_from_c(c.sprite2s);
    r.enemydatofs = enemydatofs_from_c(c.enemydatofs);
    r
}

/// Converts a core enemy.
pub fn enemy_to_c(r: &JeSingleEnemyType) -> b::JE_SingleEnemyType {
    // SAFETY: an all-zero JE_SingleEnemyType is valid (NULL pointers).
    let mut c: b::JE_SingleEnemyType = unsafe { std::mem::zeroed() };
    same!(c, r; fillbyte, ex, ey, exc, eyc, exca, eyca, excc, eycc, exccw, eyccw, armorleft,
        eshotwait, eshotmultipos, enemycycle, ani, egr, size, linknum, aniactive, animax,
        aniwhenfire, exrev, eyrev, exccadd, eyccadd, exccwmax, eyccwmax, edamaged, enemytype,
        animin, edgr, edlevel, edani, fill1, filter, evalue, fixedmovey, freq, launchwait,
        launchtype, launchfreq, xaccel, yaccel, tur, enemydie, enemyground, explonum, mapoffset,
        scoreitem, special, flagnum, setto, iced, launchspecial, xminbounce, xmaxbounce,
        yminbounce, ymaxbounce, fill);
    c.sprite2s = sprite2_ref_to_c(r.sprite2s);
    c.enemydatofs = enemydatofs_to_c(r.enemydatofs);
    c
}

/// Converts a C event record.
pub fn event_from_c(c: &b::JE_EventRecType) -> JeEventRecType {
    let mut r = JeEventRecType::default();
    same!(r, c; eventtime, eventtype, eventdat, eventdat2, eventdat3, eventdat5, eventdat6,
        eventdat4);
    r
}

/// Converts a core event record.
pub fn event_to_c(r: &JeEventRecType) -> b::JE_EventRecType {
    let mut c = b::JE_EventRecType::default();
    same!(c, r; eventtime, eventtype, eventdat, eventdat2, eventdat3, eventdat5, eventdat6,
        eventdat4);
    c
}

fn shot_from_c(c: &b::EnemyShotType) -> EnemyShotType {
    let mut r = EnemyShotType::default();
    same!(r, c; sx, sy, sxm, sym, sxc, syc, tx, ty, sgr, sdmg, duration, animate, animax, fill);
    r
}

fn shot_to_c(r: &EnemyShotType) -> b::EnemyShotType {
    // SAFETY: plain integers.
    let mut c: b::EnemyShotType = unsafe { std::mem::zeroed() };
    same!(c, r; sx, sy, sxm, sym, sxc, syc, tx, ty, sgr, sdmg, duration, animate, animax, fill);
    c
}

fn explosion_from_c(c: &b::Explosion) -> Explosion {
    Explosion {
        ttl: c.ttl,
        x: c.x,
        y: c.y,
        sprite: c.sprite,
        follow_player: c.followPlayer,
        fixed_position: c.fixedPosition,
        delta_y: c.deltaY,
    }
}

fn explosion_to_c(r: &Explosion) -> b::Explosion {
    b::Explosion {
        ttl: r.ttl,
        x: r.x,
        y: r.y,
        sprite: r.sprite,
        followPlayer: r.follow_player,
        fixedPosition: r.fixed_position,
        deltaY: r.delta_y,
    }
}

fn rep_from_c(c: &b::rep_explosion_type) -> RepExplosionType {
    RepExplosionType {
        delay: c.delay,
        ttl: c.ttl,
        x: c.x,
        y: c.y,
        big: c.big,
    }
}

fn rep_to_c(r: &RepExplosionType) -> b::rep_explosion_type {
    b::rep_explosion_type {
        delay: r.delay,
        ttl: r.ttl,
        x: r.x,
        y: r.y,
        big: r.big,
    }
}

fn sp_from_c(c: &b::superpixel_type) -> SuperpixelType {
    SuperpixelType {
        x: c.x,
        y: c.y,
        z: c.z,
        delta_x: c.delta_x,
        delta_y: c.delta_y,
        color: c.color,
    }
}

fn sp_to_c(r: &SuperpixelType) -> b::superpixel_type {
    b::superpixel_type {
        x: r.x,
        y: r.y,
        z: r.z,
        delta_x: r.delta_x,
        delta_y: r.delta_y,
        color: r.color,
    }
}

/// Maps a `mainmap` entry (`&shapes[i].sh` or NULL) to `i`.
fn tile_from_c<S>(p: *const u8, shapes: &[S], sh_offset: usize) -> Option<u8> {
    if p.is_null() {
        return None;
    }
    let base = shapes.as_ptr() as usize + sh_offset;
    let size = std::mem::size_of::<S>();
    let off = (p as usize)
        .checked_sub(base)
        .expect("map tile before shapes");
    assert_eq!(off % size, 0, "map tile not at a shape");
    let i = off / size;
    assert!(i < shapes.len(), "map tile past shapes");
    Some(u8::try_from(i).expect("shape index"))
}

/// The `mainmap` entry for shape `i`.
fn tile_to_c<S>(i: Option<u8>, shapes: &mut [S], sh_offset: usize) -> *mut u8 {
    i.map_or(std::ptr::null_mut(), |i| {
        let s: *mut S = &raw mut shapes[usize::from(i)];
        s.cast::<u8>().wrapping_add(sh_offset)
    })
}

/// Maps a `JE_byte **` into `mainmap` to its flattened element index.
fn wrap_from_c(p: *const *mut u8, base: *const *mut u8) -> Option<isize> {
    if p.is_null() {
        return None;
    }
    let size = std::mem::size_of::<*mut u8>() as isize;
    let off = (p as isize).wrapping_sub(base as isize);
    assert_eq!(off % size, 0, "BKwrap not at an element");
    Some(off / size)
}

fn wrap_to_c(i: Option<isize>, base: *mut *mut u8) -> *mut *mut u8 {
    i.map_or(std::ptr::null_mut(), |i| base.wrapping_offset(i))
}

fn shape_from_c(nothing: [u8; 3], fill: u8, sh: [u8; 672]) -> JeMegaDataShape {
    JeMegaDataShape { nothing, fill, sh }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // global is read through its own C type, one reference at a time.
        unsafe {
            self.temp_dat = b::tempDat;
            self.temp_dat2 = b::tempDat2;
            self.temp_dat3 = b::tempDat3;
            self.sf_current_code = b::SFCurrentCode;
            self.sf_executed = b::SFExecuted;
            self.lvl_file_num = b::lvlFileNum;
            self.max_event = b::maxEvent;
            self.event_loc = b::eventLoc;
            self.temp_back_move = b::tempBackMove;
            self.explode_move = b::explodeMove;
            self.level_end = b::levelEnd;
            self.level_end_fx_wait = b::levelEndFxWait;
            self.level_end_warp = b::levelEndWarp;
            self.end_level = b::endLevel;
            self.really_end_level = b::reallyEndLevel;
            self.wait_to_end_level = b::waitToEndLevel;
            self.player_end_level = b::playerEndLevel;
            self.normal_bonus_level_current = b::normalBonusLevelCurrent;
            self.bonus_level_current = b::bonusLevelCurrent;
            self.small_enemy_adjust = b::smallEnemyAdjust;
            self.ready_to_end_level = b::readyToEndLevel;
            self.quit_requested = b::quitRequested;
            self.new_pl = b::newPL;
            self.return_loc = b::returnLoc;
            self.return_active = b::returnActive;
            self.galaga_shot_freq = b::galagaShotFreq;
            self.galaga_life = b::galagaLife;
            self.debug = b::debug;
            self.debug_time = b::debugTime;
            self.last_debug_time = b::lastDebugTime;
            self.debug_hist_count = b::debugHistCount;
            self.debug_hist = b::debugHist;
            self.cur_loc = b::curLoc;
            self.first_game_over = b::firstGameOver;
            self.game_loaded = b::gameLoaded;
            self.enemy_still_exploding = b::enemyStillExploding;
            self.total_enemy = b::totalEnemy;
            self.enemy_killed = b::enemyKilled;

            let m1 = global(&raw const b::megaData1);
            for (r, c) in self.mega_data1.mainmap.iter_mut().zip(m1.mainmap.iter()) {
                for (r, &c) in r.iter_mut().zip(c.iter()) {
                    *r = tile_from_c(c, &m1.shapes, 0);
                }
            }
            for (r, c) in self.mega_data1.shapes.iter_mut().zip(m1.shapes.iter()) {
                r.sh = c.sh;
            }
            self.mega_data1.tempdat1 = m1.tempdat1;
            let m2 = global(&raw const b::megaData2);
            let off2 = std::mem::offset_of!(b::JE_MegaDataType2__bindgen_ty_1, sh);
            for (r, c) in self.mega_data2.mainmap.iter_mut().zip(m2.mainmap.iter()) {
                for (r, &c) in r.iter_mut().zip(c.iter()) {
                    *r = tile_from_c(c, &m2.shapes, off2);
                }
            }
            for (r, c) in self.mega_data2.shapes.iter_mut().zip(m2.shapes.iter()) {
                *r = shape_from_c(c.nothing, c.fill, c.sh);
            }
            self.mega_data2.tempdat2 = m2.tempdat2;
            let m3 = global(&raw const b::megaData3);
            let off3 = std::mem::offset_of!(b::JE_MegaDataType3__bindgen_ty_1, sh);
            for (r, c) in self.mega_data3.mainmap.iter_mut().zip(m3.mainmap.iter()) {
                for (r, &c) in r.iter_mut().zip(c.iter()) {
                    *r = tile_from_c(c, &m3.shapes, off3);
                }
            }
            for (r, c) in self.mega_data3.shapes.iter_mut().zip(m3.shapes.iter()) {
                *r = shape_from_c(c.nothing, c.fill, c.sh);
            }
            self.mega_data3.tempdat3 = m3.tempdat3;

            self.flash = b::flash;
            self.flash_change = b::flashChange;
            self.display_time = b::displayTime;
            self.sound_queue = b::soundQueue;
            self.enemy_continual_damage = b::enemyContinualDamage;
            self.enemies_active = b::enemiesActive;
            self.force_events = b::forceEvents;
            self.stop_backgrounds = b::stopBackgrounds;
            self.stop_background_num = b::stopBackgroundNum;
            self.damage_rate = b::damageRate;
            self.background3x1 = b::background3x1;
            self.background3x1b = b::background3x1b;
            self.level_timer = b::levelTimer;
            self.level_timer_countdown = b::levelTimerCountdown;
            self.level_timer_jump_to = b::levelTimerJumpTo;
            self.random_explosions = b::randomExplosions;
            self.edit_ship1 = b::editShip1;
            self.edit_ship2 = b::editShip2;
            self.global_flags = b::globalFlags;
            self.level_song = b::levelSong;
            self.load_destruct = b::loadDestruct;
            self.map_origin = b::mapOrigin;
            self.map_p_num = b::mapPNum;
            self.map_planet = b::mapPlanet;
            self.map_section = b::mapSection;
            self.move_tyrian_logo_up = b::moveTyrianLogoUp;
            self.skip_star_show_vga = b::skipStarShowVGA;
            let enemy = global(&raw const b::enemy);
            for (r, c) in self.enemy.iter_mut().zip(enemy.iter()) {
                *r = enemy_from_c(c);
            }
            self.enemy_avail = b::enemyAvail;
            self.enemy_offset = b::enemyOffset;
            self.enemy_on_screen = b::enemyOnScreen;
            self.super_enemy254_jump = b::superEnemy254Jump;
            self.fire_button_held = b::fireButtonHeld;
            self.enemy_shot_avail = b::enemyShotAvail;
            let shots = global(&raw const b::enemyShot);
            for (r, c) in self.enemy_shot.iter_mut().zip(shots.iter()) {
                *r = shot_from_c(c);
            }
            self.zinglon_duration = b::zinglonDuration;
            self.astral_duration = b::astralDuration;
            self.flare_duration = b::flareDuration;
            self.flare_start = b::flareStart;
            self.flare_col_chg = b::flareColChg;
            self.special_wait = b::specialWait;
            self.next_special_wait = b::nextSpecialWait;
            self.spray_special = b::spraySpecial;
            self.do_iced = b::doIced;
            self.infinite_shot = b::infiniteShot;
            self.all_players_gone = b::allPlayersGone;
            self.option_satellite_rotate = b::optionSatelliteRotate;
            self.option_attachment_move = b::optionAttachmentMove;
            self.option_attachment_linked = b::optionAttachmentLinked;
            self.option_attachment_return = b::optionAttachmentReturn;
            self.charge_wait = b::chargeWait;
            self.charge_level = b::chargeLevel;
            self.charge_max = b::chargeMax;
            self.charge_gr = b::chargeGr;
            self.charge_gr_wait = b::chargeGrWait;
            self.neat = b::neat;
            let ex = global(&raw const b::explosions);
            for (r, c) in self.explosions.iter_mut().zip(ex.iter()) {
                *r = explosion_from_c(c);
            }
            self.explosion_follow_amount_x = b::explosionFollowAmountX;
            self.explosion_follow_amount_y = b::explosionFollowAmountY;
            let rep = global(&raw const b::rep_explosions);
            for (r, c) in self.rep_explosions.iter_mut().zip(rep.iter()) {
                *r = rep_from_c(c);
            }
            let sp = global(&raw const b::superpixels);
            for (r, c) in self.superpixels.iter_mut().zip(sp.iter()) {
                *r = sp_from_c(c);
            }
            self.last_superpixel = b::last_superpixel;
            self.temp = b::temp;
            self.temp2 = b::temp2;
            self.temp3 = b::temp3;
            self.temp_w = b::tempW;
            self.do_not_save_backup = b::doNotSaveBackup;
            self.x = b::x;
            self.y = b::y;
            self.b = b::b;
            let base1 = (&raw const b::megaData1.mainmap).cast::<*mut u8>();
            let base2 = (&raw const b::megaData2.mainmap).cast::<*mut u8>();
            let base3 = (&raw const b::megaData3.mainmap).cast::<*mut u8>();
            self.bkwrap1to = wrap_from_c(b::BKwrap1to, base1);
            self.bkwrap2to = wrap_from_c(b::BKwrap2to, base2);
            self.bkwrap3to = wrap_from_c(b::BKwrap3to, base3);
            self.bkwrap1 = wrap_from_c(b::BKwrap1, base1);
            self.bkwrap2 = wrap_from_c(b::BKwrap2, base2);
            self.bkwrap3 = wrap_from_c(b::BKwrap3, base3);
            self.special_weapon_filter = b::specialWeaponFilter;
            self.special_weapon_freq = b::specialWeaponFreq;
            self.special_weapon_wpn = b::specialWeaponWpn;
            self.link_to_player = b::linkToPlayer;
            self.ship_gr = b::shipGr;
            self.ship_gr2 = b::shipGr2;
            self.ship_gr_ptr = sprite2_ref_from_c(b::shipGrPtr);
            self.ship_gr2ptr = sprite2_ref_from_c(b::shipGr2ptr);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            b::tempDat = self.temp_dat;
            b::tempDat2 = self.temp_dat2;
            b::tempDat3 = self.temp_dat3;
            b::SFCurrentCode = self.sf_current_code;
            b::SFExecuted = self.sf_executed;
            b::lvlFileNum = self.lvl_file_num;
            b::maxEvent = self.max_event;
            b::eventLoc = self.event_loc;
            b::tempBackMove = self.temp_back_move;
            b::explodeMove = self.explode_move;
            b::levelEnd = self.level_end;
            b::levelEndFxWait = self.level_end_fx_wait;
            b::levelEndWarp = self.level_end_warp;
            b::endLevel = self.end_level;
            b::reallyEndLevel = self.really_end_level;
            b::waitToEndLevel = self.wait_to_end_level;
            b::playerEndLevel = self.player_end_level;
            b::normalBonusLevelCurrent = self.normal_bonus_level_current;
            b::bonusLevelCurrent = self.bonus_level_current;
            b::smallEnemyAdjust = self.small_enemy_adjust;
            b::readyToEndLevel = self.ready_to_end_level;
            b::quitRequested = self.quit_requested;
            b::newPL = self.new_pl;
            b::returnLoc = self.return_loc;
            b::returnActive = self.return_active;
            b::galagaShotFreq = self.galaga_shot_freq;
            b::galagaLife = self.galaga_life;
            b::debug = self.debug;
            b::debugTime = self.debug_time;
            b::lastDebugTime = self.last_debug_time;
            b::debugHistCount = self.debug_hist_count;
            b::debugHist = self.debug_hist;
            b::curLoc = self.cur_loc;
            b::firstGameOver = self.first_game_over;
            b::gameLoaded = self.game_loaded;
            b::enemyStillExploding = self.enemy_still_exploding;
            b::totalEnemy = self.total_enemy;
            b::enemyKilled = self.enemy_killed;

            let m1 = global_mut(&raw mut b::megaData1);
            for (r, c) in self.mega_data1.shapes.iter().zip(m1.shapes.iter_mut()) {
                c.sh = r.sh;
            }
            for (r, c) in self.mega_data1.mainmap.iter().zip(m1.mainmap.iter_mut()) {
                for (&r, c) in r.iter().zip(c.iter_mut()) {
                    *c = tile_to_c(r, &mut m1.shapes, 0);
                }
            }
            m1.tempdat1 = self.mega_data1.tempdat1;
            let m2 = global_mut(&raw mut b::megaData2);
            let off2 = std::mem::offset_of!(b::JE_MegaDataType2__bindgen_ty_1, sh);
            for (r, c) in self.mega_data2.shapes.iter().zip(m2.shapes.iter_mut()) {
                c.nothing = r.nothing;
                c.fill = r.fill;
                c.sh = r.sh;
            }
            for (r, c) in self.mega_data2.mainmap.iter().zip(m2.mainmap.iter_mut()) {
                for (&r, c) in r.iter().zip(c.iter_mut()) {
                    *c = tile_to_c(r, &mut m2.shapes, off2);
                }
            }
            m2.tempdat2 = self.mega_data2.tempdat2;
            let m3 = global_mut(&raw mut b::megaData3);
            let off3 = std::mem::offset_of!(b::JE_MegaDataType3__bindgen_ty_1, sh);
            for (r, c) in self.mega_data3.shapes.iter().zip(m3.shapes.iter_mut()) {
                c.nothing = r.nothing;
                c.fill = r.fill;
                c.sh = r.sh;
            }
            for (r, c) in self.mega_data3.mainmap.iter().zip(m3.mainmap.iter_mut()) {
                for (&r, c) in r.iter().zip(c.iter_mut()) {
                    *c = tile_to_c(r, &mut m3.shapes, off3);
                }
            }
            m3.tempdat3 = self.mega_data3.tempdat3;

            b::flash = self.flash;
            b::flashChange = self.flash_change;
            b::displayTime = self.display_time;
            b::soundQueue = self.sound_queue;
            b::enemyContinualDamage = self.enemy_continual_damage;
            b::enemiesActive = self.enemies_active;
            b::forceEvents = self.force_events;
            b::stopBackgrounds = self.stop_backgrounds;
            b::stopBackgroundNum = self.stop_background_num;
            b::damageRate = self.damage_rate;
            b::background3x1 = self.background3x1;
            b::background3x1b = self.background3x1b;
            b::levelTimer = self.level_timer;
            b::levelTimerCountdown = self.level_timer_countdown;
            b::levelTimerJumpTo = self.level_timer_jump_to;
            b::randomExplosions = self.random_explosions;
            b::editShip1 = self.edit_ship1;
            b::editShip2 = self.edit_ship2;
            b::globalFlags = self.global_flags;
            b::levelSong = self.level_song;
            b::loadDestruct = self.load_destruct;
            b::mapOrigin = self.map_origin;
            b::mapPNum = self.map_p_num;
            b::mapPlanet = self.map_planet;
            b::mapSection = self.map_section;
            b::moveTyrianLogoUp = self.move_tyrian_logo_up;
            b::skipStarShowVGA = self.skip_star_show_vga;
            let enemy = global_mut(&raw mut b::enemy);
            for (r, c) in self.enemy.iter().zip(enemy.iter_mut()) {
                *c = enemy_to_c(r);
            }
            b::enemyAvail = self.enemy_avail;
            b::enemyOffset = self.enemy_offset;
            b::enemyOnScreen = self.enemy_on_screen;
            b::superEnemy254Jump = self.super_enemy254_jump;
            b::fireButtonHeld = self.fire_button_held;
            b::enemyShotAvail = self.enemy_shot_avail;
            let shots = global_mut(&raw mut b::enemyShot);
            for (r, c) in self.enemy_shot.iter().zip(shots.iter_mut()) {
                *c = shot_to_c(r);
            }
            b::zinglonDuration = self.zinglon_duration;
            b::astralDuration = self.astral_duration;
            b::flareDuration = self.flare_duration;
            b::flareStart = self.flare_start;
            b::flareColChg = self.flare_col_chg;
            b::specialWait = self.special_wait;
            b::nextSpecialWait = self.next_special_wait;
            b::spraySpecial = self.spray_special;
            b::doIced = self.do_iced;
            b::infiniteShot = self.infinite_shot;
            b::allPlayersGone = self.all_players_gone;
            b::optionSatelliteRotate = self.option_satellite_rotate;
            b::optionAttachmentMove = self.option_attachment_move;
            b::optionAttachmentLinked = self.option_attachment_linked;
            b::optionAttachmentReturn = self.option_attachment_return;
            b::chargeWait = self.charge_wait;
            b::chargeLevel = self.charge_level;
            b::chargeMax = self.charge_max;
            b::chargeGr = self.charge_gr;
            b::chargeGrWait = self.charge_gr_wait;
            b::neat = self.neat;
            let ex = global_mut(&raw mut b::explosions);
            for (r, c) in self.explosions.iter().zip(ex.iter_mut()) {
                *c = explosion_to_c(r);
            }
            b::explosionFollowAmountX = self.explosion_follow_amount_x;
            b::explosionFollowAmountY = self.explosion_follow_amount_y;
            let rep = global_mut(&raw mut b::rep_explosions);
            for (r, c) in self.rep_explosions.iter().zip(rep.iter_mut()) {
                *c = rep_to_c(r);
            }
            let sp = global_mut(&raw mut b::superpixels);
            for (r, c) in self.superpixels.iter().zip(sp.iter_mut()) {
                *c = sp_to_c(r);
            }
            b::last_superpixel = self.last_superpixel;
            b::temp = self.temp;
            b::temp2 = self.temp2;
            b::temp3 = self.temp3;
            b::tempW = self.temp_w;
            b::doNotSaveBackup = self.do_not_save_backup;
            b::x = self.x;
            b::y = self.y;
            b::b = self.b;
            let base1 = (&raw mut b::megaData1.mainmap).cast::<*mut u8>();
            let base2 = (&raw mut b::megaData2.mainmap).cast::<*mut u8>();
            let base3 = (&raw mut b::megaData3.mainmap).cast::<*mut u8>();
            b::BKwrap1to = wrap_to_c(self.bkwrap1to, base1);
            b::BKwrap2to = wrap_to_c(self.bkwrap2to, base2);
            b::BKwrap3to = wrap_to_c(self.bkwrap3to, base3);
            b::BKwrap1 = wrap_to_c(self.bkwrap1, base1);
            b::BKwrap2 = wrap_to_c(self.bkwrap2, base2);
            b::BKwrap3 = wrap_to_c(self.bkwrap3, base3);
            b::specialWeaponFilter = self.special_weapon_filter;
            b::specialWeaponFreq = self.special_weapon_freq;
            b::specialWeaponWpn = self.special_weapon_wpn;
            b::linkToPlayer = self.link_to_player;
            b::shipGr = self.ship_gr;
            b::shipGr2 = self.ship_gr2;
            b::shipGrPtr = sprite2_ref_to_c(self.ship_gr_ptr);
            b::shipGr2ptr = sprite2_ref_to_c(self.ship_gr2ptr);
        }
    }
}
