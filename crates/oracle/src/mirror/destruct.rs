//! Mirror of `rustyrian_core::destruct::State` <-> the C globals of
//! `destruct.c`.
//!
//! `destruct.c`'s structs are file-local, so their C layouts are declared
//! here (`#[repr(C)]`). Heap arrays (`shotRec`, `exploRec`,
//! `world.mapWalls`, `destruct_player[p].unit`) are mirrored by contents:
//! loading reads as many entries as the live block holds (from the shim's
//! allocation registry, so a later change to the `config` field that sized
//! the `malloc` does not matter); storing frees C's block through the shim
//! and allocates a copy of the Rust one the same way. Pointer values are not
//! preserved. `JE_destructGame` frees these blocks without clearing the
//! pointers; such dangling pointers load as `None` (and store as NULL),
//! which upstream cannot observe because it reallocates before any use.

use super::Mirror;
use super::video::{c_heap_free, c_heap_from, c_heap_len, screen_from_c, screen_to_c, sv};
use crate::bindings as b;
use rustyrian_core::destruct::{
    DestructAiS, DestructConfigS, DestructExploS, DestructKeysS, DestructMovesS, DestructPlayerS,
    DestructShotS, DestructUnitS, DestructWallS, DestructWorldS, MAX_KEY, MAX_KEY_OPTIONS,
    MAX_MOVE, MAX_PLAYERS, MAX_SHOT_TYPES, MAX_UNITS, State,
};
use std::ffi::{c_int, c_uint};

/// C `struct destruct_config_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CConfig {
    pub max_shots: c_uint,
    pub min_walls: c_uint,
    pub max_walls: c_uint,
    pub max_explosions: c_uint,
    pub max_installations: c_uint,
    pub allow_custom: bool,
    pub alwaysalias: bool,
    pub jumper_straight: [bool; 2],
    pub ai: [bool; 2],
}

/// C `struct destruct_unit_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CUnit {
    pub unit_x: c_uint,
    pub unit_y: f32,
    pub unit_y_mov: f32,
    pub is_y_in_air: bool,
    pub unit_type: c_int,
    pub shot_type: c_int,
    pub angle: f32,
    pub power: f32,
    pub last_move: c_int,
    pub ani_frame: c_uint,
    pub health: c_int,
}

/// C `struct destruct_shot_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CShot {
    pub is_available: bool,
    pub x: f32,
    pub y: f32,
    pub xmov: f32,
    pub ymov: f32,
    pub gravity: bool,
    pub shottype: c_uint,
    pub trailx: [c_uint; 4],
    pub traily: [c_uint; 4],
    pub trailc: [c_uint; 4],
}

/// C `struct destruct_explo_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CExplo {
    pub is_available: bool,
    pub x: c_uint,
    pub y: c_uint,
    pub explowidth: c_uint,
    pub explomax: c_uint,
    pub explofill: c_uint,
    pub explo_type: c_uint,
}

/// C `struct destruct_ai_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAi {
    pub c_angle: c_int,
    pub c_power: c_int,
    pub c_fire: c_int,
    pub c_no_down: c_uint,
}

/// C `struct destruct_player_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CPlayer {
    pub is_cpu: bool,
    pub ai_memory: CAi,
    pub unit: *mut CUnit,
    pub moves: [bool; MAX_MOVE],
    pub keys: [[c_uint; MAX_KEY_OPTIONS]; MAX_KEY],
    pub team: c_uint,
    pub units_remaining: c_uint,
    pub unit_selected: c_uint,
    pub shot_delay: c_uint,
    pub score: c_uint,
}

/// C `struct destruct_wall_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CWall {
    pub wall_exist: bool,
    pub wall_x: c_uint,
    pub wall_y: c_uint,
}

/// C `struct destruct_world_s`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CWorld {
    pub base_map: [c_uint; 320],
    pub vga_screen: *mut b::SDL_Surface,
    pub map_walls: *mut CWall,
    pub destruct_mode: c_int,
    pub map_flags: c_uint,
}

fn config_from_c(c: &CConfig) -> DestructConfigS {
    DestructConfigS {
        max_shots: c.max_shots,
        min_walls: c.min_walls,
        max_walls: c.max_walls,
        max_explosions: c.max_explosions,
        max_installations: c.max_installations,
        allow_custom: c.allow_custom,
        alwaysalias: c.alwaysalias,
        jumper_straight: c.jumper_straight,
        ai: c.ai,
    }
}

fn config_to_c(r: &DestructConfigS) -> CConfig {
    CConfig {
        max_shots: r.max_shots,
        min_walls: r.min_walls,
        max_walls: r.max_walls,
        max_explosions: r.max_explosions,
        max_installations: r.max_installations,
        allow_custom: r.allow_custom,
        alwaysalias: r.alwaysalias,
        jumper_straight: r.jumper_straight,
        ai: r.ai,
    }
}

