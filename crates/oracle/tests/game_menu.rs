//! Differential tests for `game_menu.c` / `game_menu.h`.

mod net_support;
mod platform_support;

use platform_support::Both;
use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::game::Halt;
use rustyrian_core::game_menu::{self as gm, State};
use rustyrian_core::lvlmast::{OPTION_NUM, PORT_NUM, POWER_NUM, SHIELD_NUM, SHIP_NUM};
use rustyrian_core::platform::Event;
use rustyrian_core::player::PlayerItems;
use rustyrian_core::video::ScreenId;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::guard_call;
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        joystick_config: 1,
        y_loc: 2,
        y_chg: -3,
        new_pal: 4,
        cur_pal: 5,
        old_pal: 6,
        quik_save: true,
        old_menu: 7,
        back_from_help: true,
        last_direction: -8,
        first_menu9: true,
        palette_changed: true,
        col: -9,
        col_c: 10,
        last_cur_sel: 11,
        cur_menu: 12,
        cur_item_type: 13,
        cur_item: 14,
        cursor: 15,
        left_power: true,
        right_power: true,
        right_power_afford: true,
        current_cube: 3,
        planet_ani: 16,
        planet_ani_wait: 17,
        current_dot_num: 18,
        current_dot_wait: 19,
        nav_x: 1.5,
        nav_y: -2.25,
        new_nav_x: 3.0,
        new_nav_y: 4.5,
        temp_nav_x: -20,
        temp_nav_y: 21,
        ..State::default()
    };
    s.menu_choices[13] = 5;
    s.cur_sel[0] = 2;
    s.planet_dots[4] = 9;
    s.planet_dot_x[4][9] = -100;
    s.planet_dot_y[0][0] = 100;
    s.old_items[1].ship = 3;
    s.old_items[1].weapon[1].power = 5;
    s.old_items[0].super_arcade_mode = 1;
    for (i, c) in s.cube.iter_mut().enumerate() {
        c.title[..3].copy_from_slice(b"T\0x");
        c.header[12] = i as u8;
        c.face_sprite = -(i as i32);
        c.text[89][35] = 0x7f;
        c.text[0][0] = b'A' + i as u8;
        c.last_line = 40 + i as i32;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// The decoded `demo-cycle` fixture at `frame` (decoded once per process).
fn fixture(frame: u64) -> rustyrian_core::game::Snapshot {
    static CACHE: std::sync::Mutex<Vec<(u64, rustyrian_core::game::Snapshot)>> =
        std::sync::Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, s)) = cache.iter().find(|(f, _)| *f == frame) {
        return s.clone();
    }
    let s = diff::fixture("demo-cycle", frame);
    cache.push((frame, s.clone()));
    s
}

/// Case count of the proptests that set up a whole game per case (fixture,
/// `init_video`, full store and full-state comparison): a hard cap, since
/// nightly runs `PROPTEST_CASES=10000`.
const HEAVY_CASES: u32 = 48;

/// Both engines with live video (`init_video`) in the `demo-cycle` fixture
/// state at `frame`: items, shapes, help text and screens as in a game.
fn setup(frame: u64) -> Both {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    let fx = fixture(frame);
    let mut video = d.game.video.clone();
    d.game.restore(fx.clone());
    for id in ScreenId::ALL {
        if let (Some(dst), Some(src)) = (
            video.screens[id as usize].as_mut(),
            fx.video.screens[id as usize].as_ref(),
        ) {
            dst.pixels.copy_from_slice(&src.pixels);
        }
    }
    video.vga_screen = fx.video.vga_screen;
    d.game.video = video;
    d.store();
    d.clear_calls();
    d
}

/// Applies `f` to the Rust game, copies the state into C, runs `c` and
/// `rust`, and asserts equal states and platform calls.
fn run(d: &mut Both, f: impl FnOnce(&mut Game), c: impl FnOnce(), rust: impl FnOnce(&mut Game)) {
    f(&mut d.game);
    d.store();
    d.clear_calls();
    c();
    rust(&mut d.game);
    d.assert_same();
}

#[test]
fn playeritem_map_matches() {
    let _d = Diff::new();
    for i in 0..7u32 {
        let mut c = b::PlayerItems::default();
        // SAFETY: a valid PlayerItems and an in-range row.
        unsafe { *b::ot_s_game_menu_playeritem_map(&raw mut c, i) = 0xAB };
        let mut r = PlayerItems::default();
        *gm::playeritem_map(&mut r, i) = 0xAB;
        let c_fields = [
            c.ship,
            c.weapon[0].id,
            c.weapon[1].id,
            c.shield,
            c.generator,
            c.sidekick[0],
            c.sidekick[1],
            c.weapon[0].power,
            c.weapon[1].power,
            c.special,
        ];
        let r_fields = [
            r.ship,
            r.weapon[0].id,
            r.weapon[1].id,
            r.shield,
            r.generator,
            r.sidekick[0],
            r.sidekick[1],
            r.weapon[0].power,
            r.weapon[1].power,
            r.special,
        ];
        assert_eq!(c_fields, r_fields, "row {i}");
    }
}

proptest! {
    #[test]
    fn je_part_way_matches(start in any::<i16>(), finish in any::<i16>(), dots in any::<u8>(), dist in any::<u8>()) {
        // SAFETY: a pure C function.
        let c = unsafe { b::JE_partWay(start, finish, dots, dist) };
        prop_assert_eq!(c, gm::je_part_way(start, finish, dots, dist));
    }

    /// Any origin and destinations among the 21 planets.
    #[test]
    fn je_compute_dots_matches(origin in 1u16..=21, num in 0u16..=5, planets in any::<[u8; 5]>(),
                               old in any::<[[i16; 10]; 5]>()) {
        let d = Diff::new();
        let mut g = d.game;
        g.varz.map_origin = origin;
        g.varz.map_p_num = num;
        g.varz.map_planet = planets.map(|p| p % 21 + 1);
        g.game_menu.planet_dot_x = old;
        g.game_menu.planet_dots = [7; 5];
        g.varz.store_to_c();
        g.game_menu.store_to_c();
        // SAFETY: plain C call; every index is in range.
        unsafe { b::JE_computeDots() };
        gm::je_compute_dots(&mut g);
        prop_assert_eq!(diff::load::<State>(), g.game_menu);
    }
}

#[test]
fn je_draw_lines_matches() {
    let mut d = setup(2000);
    for id in ScreenId::ALL {
        for dark in [true, false] {
            run(
                &mut d,
                |_| {},
                // SAFETY: a live surface.
                || unsafe { b::JE_drawLines(screen_to_c(Some(id)), dark) },
                |g| gm::je_draw_lines(g, id, dark),
            );
        }
    }
}

