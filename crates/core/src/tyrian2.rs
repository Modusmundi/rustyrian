// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `tyrian2.c` / `tyrian2.h`.

use crate::Game;
use crate::config::{DIFFICULTY_SUICIDE, DIFFICULTY_ZINGLON};
use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::keyboard::{INPUT_NO_MOTION, KeyboardInput};
use crate::lvlmast::EVENT_MAXIMUM;
use crate::mtrand::mt_rand;
use crate::sprite::FONT_SHAPES;
use crate::varz::{JeEventRecType, SA_NONE, Sprite2Ref};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// `boss_bar_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossBarT {
    pub link_num: u8,
    pub armor: u8,
    pub color: u8,
}

/// Mutable globals of `tyrian2.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub boss_bar: [BossBarT; 2],
    pub quit: bool,
    pub load_level_ok: bool,
    #[serde(with = "crate::serde_big::boxed")]
    pub event_rec: Box<[JeEventRecType; EVENT_MAXIMUM]>,
    pub level_enemy_max: u16,
    pub level_enemy_frequency: u16,
    #[serde(with = "crate::serde_big")]
    pub level_enemy: [u16; 40],
    pub temp_str: [u8; 31],
    pub item_avail: [[u8; 10]; 9],
    pub item_avail_max: [u8; 9],
}

impl Default for State {
    fn default() -> Self {
        State {
            boss_bar: [BossBarT::default(); 2],
            quit: false,
            load_level_ok: false,
            event_rec: vec![JeEventRecType::default(); EVENT_MAXIMUM]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("length is EVENT_MAXIMUM")),
            level_enemy_max: 0,
            level_enemy_frequency: 0,
            level_enemy: [0; 40],
            temp_str: [0; 31],
            item_avail: [[0; 10]; 9],
            item_avail_max: [0; 9],
        }
    }
}

/// `SDL_SCANCODE_SCROLLLOCK`.
const SDL_SCANCODE_SCROLLLOCK: u16 = 71;

/// `SDL_LOG_CATEGORY_APPLICATION` (upstream's `logWarn`).
const LOG_CATEGORY: i32 = 0;

/// The C string held in `b` (up to its first NUL).
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `blit_enemy(surface, i, x_offset, y_offset, sprite_offset)`: draws enemy
/// `i`'s current animation frame, offset by `sprite_offset`.
pub fn blit_enemy(
    g: &mut Game,
    surface: ScreenId,
    i: u32,
    x_offset: i32,
    y_offset: i32,
    sprite_offset: i32,
) {
    let e = g.varz.enemy[i as usize];
    let Some(sprite2s) = e.sprite2s else {
        return;
    };

    let x = i32::from(e.ex) + x_offset + i32::from(g.backgrnd.temp_map_x_ofs);
    let y = i32::from(e.ey) + y_offset;
    let index = u32::from(e.egr[usize::from(e.enemycycle) - 1]).wrapping_add(sprite_offset as u32);

    if e.filter != 0 {
        crate::sprite::blit_sprite2_filter(g, surface, x, y, sprite2s, index, e.filter);
    } else {
        crate::sprite::blit_sprite2(g, surface, x, y, sprite2s, index);
    }
}

/// `readEpisodeString(file, dst, size)` with `size = dst.len()`.
#[doc(alias = "readEpisodeString")]
pub fn read_episode_string(g: &mut Game, file: &mut File, dst: &mut [u8]) -> Flow {
    crate::helptext::read_encrypted_string(file, Some(dst));

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            String::from_utf8_lossy(c_str(&g.episodes.episode_filename)),
            file::file_get_error(file)
        );
        crate::logging::log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }
    Ok(())
}

