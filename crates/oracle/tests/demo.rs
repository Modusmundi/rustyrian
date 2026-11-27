//! Differential tests for `demo.c` / `demo.h`: the five shipped demos played
//! to their end, damaged copies, and record/replay round trips whose
//! `demorec.N` files (written into separate user directories) must match byte
//! for byte.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::config::KEY_SETTING_FIRE;
use rustyrian_core::demo::{self, State};
use rustyrian_core::game::Halt;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::guard_call;
use rustyrian_oracle::mirror::Mirror;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        play_demo: true,
        record_demo: true,
        stopped_demo: true,
        demo_num: 4,
        demo_keys: 0x1f,
        demo_keys_wait: 600,
        ..State::default()
    };
    s.demo_file.errnum = 2;
    s.demo_file.error = true;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-demo-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn bytes(p: &Path) -> Vec<u8> {
    p.as_os_str().as_bytes().to_vec()
}

/// A data directory holding every freeware file, with `replace` substituted
/// (`None` content removes the file).
fn data_copy(name: &str, replace: &[(&str, Option<&[u8]>)]) -> PathBuf {
    let dir = tmp(name);
    for e in std::fs::read_dir(data_dir()).expect("data dir") {
        let e = e.expect("entry");
        let n = e.file_name();
        if replace.iter().any(|(r, _)| n.as_bytes() == r.as_bytes()) {
            continue;
        }
        std::os::unix::fs::symlink(e.path(), dir.join(&n)).expect("symlink");
    }
    for (n, content) in replace {
        if let Some(c) = content {
            std::fs::write(dir.join(n), c).expect("write");
        }
    }
    dir
}

/// Both engines with game data in `data`; C uses user directory `c_user`,
/// Rust `r_user`.
struct Demo {
    d: Diff,
    c_user: PathBuf,
    r_user: PathBuf,
}

impl Demo {
    fn new(data: &Path, c_user: PathBuf, r_user: PathBuf) -> Demo {
        let mut d = Diff::new();
        d.game.file.custom_data_dir_path = Some(bytes(data));
        let mut demo = Demo { d, c_user, r_user };
        demo.store();
        demo
    }

    fn g(&mut self) -> &mut Game {
        &mut self.d.game
    }

    fn set_user(&mut self, dir: &Path) {
        let f = &mut self.d.game.file;
        f.user_dir_path = Some(bytes(dir));
        f.user_dir_path_len = bytes(dir).len();
    }

    /// Copies the Rust state into C (with C's user directory).
    fn store(&mut self) {
        let (c, r) = (self.c_user.clone(), self.r_user.clone());
        self.set_user(&c);
        self.d.store_to_c();
        self.set_user(&r);
    }

    /// Runs `c` and `r`; their results (or exit codes) must agree.
    fn run<T: PartialEq + std::fmt::Debug>(
        &mut self,
        c: impl FnOnce() -> T,
        r: impl FnOnce(&mut Game) -> Result<T, Halt>,
    ) -> Result<T, i32> {
        let mut out = None;
        // SAFETY: C runs on the test thread; `exit` is caught by the guard.
        let code = unsafe { guard_call(|| out = Some(c())) };
        let c = code.map_or_else(|| Ok(out.expect("returned")), Err);
        let r = r(&mut self.d.game).map_err(|Halt { code }| code);
        assert_eq!(c, r, "results differ");
        r
    }

    /// Equal platform calls and equal states (the user directories differ by
    /// design).
    fn check(&mut self) {
        assert_eq!(
            self.d.c_calls(),
            self.d.rust_calls(),
            "platform calls differ"
        );
        let mut c = self.d.c_game_state();
        c.file.user_dir_path = self.d.game.file.user_dir_path.clone();
        c.file.user_dir_path_len = self.d.game.file.user_dir_path_len;
        if let Some(d) = diff::first_difference(&c, &self.d.game.snapshot()) {
            panic!("states differ at {d}");
        }
    }
}

/// `beginPlayDemo` then `playDemoKeys` until the recording ends, then
/// `endPlayDemo`. Returns the number of frames played (`None` if C exited).
fn play_through(demo: &mut Demo) -> Option<usize> {
    // SAFETY: plain C call inside the guard.
    let r = demo.run(|| unsafe { b::beginPlayDemo() }, demo::begin_play_demo);
    if r.is_err() {
        demo.check();
        return None;
    }
    let mut frames = 0;
    // SAFETY: as above.
    while demo.run(|| unsafe { b::playDemoKeys() }, demo::play_demo_keys) == Ok(true) {
        frames += 1;
        if frames % 512 == 0 {
            assert_eq!(diff::load::<State>(), demo.d.game.demo);
            assert_eq!(
                diff::load::<rustyrian_core::player::State>(),
                demo.d.game.player
            );
            assert_eq!(
                diff::load::<rustyrian_core::mainint::State>(),
                demo.d.game.mainint
            );
        }
    }
    demo.run(
        // SAFETY: as above.
        || unsafe { b::endPlayDemo() },
        |g| {
            demo::end_play_demo(g);
            Ok(())
        },
    )
    .expect("no exit");
    demo.check();
    Some(frames)
}

