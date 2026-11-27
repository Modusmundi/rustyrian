//! Input scripts for the menu and prompt tests (`menus`, `xmas`): keyboard
//! and mouse interactions in screen coordinates, always followed by ESC taps
//! so every menu loop ends.

#![allow(dead_code)]

use proptest::prelude::*;
use rustyrian_core::platform::Rect;

/// One interaction.
#[derive(Clone, Debug)]
pub enum Act {
    /// A key tap (script key name).
    Key(&'static str),
    /// A key tap with left shift held.
    Shift(&'static str),
    /// Keys pressed one after another and released together.
    Hold(Vec<&'static str>),
    /// Mouse moved to a screen position.
    Move(i32, i32),
    /// Mouse button tap at the current position.
    Click(u8),
}

/// The output rectangle the fake platform's default window gets after
/// `init_video`: SDL's 640×400 window with the default scaler.
pub fn output_rect() -> Rect {
    use rustyrian_harness::test_session;
    let mut g = rustyrian_core::Game::new(Box::new(test_session().0));
    rustyrian_core::video::init_video(&mut g).expect("init_video");
    rustyrian_core::video::je_show_vga(&mut g);
    g.video.last_output_rect
}

/// The window point that `mapWindowPointToScreen` maps to screen point
/// (`sx`, `sy`) under `r` (the nearest one when none maps exactly).
pub fn to_window(r: Rect, sx: i32, sy: i32) -> (i32, i32) {
    let forward = |w: i32, o: i32, size: i32, v: i32| (2 * (w - o) + 1) * v / (2 * size);
    let find = |s: i32, o: i32, size: i32, v: i32| {
        let guess = o + s * size / v;
        (guess - 4..=guess + 4)
            .min_by_key(|&w| (forward(w, o, size, v) - s).abs())
            .expect("candidates")
    };
    (find(sx, r.x, r.w, 320), find(sy, r.y, r.h, 200))
}

/// The input script for `acts` (each `gap` ms after the previous one,
/// starting at `start` ms), then ESC taps every 400 ms for a minute.
pub fn script(r: Rect, start: u32, acts: &[(Act, u32)]) -> String {
    let mut s = String::from("ots 1\n");
    let mut t = start;
    for (act, gap) in acts {
        t += gap;
        match act {
            Act::Key(k) => s.push_str(&format!("@{t} key-down {k}\n@{} key-up {k}\n", t + 30)),
            Act::Shift(k) => s.push_str(&format!(
                "@{t} key-down LSHIFT\n@{} key-down {k}\n@{} key-up {k}\n@{} key-up LSHIFT\n",
                t + 10,
                t + 40,
                t + 50
            )),
            Act::Hold(keys) => {
                for (i, k) in keys.iter().enumerate() {
                    s.push_str(&format!("@{} key-down {k}\n", t + 5 * i as u32));
                }
                for k in keys {
                    s.push_str(&format!("@{} key-up {k}\n", t + 60));
                }
            }
            Act::Move(x, y) => {
                let (wx, wy) = to_window(r, *x, *y);
                s.push_str(&format!("@{t} mouse-move {wx} {wy}\n"));
            }
            Act::Click(b) => {
                s.push_str(&format!("@{t} mouse-down {b}\n@{} mouse-up {b}\n", t + 30))
            }
        }
        t += 70;
    }
    let mut e = t + 1000;
    while e < t + 61_000 {
        s.push_str(&format!(
            "@{e} key-down ESCAPE\n@{} key-up ESCAPE\n",
            e + 30
        ));
        e += 400;
    }
    s
}

/// Any interaction; mouse positions near `targets` (screen points of the
/// items under test) or anywhere.
pub fn any_act(targets: Vec<(i32, i32)>, keys: Vec<Act>) -> impl Strategy<Value = Act> {
    prop_oneof![
        4 => prop::sample::select(keys),
        2 => (prop::sample::select(targets), -12i32..12, -6i32..8)
            .prop_map(|((x, y), dx, dy)| Act::Move(x + dx, y + dy)),
        1 => (0i32..320, 0i32..200).prop_map(|(x, y)| Act::Move(x, y)),
        2 => prop_oneof![Just(1u8), Just(3u8), Just(2u8)].prop_map(Act::Click),
    ]
}

/// A NUL-terminated printable name of at most `N - 1` bytes.
pub fn any_name<const N: usize>() -> impl Strategy<Value = [u8; N]> {
    prop::collection::vec(32u8..127, 1..N).prop_map(|v| {
        let mut a = [0u8; N];
        a[..v.len()].copy_from_slice(&v);
        a
    })
}
