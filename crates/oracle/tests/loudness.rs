//! Differential tests for `loudness.c` / `loudness.h` (`State` and the
//! audio-thread `AudioState`, which also carries the `lds_play.c` and `opl.c`
//! globals).

use rustyrian_core::loudness::{AudioState, ChannelSamples, State};
use rustyrian_core::nortsong::{self, SampleBuf};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
    assert_eq!(diff::load::<AudioState>(), AudioState::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        audio_sample_rate: 44100,
        song_playing: 29,
        audio_disabled: true,
        audio_device: 3,
        song_positions: Some(vec![10, 2000, -5, 1 << 40]),
        songs_count: 3,
        load_song_first: false,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn audio_state_mirror_round_trip() {
    let _d = Diff::new();
    // Channels point into `soundSamples`, so store those first (as
    // `store_game` does).
    let mut sounds = nortsong::State::default();
    for i in [0, 4, 7] {
        let v: Vec<i16> = (0..100 + i).map(|x| x as i16 * 3 - 50).collect();
        sounds.sound_sample_count[i] = v.len();
        sounds.sound_samples[i] = Some(SampleBuf(v.into()));
    }
    sounds.store_to_c();
    let buf = |i: usize| sounds.sound_samples[i].clone().expect("stored above");

    let mut s = AudioState {
        music_stopped: false,
        music_disabled: true,
        samples_disabled: true,
        music_volume: 10,
        sample_volume: 20,
        samples_per_lds_update: 634,
        samples_per_lds_update_frac: 122,
        samples_until_lds_update: 5,
        samples_until_lds_update_frac: 6,
        channel_sample_count: [50, 0, 3, 0, 0, 0, 0, 1],
        channel_volume: [1, 2, 3, 4, 5, 6, 7, 0],
        ..AudioState::default()
    };
    for (i, v) in s.volume_factor_table.iter_mut().enumerate() {
        *v = i as i32 * 4096 - 7;
    }
    s.channel_samples[0] = Some(ChannelSamples {
        sound: 0,
        buf: buf(0),
        pos: 50,
    });
    s.channel_samples[2] = Some(ChannelSamples {
        sound: 7,
        buf: buf(7),
        pos: 104,
    });
    // A finished channel: pointer at the end of its buffer.
    s.channel_samples[3] = Some(ChannelSamples {
        sound: 4,
        buf: buf(4),
        pos: 104,
    });
    s.channel_samples[7] = Some(ChannelSamples {
        sound: 4,
        buf: buf(4),
        pos: 0,
    });

    // lds_play.c
    s.lds.soundbank = Some(
        (0..5)
            .map(|i| rustyrian_core::lds_play::SoundBank {
                mod_misc: i,
                car_vol: 2 * i,
                arp_tab: [i; 12],
                start: 300,
                size: 7,
                transp: 0xffff,
                middum2: 9,
                ..Default::default()
            })
            .collect(),
    );
    for (i, c) in s.lds.channel.iter_mut().enumerate() {
        c.gototune = i as u16 * 100;
        c.packwait = 3;
        c.arp_tab = [i as u8; 12];
        c.chancheat.high = 0x1234;
        c.chancheat.sound = 5;
    }
    s.lds.positions = Some(
        (0..18)
            .map(|i| rustyrian_core::lds_play::Position {
                patnum: i * 7,
                transpose: i as u8,
            })
            .collect(),
    );
    s.lds.fmchip = std::array::from_fn(|i| i as u8);
    s.lds.jumping = 1;
    s.lds.fadeonoff = 2;
    s.lds.allvolume = 3;
    s.lds.hardfade = 4;
    s.lds.tempo_now = 5;
    s.lds.pattplay = 6;
    s.lds.tempo = 7;
    s.lds.regbd = 8;
    s.lds.chandelay = [9; 9];
    s.lds.mode = 2;
    s.lds.pattlen = 64;
    s.lds.posplay = 11;
    s.lds.jumppos = 12;
    s.lds.speed = 13;
    s.lds.patterns = Some((0..333).map(|x| x * 3).collect());
    s.lds.numpatch = 5;
    s.lds.numposi = 2;
    s.lds.mainvolume = 14;
    s.lds.playing = true;
    s.lds.songlooped = true;

    // opl.c
    for (i, op) in s.opl.op.iter_mut().enumerate() {
        op.cval = -(i as i32);
        op.tcount = 0x1_0000 * i as u32;
        op.amp = 0.5 + i as f64;
        op.decaymul = 1.0 / 3.0;
        op.op_state = 3;
        op.cur_wform = (i % 3 != 0).then_some(i * 100);
        op.sus_keep = true;
        op.tremolo = i % 2 == 0;
        op.cur_env_step = -(i as isize);
        op.env_step_skip_a = 0xaa;
        op.step_skip_pos_a = 4;
    }
    s.opl.int_samplerate = 44100;
    s.opl.status = 0x60;
    s.opl.opl_index = 0xbd;
    s.opl.adlibreg = std::array::from_fn(|i| (i * 3) as u8);
    s.opl.wave_sel = [3; 22];
    s.opl.vibtab_pos = 1;
    s.opl.vibtab_add = 2;
    s.opl.tremtab_pos = 3;
    s.opl.tremtab_add = 4;
    s.opl.generator_add = 5;
    s.opl.recipsamp = 1.0 / 44100.0;
    for (i, w) in s.opl.wavtable.iter_mut().enumerate() {
        *w = (i as i16).wrapping_mul(31);
    }
    s.opl.vib_table = [1, -2, 3, -4, 5, -6, 7, -8];
    s.opl.trem_table = std::array::from_fn(|i| i as i32 * 1000);
    s.opl.vibval_const = std::array::from_fn(|i| i as i32);
    s.opl.tremval_const = std::array::from_fn(|i| -(i as i32));
    s.opl.vibval_var1 = std::array::from_fn(|i| 2 * i as i32);
    s.opl.vibval_var2 = std::array::from_fn(|i| 3 * i as i32);
    s.opl.vibval1 = rustyrian_core::opl::VibvalTable::Var1;
    s.opl.vibval2 = rustyrian_core::opl::VibvalTable::Var2;
    s.opl.vibval3 = rustyrian_core::opl::VibvalTable::Const;
    s.opl.vibval4 = rustyrian_core::opl::VibvalTable::Null;
    s.opl.frqmul = std::array::from_fn(|i| i as f64 * 0.75);
    s.opl.kslev = std::array::from_fn(|i| std::array::from_fn(|j| (i * 16 + j) as u8));
    s.opl.adlib_init_initfirstime = 1;

    s.store_to_c();
    assert_eq!(diff::load::<AudioState>(), s);
}