/// The navigation-screen state: views anywhere around the map, any
/// animation counters, dot routes, origins, destinations and selections
/// whose indices upstream keeps in range (`curSel` and `menuChoices` of the
/// next-level menu within the 5 destinations).
fn nav_state() -> impl Strategy<Value = NavState> {
    let coord = || prop::array::uniform5(prop::array::uniform10(-50i16..400));
    (
        // the view, and a target near it or anywhere
        (
            -300f32..500.0,
            -300f32..500.0,
            -3f32..3.0,
            -3f32..3.0,
            -300f32..500.0,
            any::<bool>(),
        ),
        // animation counters near their wraps
        (0u8..16, 0u8..4, 0u8..12, 0u8..6),
        (1u16..=21, 0u16..=5, any::<[u8; 5]>()),
        (2u8..=6, 2u8..=6),
        (prop::array::uniform5(0u8..=10), coord(), coord()),
    )
        .prop_map(|((x, y, dx, dy, far, near), counters, map, sel, dots)| {
            let nav = if near {
                (x, y, x + dx, y + dy)
            } else {
                (x, y, far, far - y)
            };
            NavState {
                nav,
                counters,
                map,
                sel,
                dots,
            }
        })
}

#[derive(Clone, Debug)]
struct NavState {
    nav: (f32, f32, f32, f32),
    counters: (u8, u8, u8, u8),
    map: (u16, u16, [u8; 5]),
    sel: (u8, u8),
    dots: ([u8; 5], [[i16; 10]; 5], [[i16; 10]; 5]),
}

impl NavState {
    fn apply(&self, g: &mut Game) {
        let m = &mut g.game_menu;
        (m.nav_x, m.nav_y, m.new_nav_x, m.new_nav_y) = self.nav;
        m.temp_nav_x = self.nav.0.round() as i16;
        m.temp_nav_y = self.nav.1.round() as i16;
        (
            m.planet_ani,
            m.planet_ani_wait,
            m.current_dot_num,
            m.current_dot_wait,
        ) = self.counters;
        m.planet_ani %= 15;
        let next = usize::from(gm::MENU_PLAY_NEXT_LEVEL);
        m.cur_sel[next] = self.sel.0;
        m.menu_choices[next] = self.sel.1;
        (m.planet_dots, m.planet_dot_x, m.planet_dot_y) = self.dots;
        g.varz.map_origin = self.map.0;
        g.varz.map_p_num = self.map.1;
        g.varz.map_planet = self.map.2.map(|p| p % 21 + 1);
    }
}

#[test]
fn je_update_nav_screen_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(&(nav_state(),), |(s,)| {
            let mut d = setup(2000);
            // SAFETY: plain C call; indices in range per the strategy.
            run(
                &mut d,
                |g| s.apply(g),
                || unsafe { b::JE_updateNavScreen() },
                gm::je_update_nav_screen,
            );

            Ok(())
        })
        .unwrap();
}

#[test]
fn je_draw_nav_lines_dots_and_planets_match() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(nav_state(), 0u8..21, any::<bool>()),
            |(s, planet, dark)| {
                let mut d = setup(500);
                // SAFETY: (each) plain C call; indices in range per the strategy.
                run(
                    &mut d,
                    |g| s.apply(g),
                    || unsafe { b::JE_drawNavLines(dark) },
                    |g| gm::je_draw_nav_lines(g, dark),
                );
                // SAFETY: as above.
                run(
                    &mut d,
                    |_| {},
                    || unsafe { b::JE_drawDots() },
                    gm::je_draw_dots,
                );
                // SAFETY: as above.
                run(
                    &mut d,
                    |_| {},
                    || unsafe { b::JE_drawPlanet(planet) },
                    |g| gm::je_draw_planet(g, planet),
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Every menu's header; data-cube headers of any text.
#[test]
fn je_draw_menu_header_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                0i16..14,
                2u8..=5,
                any::<bool>(),
                prop::collection::vec(32u8..127, 0..12),
            ),
            |(menu, cube, save, header)| {
                let mut d = setup(500);
                let f = |g: &mut Game| {
                    g.game_menu.cur_menu = menu;
                    g.game_menu.cur_sel[usize::from(gm::MENU_DATA_CUBES)] = cube;
                    let h = &mut g.game_menu.cube[usize::from(cube) - 2].header;
                    h.fill(0);
                    h[..header.len()].copy_from_slice(&header);
                    g.mainint.perform_save = save;
                };
                // SAFETY: plain C call; indices in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_drawMenuHeader() },
                    gm::je_draw_menu_header,
                );

                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn je_draw_menu_choices_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(0i16..14, 0u8..=11, any::<u8>()),
            |(menu, choices, sel)| {
                let mut d = setup(500);
                let f = |g: &mut Game| {
                    let m = &mut g.game_menu;
                    m.cur_menu = menu;
                    m.menu_choices[menu as usize] = choices;
                    m.cur_sel[menu as usize] = if sel % 2 == 0 { sel % 12 } else { sel };
                };
                // SAFETY: plain C call; indices in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_drawMenuChoices() },
                    gm::je_draw_menu_choices,
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Selections whose help row exists: `menuHelp` entries of 0 would make
/// upstream read before `mainMenuHelp` (game_menu.c:2336), so only
/// nonzero entries are generated for the menus that use the table.
#[test]
fn je_draw_main_menu_help_text_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                (0i16..14).prop_flat_map(|m| {
                    let uses_table = m < 3 || m == 9 || m > 10;
                    let sel: BoxedStrategy<u8> = if m == 12 {
                        (2u8..=17).boxed()
                    } else if uses_table {
                        let valid: Vec<u8> = (0..11u8)
                            .filter(|&t| {
                                rustyrian_core::helptext::MENU_HELP[m as usize][usize::from(t)] != 0
                            })
                            .map(|t| t + 2)
                            .collect();
                        prop::sample::select(valid).boxed()
                    } else {
                        any::<u8>().boxed()
                    };
                    (Just(m), sel)
                }),
                any::<u8>(),
                any::<bool>(),
                any::<bool>(),
                0u16..3,
                any::<bool>(),
            ),
            |((menu, sel), choices, left, right, cubes, key_sel)| {
                let mut d = setup(500);
                let f = |g: &mut Game| {
                    let m = &mut g.game_menu;
                    m.cur_menu = menu;
                    m.cur_sel[menu as usize] = sel;
                    if key_sel && menu == i16::from(gm::MENU_KEYBOARD_CONFIG) {
                        m.cur_sel[menu as usize] = 10;
                    }
                    m.menu_choices[menu as usize] = if choices % 3 == 0 { sel } else { choices };
                    m.left_power = left;
                    m.right_power = right;
                    g.config.cube_max = cubes;
                };
                // SAFETY: plain C call; indices in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_drawMainMenuHelpText() },
                    gm::je_draw_main_menu_help_text,
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Every upgrade row with any available-item list of real items.
#[test]
fn je_gen_item_menu_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                2u8..=8,
                0u8..=10,
                any::<[u8; 10]>(),
                any::<prop::sample::Index>(),
                any::<[u8; 8]>(),
            ),
            |(item, max, avail, pick, items)| {
                let mut d = setup(500);
                let limit = match item {
                    2 => SHIP_NUM,
                    3 | 4 => PORT_NUM,
                    5 => SHIELD_NUM,
                    6 => POWER_NUM,
                    _ => OPTION_NUM,
                } + 1;
                let f = |g: &mut Game| {
                    let slot = usize::from(gm::ITEM_AVAIL_MAP[usize::from(item) - 2]) - 1;
                    g.tyrian2.item_avail_max[slot] = max;
                    g.tyrian2.item_avail[slot] = avail.map(|a| (usize::from(a) % limit) as u8);
                    let p = &mut g.player.player[0].items;
                    *p = PlayerItems {
                        ship: items[0],
                        generator: items[1],
                        shield: items[2],
                        sidekick: [items[3], items[4]],
                        ..*p
                    };
                    p.weapon[0].id = items[5];
                    p.weapon[1].id = items[6];
                    // sometimes the player has one of the listed items
                    if max > 0 && items[7] % 2 == 0 {
                        let have = g.tyrian2.item_avail[slot][pick.index(usize::from(max))];
                        *gm::playeritem_map(&mut g.player.player[0].items, u32::from(item) - 2) =
                            have;
                    }
                };
                // SAFETY: plain C call; indices in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_genItemMenu(item) },
                    |g| gm::je_gen_item_menu(g, item),
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Any destination box inside the source surface's bounds, between any
/// two distinct screens.
#[test]
fn je_scale_bitmap_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                (0i32..320).prop_flat_map(|a| (Just(a), a..320)),
                (0i32..200).prop_flat_map(|a| (Just(a), a..200)),
                0usize..6,
            ),
            |((x1, x2), (y1, y2), pair)| {
                let mut d = setup(2000);
                let pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
                let (dst, src) = (ScreenId::ALL[pairs[pair].0], ScreenId::ALL[pairs[pair].1]);
                run(
                    &mut d,
                    |_| {},
                    // SAFETY: two live surfaces and an in-bounds box.
                    || unsafe {
                        b::JE_scaleBitmap(
                            screen_to_c(Some(dst)),
                            screen_to_c(Some(src)),
                            x1,
                            y1,
                            x2,
                            y2,
                        )
                    },
                    |g| gm::je_scale_bitmap(g, dst, src, x1, y1, x2, y2),
                );

                Ok(())
            },
        )
        .unwrap();
}

