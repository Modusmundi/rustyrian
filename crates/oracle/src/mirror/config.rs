//! Mirror of `rustyrian_core::config::State` <-> the C globals of `config.c`.

use super::config_file::{config_from_c, config_to_c};
use super::lvllib::{chars_from_c, chars_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::config::{JeSaveFileType, State};

fn save_from_c(c: &b::JE_SaveFileType) -> JeSaveFileType {
    JeSaveFileType {
        encode: c.encode,
        level: c.level,
        items: c.items,
        score: c.score,
        score2: c.score2,
        level_name: chars_from_c(&c.levelName),
        name: chars_from_c(&c.name),
        cubes: c.cubes,
        power: c.power,
        episode: c.episode,
        last_items: c.lastItems,
        difficulty: c.difficulty,
        secret_hint: c.secretHint,
        input1: c.input1,
        input2: c.input2,
        game_has_repeated: c.gameHasRepeated,
        initial_difficulty: c.initialDifficulty,
        high_score1: c.highScore1,
        high_score2: c.highScore2,
        high_score_name: chars_from_c(&c.highScoreName),
        high_score_diff: c.highScoreDiff,
    }
}

fn save_to_c(r: &JeSaveFileType) -> b::JE_SaveFileType {
    b::JE_SaveFileType {
        encode: r.encode,
        level: r.level,
        items: r.items,
        score: r.score,
        score2: r.score2,
        levelName: chars_to_c(&r.level_name),
        name: chars_to_c(&r.name),
        cubes: r.cubes,
        power: r.power,
        episode: r.episode,
        lastItems: r.last_items,
        difficulty: r.difficulty,
        secretHint: r.secret_hint,
        input1: r.input1,
        input2: r.input2,
        gameHasRepeated: r.game_has_repeated,
        initialDifficulty: r.initial_difficulty,
        highScore1: r.high_score1,
        highScore2: r.high_score2,
        highScoreName: chars_to_c(&r.high_score_name),
        highScoreDiff: r.high_score_diff,
    }
}

/// Copies plain (`Copy`, identical-representation) extern globals.
macro_rules! plain {
    (load $s:ident; $($f:ident = $c:ident),* $(,)?) => {
        $( $s.$f = *global(&raw const b::$c); )*
    };
    (store $s:ident; $($f:ident = $c:ident),* $(,)?) => {
        $( *global_mut(&raw mut b::$c) = $s.$f; )*
    };
}

/// Copies the `Uint8`/`Uint8[4]`/`Uint8[8]` file statics.
macro_rules! statics {
    (load $s:ident; $($f:ident: $t:ty = $g:ident),* $(,)?) => {
        $( $s.$f = *sv::<$t>(b::$g); )*
    };
    (store $s:ident; $($f:ident: $t:ty = $g:ident),* $(,)?) => {
        $( *sv::<$t>(b::$g) = $s.$f; )*
    };
}

macro_rules! all_plain {
    ($m:ident $s:ident) => {
        plain!($m $s;
            smoothies = smoothies,
            star_show_vga_special_code = starShowVGASpecialCode,
            last_cube_max = lastCubeMax,
            cube_max = cubeMax,
            cube_list = cubeList,
            game_has_repeated = gameHasRepeated,
            difficulty_level = difficultyLevel,
            old_difficulty_level = oldDifficultyLevel,
            initial_difficulty = initialDifficulty,
            power = power,
            last_power = lastPower,
            power_add = powerAdd,
            shield_wait = shieldWait,
            shield_t = shieldT,
            shot_repeat = shotRepeat,
            shot_multi_pos = shotMultiPos,
            port_config_change = portConfigChange,
            port_config_done = portConfigDone,
            main_level = mainLevel,
            next_level = nextLevel,
            save_level = saveLevel,
            key_settings = keySettings,
            level_filter = levelFilter,
            level_filter_new = levelFilterNew,
            level_brightness = levelBrightness,
            level_brightness_chg = levelBrightnessChg,
            filtration_avail = filtrationAvail,
            filter_active = filterActive,
            filter_fade = filterFade,
            filter_fade_start = filterFadeStart,
            game_just_loaded = gameJustLoaded,
            galaga_mode = galagaMode,
            extra_game = extraGame,
            two_player_mode = twoPlayerMode,
            two_player_linked = twoPlayerLinked,
            one_player_action = onePlayerAction,
            super_tyrian = superTyrian,
            trent_win = trentWin,
            super_arcade_mode = superArcadeMode,
            super_arcade_power_up = superArcadePowerUp,
            link_gun_direc = linkGunDirec,
            input_device = inputDevice,
            secret_hint = secretHint,
            background3over = background3over,
            background2over = background2over,
            gamma_correction = gammaCorrection,
            super_pause = superPause,
            explosion_transparent = explosionTransparent,
            you_are_cheating = youAreCheating,
            display_score = displayScore,
            background2 = background2,
            smooth_scroll = smoothScroll,
            wild = wild,
            super_wild = superWild,
            star_active = starActive,
            top_enemy_over = topEnemyOver,
            sky_enemy_over_all = skyEnemyOverAll,
            background2not_transparent = background2notTransparent,
            fast_play = fastPlay,
            game_speed = gameSpeed,
            processor_type = processorType,
            editor_item_avail = editorItemAvail,
            editor_level = editorLevel,
        );
        statics!($m $s;
            input_device_: u8 = ot_sv_config_inputDevice_,
            j_configure: u8 = ot_sv_config_jConfigure,
            midi_port: u8 = ot_sv_config_midiPort,
            sound_effects: u8 = ot_sv_config_soundEffects,
            version_num: u8 = ot_sv_config_versionNum,
            joy_button_assign: [u8; 4] = ot_sv_config_joyButtonAssign,
            input_device1: u8 = ot_sv_config_inputDevice1,
            input_device2: u8 = ot_sv_config_inputDevice2,
            dos_key_settings: [u8; 8] = ot_sv_config_dosKeySettings,
        );
    };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each `sv`
        // type is the C type of that static.
        unsafe {
            all_plain!(load self);
            self.last_level_name = chars_from_c(global(&raw const b::lastLevelName));
            self.level_name = chars_from_c(global(&raw const b::levelName));
            let saves = global(&raw const b::saveFiles);
            for (d, s) in self.save_files.iter_mut().zip(saves.iter()) {
                *d = save_from_c(s);
            }
            self.opentyrian_config = config_from_c(global(&raw const b::opentyrian_config));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            all_plain!(store self);
            *global_mut(&raw mut b::lastLevelName) = chars_to_c(&self.last_level_name);
            *global_mut(&raw mut b::levelName) = chars_to_c(&self.level_name);
            let saves = global_mut(&raw mut b::saveFiles);
            for (d, s) in saves.iter_mut().zip(self.save_files.iter()) {
                *d = save_to_c(s);
            }
            config_to_c(
                &self.opentyrian_config,
                global_mut(&raw mut b::opentyrian_config),
            );
        }
    }
}
