//! Differential tests for `jukebox.c` / `jukebox.h` (no globals).
//!
//! Both engines run the jukebox on the same timed input script (the
//! `net_support` rig); each test compares every platform call (frames,
//! audio-device locking, palette) and the whole game state, the mixer, song
//! player and OPL chip included.

use rustyrian_core::jukebox::{self as jb, State};
use rustyrian_core::{loudness, nortsong};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

mod net_support;
use net_support::Rig;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// A tap of `key` (a script key name or scancode) at `t` ms.
fn tap(t: u32, key: &str) -> String {
    format!("@{t} key-down {key}\n@{} key-up {key}\n", t + 50)
}

/// A shifted tap of `key` at `t` ms.
fn shift_tap(t: u32, key: &str) -> String {
    format!(
        "@{t} key-down LSHIFT\n{}@{} key-up LSHIFT\n",
        tap(t + 10, key),
        t + 100
    )
}

/// Both engines with video, audio and the sounds set up, on `script`.
fn rig(script: &str, audio: bool) -> Rig {
    let mut rig = Rig::new(script);
    rig.g.nortsong.tyr_music_volume = 200;
    rig.g.nortsong.fx_volume = 128;
    rig.g.loudness.audio_disabled = !audio;
    rig.store();
    rig.video();
    // SAFETY: plain C calls inside the guard.
    let _ = rig.run(
        || unsafe { b::init_audio() },
        |g| Ok(loudness::init_audio(g)),
    );
    let _ = rig.run(
        // SAFETY: as above.
        || unsafe { b::loadSndFile(false) },
        |g| nortsong::load_snd_file(g, false),
    );
    rig.clear();
    rig
}

fn run_jukebox(rig: &mut Rig) {
    // SAFETY: plain C call inside the guard.
    assert_eq!(rig.run(|| unsafe { b::jukebox() }, jb::jukebox), Ok(()));
    rig.check();
}

/// Every key: text hiding, manual and looped-song fades, the sound-effect
/// browser and player, previous/next song, stop and restart, the
/// unimplemented keys, then ESC and the fade-out.
#[test]
fn jukebox_keys_match() {
    let mut s = String::from("ots 1\n");
    let mut t = 300;
    let mut at = |s: &mut String, f: &dyn Fn(u32) -> String, gap: u32| {
        s.push_str(&f(t));
        t += gap;
    };
    at(&mut s, &|t| tap(t, "SPACE"), 300);
    at(&mut s, &|t| tap(t, "SPACE"), 300);
    // A manual fade runs for about two seconds.
    at(&mut s, &|t| tap(t, "F"), 2600);
    at(&mut s, &|t| shift_tap(t, "F"), 300);
    at(&mut s, &|t| shift_tap(t, "F"), 300);
    at(&mut s, &|t| tap(t, "N"), 300);
    at(&mut s, &|t| shift_tap(t, "N"), 300);
    at(&mut s, &|t| tap(t, "V"), 200);
    at(&mut s, &|t| shift_tap(t, "V"), 200);
    at(&mut s, &|t| tap(t, "T"), 200);
    at(&mut s, &|t| shift_tap(t, "T"), 200);
    // Sound effects: off-mode keys do nothing, then browse both ways past
    // the ends and play.
    at(&mut s, &|t| tap(t, "54"), 200);
    at(&mut s, &|t| tap(t, "55"), 200);
    at(&mut s, &|t| tap(t, "51"), 200);
    at(&mut s, &|t| tap(t, "56"), 200);
    at(&mut s, &|t| tap(t, "54"), 200);
    at(&mut s, &|t| tap(t, "51"), 400);
    at(&mut s, &|t| tap(t, "55"), 200);
    at(&mut s, &|t| tap(t, "55"), 200);
    at(&mut s, &|t| tap(t, "51"), 400);
    at(&mut s, &|t| tap(t, "56"), 200);
    // Songs: previous (also from song 0), next, stop, restart.
    for key in ["LEFT", "UP", "RIGHT", "DOWN", "RETURN"] {
        at(&mut s, &|t| tap(t, key), 400);
    }
    at(&mut s, &|t| tap(t, "S"), 400);
    at(&mut s, &|t| shift_tap(t, "S"), 400);
    at(&mut s, &|t| tap(t, "R"), 400);
    at(&mut s, &|t| shift_tap(t, "R"), 400);
    at(&mut s, &|t| tap(t, "A"), 200);
    at(&mut s, &|t| tap(t, "ESCAPE"), 0);
    // A second quit request during the fade-out changes nothing.
    s.push_str(&tap(t + 100, "Q"));
    s.push_str(&format!("@{} quit\n", t + 20_000));

    let mut rig = rig(&s, true);
    run_jukebox(&mut rig);
}

/// Quitting with Q, shift-Q and a mouse click; a looping song fades out and
/// a new one starts.
#[test]
fn jukebox_quit_keys_and_click_match() {
    for (script, audio) in [
        (format!("ots 1\n{}", tap(400, "Q")), true),
        (format!("ots 1\n{}", shift_tap(400, "Q")), true),
        (
            "ots 1\n@400 mouse-move 100 100\n@450 mouse-down 1\n@500 mouse-up 1\n".to_owned(),
            true,
        ),
        // Without audio the song logic is skipped.
        (
            format!("ots 1\n{}{}", tap(300, "RIGHT"), tap(600, "ESCAPE")),
            false,
        ),
    ] {
        let mut rig = rig(&script, audio);
        run_jukebox(&mut rig);
    }
}

/// The looped-song fade: the current song is marked as looped, so the
/// jukebox fades it out and picks another (with looped-song fades turned
/// off, it keeps playing).
#[test]
fn jukebox_looped_song_fade_matches() {
    for keys in ["", "N"] {
        let mut script = String::from("ots 1\n");
        if !keys.is_empty() {
            script.push_str(&tap(100, keys));
        }
        script.push_str(&tap(4000, "ESCAPE"));
        let mut rig = rig(&script, true);
        // Start a song, then mark it looped in both engines.
        assert_eq!(
            // SAFETY: plain C call inside the guard.
            rig.run(|| unsafe { b::play_song(3) }, |g| loudness::play_song(g, 3)),
            Ok(())
        );
        loudness::audio(&rig.g).lds.songlooped = true;
        // SAFETY: C runs only on this thread.
        unsafe { *rustyrian_oracle::mirror::global_mut(&raw mut b::songlooped) = true };
        rig.clear();
        run_jukebox(&mut rig);
    }
}