/// The weapon simulator from its initial view, over several frames, with
/// any real weapons, powers, modes and sidekicks.
#[test]
fn je_init_weapon_view_and_weapon_view_frame_match() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                0u8..=PORT_NUM as u8,
                0u8..=PORT_NUM as u8,
                (1u8..=11, 1u8..=11),
                1u32..=2,
                0u8..=OPTION_NUM as u8,
                0u8..=OPTION_NUM as u8,
                0u32..40,
                0u32..1200,
                1usize..6,
                any::<[i8; 2]>(),
            ),
            |(front, rear, powers, mode, left, right, power_add, start_power, frames, moves)| {
                let mut d = setup(2000);
                let f = |g: &mut Game| {
                    let p = &mut g.player.player[0];
                    p.items.weapon[0].id = front;
                    p.items.weapon[1].id = rear;
                    p.items.weapon[0].power = powers.0;
                    p.items.weapon[1].power = powers.1;
                    p.weapon_mode = mode;
                    p.items.sidekick = [left, right];
                    g.config.power_add = power_add;
                };
                // SAFETY: plain C call.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_initWeaponView() },
                    gm::je_init_weapon_view,
                );
                for k in 0..frames {
                    let f = |g: &mut Game| {
                        if k == 0 {
                            g.config.power = start_power;
                        }
                        let p = &mut g.player.player[0];
                        p.x += i32::from(moves[0] % 8);
                        p.y += i32::from(moves[1] % 8);
                    };
                    // SAFETY: plain C call; item indices in range per the strategy.
                    run(
                        &mut d,
                        f,
                        || unsafe { b::JE_weaponViewFrame() },
                        gm::je_weapon_view_frame,
                    );
                }

                Ok(())
            },
        )
        .unwrap();
}

