// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `demo.c` / `demo.h`.

use crate::Game;
use crate::config::{
    DIFFICULTY_NORMAL, KEY_SETTING_CHANGE_FIRE, KEY_SETTING_DOWN, KEY_SETTING_FIRE,
    KEY_SETTING_LEFT, KEY_SETTING_LEFT_SIDEKICK, KEY_SETTING_RIGHT, KEY_SETTING_RIGHT_SIDEKICK,
    KEY_SETTING_UP,
};
use crate::episodes::je_init_episode;
use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::{SDL_LOG_CATEGORY_APPLICATION, log_fatal};
use crate::memreader::{MemReader, mem_read_u8, mem_read_u16be};
use crate::memwriter::{MemWriter, mem_write_u8, mem_write_u16be};
use crate::mtrand::mt_srand;
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, REAR_WEAPON, RIGHT_SIDEKICK};
use crate::varz::CURRENT_KEY_SPEED;
use serde::{Deserialize, Serialize};

/// `seed`: the RNG seed demos play and record with.
pub const SEED: u64 = 32_402_394;

/// Mutable globals of `demo.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub play_demo: bool,
    pub record_demo: bool,
    pub stopped_demo: bool,
    pub demo_num: u8,
    pub demo_file: File,
    pub demo_keys: u8,
    /// FKA `Varz.lastMoveWait`.
    pub demo_keys_wait: u16,
}

/// `logFatal(message)` then `exit(EXIT_FAILURE)`.
fn fatal(g: &mut Game, message: &str) -> Flow {
    log_fatal(g, message);
    Err(Halt { code: EXIT_FAILURE })
}

/// Opens the next shipped demo (`demo.1`..`demo.5` in turn) and reads its
/// header into the game state.
#[doc(alias = "beginPlayDemo")]
pub fn begin_play_demo(g: &mut Game) -> Flow {
    debug_assert!(g.demo.demo_file.f.is_none());

    g.demo.demo_num = g.demo.demo_num.wrapping_add(1);
    if g.demo.demo_num > 5 {
        g.demo.demo_num = 1;
    }

    let demo_filename = format!("demo.{}", g.demo.demo_num);

    g.platform.log_debug(
        SDL_LOG_CATEGORY_APPLICATION,
        &format!("Playing demo '{demo_filename}'."),
    );

    g.demo.demo_file = file::data_file_open(g, &demo_filename, "rb");
    if g.demo.demo_file.error {
        let e = file::file_get_error(&g.demo.demo_file);
        return fatal(g, &format!("Failed to open file '{demo_filename}': {e}"));
    }

    mt_srand(g, SEED);

    g.config.difficulty_level = DIFFICULTY_NORMAL;

    let new_episode = file::file_read_u8(&mut g.demo.demo_file);
    je_init_episode(g, new_episode)?;
    let f = &mut g.demo.demo_file;
    file::file_read_char_array(f, &mut g.config.level_name[..10]);
    g.config.level_name[10] = 0;
    g.varz.lvl_file_num = file::file_read_u8(f);
    let items = &mut g.player.player[0].items;
    items.weapon[FRONT_WEAPON].id = file::file_read_u8(f);
    items.weapon[REAR_WEAPON].id = file::file_read_u8(f);
    items.super_arcade_mode = file::file_read_u8(f);
    items.sidekick[LEFT_SIDEKICK] = file::file_read_u8(f);
    items.sidekick[RIGHT_SIDEKICK] = file::file_read_u8(f);
    items.generator = file::file_read_u8(f);
    items.sidekick_level = file::file_read_u8(f);
    items.sidekick_series = file::file_read_u8(f);
    g.episodes.initial_episode_num = file::file_read_u8(f);
    items.shield = file::file_read_u8(f);
    items.special = file::file_read_u8(f);
    items.ship = file::file_read_u8(f);
    items.weapon[FRONT_WEAPON].power = file::file_read_u8(f);
    items.weapon[REAR_WEAPON].power = file::file_read_u8(f);
    file::file_read_exactly(f, &mut [0u8; 3]); // unused
    g.varz.level_song = file::file_read_u8(f);

    let mut data = [0u8; 2];
    let size = file::file_read_at_most(f, &mut data);
    if f.error {
        let e = file::file_get_error(f);
        return fatal(g, &format!("Failed to read from demo recording file: {e}"));
    }

    let mut reader = MemReader::new(&data[..size]);

    g.demo.demo_keys = 0;
    g.demo.demo_keys_wait = mem_read_u16be(&mut reader);

    debug_assert!(reader.size() == 0 || reader.error);
    Ok(())
}

/// Applies the recorded keys of the next frame to player 1; returns false at
/// the end of the recording.
#[doc(alias = "playDemoKeys")]
pub fn play_demo_keys(g: &mut Game) -> Flow<bool> {
    while g.demo.demo_keys_wait == 0 {
        let mut data = [0u8; 3];
        let f = &mut g.demo.demo_file;
        let size = file::file_read_at_most(f, &mut data);
        if f.error {
            let e = file::file_get_error(f);
            fatal(g, &format!("Failed to read from demo recording file: {e}"))?;
        }

        if size < data.len() {
            return Ok(false);
        }

        let mut reader = MemReader::new(&data);

        g.demo.demo_keys = mem_read_u8(&mut reader);
        g.demo.demo_keys_wait = mem_read_u16be(&mut reader);

        debug_assert!(reader.size() == 0 && !reader.error);
    }

    g.demo.demo_keys_wait -= 1;

    let keys = g.demo.demo_keys;
    let pressed = |k: usize| keys & (1 << k) != 0;
    let p = &mut g.player.player[0];

    if pressed(KEY_SETTING_UP) {
        p.y = p.y.wrapping_sub(CURRENT_KEY_SPEED);
    }
    if pressed(KEY_SETTING_DOWN) {
        p.y = p.y.wrapping_add(CURRENT_KEY_SPEED);
    }

    if pressed(KEY_SETTING_LEFT) {
        p.x = p.x.wrapping_sub(CURRENT_KEY_SPEED);
    }
    if pressed(KEY_SETTING_RIGHT) {
        p.x = p.x.wrapping_add(CURRENT_KEY_SPEED);
    }

    let button = &mut g.mainint.button;
    button[0] = pressed(KEY_SETTING_FIRE);
    button[3] = pressed(KEY_SETTING_CHANGE_FIRE);
    button[1] = pressed(KEY_SETTING_LEFT_SIDEKICK);
    button[2] = pressed(KEY_SETTING_RIGHT_SIDEKICK);

    Ok(true)
}

