//! Differential tests for `menus.c` / `menus.h`.

use proptest::prelude::*;
use rustyrian_core::menus::{self, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

mod net_support;
use net_support::Rig;
mod menus_support;
use menus_support::{Act, any_act, any_name, output_rect, script};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, row) in s.episode_name.iter_mut().enumerate() {
        row[..5].copy_from_slice(b"Ep #\0");
        row[3] = b'0' + i as u8;
        row[30] = 0xff;
    }
    for (i, row) in s.difficulty_name.iter_mut().enumerate() {
        row[0] = b'a' + i as u8;
        row[20] = 0x80;
    }
    for (i, row) in s.gameplay_name.iter_mut().enumerate() {
        row[0] = b'A' + i as u8;
        row[25] = 1;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// The keys every menu handles, and some it ignores.
fn menu_keys() -> Vec<Act> {
    vec![
        Act::Key("UP"),
        Act::Key("DOWN"),
        Act::Key("UP"),
        Act::Key("DOWN"),
        Act::Key("SPACE"),
        Act::Key("RETURN"),
        Act::Key("ESCAPE"),
        Act::Key("A"),
    ]
}

/// Runs menu `which` (0 gameplay, 1 episode, 2 difficulty) on both engines
/// with the given interactions and names; compares the outcome, every call
/// and the whole state. Returns the game's `difficultyLevel` afterwards.
fn run_menu(which: u8, acts: &[(Act, u32)], names: &State, avail: [bool; 5]) -> i8 {
    let mut rig = Rig::new(&script(output_rect(), 300, acts));
    rig.video();
    rig.g.menus = names.clone();
    rig.g.episodes.episode_avail = avail;
    rig.store();
    rig.clear();
    let r = match which {
        // SAFETY: plain C call inside the guard.
        0 => rig.run(|| unsafe { b::gameplaySelect() }, menus::gameplay_select),
        // SAFETY: plain C call inside the guard.
        1 => rig.run(|| unsafe { b::episodeSelect() }, menus::episode_select),
        // SAFETY: plain C call inside the guard.
        _ => rig.run(
            || unsafe { b::difficultySelect() },
            menus::difficulty_select,
        ),
    };
    assert!(r.is_ok(), "menu {which} exited");
    rig.check();
    rig.g.config.difficulty_level
}

fn any_names() -> impl Strategy<Value = State> {
    (
        prop::array::uniform6(any_name::<31>()),
        prop::array::uniform7(any_name::<21>()),
        prop::array::uniform5(any_name::<26>()),
    )
        .prop_map(|(episode_name, difficulty_name, gameplay_name)| State {
            episode_name,
            difficulty_name,
            gameplay_name,
        })
}

fn acts(targets: Vec<(i32, i32)>, keys: Vec<Act>) -> impl Strategy<Value = Vec<(Act, u32)>> {
    prop::collection::vec((any_act(targets, keys), 20u32..200), 0..10)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// `gameplaySelect`: keyboard and mouse navigation, the disabled network
    /// item, choosing a mode or cancelling.
    #[test]
    fn gameplay_select_matches(
        acts in acts((0..4).map(|i| (160, 60 + 24 * i)).collect(), menu_keys()),
        names in any_names(),
    ) {
        run_menu(0, &acts, &names, [true; 5]);
    }

    /// `episodeSelect`: available and unavailable episodes; choosing one
    /// loads it.
    #[test]
    fn episode_select_matches(
        acts in acts((0..4).map(|i| (30, 56 + 30 * i)).collect(), menu_keys()),
        names in any_names(),
        avail: [bool; 4],
    ) {
        let mut a = [false; 5];
        a[..4].copy_from_slice(&avail);
        run_menu(1, &acts, &names, a);
    }

    /// `difficultySelect`: the hidden levels revealed by shift+G, shift+]
    /// and the four "lord" keys (held together or typed in order).
    #[test]
    fn difficulty_select_matches(
        acts in acts(
            (0..6).map(|i| (160, 60 + 24 * i)).collect(),
            {
                let mut k = menu_keys();
                k.extend([
                    Act::Shift("G"),
                    Act::Shift("G"),
                    Act::Shift("RIGHTBRACKET"),
                    Act::Shift("RIGHTBRACKET"),
                    Act::Hold(vec!["L", "O", "R", "D"]),
                    Act::Key("L"),
                    Act::Key("O"),
                    Act::Key("R"),
                    Act::Key("D"),
                ]);
                k
            },
        ),
        names in any_names(),
    ) {
        run_menu(2, &acts, &names, [true; 5]);
    }
}

/// Every difficulty, reached through all hidden levels (the "lord" keys typed
/// in order).
#[test]
fn difficulty_select_every_level_matches() {
    use rustyrian_core::config::{
        DIFFICULTY_EASY, DIFFICULTY_HARD, DIFFICULTY_IMPOSSIBLE, DIFFICULTY_NORMAL,
        DIFFICULTY_SUICIDE, DIFFICULTY_ZINGLON,
    };
    let expected = [
        DIFFICULTY_EASY,
        DIFFICULTY_NORMAL,
        DIFFICULTY_HARD,
        DIFFICULTY_IMPOSSIBLE,
        DIFFICULTY_SUICIDE,
        DIFFICULTY_ZINGLON,
    ];
    let names = State::default();
    for (level, want) in expected.into_iter().enumerate() {
        let mut acts = vec![
            (Act::Shift("G"), 20),
            (Act::Shift("RIGHTBRACKET"), 20),
            (Act::Key("L"), 20),
            (Act::Key("O"), 20),
            (Act::Key("R"), 20),
            (Act::Key("D"), 20),
            (Act::Key("UP"), 20),
        ];
        // From "normal" (1), UP lands on 0; step down to `level`.
        acts.extend((0..level).map(|_| (Act::Key("DOWN"), 20)));
        acts.push((Act::Key("RETURN"), 20));
        assert_eq!(run_menu(2, &acts, &names, [true; 5]), want, "level {level}");
    }
}

/// The last level revealed by holding the four "lord" keys together, after a
/// wrong key resets the typed sequence.
#[test]
fn difficulty_select_held_lord_keys_match() {
    let acts = vec![
        (Act::Shift("G"), 20),
        (Act::Shift("RIGHTBRACKET"), 20),
        (Act::Key("L"), 20),
        (Act::Key("A"), 20),
        (Act::Hold(vec!["L", "O", "R", "D"]), 20),
        (Act::Key("UP"), 20),
        (Act::Key("RETURN"), 20),
    ];
    run_menu(2, &acts, &State::default(), [true; 5]);
}

/// Names "Item" everywhere, so every menu item has a width to click on.
fn item_names() -> State {
    let mut s = State::default();
    for row in &mut s.episode_name {
        row[..4].copy_from_slice(b"Item");
    }
    for row in &mut s.difficulty_name {
        row[..4].copy_from_slice(b"Item");
    }
    for row in &mut s.gameplay_name {
        row[..4].copy_from_slice(b"Item");
    }
    s
}

/// Left clicks on items (choosing them), the disabled network item chosen by
/// keyboard, and wrapping cursor moves, in every menu.
#[test]
fn menus_clicks_and_keys_match() {
    let names = item_names();
    let click = |x: i32, y: i32| vec![(Act::Move(x, y), 20), (Act::Click(1), 20)];
    // Each item of each menu, chosen by a left click.
    for i in 0..4 {
        run_menu(0, &click(160, 60 + 24 * i), &names, [true; 5]);
        run_menu(1, &click(30, 56 + 30 * i), &names, [true; 5]);
    }
    for i in 0..3 {
        run_menu(2, &click(160, 60 + 24 * i), &names, [true; 5]);
    }
    // The network item does nothing; cancel afterwards.
    let keys = |k: &[&'static str]| k.iter().map(|&k| (Act::Key(k), 20)).collect::<Vec<_>>();
    run_menu(
        0,
        &keys(&["DOWN", "DOWN", "DOWN", "RETURN", "UP", "ESCAPE"]),
        &names,
        [true; 5],
    );
    // Wrapping both ways.
    run_menu(0, &keys(&["UP", "DOWN", "RETURN"]), &names, [true; 5]);
    run_menu(1, &keys(&["UP", "DOWN", "SPACE"]), &names, [true; 5]);
    run_menu(
        2,
        &keys(&["UP", "UP", "DOWN", "DOWN", "SPACE"]),
        &names,
        [true; 5],
    );
}