/// Every ship with a description (ships 1..=SHIP_NUM).
#[test]
fn je_draw_ship_specs_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                1u8..=SHIP_NUM as u8,
                prop::sample::select(vec![500u64, 2000]),
            ),
            |(ship, frame)| {
                let mut d = setup(frame);
                let f = |g: &mut Game| g.player.player[0].items.ship = ship;
                let (gs, v2) = (Some(ScreenId::GameScreen), Some(ScreenId::VgaScreen2));
                // SAFETY: two live surfaces; the ship has a description.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_drawShipSpecs(screen_to_c(gs), screen_to_c(v2)) },
                    |g| gm::je_draw_ship_specs(g, ScreenId::GameScreen, ScreenId::VgaScreen2),
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Rust only (upstream reads outside `shipInfo`/`ships` for these): ship 0
/// (illustrated from the old items) and ships past the table draw without
/// a description (`D-UB-GAMEMENU-1`).
#[test]
fn je_draw_ship_specs_without_description() {
    for ship in [0u8, 14, 91, 255] {
        let mut d = setup(500);
        d.game.player.player[0].items.ship = ship;
        d.game.game_menu.old_items[0].ship = 2;
        gm::je_draw_ship_specs(&mut d.game, ScreenId::GameScreen, ScreenId::VgaScreen2);
    }
}

fn key(scancode: u16) -> Event {
    Event::KeyDown {
        scancode,
        keycode: 0,
        modifiers: 0,
        repeat: false,
    }
}

/// `JE_scaleInPicture` run to the end, and interrupted by a key press.
#[test]
fn je_scale_in_picture_matches() {
    for interrupt in [false, true] {
        let mut d = setup(2000);
        if interrupt {
            d.inject(key(40));
        }
        let (dst, src) = (ScreenId::VgaScreenSeg, ScreenId::GameScreen);
        let mut c_exit = None;
        run(
            &mut d,
            |_| {},
            || {
                // SAFETY: two live surfaces; exit is caught by the guard.
                c_exit = unsafe {
                    guard_call(|| {
                        b::JE_scaleInPicture(screen_to_c(Some(dst)), screen_to_c(Some(src)))
                    })
                }
            },
            |g| assert_eq!(gm::je_scale_in_picture(g, dst, src), Ok(())),
        );
        assert_eq!(c_exit, None);
    }
}

/// The whole ship-specs sequence, dismissed by a key.
#[test]
fn je_do_ship_specs_matches() {
    for ship in [1u8, 5, 12] {
        let mut d = setup(2000);
        d.game.player.player[0].items.ship = ship;
        // The first key may end the zoom; the second dismisses the screen.
        d.inject(key(44));
        d.inject(key(44));
        let mut c_exit = None;
        run(
            &mut d,
            |_| {},
            // SAFETY: plain C call; exit is caught by the guard.
            || c_exit = unsafe { guard_call(|| b::JE_doShipSpecs()) },
            |g| assert_eq!(gm::je_do_ship_specs(g), Ok(())),
        );
        assert_eq!(c_exit, None);
    }
}

/// The quit dialog answered by keys and mouse clicks, and closed by the
/// window.
#[test]
fn je_quit_request_matches() {
    // window coordinates of screen point (x, y) for the output rectangle
    // `init_video` leaves (mapped back exactly)
    let r = setup(500).game.video.last_output_rect;
    let click = move |x: i32, y: i32| {
        let (wx, wy) = (r.x + x * r.w / 320, r.y + y * r.h / 200);
        vec![
            Event::MouseButtonDown {
                button: 1,
                x: wx,
                y: wy,
            },
            Event::MouseButtonUp {
                button: 1,
                x: wx,
                y: wy,
            },
        ]
    };
    let cases: Vec<Vec<Event>> = vec![
        vec![key(40)],
        vec![key(44)],
        vec![key(41)],
        vec![key(80), key(40)],
        vec![key(79), key(43), key(80), key(40)],
        vec![key(4), key(41)],
        click(100, 130),
        click(200, 130),
        [
            click(10, 10),
            click(150, 140),
            click(100, 160),
            vec![key(40)],
        ]
        .concat(),
        vec![Event::Quit],
    ];
    for (k, events) in cases.into_iter().enumerate() {
        let mut d = setup(500);
        d.game.mouse.has_mouse = k % 2 == 0;
        // A final Escape closes the dialog when the events above do not.
        for e in events.into_iter().chain([key(41)]) {
            d.inject(e);
        }
        let mut c = (None, false);
        let mut r = Ok(false);
        run(
            &mut d,
            |_| {},
            // SAFETY: plain C call; exit is caught by the guard.
            || c.0 = unsafe { guard_call(|| c.1 = b::JE_quitRequest()) },
            |g| r = gm::je_quit_request(g),
        );
        match c.0 {
            Some(code) => assert_eq!(r, Err(Halt { code }), "case {k}"),
            None => assert_eq!(r, Ok(c.1), "case {k}"),
        }
    }
}

/// The quit dialog in a network game, confirmed by Return: the quit packet
/// goes to the opponent and the game halts (exit code 0) in both engines.
#[test]
fn je_quit_request_network_game_matches() {
    use net_support::{Rig, store_net, taps};
    let mut rig = Rig::new(&taps(1000, 120_000));
    rig.video();
    rig.net();
    // SAFETY: plain C call (the real help text loads without exiting).
    unsafe { b::JE_loadHelpText() };
    rustyrian_core::helptext::je_load_help_text(&mut rig.g).expect("help text");
    let mut s = rig.g.network.clone();
    s.connected = true;
    s.is_network_game = true;
    store_net(&mut rig, s);
    rig.clear();
    let r = rig.run(
        // SAFETY: plain C call (inside the guard).
        || unsafe { b::JE_quitRequest() },
        gm::je_quit_request,
    );
    assert_eq!(r, Err(0));
    let sent = rig.check_io();
    assert!(!sent.is_empty(), "the quit packet is sent");
    rig.check_halted();
}

/// One case of every branch of `JE_drawMainMenuHelpText`, independent of
/// the random selection above.
#[test]
fn je_draw_main_menu_help_text_every_branch_matches() {
    // (menu, selection, choices, left power, cube count)
    let cases: [(i16, u8, u8, bool, u16); 8] = [
        (12, 16, 17, false, 1), // joystick configuration
        (0, 2, 7, false, 1),    // help table
        (5, 10, 11, false, 1),  // keyboard configuration, row 10
        (4, 3, 9, true, 1),     // upgrade with power adjusted
        (6, 9, 9, false, 1),    // last row ("done")
        (7, 3, 6, false, 0),    // data cubes, none collected
        (3, 3, 6, false, 1),    // any other row
        (10, 4, 6, false, 1),   // one-player arcade
    ];
    let mut d = setup(500);
    for (menu, sel, choices, left, cubes) in cases {
        let f = |g: &mut Game| {
            let m = &mut g.game_menu;
            m.cur_menu = menu;
            m.cur_sel[menu as usize] = sel;
            m.menu_choices[menu as usize] = choices;
            m.left_power = left;
            m.right_power = false;
            g.config.cube_max = cubes;
        };
        // SAFETY: plain C call; every index is in range.
        run(
            &mut d,
            f,
            || unsafe { b::JE_drawMainMenuHelpText() },
            gm::je_draw_main_menu_help_text,
        );
    }
}

/// Every menu with all of its rows (selection on each row in turn), so every
/// layout branch runs regardless of the random cases above.
#[test]
fn je_draw_menu_choices_every_menu_matches() {
    let mut d = setup(500);
    for menu in 0..14i16 {
        for sel in [2u8, 7, 11] {
            let f = |g: &mut Game| {
                let m = &mut g.game_menu;
                m.cur_menu = menu;
                m.menu_choices[menu as usize] = 11;
                m.cur_sel[menu as usize] = sel;
            };
            // SAFETY: plain C call; every index is in range.
            run(
                &mut d,
                f,
                || unsafe { b::JE_drawMenuChoices() },
                gm::je_draw_menu_choices,
            );
        }
    }
}

/// Item numbers whose illustration tables upstream reads inside: front and
/// rear weapons with a position slot (or none).
fn front_ids() -> Vec<u8> {
    let list = [
        -1, 4, 9, 3, 8, 2, 5, 10, 1, -1, -1, -1, -1, 7, 8, -1, -1, 0, -1, 4, 0, -1, -1, 3, -1, 4,
        -1, 4, -1, -1, -1, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 9,
    ];
    (0..43u8)
        .filter(|&i| i == 0 || list[usize::from(i)] >= 0)
        .collect()
}

fn rear_ids() -> Vec<u8> {
    let list = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, -1, 4, 5, -1, -1, 6, -1, -1, 1, 0, -1, 6,
        -1, 5, -1, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, -1,
    ];
    (0..43u8)
        .filter(|&i| i == 0 || list[usize::from(i)] >= 0)
        .collect()
}

