//! Mirrors: copy each core module `State` from and to the C globals of the
//! same upstream file (plan step 2, "Mirrors").
//!
//! Every inventory global of a file with a `verified` function must be
//! referenced in that module's mirror (enforced by `cargo xtask check-trace`):
//! by its C name for extern globals, or by its `ot_sv_*` getter for statics.

use rustyrian_core::Game;

/// A shared reference to a C global, from `&raw const GLOBAL`.
///
/// # Safety
/// C runs only on the calling thread, inside calls the caller makes, and no
/// mutable reference to the same global is live.
pub unsafe fn global<'a, T>(p: *const T) -> &'a T {
    // SAFETY: per the caller; the address of a static is non-null.
    unsafe { p.as_ref() }.expect("address of a C global")
}

/// A mutable reference to a C global, from `&raw mut GLOBAL`.
///
/// # Safety
/// As [`global`], and no other reference to the same global is live.
pub unsafe fn global_mut<'a, T>(p: *mut T) -> &'a mut T {
    // SAFETY: per the caller; the address of a static is non-null.
    unsafe { p.as_mut() }.expect("address of a C global")
}

/// Two-way copy between a core `State` and upstream's C globals.
pub trait Mirror {
    /// Overwrites `self` with the current C globals.
    fn load_from_c(&mut self);
    /// Overwrites the C globals with `self`.
    fn store_to_c(&self);
}

pub mod animlib;
pub mod arg_parse;
pub mod backgrnd;
pub mod config;
pub mod config_file;
pub mod demo;
pub mod destruct;
pub mod editship;
pub mod episodes;
pub mod file;
pub mod font;
pub mod fonthand;
pub mod game_menu;
pub mod helptext;
pub mod joystick;
pub mod jukebox;
pub mod keyboard;
pub mod lds_play;
pub mod logging;
pub mod loudness;
pub mod lvllib;
pub mod lvlmast;
pub mod mainint;
pub mod memreader;
pub mod memwriter;
pub mod menus;
pub mod mouse;
pub mod mtrand;
pub mod musmast;
pub mod network;
pub mod nortsong;
pub mod nortvars;
pub mod opentyr;
pub mod opl;
pub mod palette;
pub mod params;
pub mod pcxload;
pub mod pcxmast;
pub mod picload;
pub mod player;
pub mod shots;
pub mod sndmast;
pub mod sprite;
pub mod starlib;
pub mod swar;
pub mod tyrian2;
pub mod varz;
pub mod vga256d;
pub mod vga_palette;
pub mod video;
pub mod video_scale;
pub mod video_scale_hqnx;
pub mod xmas;

/// Loads every module state of `g` (not the platform) from C.
pub fn load_game(g: &mut Game) {
    *g.crand.lock().unwrap_or_else(|e| e.into_inner()) = crate::session::crand_state();
    g.animlib.load_from_c();
    g.arg_parse.load_from_c();
    g.backgrnd.load_from_c();
    g.config.load_from_c();
    g.config_file.load_from_c();
    g.demo.load_from_c();
    g.destruct.load_from_c();
    g.editship.load_from_c();
    g.episodes.load_from_c();
    g.file.load_from_c();
    g.font.load_from_c();
    g.fonthand.load_from_c();
    g.game_menu.load_from_c();
    g.helptext.load_from_c();
    g.joystick.load_from_c();
    g.jukebox.load_from_c();
    g.keyboard.load_from_c();
    g.lds_play.load_from_c();
    g.logging.load_from_c();
    g.loudness.load_from_c();
    g.lvllib.load_from_c();
    g.lvlmast.load_from_c();
    g.mainint.load_from_c();
    g.memreader.load_from_c();
    g.memwriter.load_from_c();
    g.menus.load_from_c();
    g.mouse.load_from_c();
    g.mtrand.load_from_c();
    g.musmast.load_from_c();
    g.network.load_from_c();
    g.nortsong.load_from_c();
    g.nortvars.load_from_c();
    g.opentyr.load_from_c();
    g.opl.load_from_c();
    g.palette.load_from_c();
    g.params.load_from_c();
    g.pcxload.load_from_c();
    g.pcxmast.load_from_c();
    g.picload.load_from_c();
    g.player.load_from_c();
    g.shots.load_from_c();
    g.sndmast.load_from_c();
    g.sprite.load_from_c();
    g.starlib.load_from_c();
    g.swar.load_from_c();
    g.tyrian2.load_from_c();
    g.varz.load_from_c();
    g.vga256d.load_from_c();
    g.vga_palette.load_from_c();
    g.video.load_from_c();
    g.video_scale.load_from_c();
    g.video_scale_hqnx.load_from_c();
    g.xmas.load_from_c();
}

/// Stores every module state of `g` (not the platform) into C.
pub fn store_game(g: &Game) {
    crate::session::set_crand_state(g.crand.lock().unwrap_or_else(|e| e.into_inner()).clone());
    // First: it creates the C surfaces that other mirrors map ScreenIds onto.
    g.video.store_to_c();
    g.animlib.store_to_c();
    g.arg_parse.store_to_c();
    g.backgrnd.store_to_c();
    g.config.store_to_c();
    g.config_file.store_to_c();
    g.demo.store_to_c();
    g.destruct.store_to_c();
    g.editship.store_to_c();
    g.episodes.store_to_c();
    g.file.store_to_c();
    g.font.store_to_c();
    g.fonthand.store_to_c();
    g.game_menu.store_to_c();
    g.helptext.store_to_c();
    g.joystick.store_to_c();
    g.jukebox.store_to_c();
    g.keyboard.store_to_c();
    g.lds_play.store_to_c();
    g.logging.store_to_c();
    g.loudness.store_to_c();
    g.lvllib.store_to_c();
    g.lvlmast.store_to_c();
    g.mainint.store_to_c();
    g.memreader.store_to_c();
    g.memwriter.store_to_c();
    g.menus.store_to_c();
    g.mouse.store_to_c();
    g.mtrand.store_to_c();
    g.musmast.store_to_c();
    g.network.store_to_c();
    g.nortsong.store_to_c();
    g.nortvars.store_to_c();
    g.opentyr.store_to_c();
    g.opl.store_to_c();
    g.palette.store_to_c();
    g.params.store_to_c();
    g.pcxload.store_to_c();
    g.pcxmast.store_to_c();
    g.picload.store_to_c();
    g.player.store_to_c();
    g.shots.store_to_c();
    g.sndmast.store_to_c();
    g.sprite.store_to_c();
    g.starlib.store_to_c();
    g.swar.store_to_c();
    g.tyrian2.store_to_c();
    g.varz.store_to_c();
    g.vga256d.store_to_c();
    g.vga_palette.store_to_c();
    g.video_scale.store_to_c();
    g.video_scale_hqnx.store_to_c();
    g.xmas.store_to_c();
}