/// Every shipped demo, in the order `beginPlayDemo` cycles through them
/// (including the wrap from 5 back to 1), played to its end.
#[test]
fn shipped_demos_play_identically() {
    for start in [0u8, 1, 2, 3, 4, 5] {
        let mut demo = Demo::new(&data_dir(), tmp("u-c"), tmp("u-r"));
        demo.g().demo.demo_num = start;
        demo.g().player.player[0].x = 100;
        demo.g().player.player[0].y = 180;
        demo.store();
        let frames = play_through(&mut demo).expect("plays");
        assert!(frames > 100, "demo after {start} played {frames} frames");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Truncated or missing demo files: header read errors exit, recordings
    /// cut anywhere end early.
    #[test]
    fn damaged_demos_match(which in 1u8..=5, cut in prop_oneof![1usize..30, 30usize..3000], missing in prop::bool::weighted(0.1)) {
        let name = format!("demo.{which}");
        let full = std::fs::read(data_dir().join(&name)).expect("demo");
        let content = full[..cut.min(full.len())].to_vec();
        let data = data_copy("data", &[(&name, if missing { None } else { Some(&content) })]);
        let mut demo = Demo::new(&data, tmp("u-c"), tmp("u-r"));
        demo.g().demo.demo_num = which - 1;
        demo.store();
        play_through(&mut demo);
    }
}

/// One recorded frame: which of the eight game keys are held.
fn any_frames() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![3 => Just(None), 1 => any::<u8>().prop_map(Some)],
        1..400,
    )
    .prop_map(|v| {
        // Hold each pattern for a while, as a player would.
        let mut keys = 0u8;
        v.into_iter()
            .map(|k| {
                if let Some(k) = k {
                    keys = k;
                }
                keys
            })
            .collect()
    })
}

/// Sets the eight game keys of `pattern` in `keysactive` on both engines.
fn hold_keys(demo: &mut Demo, pattern: u8) {
    let settings = demo.d.game.config.key_settings;
    let k = &mut demo.d.game.keyboard.keysactive;
    for (i, &sc) in settings.iter().enumerate() {
        k[sc as usize] = pattern & (1 << i) != 0;
    }
    demo.d.game.keyboard.store_to_c();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Records a demo on both engines (files byte-identical), then replays the
    /// recording on both: the replay reproduces the recorded keys frame by
    /// frame.
    #[test]
    fn record_then_replay_round_trips(
        frames in any_frames(),
        existing in 0u8..4,
        episode in 1u8..=4,
        level_name in "[A-Z ]{0,10}",
        items: [u8; 15],
        lvl_file_num: u8,
        level_song: u8,
        near_full_wait: bool,
    ) {
        let (c_user, r_user) = (tmp("rec-c"), tmp("rec-r"));
        for n in 1..=existing {
            std::fs::write(c_user.join(format!("demorec.{n}")), b"x").expect("write");
            std::fs::write(r_user.join(format!("demorec.{n}")), b"x").expect("write");
        }
        let mut demo = Demo::new(&data_dir(), c_user.clone(), r_user.clone());
        let g = demo.g();
        g.episodes.episode_num = episode;
        g.config.key_settings = rustyrian_core::config::DEFAULT_KEY_SETTINGS;
        g.config.level_name = [0; 11];
        g.config.level_name[..level_name.len()].copy_from_slice(level_name.as_bytes());
        // Bytes after the name's NUL are cleared before writing.
        if level_name.len() < 9 {
            g.config.level_name[level_name.len() + 1] = b'?';
        }
        g.varz.lvl_file_num = lvl_file_num;
        g.varz.level_song = level_song;
        g.episodes.initial_episode_num = items[14];
        let it = &mut g.player.player[0].items;
        it.weapon[0].id = items[0];
        it.weapon[1].id = items[1];
        it.super_arcade_mode = items[2];
        it.sidekick = [items[3], items[4]];
        it.generator = items[5];
        it.sidekick_level = items[6];
        it.sidekick_series = items[7];
        it.shield = items[8];
        it.special = items[9];
        it.ship = items[10];
        it.weapon[0].power = items[11];
        it.weapon[1].power = items[12];
        demo.store();

        let begin = demo.run(
            // SAFETY: plain C call inside the guard.
            || unsafe { b::beginRecordDemo() },
            demo::begin_record_demo,
        );
        prop_assert_eq!(begin, Ok(()));
        if near_full_wait {
            demo.d.game.demo.demo_keys_wait = u16::MAX - 2;
            // Only the counter: storing the whole state would reopen the file.
            // SAFETY: `demoKeysWait` is a `Uint16` static.
            unsafe {
                *rustyrian_oracle::mirror::video::sv::<u16>(b::ot_sv_demo_demoKeysWait) =
                    u16::MAX - 2;
            }
        }
        for &pattern in &frames {
            hold_keys(&mut demo, pattern);
            demo.run(
                // SAFETY: as above.
                || unsafe { b::recordDemoKeys() },
                |g| {
                    demo::record_demo_keys(g);
                    Ok(())
                },
            )
            .expect("no exit");
        }
        // SAFETY: as above.
        prop_assert_eq!(demo.run(|| unsafe { b::endRecordDemo() }, demo::end_record_demo), Ok(()));
        demo.check();

        let file = format!("demorec.{}", existing + 1);
        let recorded = std::fs::read(c_user.join(&file)).expect("C recording");
        prop_assert_eq!(&recorded, &std::fs::read(r_user.join(&file)).expect("Rust recording"));

        // Replay it as `demo.1` (the header's episode loads its data).
        let data = data_copy("replay", &[("demo.1", Some(&recorded))]);
        let mut replay = Demo::new(&data, tmp("u-c"), tmp("u-r"));
        let begin = replay.run(
            // SAFETY: plain C call inside the guard.
            || unsafe { b::beginPlayDemo() },
            demo::begin_play_demo,
        );
        prop_assert_eq!(begin, Ok(()));
        // A recording that starts near a full wait counter begins with that
        // many idle frames; only the engines' agreement is checked then.
        let check_keys = !near_full_wait;
        for (n, &pattern) in frames.iter().enumerate() {
            // SAFETY: as above.
            let more = replay.run(|| unsafe { b::playDemoKeys() }, demo::play_demo_keys);
            prop_assert_eq!(more, Ok(true));
            if check_keys {
                let fire = pattern & (1 << KEY_SETTING_FIRE) != 0;
                prop_assert_eq!(replay.d.game.mainint.button[0], fire, "frame {}", n);
            }
        }
        replay.check();
    }
}