/// Waits for a key other than SCROLL LOCK, or a mouse button.
fn wait_for_key_not_scroll_lock(g: &mut Game) -> Flow {
    loop {
        crate::keyboard::wait_until_has_input(g, INPUT_NO_MOTION)?;

        let mut keyboard_input = KeyboardInput::default();
        if (crate::keyboard::keyboard_get_input(g, Some(&mut keyboard_input))
            && keyboard_input.scancode != SDL_SCANCODE_SCROLLLOCK)
            || crate::keyboard::mouse_get_input(g, INPUT_NO_MOTION, None)
        {
            return Ok(());
        }
    }
}

#[doc(alias = "newSuperTyrianGame")]
pub fn new_super_tyrian_game(g: &mut Game) -> Flow {
    // SuperTyrian

    g.config.initial_difficulty = if g.keyboard.keysactive[usize::from(SDL_SCANCODE_SCROLLLOCK)] {
        DIFFICULTY_SUICIDE
    } else {
        DIFFICULTY_ZINGLON
    };

    let vga = g.video.vga_screen_id();
    crate::video::je_clr256(g, vga);
    let out = |g: &mut Game, y: i32, s: &str| {
        crate::fonthand::je_out_text(g, vga, 10, y, s.as_bytes(), 15, 4);
    };
    out(g, 10, "Cheat codes have been disabled.");
    if g.config.initial_difficulty == DIFFICULTY_ZINGLON {
        out(g, 20, "Difficulty level has been set to Lord of Game.");
    } else {
        out(g, 20, "Difficulty level has been set to Suicide.");
    }
    out(
        g,
        30,
        "It is imperative that you discover the special codes.",
    );
    if g.config.initial_difficulty == DIFFICULTY_ZINGLON {
        out(
            g,
            40,
            "(Next time, for an easier challenge hold down SCROLL LOCK.)",
        );
    }
    out(g, 60, "Prepare to play...");

    // snprintf(buf, 10 + 1 + 15 + 1, "%s %s", miscTextB[4], pName[0])
    let mut buf = [0u8; 10 + 1 + 15 + 1];
    let text = [
        c_str(&g.helptext.misc_text_b[4]),
        b" ",
        c_str(&g.helptext.p_name[0]),
    ]
    .concat();
    let n = text.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&text[..n]);
    let x = crate::fonthand::je_font_center(g, &buf, FONT_SHAPES as u32);
    crate::fonthand::je_d_string(g, vga, x, 110, &buf, FONT_SHAPES as u32);

    crate::loudness::play_song(g, 16)?;
    crate::nortsong::je_play_sample_num(g, crate::sndmast::V_DANGER);

    crate::video::je_show_vga(g);
    let colors = g.palette.colors;
    crate::palette::fade_palette(g, &colors, 10, 0, 255)?;

    wait_for_key_not_scroll_lock(g)?;

    crate::episodes::je_init_episode(g, 1)?;
    g.params.constant_die = false;
    g.config.super_tyrian = true;
    g.config.one_player_action = true;
    g.varz.game_loaded = true;
    g.config.difficulty_level = g.config.initial_difficulty;

    let p = &mut g.player.player[0];
    p.cash = 0;

    p.items.ship = 13; // The Stalker 21.126
    p.items.weapon[crate::player::FRONT_WEAPON].id = 39; // Atomic RailGun

    crate::palette::fade_black(g, 10)
}

pub fn intro_logos(g: &mut Game) -> Flow {
    g.varz.move_tyrian_logo_up = true;

    g.video.vga_mut().fill_rect(None, 0);

    crate::palette::fade_white(g, 25)?;

    for (pic, steps) in [(10, 25), (12, 10)] {
        let vga = g.video.vga_screen_id();
        crate::picload::je_load_pic(g, vga, pic, false)?;
        crate::video::je_show_vga(g);

        let colors = g.palette.colors;
        crate::palette::fade_palette(g, &colors, steps, 0, 255)?;

        crate::nortsong::set_frame_count(g, 200);
        crate::keyboard::wait_until_get_input_or_elapsed(g)?;

        crate::palette::fade_black(g, 10)?;
    }
    Ok(())
}