/// A ship number: one of the table's ships, or a custom ship (91..=100)
/// whose graphic number `graphic` (1..=15) is set in `extraShips`; graphics
/// above 7 come from `extraShapes`, filled here with the player-ship sheet.
fn set_ship(g: &mut Game, ship: u8, graphic: u8) {
    if ship > 90 {
        g.editship.extra_ships[usize::from(ship - 91) * 15] = graphic;
        g.editship.extra_shapes = g.sprite.sprite_sheet9.clone();
    }
}

/// Custom-ship graphic numbers upstream can draw: number 6 maps to sprite 0,
/// which makes `blit_sprite2` read before the sheet (varz.c:381 then
/// sprite.c:550; guarded in the port by D-UB-SPRITE-1).
fn custom_graphic() -> impl Strategy<Value = u8> {
    prop::sample::select((1u8..=15).filter(|&n| n != 6).collect::<Vec<_>>())
}

fn ship_strategy() -> impl Strategy<Value = (u8, u8)> {
    (
        prop_oneof![1u8..=SHIP_NUM as u8, 91u8..=100],
        custom_graphic(),
    )
}

// whole game per case (see HEAVY_CASES)

/// Every upgrade row's items (real table entries, custom ships) at any
/// position.
#[test]
fn je_draw_item_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                (0u8..9).prop_flat_map(|t| {
                    let max = match t {
                        1 => SHIP_NUM,
                        2 | 3 => PORT_NUM,
                        4 => SHIELD_NUM,
                        5 => POWER_NUM,
                        6 | 7 => OPTION_NUM,
                        _ => 300,
                    } as u16;
                    let nums: BoxedStrategy<u16> = if t == 1 {
                        prop_oneof![0..=max, 91u16..=100].boxed()
                    } else {
                        (0..=max).boxed()
                    };
                    (Just(t), nums)
                }),
                custom_graphic(),
                any::<u16>(),
                any::<u16>(),
            ),
            |((item_type, item_num), graphic, x, y)| {
                let mut d = setup(500);
                let f = |g: &mut Game| {
                    if item_type == 1 && item_num > 90 {
                        set_ship(g, item_num as u8, graphic);
                    }
                    g.varz.ship_gr_ptr = None;
                };
                let (x, y) = (x % 400, y % 260);
                // SAFETY: plain C call; table indices in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_drawItem(item_type, item_num, x, y) },
                    |g| gm::je_draw_item(g, item_type, item_num, x, y),
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Any equipment the illustration tables cover.
#[test]
fn draw_ship_illustration_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                ship_strategy(),
                1u8..=6,
                prop::sample::select(front_ids()),
                prop::sample::select(rear_ids()),
                (0u8..=OPTION_NUM as u8, 0u8..=OPTION_NUM as u8),
                0u8..=SHIELD_NUM as u8,
            ),
            |((ship, graphic), generator, front, rear, sidekicks, shield)| {
                let mut d = setup(500);
                let f = |g: &mut Game| {
                    set_ship(g, ship, graphic);
                    let it = &mut g.player.player[0].items;
                    it.ship = ship;
                    it.generator = generator;
                    it.weapon[0].id = front;
                    it.weapon[1].id = rear;
                    it.sidekick = [sidekicks.0, sidekicks.1];
                    it.shield = shield;
                };
                // SAFETY: plain C call; every index is in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::draw_ship_illustration() },
                    gm::draw_ship_illustration,
                );

                Ok(())
            },
        )
        .unwrap();
}

/// The simulator frame with every power-control state: weapon rows with
/// and without a selected item, affordable or not, any costs and powers.
#[test]
fn je_weapon_sim_update_matches() {
    // whole game per case: capped (see HEAVY_CASES)
    diff::capped_runner(HEAVY_CASES)
        .run(
            &(
                2u8..=8,
                (2u8..=12).prop_flat_map(|c| (Just(c), 2u8..=12)),
                prop::array::uniform10(0u8..=PORT_NUM as u8),
                (any::<bool>(), any::<bool>(), any::<bool>()),
                any::<(u16, u16)>(),
                (1u8..=11, 1u8..=11),
                ship_strategy(),
                (-20i32..200, -20i32..220),
            ),
            |(upgrades, (choices, sub), avail, power, costs, powers, (ship, graphic), pos)| {
                let mut d = setup(2000);
                let f = |g: &mut Game| {
                    set_ship(g, ship, graphic);
                    let m = &mut g.game_menu;
                    m.cur_sel[usize::from(gm::MENU_UPGRADES)] = upgrades;
                    m.cur_sel[usize::from(gm::MENU_UPGRADE_SUB)] = sub;
                    m.menu_choices[usize::from(gm::MENU_UPGRADE_SUB)] = choices;
                    (m.left_power, m.right_power, m.right_power_afford) = power;
                    let slot = usize::from(gm::ITEM_AVAIL_MAP[usize::from(upgrades) - 2]) - 1;
                    g.tyrian2.item_avail[slot] = avail;
                    (g.mainint.upgrade_cost, g.mainint.downgrade_cost) = costs;
                    let p = &mut g.player.player[0];
                    p.items.ship = ship;
                    p.items.weapon[0].power = powers.0;
                    p.items.weapon[1].power = powers.1;
                    (p.x, p.y) = pos;
                };
                // SAFETY: plain C call; every index is in range per the strategy.
                run(
                    &mut d,
                    f,
                    || unsafe { b::JE_weaponSimUpdate() },
                    gm::je_weapon_sim_update,
                );

                Ok(())
            },
        )
        .unwrap();
}