/// `beginRecordDemo` when all 255 recording names are taken.
#[test]
fn record_without_free_name_exits() {
    let (c_user, r_user) = (tmp("full-c"), tmp("full-r"));
    for n in 1..=255 {
        std::fs::write(c_user.join(format!("demorec.{n}")), b"").expect("write");
        std::fs::write(r_user.join(format!("demorec.{n}")), b"").expect("write");
    }
    let mut demo = Demo::new(&data_dir(), c_user, r_user);
    // SAFETY: plain C call inside the guard.
    let r = demo.run(|| unsafe { b::beginRecordDemo() }, demo::begin_record_demo);
    assert_eq!(r, Err(1));
    demo.check();
}

/// `beginRecordDemo` when the user directory cannot be written.
#[test]
fn record_into_unusable_directory_exits() {
    // The user "directory" is a regular file: it can be neither created nor
    // opened into.
    let base = tmp("not-a-dir");
    let (c, r) = (base.join("c"), base.join("r"));
    std::fs::write(&c, b"").expect("write");
    std::fs::write(&r, b"").expect("write");
    let mut demo = Demo::new(&data_dir(), c, r);
    // SAFETY: plain C call inside the guard.
    let r = demo.run(|| unsafe { b::beginRecordDemo() }, demo::begin_record_demo);
    assert_eq!(r, Err(1));
    demo.check();
}

/// Keys recorded with `keysactive` hold the game's key bindings.
#[test]
fn recorded_keys_follow_key_settings() {
    let mut demo = Demo::new(&data_dir(), tmp("ks-c"), tmp("ks-r"));
    demo.d.game.config.key_settings = [4, 5, 6, 7, 8, 9, 10, 11];
    demo.store();
    // SAFETY: plain C call inside the guard.
    demo.run(|| unsafe { b::beginRecordDemo() }, demo::begin_record_demo)
        .expect("no exit");
    let mut k = demo.d.game.keyboard.clone();
    k.keysactive[8] = true;
    demo.d.game.keyboard = k;
    demo.d.game.keyboard.store_to_c();
    demo.run(
        // SAFETY: plain C call inside the guard.
        || unsafe { b::recordDemoKeys() },
        |g| {
            demo::record_demo_keys(g);
            Ok(())
        },
    )
    .expect("no exit");
    assert_eq!(demo.d.game.demo.demo_keys, 1 << KEY_SETTING_FIRE);
    // SAFETY: plain C call inside the guard.
    demo.run(|| unsafe { b::endRecordDemo() }, demo::end_record_demo)
        .expect("no exit");
    demo.check();
}