#[doc(alias = "JE_readTextSync")]
pub fn je_read_text_sync(_g: &mut Game) {
    // this function seems to be unnecessary
}

#[doc(alias = "JE_newEnemy")]
pub fn je_new_enemy(
    g: &mut Game,
    enemy_offset: i32,
    e_dat_i: u16,
    unique_shape_table_i: i16,
) -> i16 {
    for i in enemy_offset..enemy_offset + 25 {
        let i = i as usize;
        if g.varz.enemy_avail[i] == 1 {
            g.varz.enemy_avail[i] = je_make_enemy(g, i, e_dat_i, unique_shape_table_i) as u8;
            return (i + 1) as i16;
        }
    }

    0
}

/// `JE_makeEnemy(&enemy[slot], eDatI, uniqueShapeTableI)`. `eDatI` indexes
/// `enemyDat`; the level loader keeps event data within it.
#[doc(alias = "JE_makeEnemy")]
pub fn je_make_enemy(
    g: &mut Game,
    slot: usize,
    mut e_dat_i: u16,
    unique_shape_table_i: i16,
) -> u32 {
    let avail: u32;

    if g.config.super_arcade_mode != SA_NONE && e_dat_i == 534 {
        e_dat_i = 533;
    }
    let dat = g.episodes.enemy_dat[usize::from(e_dat_i)];

    let shape_table_i: u8 = if unique_shape_table_i > 0 {
        unique_shape_table_i as u8
    } else {
        dat.shapebank
    };

    let mut sprite2s = None;
    if shape_table_i == 21 {
        sprite2s = Some(Sprite2Ref::SpriteSheet11); // Coins&Gems
    } else if shape_table_i == 26 {
        sprite2s = Some(Sprite2Ref::SpriteSheet10); // Two-Player Stuff
    } else {
        for (i, &id) in g.sprite.enemy_sprite_sheet_ids.iter().enumerate() {
            if shape_table_i == id {
                sprite2s = Some(Sprite2Ref::EnemySpriteSheets(i as u8));
            }
        }
    }

    if sprite2s.is_some() {
        g.varz.enemy[slot].sprite2s = sprite2s;
    } else {
        // Use shape table value from previous enemy that occupied the enemy
        // slot. (Ex. APPROACH.)
        let message = format!("Ignoring sprite from unloaded shape table {shape_table_i}.");
        g.platform.log_warn(LOG_CATEGORY, &message);
    }

    if g.varz.enemy[slot].sprite2s.is_none() {
        g.platform
            .log_warn(LOG_CATEGORY, "Enemy sprite is missing.");
    }

    // `startx + (mt_rand() % (startxc * 2)) - startxc + 1`, computed in
    // `unsigned long` as C does, then stored in a 16-bit field.
    let start = |g: &mut Game, s: i16, c: i8| -> i16 {
        if c != 0 {
            let m = (i64::from(c) * 2) as u64;
            let r = mt_rand(g) % m;
            (i64::from(s) as u64)
                .wrapping_add(r)
                .wrapping_sub(i64::from(c) as u64)
                .wrapping_add(1) as i16
        } else {
            s.wrapping_add(1)
        }
    };
    let ex = start(g, dat.startx, dat.startxc);
    let ey = start(g, dat.starty, dat.startyc);

    let difficulty_level = g.config.difficulty_level;
    let enemy = &mut g.varz.enemy[slot];

    enemy.enemydatofs = Some(usize::from(e_dat_i));

    enemy.mapoffset = 0;

    enemy.eshotmultipos = [0; 3];

    enemy.enemyground = (dat.explosiontype & 1) == 0;
    enemy.explonum = dat.explosiontype >> 1;

    enemy.launchfreq = dat.elaunchfreq;
    enemy.launchwait = dat.elaunchfreq;
    enemy.launchtype = dat.elaunchtype % 1000;
    enemy.launchspecial = (dat.elaunchtype / 1000) as u8;

    enemy.xaccel = dat.xaccel as u8;
    enemy.yaccel = dat.yaccel as u8;

    // Far enough away to be impossible to reach
    enemy.xminbounce = -10000;
    enemy.xmaxbounce = 10000;
    enemy.yminbounce = -10000;
    enemy.ymaxbounce = 10000;

    enemy.tur = dat.tur;

    enemy.ani = dat.ani;
    enemy.animin = 1;

    match dat.animate {
        0 => {
            enemy.enemycycle = 1;
            enemy.aniactive = 0;
            enemy.animax = 0;
            enemy.aniwhenfire = 0;
        }
        1 => {
            enemy.enemycycle = 0;
            enemy.aniactive = 1;
            enemy.animax = 0;
            enemy.aniwhenfire = 0;
        }
        2 => {
            enemy.enemycycle = 1;
            enemy.aniactive = 2;
            enemy.animax = enemy.ani;
            enemy.aniwhenfire = 2;
        }
        _ => {}
    }

    enemy.ex = ex;
    enemy.ey = ey;

    enemy.exc = dat.xmove;
    enemy.eyc = dat.ymove;
    enemy.excc = dat.xcaccel;
    enemy.eycc = dat.ycaccel;
    enemy.exccw = i32::from(enemy.excc).abs() as i8;
    enemy.exccwmax = enemy.exccw as u8;
    enemy.eyccw = i32::from(enemy.eycc).abs() as i8;
    enemy.eyccwmax = enemy.eyccw as u8;
    enemy.exccadd = if enemy.excc > 0 { 1 } else { -1 };
    enemy.eyccadd = if enemy.eycc > 0 { 1 } else { -1 };
    enemy.special = false;
    enemy.iced = 0;

    enemy.exrev = match dat.xrev {
        0 => 100,
        -99 => 0,
        v => v,
    };
    enemy.eyrev = match dat.yrev {
        0 => 100,
        -99 => 0,
        v => v,
    };

    enemy.exca = if enemy.xaccel > 0 { 1 } else { -1 };
    enemy.eyca = if enemy.yaccel > 0 { 1 } else { -1 };

    enemy.enemytype = e_dat_i;

    for i in 0..3 {
        enemy.eshotwait[i] = if enemy.tur[i] == 252 {
            1
        } else if enemy.tur[i] > 0 {
            20
        } else {
            255
        };
    }
    enemy.egr = dat.egraphic;
    enemy.size = dat.esize;
    enemy.linknum = 0;
    enemy.edamaged = dat.dani < 0;
    enemy.enemydie = dat.eenemydie;

    enemy.freq = dat.freq;

    enemy.edani = dat.dani;
    enemy.edgr = dat.dgr;
    enemy.edlevel = dat.dlevel;

    enemy.fixedmovey = 0;

    enemy.filter = 0x00;

    let value = i32::from(dat.value);
    let mut temp_value: i32 = 0;
    if value > 1 && value < 10000 {
        let scaled = |f: f32| (value as f32 * f) as i32;
        match difficulty_level {
            -1 | 0 => temp_value = scaled(0.75),
            1 | 2 => temp_value = value,
            3 => temp_value = scaled(1.125),
            4 => temp_value = scaled(1.5),
            5 => temp_value = value * 2,
            6 => temp_value = scaled(2.5),
            7 | 8 => temp_value = value * 4,
            9 | 10 => temp_value = value * 8,
            _ => {}
        }
        if temp_value > 10000 {
            temp_value = 10000;
        }
        enemy.evalue = temp_value as i16;
    } else {
        enemy.evalue = dat.value;
    }

    if dat.armor > 0 {
        let armor = i32::from(dat.armor);
        let mut temp_armor: i32 = 1;
        if dat.armor != 255 {
            let scaled = |f: f32| (armor as f32 * f) as i32;
            let scaled_plus_one = |f: f32| (armor as f32 * f + 1.0) as i32;
            match difficulty_level {
                -1 | 0 => temp_armor = scaled_plus_one(0.5),
                1 => temp_armor = scaled_plus_one(0.75),
                2 => temp_armor = armor,
                3 => temp_armor = scaled(1.2),
                4 => temp_armor = scaled(1.5),
                5 => temp_armor = scaled(1.8),
                6 => temp_armor = armor * 2,
                7 => temp_armor = armor * 3,
                8 => temp_armor = armor * 4,
                9 | 10 => temp_armor = armor * 8,
                _ => {}
            }

            if temp_armor > 254 {
                temp_armor = 254;
            }
        } else {
            temp_armor = 255;
        }

        enemy.armorleft = temp_armor as u8;

        avail = 0;
        enemy.scoreitem = false;
    } else {
        avail = 2;
        enemy.armorleft = 255;
        if enemy.evalue != 0 {
            enemy.scoreitem = true;
        }
    }

    if !enemy.scoreitem {
        g.varz.total_enemy = g.varz.total_enemy.wrapping_add(1); // Destruction ratio
    }

    // indicates what to set ENEMYAVAIL to
    avail
}