/// Rust only (upstream reads outside its tables for these): items past the
/// item tables and the illustration tables draw nothing for that part, and
/// selections before the item list count as "None" (`D-UB-GAMEMENU-2`).
#[test]
fn items_outside_the_tables_draw_nothing() {
    let mut d = setup(500);
    let g = &mut d.game;
    for (t, n) in [(1u8, 50u16), (2, 200), (4, 99), (5, 99), (6, 99), (3, 43)] {
        gm::je_draw_item(g, t, n, 10, 10);
    }
    // custom ship graphic 6: sprite 0 (D-UB-SPRITE-1)
    set_ship(g, 91, 6);
    gm::je_draw_item(g, 1, 91, 10, 10);
    for (ship, generator, front, rear, shield) in [
        (0u8, 0u8, 9u8, 1u8, 200u8),
        (14, 7, 60, 60, 11),
        (50, 2, 16, 13, 0),
    ] {
        let it = &mut g.player.player[0].items;
        (
            it.ship,
            it.generator,
            it.weapon[0].id,
            it.weapon[1].id,
            it.shield,
        ) = (ship, generator, front, rear, shield);
        it.sidekick = [200, 40];
        gm::draw_ship_illustration(g);
    }
    g.player.player[0].items.sidekick = [0, 0];
    g.game_menu.cur_sel[usize::from(gm::MENU_UPGRADES)] = 3;
    g.game_menu.cur_sel[usize::from(gm::MENU_UPGRADE_SUB)] = 1;
    g.game_menu.menu_choices[usize::from(gm::MENU_UPGRADE_SUB)] = 5;
    g.player.player[0].items.ship = 1;
    gm::je_weapon_sim_update(g);
}

// ---------------------------------------------------------------------------
// Upgrade costs, data cubes and menu actions: both engines on a timed input
// script (`mainint_support::Rig`), compared by state, calls and saved files.
// ---------------------------------------------------------------------------

#[path = "mainint_support/mod.rs"]
mod mainint_support;

use mainint_support::Rig as ScriptRig;