fn unit_from_c(c: &CUnit) -> DestructUnitS {
    DestructUnitS {
        unit_x: c.unit_x,
        unit_y: c.unit_y,
        unit_y_mov: c.unit_y_mov,
        is_y_in_air: c.is_y_in_air,
        unit_type: c.unit_type,
        shot_type: c.shot_type,
        angle: c.angle,
        power: c.power,
        last_move: c.last_move,
        ani_frame: c.ani_frame,
        health: c.health,
    }
}

fn unit_to_c(r: &DestructUnitS) -> CUnit {
    CUnit {
        unit_x: r.unit_x,
        unit_y: r.unit_y,
        unit_y_mov: r.unit_y_mov,
        is_y_in_air: r.is_y_in_air,
        unit_type: r.unit_type,
        shot_type: r.shot_type,
        angle: r.angle,
        power: r.power,
        last_move: r.last_move,
        ani_frame: r.ani_frame,
        health: r.health,
    }
}

fn shot_from_c(c: &CShot) -> DestructShotS {
    DestructShotS {
        is_available: c.is_available,
        x: c.x,
        y: c.y,
        xmov: c.xmov,
        ymov: c.ymov,
        gravity: c.gravity,
        shottype: c.shottype,
        trailx: c.trailx,
        traily: c.traily,
        trailc: c.trailc,
    }
}

fn shot_to_c(r: &DestructShotS) -> CShot {
    CShot {
        is_available: r.is_available,
        x: r.x,
        y: r.y,
        xmov: r.xmov,
        ymov: r.ymov,
        gravity: r.gravity,
        shottype: r.shottype,
        trailx: r.trailx,
        traily: r.traily,
        trailc: r.trailc,
    }
}

fn explo_from_c(c: &CExplo) -> DestructExploS {
    DestructExploS {
        is_available: c.is_available,
        x: c.x,
        y: c.y,
        explowidth: c.explowidth,
        explomax: c.explomax,
        explofill: c.explofill,
        explo_type: c.explo_type,
    }
}

fn explo_to_c(r: &DestructExploS) -> CExplo {
    CExplo {
        is_available: r.is_available,
        x: r.x,
        y: r.y,
        explowidth: r.explowidth,
        explomax: r.explomax,
        explofill: r.explofill,
        explo_type: r.explo_type,
    }
}

fn wall_from_c(c: &CWall) -> DestructWallS {
    DestructWallS {
        wall_exist: c.wall_exist,
        wall_x: c.wall_x,
        wall_y: c.wall_y,
    }
}

fn wall_to_c(r: &DestructWallS) -> CWall {
    CWall {
        wall_exist: r.wall_exist,
        wall_x: r.wall_x,
        wall_y: r.wall_y,
    }
}

/// Reads a live upstream heap block (`None` for NULL or a dangling pointer).
///
/// # Safety
/// `p` is NULL, dangling, or a live shim block of `C` entries.
unsafe fn array_from_c<C, R>(p: *const C, f: impl Fn(&C) -> R) -> Option<Vec<R>> {
    c_heap_len(p).map(|n| {
        // SAFETY: a live block holds `c_heap_len` entries.
        unsafe { std::slice::from_raw_parts(p, n) }
            .iter()
            .map(f)
            .collect()
    })
}

