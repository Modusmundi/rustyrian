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

/// Cases for the proptests that set up a whole game per case: a fifth of
/// `PROPTEST_CASES` (default 256).
fn heavy_cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .map_or(48, |n| (n / 5).max(1))
}

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

proptest! {
    #![proptest_config(ProptestConfig { cases: heavy_cases(), ..ProptestConfig::default() })]

    #[test]
    fn je_update_nav_screen_matches(s in nav_state()) {
        let mut d = setup(2000);
        // SAFETY: plain C call; indices in range per the strategy.
        run(&mut d, |g| s.apply(g), || unsafe { b::JE_updateNavScreen() }, gm::je_update_nav_screen);
    }

    #[test]
    fn je_draw_nav_lines_dots_and_planets_match(s in nav_state(), planet in 0u8..21, dark in any::<bool>()) {
        let mut d = setup(500);
        // SAFETY: (each) plain C call; indices in range per the strategy.
        run(&mut d, |g| s.apply(g), || unsafe { b::JE_drawNavLines(dark) }, |g| gm::je_draw_nav_lines(g, dark));
        // SAFETY: as above.
        run(&mut d, |_| {}, || unsafe { b::JE_drawDots() }, gm::je_draw_dots);
        // SAFETY: as above.
        run(&mut d, |_| {}, || unsafe { b::JE_drawPlanet(planet) }, |g| gm::je_draw_planet(g, planet));
    }

    /// Every menu's header; data-cube headers of any text.
    #[test]
    fn je_draw_menu_header_matches(menu in 0i16..14, cube in 2u8..=5, save in any::<bool>(),
                                   header in prop::collection::vec(32u8..127, 0..12)) {
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
        run(&mut d, f, || unsafe { b::JE_drawMenuHeader() }, gm::je_draw_menu_header);
    }

    #[test]
    fn je_draw_menu_choices_matches(menu in 0i16..14, choices in 0u8..=11, sel in any::<u8>()) {
        let mut d = setup(500);
        let f = |g: &mut Game| {
            let m = &mut g.game_menu;
            m.cur_menu = menu;
            m.menu_choices[menu as usize] = choices;
            m.cur_sel[menu as usize] = if sel % 2 == 0 { sel % 12 } else { sel };
        };
        // SAFETY: plain C call; indices in range per the strategy.
        run(&mut d, f, || unsafe { b::JE_drawMenuChoices() }, gm::je_draw_menu_choices);
    }

    /// Selections whose help row exists: `menuHelp` entries of 0 would make
    /// upstream read before `mainMenuHelp` (game_menu.c:2336), so only
    /// nonzero entries are generated for the menus that use the table.
    #[test]
    fn je_draw_main_menu_help_text_matches(
        (menu, sel) in (0i16..14).prop_flat_map(|m| {
            let uses_table = m < 3 || m == 9 || m > 10;
            let sel: BoxedStrategy<u8> = if m == 12 {
                (2u8..=17).boxed()
            } else if uses_table {
                let valid: Vec<u8> = (0..11u8)
                    .filter(|&t| rustyrian_core::helptext::MENU_HELP[m as usize][usize::from(t)] != 0)
                    .map(|t| t + 2)
                    .collect();
                prop::sample::select(valid).boxed()
            } else {
                any::<u8>().boxed()
            };
            (Just(m), sel)
        }),
        choices in any::<u8>(),
        left in any::<bool>(),
        right in any::<bool>(),
        cubes in 0u16..3,
        key_sel in any::<bool>(),
    ) {
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
        run(&mut d, f, || unsafe { b::JE_drawMainMenuHelpText() }, gm::je_draw_main_menu_help_text);
    }

    /// Every upgrade row with any available-item list of real items.
    #[test]
    fn je_gen_item_menu_matches(item in 2u8..=8, max in 0u8..=10, avail in any::<[u8; 10]>(),
                                pick in any::<prop::sample::Index>(), items in any::<[u8; 8]>()) {
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
                *gm::playeritem_map(&mut g.player.player[0].items, u32::from(item) - 2) = have;
            }
        };
        // SAFETY: plain C call; indices in range per the strategy.
        run(&mut d, f, || unsafe { b::JE_genItemMenu(item) }, |g| gm::je_gen_item_menu(g, item));
    }

    /// Any destination box inside the source surface's bounds, between any
    /// two distinct screens.
    #[test]
    fn je_scale_bitmap_matches(
        (x1, x2) in (0i32..320).prop_flat_map(|a| (Just(a), a..320)),
        (y1, y2) in (0i32..200).prop_flat_map(|a| (Just(a), a..200)),
        pair in 0usize..6,
    ) {
        let mut d = setup(2000);
        let pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
        let (dst, src) = (ScreenId::ALL[pairs[pair].0], ScreenId::ALL[pairs[pair].1]);
        run(
            &mut d,
            |_| {},
            // SAFETY: two live surfaces and an in-bounds box.
            || unsafe { b::JE_scaleBitmap(screen_to_c(Some(dst)), screen_to_c(Some(src)), x1, y1, x2, y2) },
            |g| gm::je_scale_bitmap(g, dst, src, x1, y1, x2, y2),
        );
    }

    /// The weapon simulator from its initial view, over several frames, with
    /// any real weapons, powers, modes and sidekicks.
    #[test]
    fn je_init_weapon_view_and_weapon_view_frame_match(
        front in 0u8..=PORT_NUM as u8, rear in 0u8..=PORT_NUM as u8,
        powers in (1u8..=11, 1u8..=11), mode in 1u32..=2,
        left in 0u8..=OPTION_NUM as u8, right in 0u8..=OPTION_NUM as u8,
        power_add in 0u32..40, start_power in 0u32..1200,
        frames in 1usize..6, moves in any::<[i8; 2]>(),
    ) {
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
        run(&mut d, f, || unsafe { b::JE_initWeaponView() }, gm::je_init_weapon_view);
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
            run(&mut d, f, || unsafe { b::JE_weaponViewFrame() }, gm::je_weapon_view_frame);
        }
    }

    /// Every ship with a description (ships 1..=SHIP_NUM).
    #[test]
    fn je_draw_ship_specs_matches(ship in 1u8..=SHIP_NUM as u8, frame in prop::sample::select(vec![500u64, 2000])) {
        let mut d = setup(frame);
        let f = |g: &mut Game| g.player.player[0].items.ship = ship;
        let (gs, v2) = (Some(ScreenId::GameScreen), Some(ScreenId::VgaScreen2));
        // SAFETY: two live surfaces; the ship has a description.
        run(&mut d, f, || unsafe { b::JE_drawShipSpecs(screen_to_c(gs), screen_to_c(v2)) },
            |g| gm::je_draw_ship_specs(g, ScreenId::GameScreen, ScreenId::VgaScreen2));
    }
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