#[doc(alias = "JE_createNewEventEnemy")]
pub fn je_create_new_event_enemy(
    g: &mut Game,
    enemy_type_ofs: u8,
    enemy_offset: u16,
    unique_shape_table_i: i16,
) {
    g.varz.b = 0;

    for i in enemy_offset..enemy_offset + 25 {
        if g.varz.enemy_avail[usize::from(i)] == 1 {
            g.varz.b = (i + 1) as i16;
            break;
        }
    }

    if g.varz.b == 0 {
        return;
    }

    let ev = g.tyrian2.event_rec[usize::from(g.varz.event_loc) - 1];
    g.varz.temp_w = (i32::from(ev.eventdat) + i32::from(enemy_type_ofs)) as u16;

    let slot = (g.varz.b - 1) as usize;
    g.varz.enemy_avail[slot] = je_make_enemy(g, slot, g.varz.temp_w, unique_shape_table_i) as u8;

    let bk = &g.backgrnd;
    let (map_x, map_x3) = (i32::from(bk.map_x), i32::from(bk.map_x3));
    let back_moves = (bk.back_move, bk.back_move2, bk.back_move3);
    let (background3x1, background3x1b) = (g.varz.background3x1, g.varz.background3x1b);
    let small_enemy_adjust = g.varz.small_enemy_adjust;
    let e = &mut g.varz.enemy[slot];
    let sub = |v: i16, d: u16| (i32::from(v) - i32::from(d)) as i16;

    if ev.eventdat2 != -99 {
        let dat2 = i32::from(ev.eventdat2);
        match enemy_offset {
            0 => {
                e.ex = (dat2 - (map_x - 1) * 24) as i16;
                e.ey = sub(e.ey, back_moves.1);
            }
            25 | 75 => {
                e.ex = (dat2 - (map_x - 1) * 24 - 12) as i16;
                e.ey = sub(e.ey, back_moves.0);
            }
            50 => {
                e.ex = if background3x1 {
                    (dat2 - (map_x - 1) * 24 - 12) as i16
                } else {
                    (dat2 - map_x3 * 24 - 24 * 2 + 6) as i16
                };
                e.ey = sub(e.ey, back_moves.2);

                if background3x1b {
                    e.ex = e.ex.wrapping_sub(6);
                }
            }
            _ => {}
        }
        e.ey = -28;
        if background3x1b && enemy_offset == 50 {
            e.ey += 4;
        }
    }

    if small_enemy_adjust && e.size == 0 {
        e.ex = e.ex.wrapping_sub(10);
        e.ey = e.ey.wrapping_sub(7);
    }

    e.ey = e.ey.wrapping_add(i16::from(ev.eventdat5));
    e.eyc = e.eyc.wrapping_add(ev.eventdat3);
    e.linknum = ev.eventdat4;
    e.fixedmovey = i16::from(ev.eventdat6);
}