/// Frees `*slot` and replaces it with a shim-allocated copy of `r`.
///
/// # Safety
/// `*slot` is NULL, dangling, or a live shim block referenced nowhere else.
unsafe fn array_to_c<C: Copy, R>(r: Option<&Vec<R>>, slot: &mut *mut C, f: impl Fn(&R) -> C) {
    // SAFETY: per the caller.
    unsafe { c_heap_free(*slot) };
    *slot = match r {
        None => std::ptr::null_mut(),
        Some(v) => c_heap_from(&v.iter().map(f).collect::<Vec<_>>()),
    };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            self.weapon_systems =
                *sv::<[[bool; MAX_SHOT_TYPES]; MAX_UNITS]>(b::ot_sv_destruct_weaponSystems);
            self.basetypes = *sv::<[[u8; 11]; 10]>(b::ot_sv_destruct_basetypes);
            self.default_key_config = *sv::<[[[c_uint; MAX_KEY_OPTIONS]; MAX_KEY]; MAX_PLAYERS]>(
                b::ot_sv_destruct_defaultKeyConfig,
            );
            self.destruct_temp_screen = screen_from_c(*sv::<*mut b::SDL_Surface>(
                b::ot_sv_destruct_destructTempScreen,
            ));
            self.destruct_first_time = *sv::<bool>(b::ot_sv_destruct_destructFirstTime);
            self.config = config_from_c(sv::<CConfig>(b::ot_sv_destruct_config));
            let players = sv::<[CPlayer; MAX_PLAYERS]>(b::ot_sv_destruct_destruct_player);
            for (r, c) in self.destruct_player.iter_mut().zip(players.iter()) {
                *r = DestructPlayerS {
                    is_cpu: c.is_cpu,
                    ai_memory: DestructAiS {
                        c_angle: c.ai_memory.c_angle,
                        c_power: c.ai_memory.c_power,
                        c_fire: c.ai_memory.c_fire,
                        c_no_down: c.ai_memory.c_no_down,
                    },
                    unit: array_from_c(c.unit, unit_from_c),
                    moves: DestructMovesS { actions: c.moves },
                    keys: DestructKeysS { config: c.keys },
                    team: c.team,
                    units_remaining: c.units_remaining,
                    unit_selected: c.unit_selected,
                    shot_delay: c.shot_delay,
                    score: c.score,
                };
            }
            let w = sv::<CWorld>(b::ot_sv_destruct_world);
            self.world = DestructWorldS {
                base_map: w.base_map,
                vga_screen: screen_from_c(w.vga_screen),
                map_walls: array_from_c(w.map_walls, wall_from_c),
                destruct_mode: w.destruct_mode,
                map_flags: w.map_flags,
            };
            self.shot_rec = array_from_c(*sv::<*mut CShot>(b::ot_sv_destruct_shotRec), shot_from_c);
            self.explo_rec =
                array_from_c(*sv::<*mut CExplo>(b::ot_sv_destruct_exploRec), explo_from_c);
            self.je_e_sound_explo_sound_channel =
                *sv::<c_int>(b::ot_sv_destruct_JE_eSound_exploSoundChannel);
            self.de_run_tick_end_delay = *sv::<c_uint>(b::ot_sv_destruct_DE_RunTick_endDelay);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`; heap arrays were allocated by the
        // `malloc` shim or by this mirror.
        unsafe {
            *sv::<[[bool; MAX_SHOT_TYPES]; MAX_UNITS]>(b::ot_sv_destruct_weaponSystems) =
                self.weapon_systems;
            *sv::<[[u8; 11]; 10]>(b::ot_sv_destruct_basetypes) = self.basetypes;
            *sv::<[[[c_uint; MAX_KEY_OPTIONS]; MAX_KEY]; MAX_PLAYERS]>(
                b::ot_sv_destruct_defaultKeyConfig,
            ) = self.default_key_config;
            *sv::<*mut b::SDL_Surface>(b::ot_sv_destruct_destructTempScreen) =
                screen_to_c(self.destruct_temp_screen);
            *sv::<bool>(b::ot_sv_destruct_destructFirstTime) = self.destruct_first_time;
            *sv::<CConfig>(b::ot_sv_destruct_config) = config_to_c(&self.config);
            let players = sv::<[CPlayer; MAX_PLAYERS]>(b::ot_sv_destruct_destruct_player);
            for (r, c) in self.destruct_player.iter().zip(players.iter_mut()) {
                c.is_cpu = r.is_cpu;
                c.ai_memory = CAi {
                    c_angle: r.ai_memory.c_angle,
                    c_power: r.ai_memory.c_power,
                    c_fire: r.ai_memory.c_fire,
                    c_no_down: r.ai_memory.c_no_down,
                };
                array_to_c(r.unit.as_ref(), &mut c.unit, unit_to_c);
                c.moves = r.moves.actions;
                c.keys = r.keys.config;
                c.team = r.team;
                c.units_remaining = r.units_remaining;
                c.unit_selected = r.unit_selected;
                c.shot_delay = r.shot_delay;
                c.score = r.score;
            }
            let w = sv::<CWorld>(b::ot_sv_destruct_world);
            w.base_map = self.world.base_map;
            w.vga_screen = screen_to_c(self.world.vga_screen);
            array_to_c(self.world.map_walls.as_ref(), &mut w.map_walls, wall_to_c);
            w.destruct_mode = self.world.destruct_mode;
            w.map_flags = self.world.map_flags;
            array_to_c(
                self.shot_rec.as_ref(),
                sv::<*mut CShot>(b::ot_sv_destruct_shotRec),
                shot_to_c,
            );
            array_to_c(
                self.explo_rec.as_ref(),
                sv::<*mut CExplo>(b::ot_sv_destruct_exploRec),
                explo_to_c,
            );
            *sv::<c_int>(b::ot_sv_destruct_JE_eSound_exploSoundChannel) =
                self.je_e_sound_explo_sound_channel;
            *sv::<c_uint>(b::ot_sv_destruct_DE_RunTick_endDelay) = self.de_run_tick_end_delay;
        }
    }
}