/// Runs `test` for each generated case on one fixture rig (each case stores
/// the whole game into both engines first).
///
/// Hard cap (not scaled by `PROPTEST_CASES`): every case stores the whole
/// game and compares the whole state. A fixed seed keeps the C line coverage
/// of these functions reproducible.
fn run_on_rig<S: Strategy>(
    cases: u32,
    strategy: S,
    test: impl Fn(&mut ScriptRig, S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    use proptest::test_runner::{Config, TestRng, TestRunner};
    let rig = std::cell::RefCell::new(ScriptRig::from_fixture("ots 1\n", 500));
    let base = rig.borrow().g.snapshot();
    let config = Config {
        cases,
        ..Config::default()
    };
    let rng = TestRng::deterministic_rng(config.rng_algorithm);
    TestRunner::new_with_rng(config, rng)
        .run(&strategy, |v| {
            let mut rig = rig.borrow_mut();
            rig.g.restore(base.clone());
            rig.store();
            rig.clear();
            test(&mut rig, v)
        })
        .unwrap();
}

/// Real items for each upgrade row (2 = ship .. 8 = right sidekick).
fn upgrade_items() -> impl Strategy<Value = (u8, [u8; 7], (u8, u8), u64)> {
    (
        2u8..=8,
        (
            1u8..=SHIP_NUM as u8,
            0u8..=PORT_NUM as u8,
            0u8..=PORT_NUM as u8,
            1u8..=SHIELD_NUM as u8,
            1u8..=POWER_NUM as u8,
            0u8..=OPTION_NUM as u8,
            0u8..=OPTION_NUM as u8,
        ),
        (1u8..=11, 1u8..=11),
        prop_oneof![0u64..100_000, any::<u64>()],
    )
        .prop_map(|(row, it, powers, cash)| {
            (
                row,
                [it.0, it.1, it.2, it.3, it.4, it.5, it.6],
                powers,
                cash,
            )
        })
}

fn set_upgrade(g: &mut Game, row: u8, it: [u8; 7], powers: (u8, u8), cash: u64) {
    g.game_menu.cur_sel[usize::from(gm::MENU_UPGRADES)] = row;
    let p = &mut g.player.player[0];
    for (i, &v) in it.iter().enumerate() {
        *gm::playeritem_map(&mut p.items, i as u32) = v;
    }
    p.items.weapon[0].power = powers.0;
    p.items.weapon[1].power = powers.1;
    p.cash = cash;
}

#[test]
fn je_cash_left_and_draw_score_match() {
    run_on_rig(
        48,
        (upgrade_items(), any::<bool>()),
        |rig, ((row, it, powers, cash), sub)| {
            set_upgrade(&mut rig.g, row, it, powers, cash);
            rig.g.game_menu.cur_menu = if sub { 4 } else { 1 };
            rig.store();
            rig.clear();
            // SAFETY: plain C call; item numbers inside the item tables.
            let r = rig.run(|| unsafe { b::JE_cashLeft() }, |g| Ok(gm::je_cash_left(g)));
            prop_assert!(r.is_ok());
            // SAFETY: as above.
            rig.run(
                || unsafe { b::JE_drawScore() },
                |g| {
                    gm::je_draw_score(g);
                    Ok(())
                },
            )
            .expect("returns");
            rig.check();
            Ok(())
        },
    );
}

/// Every cube of every episode's cube file, into every slot, until the file
/// runs out (where both engines exit with the read error); then
/// `load_cubes` for every collected-cube count.
#[test]
fn load_cube_and_load_cubes_match_on_every_cube_file() {
    let mut rig = ScriptRig::from_fixture("ots 1\n", 500);
    for episode in 1..=4u8 {
        let name = format!("cubetxt{episode}.dat");
        let mut cubes = 0;
        for index in 0..200i32 {
            let f = &mut rig.g.episodes.cube_filename;
            *f = [0; 13];
            f[..name.len()].copy_from_slice(name.as_bytes());
            let slot = index % 4;
            // as `load_cubes` does: text is appended to, so start from empty
            // lines (text left in a slot would run past its last line)
            rig.g.game_menu.cube[slot as usize].text = [[0; 36]; 90];
            rig.store();
            rig.clear();
            let r = rig.run(
                // SAFETY: plain C call; exit is caught by the guard.
                || unsafe { b::load_cube(slot, index) },
                |g| gm::load_cube(g, slot, index),
            );
            let c_state = diff::load::<State>();
            for (k, (c, r)) in c_state
                .cube
                .iter()
                .zip(rig.g.game_menu.cube.iter())
                .enumerate()
            {
                for (l, (x, y)) in c.text.iter().zip(r.text.iter()).enumerate() {
                    assert_eq!(
                        String::from_utf8_lossy(x),
                        String::from_utf8_lossy(y),
                        "{name} cube {index} slot {k} line {l}"
                    );
                }
                assert_eq!(
                    (c.face_sprite, c.last_line, c.title, c.header),
                    (r.face_sprite, r.last_line, r.title, r.header),
                    "{name} cube {index} slot {k}"
                );
            }
            assert_eq!(c_state, rig.g.game_menu, "{name} cube {index}");
            assert_eq!(rig.c_calls(), rig.rust_calls(), "{name} cube {index}");
            if r.is_err() {
                break;
            }
            cubes += 1;
        }
        assert!(cubes >= 4, "{name}: {cubes} cubes");

        for max in 0..=4u16 {
            rig.g.config.cube_max = max;
            rig.g.config.cube_list = [3, 0, 7, 1];
            rig.store();
            rig.clear();
            let r = rig.run(
                // SAFETY: plain C call; exit is caught by the guard.
                || unsafe { b::load_cubes() },
                gm::load_cubes,
            );
            assert_eq!(r, Ok(()), "{name} {max} cubes");
            rig.check();
        }
    }
}

/// Rust only: a missing cube file exits as upstream's open error does, and
/// a collected-cube count past the 4 slots (C writes past `cube`) loads the
/// 4 slots (`D-UB-GAMEMENU-3`).
#[test]
fn load_cubes_guards() {
    let mut rig = ScriptRig::from_fixture("ots 1\n", 500);
    rig.g.config.cube_max = 9;
    rig.g.config.cube_list = [0, 1, 2, 3];
    assert_eq!(gm::load_cubes(&mut rig.g), Ok(()));
    rig.g.episodes.cube_filename[..6].copy_from_slice(b"nope\0\0");
    assert_eq!(gm::load_cube(&mut rig.g, 0, 0), Err(Halt { code: 1 }));
}

/// One `JE_menuFunction` case: the input script, the state before, the
/// selected row, and whether a virtual joystick is attached.
struct MenuCase {
    label: &'static str,
    script: String,
    joystick: bool,
    select: u8,
    setup: Box<dyn Fn(&mut Game)>,
}

fn menu_case(
    label: &'static str,
    menu: u8,
    select: u8,
    setup: impl Fn(&mut Game) + 'static,
) -> MenuCase {
    MenuCase {
        label,
        script: "ots 1\n".to_owned(),
        joystick: false,
        select,
        setup: Box::new(move |g| {
            g.game_menu.cur_menu = i16::from(menu);
            g.game_menu.cur_sel[usize::from(menu)] = select;
            setup(g);
        }),
    }
}

impl MenuCase {
    fn script(mut self, s: String) -> MenuCase {
        self.script = s;
        self
    }

    fn joystick(mut self) -> MenuCase {
        self.joystick = true;
        self
    }
}

fn keys(keys: &[(u32, &str)]) -> String {
    mainint_support::taps(keys, &[])
}

/// Every action of every menu.
fn menu_cases() -> Vec<MenuCase> {
    use gm::*;
    let pad = |extra: &str| format!("ots 1\njoystick \"Pad\" 2 4 1\n{extra}");
    let mut v = vec![
        menu_case("full: cubes", MENU_FULL_GAME, 2, |_| {}),
        menu_case("full: ship specs", MENU_FULL_GAME, 3, |_| {})
            .script(keys(&[(500, "SPACE"), (1500, "SPACE")])),
        menu_case("full: upgrade", MENU_FULL_GAME, 4, |_| {}),
        menu_case("full: options", MENU_FULL_GAME, 5, |_| {}),
        menu_case("full: next level", MENU_FULL_GAME, 6, |g| {
            g.varz.map_origin = 3;
            g.varz.map_p_num = 4;
            g.varz.map_planet = [1, 7, 12, 20, 2];
        }),
        menu_case("full: quit confirmed", MENU_FULL_GAME, 7, |_| {})
            .script(keys(&[(500, "RETURN")])),
        menu_case("full: quit cancelled", MENU_FULL_GAME, 7, |_| {})
            .script(keys(&[(500, "ESCAPE")])),
        menu_case("full: other row", MENU_FULL_GAME, 8, |_| {}),
        menu_case("upgrades: done", MENU_UPGRADES, 9, |_| {}),
        menu_case("options: load", MENU_OPTIONS, 2, |_| {}),
        menu_case("options: save", MENU_OPTIONS, 3, |_| {}),
        menu_case("options: joystick", MENU_OPTIONS, 6, |_| {}),
        menu_case("options: keyboard", MENU_OPTIONS, 7, |_| {}),
        menu_case("options: done", MENU_OPTIONS, 8, |_| {}),
        menu_case("options: other row", MENU_OPTIONS, 4, |_| {}),
        menu_case("next level: exit", MENU_PLAY_NEXT_LEVEL, 5, |g| {
            g.game_menu.menu_choices[usize::from(MENU_PLAY_NEXT_LEVEL)] = 5;
        }),
        menu_case("next level: go", MENU_PLAY_NEXT_LEVEL, 3, |g| {
            g.game_menu.menu_choices[usize::from(MENU_PLAY_NEXT_LEVEL)] = 5;
            g.varz.map_section = [4, 9, 13, 2, 7];
        }),
        menu_case("upgrade sub: to done", MENU_UPGRADE_SUB, 3, |g| {
            g.game_menu.menu_choices[usize::from(MENU_UPGRADE_SUB)] = 6;
            g.game_menu.cur_sel[usize::from(MENU_UPGRADES)] = 3;
        }),
        menu_case("upgrade sub: done", MENU_UPGRADE_SUB, 6, |g| {
            g.game_menu.menu_choices[usize::from(MENU_UPGRADE_SUB)] = 6;
            g.game_menu.cur_sel[usize::from(MENU_UPGRADES)] = 4;
        }),
        menu_case("keyboard: defaults", MENU_KEYBOARD_CONFIG, 10, |g| {
            g.config.key_settings = [1, 2, 3, 4, 5, 6, 7, 8]
        }),
        menu_case("keyboard: done", MENU_KEYBOARD_CONFIG, 11, |_| {}),
        menu_case("keyboard: done (network)", MENU_KEYBOARD_CONFIG, 11, |g| {
            g.network.is_network_game = true
        }),
        menu_case("keyboard: new key", MENU_KEYBOARD_CONFIG, 3, |_| {}).script(keys(&[(700, "A")])),
        menu_case("keyboard: key in use", MENU_KEYBOARD_CONFIG, 2, |_| {})
            .script(keys(&[(300, "DOWN")])),
        menu_case("keyboard: reserved key", MENU_KEYBOARD_CONFIG, 5, |_| {})
            .script(keys(&[(300, "ESCAPE")])),
        menu_case("keyboard: reserved F11", MENU_KEYBOARD_CONFIG, 6, |_| {})
            .script(keys(&[(300, "F11")])),
        menu_case("keyboard: reserved P", MENU_KEYBOARD_CONFIG, 9, |_| {})
            .script(keys(&[(300, "P")])),
        menu_case("keyboard: click cancels", MENU_KEYBOARD_CONFIG, 4, |_| {}).script(
            mainint_support::script(&[(400, "mouse-down 1"), (450, "mouse-up 1")]),
        ),
        menu_case("load/save: back (quick)", MENU_LOAD_SAVE, 13, |g| {
            g.game_menu.quik_save = true;
            g.game_menu.old_menu = 2;
            g.game_menu.old_pal = 7;
        }),
        menu_case("load/save: back", MENU_LOAD_SAVE, 13, |_| {}),
        menu_case("load/save: load empty slot", MENU_LOAD_SAVE, 4, |g| {
            g.mainint.perform_save = false;
            g.game_menu.quik_save = true;
            g.game_menu.old_menu = 1;
        }),
        menu_case("load/save: save (two players)", MENU_LOAD_SAVE, 3, |g| {
            g.mainint.perform_save = true;
            g.config.two_player_mode = true;
        })
        .script(keys(&[(500, "RETURN"), (1500, "RETURN")])),
        menu_case("cubes: back", MENU_DATA_CUBES, 4, |g| {
            g.game_menu.menu_choices[usize::from(MENU_DATA_CUBES)] = 4;
        }),
        menu_case("cubes: read", MENU_DATA_CUBES, 3, |g| {
            g.game_menu.menu_choices[usize::from(MENU_DATA_CUBES)] = 4;
            g.config.cube_max = 2;
        }),
        menu_case("cubes: none", MENU_DATA_CUBES, 2, |g| {
            g.game_menu.menu_choices[usize::from(MENU_DATA_CUBES)] = 4;
            g.config.cube_max = 0;
        }),
        menu_case("cube: back", MENU_DATA_CUBE_SUB, 2, |_| {}),
        menu_case("2p: play", MENU_2_PLAYER_ARCADE, 2, |g| {
            g.varz.map_p_num = 2
        }),
        menu_case("2p: device 1", MENU_2_PLAYER_ARCADE, 3, |g| {
            g.config.input_device = [1, 2]
        }),
        menu_case("2p: device 2", MENU_2_PLAYER_ARCADE, 4, |g| {
            g.config.input_device = [2, 1]
        }),
        menu_case("2p: options", MENU_2_PLAYER_ARCADE, 5, |_| {}),
        menu_case("2p: quit", MENU_2_PLAYER_ARCADE, 6, |_| {}).script(keys(&[(500, "RETURN")])),
        menu_case("1p: play (no sections)", MENU_1_PLAYER_ARCADE, 2, |g| {
            g.varz.map_p_num = 1
        }),
        menu_case("1p: options", MENU_1_PLAYER_ARCADE, 3, |_| {}),
        menu_case("1p: quit", MENU_1_PLAYER_ARCADE, 4, |_| {}).script(keys(&[(500, "ESCAPE")])),
        menu_case("limited: joystick", MENU_LIMITED_OPTIONS, 2, |_| {}),
        menu_case("limited: keyboard", MENU_LIMITED_OPTIONS, 3, |_| {}),
        menu_case("limited: back", MENU_LIMITED_OPTIONS, 6, |_| {}),
        menu_case("limited: other", MENU_LIMITED_OPTIONS, 4, |_| {}),
        menu_case("joystick: none", MENU_JOYSTICK_CONFIG, 3, |_| {}),
        menu_case("joystick: none, done", MENU_JOYSTICK_CONFIG, 17, |_| {}),
        menu_case("super: play", MENU_SUPER_TYRIAN, 2, |g| {
            g.varz.map_p_num = 3
        }),
        menu_case("super: ship specs", MENU_SUPER_TYRIAN, 3, |_| {})
            .script(keys(&[(500, "SPACE"), (1500, "SPACE")])),
        menu_case("super: options", MENU_SUPER_TYRIAN, 4, |_| {}),
        menu_case("super: quit", MENU_SUPER_TYRIAN, 5, |_| {}).script(keys(&[(500, "RETURN")])),
        menu_case("super: other", MENU_SUPER_TYRIAN, 6, |_| {}),
    ];
    for row in 2..=8u8 {
        v.push(menu_case("upgrades: item row", MENU_UPGRADES, row, |_| {}));
    }
    // a joystick: every option, and assignments detected into an empty,
    // a matching and a full slot pair
    for (select, analog) in [
        (2u8, false),
        (3, true),
        (4, true),
        (4, false),
        (5, true),
        (5, false),
        (16, true),
        (17, true),
    ] {
        v.push(
            menu_case("joystick: option", MENU_JOYSTICK_CONFIG, select, move |g| {
                if let Some(j) = g.joystick.joystick.as_mut() {
                    j[0].analog = analog;
                    j[0].sensitivity = 10;
                    j[0].threshold = 3;
                }
            })
            .script(pad(""))
            .joystick(),
        );
    }
    for (label, preset) in [
        ("joystick: assign (empty)", 0u8),
        ("joystick: assign (same)", 1),
        ("joystick: assign (full)", 2),
    ] {
        v.push(
            menu_case(label, MENU_JOYSTICK_CONFIG, 7, move |g| {
                use rustyrian_core::joystick::{
                    JoystickAssignment as A, JoystickAssignmentTypes as T,
                };
                let button = |num| A {
                    r#type: T::Button,
                    num,
                    x_axis: false,
                    negative_axis: false,
                };
                if let Some(j) = g.joystick.joystick.as_mut() {
                    j[0].assignment[1] = match preset {
                        0 => [A::default(), A::default()],
                        1 => [button(5), button(2)],
                        _ => [button(5), button(6)],
                    };
                }
            })
            .script(pad("@600 joy-button 0 2 1\n@700 joy-button 0 2 0\n"))
            .joystick(),
        );
    }
    v
}

/// `JE_menuFunction` for every menu row: state, platform calls and saved
/// files match after each.
#[test]
fn je_menu_function_matches_every_action() {
    for case in menu_cases() {
        let mut rig = ScriptRig::from_fixture(&case.script, 500);
        if case.joystick {
            // SAFETY: plain C call.
            unsafe { b::init_joysticks() };
            rustyrian_core::joystick::init_joysticks(&mut rig.g);
        }
        (case.setup)(&mut rig.g);
        rig.store();
        rig.clear();
        let select = case.select;
        let r = rig.run(
            // SAFETY: plain C call; exit is caught by the guard.
            || unsafe { b::JE_menuFunction(select) },
            |g| gm::je_menu_function(g, select),
        );
        assert_eq!(r, Ok(()), "{}", case.label);
        rig.check();
    }
}
