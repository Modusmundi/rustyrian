# Manual test checklist

Run at every release candidate (`v1.0.0-rc.N`) on the draft release's
artifacts, before publishing. Copy this list into the release issue, tick each
item, and note the OS, hardware and anything unexpected next to it. An item
passes only if every listed criterion holds; anything else is a failure with a
note.

Tester: ______  Release: ______  OS / machine: ______

## Install
- [ ] **Artifact for your OS.** Download the archive for your platform from the
  draft release and check it against `SHA256SUMS` (`shasum -a 256 -c` /
  `sha256sum -c` / `Get-FileHash`).
  *Pass:* the checksum matches; the archive unpacks to `rustyrian/` (Linux,
  Windows) or `Rustyrian.app` (macOS) with `data/`, `COPYING` and
  `licenses/{Tyrian.txt,SDL2.txt}`; the game starts from the unpacked
  directory without installing anything else (macOS: right-click → Open once,
  since builds are not notarized) and shows the logos and then the title menu.

## Play
- [ ] **Episode 1, levels 1–3, keyboard.** New game, episode 1, Normal; play
  through levels 1, 2 and 3 using the keyboard only, visiting the shop between
  levels.
  *Pass:* each level loads, plays and ends with the level-complete screen; ship
  movement, fire, sidekicks and the shop respond; no crash, hang or visual
  corruption.
- [ ] **Episode 1, levels 1–3, gamepad.** Repeat with a gamepad (set it up in
  Setup if needed).
  *Pass:* as above, with every action reachable from the gamepad.
- [ ] **Every scaler, windowed and fullscreen.** In Setup → Graphics, select
  each scaler (None, 2x, 3x, 4x, Scale2x, Scale3x, hq2x, hq3x, hq4x) and each
  scaling mode, once windowed and once fullscreen, and play a few seconds of a
  level with each.
  *Pass:* the picture is correct for each (no garbage, no stretching beyond the
  chosen mode); switching never crashes; the choice is kept after restarting
  the game.
- [ ] **Audio and music.** Play with music and sound effects on; change both
  volumes in Setup → Sound.
  *Pass:* the title and level songs play without dropouts or distortion;
  effects play with weapons, explosions and menus; the volume changes are
  audible and persist after a restart.
- [ ] **Save and load.** Save the game in a free slot from the Game Menu,
  quit to the title, and load that slot.
  *Pass:* the loaded game has the same episode, level, cash and ship equipment
  as when saved, and a save made by upstream OpenTyrian (same `tyrian.sav`
  format) loads as well.
- [ ] **Jukebox.** Setup → Jukebox: step through songs, stop, restart, play
  sound effects.
  *Pass:* every song plays and the controls do what the on-screen help says.
- [ ] **Destruct.** Type `DESTRUCT` on the title menu and play one round.
  *Pass:* the game starts, the round ends with a winner, and Escape returns to
  the title.
- [ ] **LAN netplay against upstream OpenTyrian.** On two machines on one LAN,
  run rustyrian on one and upstream OpenTyrian (same version as
  `vendor/opentyrian`) on the other, with
  `--net=<other host> --net-player-number=<1|2> --net-port=<port>`; play
  episode 1 level 1 together.
  *Pass:* both connect, the level stays in step on both screens to its end, and
  neither reports "unsynchronized".

## Performance
- [ ] **Frame time on the dev Mac.** Run
  `rustyrian --perf-log perf.csv --input-script tests/scenarios/demo-menu.ots --until-ms 60000`
  windowed on the development Mac, then take the 99th percentile of
  `sim_us + render_us` over the frames of `perf.csv`.
  *Pass:* p99 < 5 ms (5000 µs). Record the measured p99 in the release issue.

## Result
- [ ] All items pass, and the results are recorded in the release issue.
