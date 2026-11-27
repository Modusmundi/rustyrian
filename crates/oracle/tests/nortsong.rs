//! Differential tests for `nortsong.c` / `nortsong.h`.

use proptest::prelude::*;
use rustyrian_core::game::Halt;
use rustyrian_core::loudness::{self as ld, AudioState, audio};
use rustyrian_core::nortsong::{self as ns, SampleBuf, State};
use rustyrian_core::platform::Platform;
use rustyrian_harness::{CallLog, test_session};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::{Mirror, global_mut};
use rustyrian_oracle::session;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

/// Both engines in a fresh-process state finding data in `dir`, with
/// `audioSampleRate = rate`.
fn fresh(dir: &Path, rate: i32) -> Diff {
    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(bytes(dir));
    d.game.loudness.audio_sample_rate = rate;
    d.store_to_c();
    d
}

/// Makes the next calls to `fails` fail in both engines; returns the Rust
/// call log of the replacement session.
fn inject(d: &mut Diff, fails: &[&str]) -> CallLog {
    let (mut r, log) = test_session();
    for f in fails {
        r.inner_mut().fail_next(f);
        session::with(|s| s.inner_mut().fail_next(f));
    }
    d.game.platform = Box::new(r);
    log
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-nortsong-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn load_both(d: &mut Diff, xmas: bool) -> (Option<i32>, Result<(), Halt>) {
    // SAFETY: loadSndFile may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::loadSndFile(xmas)) };
    (c, ns::load_snd_file(&mut d.game, xmas))
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        frame_count_max: 2,
        tyr_music_volume: 255,
        fx_volume: 128,
        temp_volume: 64,
        frame_speed: 0x1234,
        frame_period: 4321,
        frame_count_end: 0xdead_beef,
        frame_count2_end: 7,
        ..State::default()
    };
    for i in 0..s.sound_samples.len() {
        if i % 4 == 3 {
            continue;
        }
        let n = i * 37;
        let v: Vec<i16> = (0..n)
            .map(|x| (x as i16).wrapping_mul(97).wrapping_sub(1000))
            .collect();
        s.sound_sample_count[i] = n;
        s.sound_samples[i] = Some(SampleBuf(v.into()));
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// All sound effects and voices (both voice sets) load identically, twice
/// (the second load frees the first).
#[test]
fn load_snd_file_matches_on_real_data() {
    let mut d = fresh(&data_dir(), 44100);
    for xmas in [false, true, false] {
        assert_eq!(load_both(&mut d, xmas), (None, Ok(())));
        let s = diff::load::<State>();
        assert_eq!(s, d.game.nortsong);
        assert!(s.sound_samples.iter().all(Option::is_some), "xmas {xmas}");
        assert_eq!(d.c_calls(), d.rust_calls());
    }
}

#[test]
fn load_snd_file_failures_match() {
    // No converter for the rate.
    let mut d = fresh(&data_dir(), 0);
    assert_eq!(load_both(&mut d, false), (None, Ok(())));
    assert_eq!(diff::load::<State>(), d.game.nortsong);
    assert_eq!(d.c_calls(), d.rust_calls());

    // One conversion fails: that sound stays NULL.
    let mut d = fresh(&data_dir(), 22050);
    let log = inject(&mut d, &["SDL_ConvertAudio"]);
    assert_eq!(load_both(&mut d, false), (None, Ok(())));
    assert_eq!(diff::load::<State>(), d.game.nortsong);
    assert_eq!(d.c_calls(), log.take());

    // Truncated files: short reads.
    let dir = scratch("short");
    let snd = std::fs::read(data_dir().join("tyrian.snd")).unwrap();
    std::fs::write(dir.join("tyrian.snd"), &snd[..snd.len() / 2]).unwrap();
    std::fs::write(dir.join("voices.snd"), &snd[..1]).unwrap();
    let mut d = fresh(&dir, 44100);
    assert_eq!(load_both(&mut d, false), (None, Ok(())));
    assert_eq!(diff::load::<State>(), d.game.nortsong);
    assert_eq!(d.c_calls(), d.rust_calls());

    // A missing voice file is fatal (after the effects loaded).
    std::fs::write(dir.join("tyrian.snd"), &snd).unwrap();
    let mut d = fresh(&dir, 44100);
    assert_eq!(load_both(&mut d, true), (Some(1), Err(Halt { code: 1 })));
    assert_eq!(diff::load::<State>(), d.game.nortsong);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// One step of a timer session.
#[derive(Clone, Debug)]
enum T {
    Speed(u16),
    Count(u16),
    Count2(u16),
    Ticks,
    Ticks2,
    Delay,
    Wait(u32),
}

/// Timer divisors. Divisors 0 and 1 give a zero period (`D-UB-TIMER-1`); C
/// is only run with them on aarch64, where the oracle's result is defined
/// by the hardware division.
fn speeds() -> BoxedStrategy<u16> {
    if cfg!(target_arch = "aarch64") {
        prop_oneof![Just(0x4300u16), any::<u16>(), 0u16..2].boxed()
    } else {
        prop_oneof![Just(0x4300u16), 2u16..=u16::MAX].boxed()
    }
}

fn any_t() -> impl Strategy<Value = T> {
    prop_oneof![
        speeds().prop_map(T::Speed),
        (0u16..20).prop_map(T::Count),
        any::<u16>().prop_map(T::Count),
        (0u16..20).prop_map(T::Count2),
        any::<u16>().prop_map(T::Count2),
        Just(T::Ticks),
        Just(T::Ticks2),
        Just(T::Delay),
        (0u32..200).prop_map(T::Wait),
        (0u32..0x0100_0000).prop_map(T::Wait),
    ]
}

proptest! {
    /// The emulated PIT timer from any timer state, with both engines'
    /// virtual clocks advanced identically.
    #[test]
    fn timer_sessions_match(
        frame_speed: u16,
        frame_count_end: u32,
        frame_count2_end: u32,
        start in 0u32..0x0100_0000,
        steps in prop::collection::vec(any_t(), 1..40),
    ) {
        let mut d = Diff::new();
        let n = &mut d.game.nortsong;
        n.frame_speed = frame_speed;
        n.frame_period = ((u64::from(frame_speed) << 10) * 1000 * 88 * 3 / 315_000_000) as u16;
        n.frame_count_end = frame_count_end;
        n.frame_count2_end = frame_count2_end;
        n.store_to_c();
        session::with(|s| s.delay(start));
        d.game.platform.delay(start);
        for t in &steps {
            match *t {
                T::Speed(v) => {
                    // SAFETY: plain C call on the test thread.
                    unsafe { b::setFrameSpeed(v) };
                    ns::set_frame_speed(&mut d.game, v);
                }
                T::Count(v) => {
                    // SAFETY: as above.
                    unsafe { b::setFrameCount(v) };
                    ns::set_frame_count(&mut d.game, v);
                }
                T::Count2(v) => {
                    // SAFETY: as above.
                    unsafe { b::setFrameCount2(v) };
                    ns::set_frame_count2(&mut d.game, v);
                }
                T::Ticks => {
                    // SAFETY: as above.
                    let c = unsafe { b::getFrameCountTicks() };
                    prop_assert_eq!(c, ns::get_frame_count_ticks(&mut d.game));
                }
                T::Ticks2 => {
                    // SAFETY: as above.
                    let c = unsafe { b::getFrameCount2Ticks() };
                    prop_assert_eq!(c, ns::get_frame_count2_ticks(&mut d.game));
                }
                T::Delay => {
                    // SAFETY: as above.
                    unsafe { b::delayUntilElapsed() };
                    ns::delay_until_elapsed(&mut d.game);
                }
                T::Wait(ms) => {
                    session::with(|s| s.delay(ms));
                    d.game.platform.delay(ms);
                }
            }
            prop_assert_eq!(&diff::load::<State>(), &d.game.nortsong);
        }
        prop_assert_eq!(d.c_calls(), d.rust_calls());
    }
}

/// `JE_changeVolume` and `JE_playSampleNum` with audio initialised and the
/// sounds loaded: volumes, clink sounds on the mixer, platform calls.
#[test]
fn change_volume_and_play_sample_match() {
    let mut d = fresh(&data_dir(), 0);
    // SAFETY: plain C call on the test thread.
    let c = unsafe { b::init_audio() };
    assert_eq!(c, ld::init_audio(&mut d.game));
    assert_eq!(load_both(&mut d, false), (None, Ok(())));
    let cases: [(u16, i32, u16, i32); 9] = [
        (0, 0, 0, 0),
        (250, 16, 10, -16),
        (5, -16, 250, 16),
        (128, 8, 128, -8),
        (255, 300, 0, -300),
        (0, -1, 255, 1),
        (40, 0, 40, 1),
        (40, 1, 40, 0),
        (300, 0, 300, 0),
    ];
    for (music, md, sample, sd) in cases {
        let (mut cm, mut cs) = (music, sample);
        // SAFETY: valid pointers to locals.
        unsafe { b::JE_changeVolume(&raw mut cm, md, &raw mut cs, sd) };
        let (mut rm, mut rs) = (music, sample);
        ns::je_change_volume(&mut d.game, &mut rm, md, &mut rs, sd);
        assert_eq!((cm, cs), (rm, rs));
        assert_eq!(diff::load::<AudioState>(), *audio(&d.game));
        assert_eq!(d.c_calls(), d.rust_calls());
    }
    for n in 1..=38u8 {
        // SAFETY: plain C call on the test thread; `n` is 1-based in range.
        unsafe { b::JE_playSampleNum(n) };
        ns::je_play_sample_num(&mut d.game, n);
        assert_eq!(diff::load::<AudioState>(), *audio(&d.game));
    }
    assert_eq!(d.c_calls(), d.rust_calls());
    // With samples disabled nothing is queued.
    audio(&d.game).samples_disabled = true;
    // SAFETY: C runs only on this thread.
    unsafe { *global_mut(&raw mut b::samples_disabled) = true };
    // SAFETY: as above.
    unsafe { b::JE_playSampleNum(3) };
    ns::je_play_sample_num(&mut d.game, 3);
    assert_eq!(diff::load::<AudioState>(), *audio(&d.game));
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// `loadSounds` directly, with a converter built by the platform.
#[test]
fn load_sounds_matches() {
    for (offset, count, file, trim) in [
        (0usize, 29usize, "tyrian.snd", false),
        (29, 9, "voices.snd", true),
        (29, 9, "voicesc.snd", true),
        (3, 5, "voices.snd", false),
    ] {
        let mut d = fresh(&data_dir(), 44100);
        let mut ccvt = b::SDL_AudioCVT::default();
        let mut rcvt = rustyrian_core::platform::AudioCvt::default();
        // SAFETY: valid converter out-pointer; the shim adapter is what C's
        // renamed `SDL_BuildAudioCVT` calls.
        let cb = unsafe {
            rustyrian_oracle::shim::ot_shim_SDL_BuildAudioCVT(
                &raw mut ccvt,
                ld::AUDIO_S8,
                1,
                11025,
                ld::AUDIO_S16SYS,
                1,
                44100,
            )
        };
        let rb = d.game.platform.build_audio_cvt(
            &mut rcvt,
            ld::AUDIO_S8,
            1,
            11025,
            ld::AUDIO_S16SYS,
            1,
            44100,
        );
        assert_eq!(cb, rb);
        let name = rustyrian_oracle::diff::cstring(file);
        // SAFETY: `name` outlives the call; `ccvt` was built above.
        let c = unsafe {
            rustyrian_oracle::guard_call(|| {
                b::ot_s_nortsong_loadSounds(
                    offset as std::ffi::c_ulong,
                    count as std::ffi::c_ulong,
                    name.as_ptr(),
                    trim,
                    &raw mut ccvt,
                )
            })
        };
        let r = ns::load_sounds(&mut d.game, offset, count, file, trim, &mut rcvt);
        assert_eq!((c, r), (None, Ok(())));
        assert_eq!(diff::load::<State>(), d.game.nortsong, "{file}");
        assert_eq!(d.c_calls(), d.rust_calls(), "{file}");
    }
}

/// The zero-period guard (`D-UB-TIMER-1`) keeps the end and adds nothing.
#[test]
fn zero_period_keeps_the_end() {
    let mut d = Diff::new();
    ns::set_frame_speed(&mut d.game, 1);
    assert_eq!(d.game.nortsong.frame_period, 0);
    d.game.platform.delay(5000);
    let end = d.game.nortsong.frame_count_end;
    ns::set_frame_count(&mut d.game, 7);
    ns::set_frame_count2(&mut d.game, 7);
    assert_eq!(d.game.nortsong.frame_count_end, end);
    assert_eq!(d.game.nortsong.frame_count2_end, 0);
}

/// Sample numbers outside `1..=SOUND_COUNT` queue silence on channel 0
/// (`D-UB-SND-1`), with C's lock/unlock calls.
#[test]
fn out_of_range_sample_numbers_queue_silence() {
    let mut d = fresh(&data_dir(), 0);
    assert!(ld::init_audio(&mut d.game));
    assert_eq!(ns::load_snd_file(&mut d.game, false), Ok(()));
    for n in [0u8, 39, 255] {
        ns::je_play_sample_num(&mut d.game, 2);
        d.rust_calls();
        ns::je_play_sample_num(&mut d.game, n);
        let a = audio(&d.game);
        assert_eq!(a.channel_samples[0], None);
        assert_eq!(a.channel_sample_count[0], 0);
        drop(a);
        let calls: Vec<_> = d.rust_calls().into_iter().map(|c| c.name).collect();
        assert_eq!(calls, ["SDL_LockAudioDevice", "SDL_UnlockAudioDevice"]);
    }
}