/// `JE_eventJump(jump)`: 65535 returns to the saved location. The event list
/// holds an event at or after every location the level script jumps to.
#[doc(alias = "JE_eventJump")]
pub fn je_event_jump(g: &mut Game, jump: u16) {
    let v = &mut g.varz;
    if jump == 65535 {
        v.cur_loc = v.return_loc;
    } else {
        v.return_loc = v.cur_loc.wrapping_add(1);
        v.cur_loc = jump;
    }
    let mut temp_w: u16 = 0;
    loop {
        temp_w = temp_w.wrapping_add(1);
        if g.tyrian2.event_rec[usize::from(temp_w) - 1].eventtime >= v.cur_loc {
            break;
        }
    }
    v.event_loc = temp_w - 1;
}

/// `JE_searchFor(PLType, out_index)`: whether a live enemy has link number
/// `pl_type`; `out_index` receives the last such enemy.
#[doc(alias = "JE_searchFor")]
pub fn je_search_for(g: &mut Game, pl_type: u8, out_index: Option<&mut u8>) -> bool {
    let mut found_id: Option<usize> = None;

    for i in 0..100 {
        if g.varz.enemy_avail[i] == 0 && g.varz.enemy[i].linknum == pl_type {
            found_id = Some(i);
            if g.config.galaga_mode {
                let e = &mut g.varz.enemy[i];
                e.evalue = e.evalue.wrapping_add(e.evalue);
            }
        }
    }

    match found_id {
        Some(i) => {
            if let Some(out) = out_index {
                *out = i as u8;
            }
            true
        }
        None => false,
    }
}

