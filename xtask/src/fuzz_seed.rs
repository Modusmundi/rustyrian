//! `cargo xtask fuzz-seed`: seeds `fuzz/corpus/<target>/` for every fuzz
//! target that exists, from the freeware data files and from generated
//! inputs where the data holds none of the target's format. A differential
//! target `diff_<target>` (`fuzz/diff_targets/<target>.rs`) gets the seeds
//! of `<target>`.

use anyhow::{Context, Result, bail};
use rustyrian_core::config::{SAVE_FILE_SIZE, SAVE_FILES_NUM, SAVE_FILES_SIZE};
use std::path::Path;

/// Where a target's seeds come from.
enum Source {
    /// The `data/` files matching a file-name pattern (at most one `*`),
    /// shaped into inputs.
    Data(&'static str, Shape),
    /// A generated seed with this name.
    Generated(&'static str, fn() -> Vec<u8>),
}

/// How a data file becomes seeds.
enum Shape {
    /// As it is.
    Whole,
    /// Its first `len` bytes (for large files whose later parts repeat the
    /// format).
    Head(usize),
    /// After a prefix.
    Prefixed(&'static [u8]),
    /// A sprite sheet followed by a plaintext custom ship table (the
    /// `extra_shapes` input format).
    WithShipTable,
    /// Every song of a `music.mus`, one seed per song.
    Songs,
}

use Shape::*;
use Source::*;

/// `sprite2_blit` calls ahead of each sheet: a count, then
/// `(variant, x: i16, y: i16, index, filter)` per call.
#[rustfmt::skip]
const SPRITE2_CALLS: &[u8] = &[
    4,
    0, 10, 0, 10, 0, 1, 0,
    7, 0xfb, 0xff, 190, 0, 2, 0x40,
    5, 0x2c, 0x01, 0xfd, 0xff, 3, 0x20,
    9, 150, 0, 100, 0, 1, 0,
];

/// Seeds per target. `level_map` waits for `JE_loadMap`.
const SEEDS: &[(&str, &[Source])] = &[
    ("cfg_text", &[Generated("opentyrian.cfg", opentyrian_cfg)]),
    ("cfg_binary", &[Generated("tyrian.cfg", tyrian_cfg)]),
    ("helptext", &[Data("tyrian.hdt", Whole)]),
    ("net_session", &[]),
    (
        "save",
        &[
            Generated("empty.sav", empty_save),
            Generated("slots.sav", slots_save),
        ],
    ),
    ("demo", &[Data("demo.*", Whole)]),
    ("level_index", &[Data("tyrian*.lvl", Whole)]),
    ("level_map", &[Data("tyrian*.lvl", Whole)]),
    ("episode_items", &[Data("tyrian.hdt", Whole)]),
    (
        "shapes",
        &[
            Data("tyrian.shp", Whole),
            Data("tyrianc.shp", Whole),
            Data("estsc.shp", Whole),
        ],
    ),
    (
        "sprite2_blit",
        &[Data("newsh*.shp", Prefixed(SPRITE2_CALLS))],
    ),
    ("extra_shapes", &[Data("newsh*.shp", WithShipTable)]),
    ("pcx", &[Data("*.pcx", Whole)]),
    ("pic", &[Data("tyrian.pic", Whole)]),
    // The header, palette and page table, then the first two 64 KiB pages.
    ("anm", &[Data("*.anm", Head(0xB00 + 2 * 0x1_0000))]),
    ("lds_song", &[Data("music.mus", Songs)]),
    ("snd", &[Data("*.snd", Whole)]),
];

fn opentyrian_cfg() -> Vec<u8> {
    b"[video]\nfullscreen = -1\nscaler = Scale2x\nscaling_mode = 8:5\n\n[keyboard]\nup = Up\ndown = Down\n"
        .to_vec()
}

/// `tyrian.cfg` as `saveConfiguration` writes the defaults.
fn tyrian_cfg() -> Vec<u8> {
    vec![
        1, 4, 0, 1, 2, 3, 1, 1, 0, 0, // flags, speeds, devices, difficulty
        1, 4, 5, 5, // joystick buttons
        223, 0, 223, 0, // music and effect volumes
        0, 0, // input devices
        72, 80, 75, 77, 57, 28, 29, 56, // DOS key settings
    ]
}

/// A save slot's size and the offsets of the fields the seeds set.
const SLOT: usize = SAVE_FILES_SIZE / SAVE_FILES_NUM;
const LEVEL: usize = 2;
const SHIP: usize = 4 + 11;
const LEVEL_NAME: usize = 4 + 12 + 8;
const EPISODE: usize = LEVEL_NAME + 1 + 9 + 14 + 1 + 2;
const DIFFICULTY: usize = EPISODE + 1 + 12;

/// `tyrian.sav` plaintext (the target encrypts it) with no slot used.
fn empty_save() -> Vec<u8> {
    vec![0; SAVE_FILE_SIZE + 4]
}

/// `tyrian.sav` plaintext with a one-player slot, a finished episode 4 (the
/// load wraps to episode 1) and a two-player slot.
fn slots_save() -> Vec<u8> {
    let mut d = empty_save();
    let slots: [(usize, u16, &[u8], u8); 3] = [
        (0, 1, b"TYRIAN", 1),
        (5, 1, b"Completed", 4),
        (11, 3, b"SAVARA", 2),
    ];
    for (slot, level, name, episode) in slots {
        let s = &mut d[slot * SLOT..(slot + 1) * SLOT];
        s[LEVEL..LEVEL + 2].copy_from_slice(&level.to_le_bytes());
        s[SHIP] = 1;
        s[LEVEL_NAME] = name.len() as u8;
        s[LEVEL_NAME + 1..LEVEL_NAME + 1 + name.len()].copy_from_slice(name);
        s[EPISODE] = episode;
        s[DIFFICULTY] = 2;
    }
    d
}

/// A plaintext custom ship table: 10 ships of 15 bytes whose graphic
/// numbers (the first byte) select the custom sheet (8 and up), then room
/// for the four checksums the target computes.
fn ship_table() -> Vec<u8> {
    let mut t = vec![0u8; 154];
    for (k, ship) in t.chunks_mut(15).take(10).enumerate() {
        ship[0] = 8 + (k % 8) as u8;
    }
    t
}

/// The songs of a `music.mus`, split as `load_song` does.
fn songs(data: &[u8]) -> Vec<Vec<u8>> {
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let count = data
        .get(..2)
        .map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    let mut pos: Vec<usize> = (0..count).map(|i| u32_at(2 + 4 * i)).collect();
    pos.push(data.len());
    (0..count)
        .filter_map(|i| data.get(pos[i]..pos[i + 1]).map(<[u8]>::to_vec))
        .collect()
}

fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((pre, post)) => {
            name.starts_with(pre) && name.ends_with(post) && name.len() >= pre.len() + post.len()
        }
        None => pattern == name,
    }
}

pub fn run(root: &Path) -> Result<()> {
    let data = root.join("data");
    if !data.join("tyrian1.lvl").is_file() {
        bail!("data/ is missing; run `cargo xtask fetch-data`");
    }
    let fuzz = root.join("fuzz");
    let mut names: Vec<String> = std::fs::read_dir(&data)?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    let read = |name: &str| std::fs::read(data.join(name)).with_context(|| format!("data/{name}"));

    let corpora = SEEDS.iter().flat_map(|&(target, sources)| {
        [
            ("fuzz_targets", target.to_owned(), target, sources),
            ("diff_targets", format!("diff_{target}"), target, sources),
        ]
    });
    for (kind, corpus, file, sources) in corpora {
        if !fuzz.join(kind).join(format!("{file}.rs")).is_file() {
            continue;
        }
        let dir = fuzz.join("corpus").join(&corpus);
        std::fs::create_dir_all(&dir)?;
        let mut n = 0;
        let mut seed = |file: String, bytes: Vec<u8>| -> Result<()> {
            std::fs::write(dir.join(file), bytes)?;
            n += 1;
            Ok(())
        };
        for source in sources {
            let (pattern, shape) = match source {
                Data(pattern, shape) => (pattern, shape),
                Generated(name, make) => {
                    seed((*name).to_owned(), make())?;
                    continue;
                }
            };
            for name in names.iter().filter(|n| matches(pattern, n)) {
                let bytes = read(name)?;
                match shape {
                    Whole => seed(name.clone(), bytes)?,
                    Head(len) => seed(
                        format!("{name}.head"),
                        bytes[..bytes.len().min(*len)].to_vec(),
                    )?,
                    Prefixed(prefix) => seed(name.clone(), [*prefix, &bytes].concat())?,
                    WithShipTable => seed(name.clone(), [bytes, ship_table()].concat())?,
                    Songs => {
                        for (i, song) in songs(&bytes).into_iter().enumerate() {
                            seed(format!("{name}.{i:02}"), song)?;
                        }
                    }
                }
            }
        }
        eprintln!("fuzz-seed: {corpus}: {n} seed(s)");
    }
    Ok(())
}
