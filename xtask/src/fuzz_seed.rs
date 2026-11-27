//! `cargo xtask fuzz-seed`: copies the freeware data files into
//! `fuzz/corpus/<target>/` for every fuzz target that exists.

use anyhow::{Result, bail};
use std::path::Path;

/// Seed files per target: file-name predicates over `data/`.
const SEEDS: &[(&str, &[&str])] = &[
    ("cfg_text", &[]),
    ("helptext", &["tyrian.hdt"]),
    ("save", &[]),
    ("demo", &["demo.1", "demo.2", "demo.3", "demo.4", "demo.5"]),
    (
        "level_index",
        &["levels1.dat", "levels2.dat", "levels3.dat", "levels4.dat"],
    ),
    (
        "level_map",
        &["tyrian1.lvl", "tyrian2.lvl", "tyrian3.lvl", "tyrian4.lvl"],
    ),
    ("episode_items", &["tyrian.hdt"]),
    ("shapes", &["*.shp"]),
    ("sprite2_blit", &["*.shp"]),
    ("extra_shapes", &["newsh*.shp"]),
    ("pcx", &["*.pcx"]),
    ("pic", &["tyrian.pic"]),
    ("anm", &["*.anm"]),
    ("lds_song", &["music.mus"]),
    ("snd", &["*.snd"]),
];

/// Text seeds for targets whose inputs are not in the game data.
const TEXT_SEEDS: &[(&str, &str, &str)] = &[(
    "cfg_text",
    "opentyrian.cfg",
    "[video]\nfullscreen = -1\nscaler = Scale2x\nscaling_mode = 8:5\n\n[keyboard]\nup = Up\ndown = Down\n",
)];

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
    let targets = root.join("fuzz/fuzz_targets");
    let mut names: Vec<String> = std::fs::read_dir(&data)?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    for (target, patterns) in SEEDS {
        if !targets.join(format!("{target}.rs")).is_file() {
            continue;
        }
        let dir = root.join("fuzz/corpus").join(target);
        std::fs::create_dir_all(&dir)?;
        let mut n = 0;
        for name in names
            .iter()
            .filter(|n| patterns.iter().any(|p| matches(p, n)))
        {
            std::fs::copy(data.join(name), dir.join(name))?;
            n += 1;
        }
        for (t, file, text) in TEXT_SEEDS {
            if t == target {
                std::fs::write(dir.join(file), text)?;
                n += 1;
            }
        }
        eprintln!("fuzz-seed: {target}: {n} seed(s)");
    }
    Ok(())
}