#[doc(alias = "endPlayDemo")]
pub fn end_play_demo(g: &mut Game) {
    file::file_close(&mut g.demo.demo_file);
}

/// Creates the first free `demorec.N` in the user directory and writes the
/// demo header from the game state.
#[doc(alias = "beginRecordDemo")]
pub fn begin_record_demo(g: &mut Game) -> Flow {
    debug_assert!(g.demo.demo_file.f.is_none());

    let mut new_demo_num: u8 = 1;
    let new_demo_filename = loop {
        let name = format!("demorec.{new_demo_num}");

        if !file::user_file_exists(g, &name) {
            break name;
        }

        if new_demo_num == u8::MAX {
            return fatal(g, "No more demo recording files can be created.");
        }
        new_demo_num += 1;
    };

    g.platform.log_debug(
        SDL_LOG_CATEGORY_APPLICATION,
        &format!("Recording demo '{new_demo_filename}'."),
    );

    g.demo.demo_file = file::user_file_open(g, &new_demo_filename, "wb");
    if g.demo.demo_file.error {
        let e = file::file_get_error(&g.demo.demo_file);
        return fatal(
            g,
            &format!("Failed to open file '{new_demo_filename}': {e}"),
        );
    }

    mt_srand(g, SEED);

    g.config.difficulty_level = DIFFICULTY_NORMAL;

    let name = &mut g.config.level_name;
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    name[len..].fill(0);

    let f = &mut g.demo.demo_file;
    let items = g.player.player[0].items;
    file::file_write_u8(f, g.episodes.episode_num);
    file::file_write_char_array(f, &g.config.level_name[..10]);
    file::file_write_u8(f, g.varz.lvl_file_num);
    file::file_write_u8(f, items.weapon[FRONT_WEAPON].id);
    file::file_write_u8(f, items.weapon[REAR_WEAPON].id);
    file::file_write_u8(f, items.super_arcade_mode);
    file::file_write_u8(f, items.sidekick[LEFT_SIDEKICK]);
    file::file_write_u8(f, items.sidekick[RIGHT_SIDEKICK]);
    file::file_write_u8(f, items.generator);
    file::file_write_u8(f, items.sidekick_level);
    file::file_write_u8(f, items.sidekick_series);
    file::file_write_u8(f, g.episodes.initial_episode_num);
    file::file_write_u8(f, items.shield);
    file::file_write_u8(f, items.special);
    file::file_write_u8(f, items.ship);
    file::file_write_u8(f, items.weapon[FRONT_WEAPON].power);
    file::file_write_u8(f, items.weapon[REAR_WEAPON].power);
    file::file_write_u8_array(f, &[0, 0, 0]); // unused
    file::file_write_u8(f, g.varz.level_song);

    g.demo.demo_keys = 0;
    g.demo.demo_keys_wait = 0;
    Ok(())
}

/// Records this frame's game keys, writing a record whenever they change (or
/// the wait counter is full).
#[doc(alias = "recordDemoKeys")]
pub fn record_demo_keys(g: &mut Game) {
    let old_demo_keys = g.demo.demo_keys;

    g.demo.demo_keys = 0;
    for i in 0..8 {
        let key = g.config.key_settings[i] as usize;
        g.demo.demo_keys |= u8::from(g.keyboard.keysactive[key]) << i;
    }

    if g.demo.demo_keys != old_demo_keys || g.demo.demo_keys_wait == u16::MAX {
        let mut data = [0u8; 3];

        let mut writer = MemWriter::new(&mut data);

        mem_write_u16be(&mut writer, g.demo.demo_keys_wait);
        mem_write_u8(&mut writer, g.demo.demo_keys);

        debug_assert!(writer.size() == 0 && !writer.error);

        file::file_write(&mut g.demo.demo_file, &data);

        g.demo.demo_keys_wait = 0;
    }

    g.demo.demo_keys_wait = g.demo.demo_keys_wait.wrapping_add(1);
}

/// Writes the final wait count and closes the recording.
#[doc(alias = "endRecordDemo")]
pub fn end_record_demo(g: &mut Game) -> Flow {
    let mut data = [0u8; 2];

    let mut writer = MemWriter::new(&mut data);

    mem_write_u16be(&mut writer, g.demo.demo_keys_wait);

    debug_assert!(writer.size() == 0 && !writer.error);

    let f = &mut g.demo.demo_file;
    file::file_write(f, &data);
    file::file_flush(f);

    if f.error {
        let e = file::file_get_error(f);
        return fatal(g, &format!("Failed to write to demo recording file: {e}"));
    }

    file::file_close(&mut g.demo.demo_file);
    Ok(())
}