/// The pixel buffers `JE_whoa` works on, taken out of their surfaces while
/// they are processed.
fn take_pixels(g: &mut Game, id: ScreenId) -> Vec<u8> {
    std::mem::take(&mut g.video.screen_mut(id).pixels)
}

#[doc(alias = "JE_whoa")]
pub fn je_whoa(g: &mut Game) -> Flow {
    // 'whoa' gets us that nifty screen fade used when you type in 'engage'.
    // game_screen and VGAScreen2 serve as the two temporary buffers; their
    // roles swap every loop.
    let seg = g.video.vga_screen_seg.expect("VGAScreenSeg");
    let mut temp1 = g.video.game_screen.expect("game_screen");
    let mut temp2 = g.video.vga_screen2.expect("VGAScreen2");

    let s = g.video.screen(seg);
    let pitch = s.pitch as usize;
    let screen_size = s.h as usize * pitch;
    let top_border = pitch * 4; // Seems an arbitrary number of lines
    let bottom_border = pitch * 7;

    debug_assert!(
        g.video.screen(temp2).pixels.len() >= screen_size
            && g.video.screen(temp1).pixels.len() >= screen_size
    );

    // Clear the top and bottom borders. We don't want to process them and
    // we don't want to draw them.
    let s = g.video.screen_mut(seg);
    s.pixels[..top_border].fill(0);
    s.pixels[screen_size - bottom_border..screen_size].fill(0);

    // Copy our test subject to one of the temporary buffers. Blank the other.
    g.video.screen_mut(temp1).pixels[..screen_size].fill(0);
    let (src, dst) = g.video.screen_pair_mut(seg, temp2);
    dst.pixels[..screen_size].copy_from_slice(&src.pixels[..screen_size]);

    for _ in 0..300 {
        crate::nortsong::set_frame_count(g, 1);

        let mut t1 = take_pixels(g, temp1);
        let t2 = take_pixels(g, temp2);
        let mut i = screen_size - bottom_border;
        let mut j = top_border / 2;
        while i > 0 {
            let offset = j + i / 8192 - 4;
            let color = (u32::from(t2[offset]) * 12
                + u32::from(t1[offset - pitch])
                + u32::from(t1[offset - 1])
                + u32::from(t1[offset + 1])
                + u32::from(t1[offset + pitch]))
                / 16;

            t1[j] = color as u8;
            i -= 1;
            j += 1;
        }

        // Now copy that mess to the buffer.
        let n = screen_size - bottom_border;
        g.video.screen_mut(seg).pixels[top_border..top_border + n]
            .copy_from_slice(&t1[top_border..top_border + n]);
        g.video.screen_mut(temp1).pixels = t1;
        g.video.screen_mut(temp2).pixels = t2;

        crate::video::je_show_vga(g);

        crate::keyboard::wait_until_elapsed(g)?;

        let mut keyboard_input = KeyboardInput::default();
        if (crate::keyboard::keyboard_get_input(g, Some(&mut keyboard_input))
            && keyboard_input.scancode != SDL_SCANCODE_SCROLLLOCK)
            || crate::keyboard::mouse_get_input(g, INPUT_NO_MOTION, None)
        {
            break;
        }

        // Flip the buffer.
        std::mem::swap(&mut temp1, &mut temp2);
    }

    g.fonthand.level_warning_lines = 4;
    Ok(())
}

