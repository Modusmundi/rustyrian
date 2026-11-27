//! State-effect assertions for the Step-11 journeys (`tests/scenarios/journey-*.ots`,
//! `demo-menu.ots`): pixels alone never pass a journey.
//!
//! Each test runs a runner binary on the scenario script: once for the trace
//! (which maps script times to frames), then once per checkpoint with
//! `--frames N+1 --snapshot-at N`, all checkpoints in parallel, and decodes the
//! snapshots as `rustyrian_core::game::Snapshot`. Checkpoints are script times
//! in milliseconds (see the scripts' comments); the snapshot is taken at the
//! first frame presented at or after that time (or the last one before it).
//!
//! The runner is this crate's `ot-ref-run` (the C engine) unless
//! `RUSTYRIAN_JOURNEY_RUNNER` names another binary with the same command line
//! (`rs-run`), so the same assertions hold the Rust engine to the C behaviour.
//!
//! Runtime (debug build, Apple M-series): 5–15 s per test; the longest are
//! `journey_destruct` (~120 s of virtual time) and `journey_new_game_shop`.

use rustyrian_core::game::Snapshot;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

fn runner() -> PathBuf {
    std::env::var_os("RUSTYRIAN_JOURNEY_RUNNER").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_ot-ref-run")),
        PathBuf::from,
    )
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(frame, ms)` of every presented frame.
type Frames = Vec<(u64, u64)>;

/// First frame presented at or after `ms`.
fn at(frames: &Frames, ms: u64) -> u64 {
    frames
        .iter()
        .find(|f| f.1 >= ms)
        .unwrap_or_else(|| panic!("no frame at or after {ms} ms"))
        .0
}

/// Last frame presented before `ms`.
fn before(frames: &Frames, ms: u64) -> u64 {
    frames
        .iter()
        .rev()
        .find(|f| f.1 < ms)
        .unwrap_or_else(|| panic!("no frame before {ms} ms"))
        .0
}

fn ms_of(frames: &Frames, frame: u64) -> u64 {
    frames
        .iter()
        .find(|f| f.0 == frame)
        .expect("frame in trace")
        .1
}

/// One journey's runs, under the test's temporary directory.
struct Journey {
    dir: PathBuf,
    script: PathBuf,
}

impl Journey {
    fn new(test: &str, script: &str) -> Journey {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("journeys")
            .join(test);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("clear the journey directory");
        }
        std::fs::create_dir_all(&dir).expect("create the journey directory");
        Journey {
            dir,
            script: repo().join("tests/scenarios").join(script),
        }
    }

    /// The same journey directory, another script (a later run).
    fn with_script(&self, script: &str) -> Journey {
        Journey {
            dir: self.dir.clone(),
            script: repo().join("tests/scenarios").join(script),
        }
    }

    fn command(&self, userdir: &Path, trace: &Path, args: &[String]) -> Command {
        let mut cmd = Command::new(runner());
        cmd.current_dir(repo())
            .arg("--data")
            .arg(repo().join("data"))
            .arg("--userdir")
            .arg(userdir)
            .arg("--input")
            .arg(&self.script)
            .arg("--trace-out")
            .arg(trace)
            .args(args);
        cmd
    }

    /// A fresh user directory `name`, copied from `base` when given.
    fn userdir(&self, name: &str, base: Option<&Path>) -> PathBuf {
        let dir = self.dir.join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("clear user directory");
        }
        std::fs::create_dir_all(&dir).expect("create user directory");
        if let Some(base) = base {
            copy_tree(base, &dir);
        }
        dir
    }

    /// Runs the script to `until_ms` in user directory `name` (fresh, or a
    /// copy of `base`) and returns its frames and the user directory.
    fn trace(&self, name: &str, base: Option<&Path>, until_ms: u64) -> (Frames, PathBuf) {
        let userdir = self.userdir(name, base);
        let trace = self.dir.join(format!("{name}.jsonl"));
        let status = self
            .command(
                &userdir,
                &trace,
                &["--until-ms".into(), until_ms.to_string()],
            )
            .status()
            .expect("run the runner");
        assert!(status.success(), "{name}: runner exited with {status}");
        let text = std::fs::read_to_string(&trace).expect("read trace");
        let frames = text
            .lines()
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).expect("trace line is JSON");
                Some((v.get("frame")?.as_u64()?, v.get("ms")?.as_u64()?))
            })
            .collect();
        (frames, userdir)
    }

    /// The game state at each of `frames`, every run starting from a copy of
    /// `base` (or an empty user directory).
    fn snapshots(&self, base: Option<&Path>, frames: &[u64]) -> Vec<Snapshot> {
        let stem = self
            .script
            .file_stem()
            .expect("script name")
            .to_string_lossy()
            .into_owned();
        let runs: Vec<(PathBuf, Child)> = frames
            .iter()
            .map(|&n| {
                let name = format!("{stem}@{n}");
                let userdir = self.userdir(&format!("{name}-user"), base);
                let out = self.dir.join(format!("{name}.postcard"));
                let args = [
                    "--frames".to_owned(),
                    (n + 1).to_string(),
                    "--snapshot-at".to_owned(),
                    n.to_string(),
                    "--snapshot-out".to_owned(),
                    out.display().to_string(),
                ];
                let trace = self.dir.join(format!("{name}.jsonl"));
                let child = self
                    .command(&userdir, &trace, &args)
                    .spawn()
                    .expect("spawn the runner");
                (out, child)
            })
            .collect();
        runs.into_iter()
            .map(|(out, mut child)| {
                let status = child.wait().expect("wait for the runner");
                assert!(
                    status.success(),
                    "{}: runner exited with {status}",
                    out.display()
                );
                let bytes = std::fs::read(&out)
                    .unwrap_or_else(|e| panic!("{}: {e} (frame not reached)", out.display()));
                postcard::from_bytes(&bytes).expect("snapshot decodes as game::Snapshot")
            })
            .collect()
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create directory");
    for e in std::fs::read_dir(from).expect("read directory") {
        let e = e.expect("directory entry");
        let dst = to.join(e.file_name());
        if e.file_type().expect("file type").is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            std::fs::copy(e.path(), dst).expect("copy file");
        }
    }
}

fn cash(s: &Snapshot) -> i64 {
    i64::try_from(s.player.player[0].cash).expect("cash fits")
}

/// Selecting Demo on the title menu plays a demo: the demo flag is set and
/// the level scrolls.
#[test]
fn demo_menu() {
    let j = Journey::new("demo_menu", "demo-menu.ots");
    let (frames, _) = j.trace("trace", None, 42_000);
    let s = j.snapshots(
        None,
        &[at(&frames, 9_000), at(&frames, 30_000), at(&frames, 40_000)],
    );
    assert!(!s[0].demo.play_demo, "title: no demo yet");
    assert!(s[1].demo.play_demo, "demo selected");
    assert!(s[2].demo.play_demo);
    assert_ne!(
        s[1].varz.cur_loc, s[2].varz.cur_loc,
        "the demo level scrolls"
    );
    assert_ne!(s[1].mtrand.mt_global, s[2].mtrand.mt_global);
}

/// Every shop category: buying an item takes its price (minus the trade-in of
/// the owned item) and equips it; trading back restores both. Then the kept
/// Multi-Cannon and Satellite Marlo are equipped in level 1.
#[test]
fn journey_new_game_shop() {
    let j = Journey::new("journey_new_game_shop", "journey-new-game-shop.ots");
    let (frames, _) = j.trace("trace", None, 69_500);
    // Checkpoints (script ms): the Game Menu, then after each buy and each
    // trade back (see the script's comments), the two kept items, level 1.
    let times = [
        15_000, 20_000, 21_900, 24_000, 25_800, 27_900, 29_700, 31_800, 33_600, 35_700, 37_500,
        39_600, 41_400, 44_700, 47_700, 69_000,
    ];
    let picks: Vec<u64> = times.iter().map(|&t| at(&frames, t)).collect();
    let s = j.snapshots(None, &picks);
    let start = &s[0];
    assert_eq!(cash(start), 10_000);

    type Slot = fn(&rustyrian_core::player::PlayerItems) -> u8;
    let categories: [(&str, Slot, i64); 6] = [
        ("front gun", |i| i.weapon[0].id, -250),
        ("rear gun", |i| i.weapon[1].id, -1000),
        ("shield", |i| i.shield, 500),
        ("generator", |i| i.generator, 1500),
        ("left sidekick", |i| i.sidekick[0], -2000),
        ("right sidekick", |i| i.sidekick[1], -2000),
    ];
    for (k, (what, slot, delta)) in categories.into_iter().enumerate() {
        let (pre, bought, sold) = (&s[2 * k], &s[2 * k + 1], &s[2 * k + 2]);
        let pre_items = &pre.player.player[0].items;
        let bought_items = &bought.player.player[0].items;
        let sold_items = &sold.player.player[0].items;
        assert_eq!(
            cash(bought) - cash(pre),
            delta,
            "{what}: buy changes cash by the price difference"
        );
        assert_ne!(
            slot(bought_items),
            slot(pre_items),
            "{what}: the bought item is equipped"
        );
        assert_eq!(cash(sold), cash(pre), "{what}: trading back refunds it");
        assert_eq!(
            slot(sold_items),
            slot(pre_items),
            "{what}: the original item is back"
        );
        assert_eq!(sold_items, pre_items, "{what}: nothing else changed");
    }
    let (front, left) = (&s[13], &s[14]);
    assert_eq!(cash(front), 9_750);
    assert_eq!(
        front.player.player[0].items.weapon[0].id,
        s[1].player.player[0].items.weapon[0].id
    );
    assert_eq!(cash(left), 7_750);
    assert_eq!(
        left.player.player[0].items.sidekick[0],
        s[9].player.player[0].items.sidekick[0]
    );
    let level = &s[15];
    assert_eq!(
        level.player.player[0].items, left.player.player[0].items,
        "kept items in level 1"
    );
    assert!(
        cash(level) > cash(left),
        "shooting enemies in level 1 earns cash"
    );
    assert_ne!(level.varz.cur_loc, s[14].varz.cur_loc);
}

/// Save to slot 1, quit to the title, load slot 1: the loaded game equals the
/// saved one.
#[test]
fn journey_save_load() {
    let j = Journey::new("journey_save_load", "journey-save-load.ots");
    let (frames, _) = j.trace("trace", None, 37_000);
    let s = j.snapshots(
        None,
        &[
            at(&frames, 15_000),
            at(&frames, 26_000),
            at(&frames, 36_000),
        ],
    );
    let (new, saved, loaded) = (&s[0], &s[1], &s[2]);
    assert_eq!(
        cash(saved),
        9_750,
        "the Multi-Cannon was bought before saving"
    );
    assert_ne!(saved.player.player[0].items, new.player.player[0].items);
    let slot = &saved.config.save_files[0];
    assert_eq!(&slot.name[..6], b"PARITY", "slot 1 holds the save");
    assert_eq!(loaded.player.player[0].items, saved.player.player[0].items);
    assert_eq!(loaded.player.player[0].cash, saved.player.player[0].cash);
    assert_eq!(loaded.episodes.episode_num, saved.episodes.episode_num);
    assert_eq!(
        loaded.config.difficulty_level,
        saved.config.difficulty_level
    );
    assert_eq!(loaded.config.save_files[0], saved.config.save_files[0]);
}

/// Settings changed in setup persist into a second process on the same user
/// directory.
#[test]
fn journey_setup() {
    let j = Journey::new("journey_setup", "journey-setup.ots");
    let (frames, userdir) = j.trace("run1", None, 40_000);
    let s1 = j.snapshots(None, &[at(&frames, 8_000), before(&frames, 28_000)]);
    let (defaults, changed) = (&s1[0], &s1[1]);
    let restart = j.with_script("runs/journey-setup.2.ots");
    let (frames2, _) = restart.trace("run2", Some(&userdir), 12_000);
    let s2 = restart.snapshots(Some(&userdir), &[at(&frames2, 8_000)]);
    let restarted = &s2[0];

    type Setting = fn(&Snapshot) -> i64;
    let settings: [(&str, Setting); 5] = [
        ("display", |s| i64::from(s.video.fullscreen_display)),
        ("scaler", |s| i64::from(s.video_scale.scaler)),
        ("scaling mode", |s| s.video.scaling_mode as i64),
        ("music volume", |s| i64::from(s.nortsong.tyr_music_volume)),
        ("sound volume", |s| i64::from(s.nortsong.fx_volume)),
    ];
    for (what, get) in settings {
        assert_ne!(get(changed), get(defaults), "{what} was changed in setup");
        assert_eq!(
            get(restarted),
            get(changed),
            "{what} persisted across the restart"
        );
    }
}

/// 1 Player Arcade: the Stalker, no cash; the pickups replace the weapons.
#[test]
fn journey_arcade_1p() {
    let j = Journey::new("journey_arcade_1p", "journey-arcade-1p.ots");
    let (frames, _) = j.trace("trace", None, 36_000);
    let s = j.snapshots(None, &[at(&frames, 22_600), at(&frames, 35_500)]);
    let (start, later) = (&s[0], &s[1]);
    assert!(start.config.one_player_action && !start.config.two_player_mode);
    assert_eq!(
        start.player.player[0].items.ship, 8,
        "arcade plays the Stalker"
    );
    let (a, b) = (&start.player.player[0].items, &later.player.player[0].items);
    assert_ne!(a.weapon[1].id, b.weapon[1].id, "picked up the Sonic Wave");
    assert_ne!(
        a.weapon[0].id, b.weapon[0].id,
        "picked up the Vulcan Cannon"
    );
    assert!(
        later.player.player[0].cash > start.player.player[0].cash,
        "scored"
    );
}

/// 2 Player Arcade: two players, both scoring; player 2 (mouse) moves and
/// picks up weapons.
#[test]
fn journey_arcade_2p() {
    let j = Journey::new("journey_arcade_2p", "journey-arcade-2p.ots");
    let (frames, _) = j.trace("trace", None, 37_000);
    let s = j.snapshots(
        None,
        &[
            at(&frames, 22_600),
            at(&frames, 26_000),
            at(&frames, 36_000),
        ],
    );
    let (start, moved, later) = (&s[0], &s[1], &s[2]);
    assert!(start.config.two_player_mode && !start.config.one_player_action);
    let (p2a, p2b) = (&start.player.player[1], &moved.player.player[1]);
    assert_ne!((p2a.x, p2a.y), (p2b.x, p2b.y), "player 2 follows the mouse");
    assert_ne!(
        start.player.player[1].items, later.player.player[1].items,
        "player 2 picked up weapons"
    );
    for p in 0..2 {
        assert!(
            later.player.player[p].cash > start.player.player[p].cash,
            "player {} scored",
            p + 1
        );
    }
}

/// The typed title code starts Destruct; after one CPU-vs-CPU round a player
/// has scored.
#[test]
fn journey_destruct() {
    let j = Journey::new("journey_destruct", "journey-destruct.ots");
    let (frames, _) = j.trace("trace", None, 113_000);
    let s = j.snapshots(None, &[at(&frames, 20_000), before(&frames, 113_000)]);
    let (early, late) = (&s[0], &s[1]);
    let players = |s: &Snapshot| {
        let d = &s.destruct.destruct_player;
        (d[0].is_cpu, d[1].is_cpu, d[0].score + d[1].score)
    };
    let (l_cpu, r_cpu, early_score) = players(early);
    assert!(l_cpu && r_cpu, "both players are CPUs after F11");
    let (_, _, late_score) = players(late);
    assert!(late_score > early_score, "the round's winner scored");
}

/// The jukebox changes songs with the arrow keys and stops and restarts them.
#[test]
fn journey_jukebox() {
    let j = Journey::new("journey_jukebox", "journey-jukebox.ots");
    let (frames, _) = j.trace("trace", None, 40_000);
    let times = [16_000, 16_300, 24_600, 24_900, 28_300, 30_100];
    let picks: Vec<u64> = times.iter().map(|&t| at(&frames, t)).collect();
    let s = j.snapshots(None, &picks);
    let song = |s: &Snapshot| s.loudness.song_playing;
    assert_eq!(song(&s[1]), song(&s[0]) + 1, "Right plays the next song");
    assert_eq!(song(&s[3]) + 1, song(&s[2]), "Left plays the previous song");
    assert!(s[4].audio.music_stopped, "S stops the music");
    assert!(!s[5].audio.music_stopped, "R restarts it");
}

/// The Data screen opens a datacube, then the second one.
#[test]
fn journey_datacube() {
    use rustyrian_core::game_menu::{MENU_DATA_CUBE_SUB, MENU_DATA_CUBES, MENU_FULL_GAME};
    let j = Journey::new("journey_datacube", "journey-datacube.ots");
    let (frames, _) = j.trace("trace", None, 27_000);
    let s = j.snapshots(
        None,
        &[
            at(&frames, 15_500),
            at(&frames, 17_000),
            at(&frames, 23_000),
            at(&frames, 26_000),
        ],
    );
    let menu = |s: &Snapshot| {
        (
            s.game_menu.cur_menu,
            s.game_menu.cur_sel[usize::from(MENU_DATA_CUBES)],
        )
    };
    assert_eq!(
        menu(&s[0]),
        (i16::from(MENU_DATA_CUBES), 2),
        "the cube list"
    );
    assert_eq!(
        menu(&s[1]),
        (i16::from(MENU_DATA_CUBE_SUB), 2),
        "reading cube 1"
    );
    assert_eq!(
        menu(&s[2]),
        (i16::from(MENU_DATA_CUBE_SUB), 3),
        "reading cube 2"
    );
    assert_eq!(
        s[3].game_menu.cur_menu,
        i16::from(MENU_FULL_GAME),
        "back at the Game Menu"
    );
}

/// P pauses the game: the pause screen is presented and then nothing until P
/// again 3.3 s later, and across that gap the simulation takes one ordinary
/// frame's step (it did not run while paused). The in-game menu raises the
/// detail level.
#[test]
fn journey_pause() {
    let j = Journey::new("journey_pause", "journey-pause.ots");
    let (frames, _) = j.trace("trace", None, 36_000);
    let last_paused = before(&frames, 27_400);
    let resumed = at(&frames, 27_400);
    assert!(
        ms_of(&frames, last_paused) < 24_200 && ms_of(&frames, resumed) >= 27_400,
        "no frame is presented while paused"
    );
    let ordinary = before(&frames, 24_000);
    let s = j.snapshots(
        None,
        &[
            at(&frames, ms_of(&frames, ordinary) - 3_300),
            ordinary - 2,
            ordinary - 1,
            ordinary,
            last_paused,
            resumed,
            at(&frames, 29_000),
            at(&frames, 35_000),
        ],
    );
    let loc = |s: &Snapshot| i32::from(s.varz.cur_loc);
    let max_step = (2..4).map(|i| loc(&s[i]) - loc(&s[i - 1])).max();
    let pause_step = loc(&s[5]) - loc(&s[4]);
    assert!(
        max_step.is_some_and(|m| pause_step <= m),
        "across the pause the level advances one frame's step at most ({pause_step})"
    );
    assert!(
        loc(&s[3]) - loc(&s[0]) > 10 * pause_step.max(1),
        "unpaused, the same 3.3 s scroll the level much further"
    );
    let ship = |s: &Snapshot| (s.player.player[0].x, s.player.player[0].y);
    assert_eq!(ship(&s[5]), ship(&s[4]), "the ship stays put");
    assert_eq!(
        s[7].config.processor_type,
        s[6].config.processor_type + 1,
        "Detail Level raised in the in-game menu"
    );
}