#[doc(alias = "JE_barX")]
pub fn je_bar_x(g: &mut Game, x1: u16, y1: u16, x2: u16, y2: u16, col: u8) {
    let vga = g.video.vga_screen_id();
    let (x1, y1, x2, y2) = (i32::from(x1), i32::from(y1), i32::from(x2), i32::from(y2));
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y1, x2, y1, col.wrapping_add(1));
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y1 + 1, x2, y2 - 1, col);
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y2, x2, y2, col.wrapping_sub(1));
}

pub fn draw_boss_bar(g: &mut Game) {
    for b in 0..2 {
        if g.tyrian2.boss_bar[b].link_num == 0 {
            continue;
        }

        let mut armor: u32 = 256; // higher than armor max

        for e in 0..100 {
            // find most damaged
            let en = &g.varz.enemy[e];
            if g.varz.enemy_avail[e] != 1
                && en.linknum == g.tyrian2.boss_bar[b].link_num
                && u32::from(en.armorleft) < armor
            {
                armor = u32::from(en.armorleft);
            }
        }

        let bar = &mut g.tyrian2.boss_bar[b];
        if armor > 255 || armor == 0 {
            // boss dead?
            bar.link_num = 0;
        } else {
            // 255 would make the bar too long
            bar.armor = if armor == 255 { 254 } else { armor as u8 };
        }
    }

    let bars = u32::from(g.tyrian2.boss_bar[0].link_num != 0)
        + u32::from(g.tyrian2.boss_bar[1].link_num != 0);

    // if only one bar left, make it the first one
    if bars == 1 && g.tyrian2.boss_bar[0].link_num == 0 {
        g.tyrian2.boss_bar[0] = g.tyrian2.boss_bar[1];
        g.tyrian2.boss_bar[1].link_num = 0;
    }

    for b in 0..bars as usize {
        let x: u32 = if bars == 2 {
            if b == 0 { 125 } else { 185 }
        } else if g.varz.level_timer {
            250 // level timer and boss bar would overlap
        } else {
            155
        };

        je_bar_x(g, (x - 25) as u16, 7, (x + 25) as u16, 12, 115);
        let bar = g.tyrian2.boss_bar[b];
        je_bar_x(
            g,
            (x - u32::from(bar.armor) / 10) as u16,
            7,
            (x + (u32::from(bar.armor) + 5) / 10) as u16,
            12,
            118u8.wrapping_add(bar.color),
        );

        if bar.color > 0 {
            g.tyrian2.boss_bar[b].color -= 1;
        }
    }
}
