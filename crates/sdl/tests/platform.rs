// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Headless checks of `SdlPlatform` against real SDL (dummy video and audio
//! drivers) and against `FakePlatform` where both must agree.

use rustyrian_core::platform::{
    AudioCvt, AudioSpec, Event, IpAddress, JoystickChange, Platform, PlatformProbe, Rect,
    UdpPacket, UdpSocketId,
};
use rustyrian_harness::{FakeConfig, FakePlatform, NetBackend};
use rustyrian_sdl::SdlPlatform;
use sdl2_sys as sys;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A windowed-style run: `Instrumented<SdlPlatform>` declares the scripted
/// joystick before `SDL_Init`, then injects key and joystick actions at poll.
#[test]
fn instrumented_sdl_platform_runs_a_script() {
    let _guard = SDL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let script = rustyrian_harness::script::parse(
        "ots 1\njoystick \"Script Pad\" 2 4 1\n@0 key-down RETURN\n@0 joy-axis 0 1 -32768\n@0 joy-button 0 3 1\n",
    )
    .expect("valid script");
    let config = rustyrian_harness::InstrumentConfig {
        script: Some(script),
        ..rustyrian_harness::InstrumentConfig::default()
    };
    let mut p = rustyrian_harness::Instrumented::new(SdlPlatform::new(), config)
        .expect("no outputs to open");
    force_dummy_drivers();
    assert_eq!(p.init(0), 0, "SDL_Init: {}", p.get_error());
    assert_eq!(
        p.init_sub_system(sys::SDL_INIT_VIDEO | sys::SDL_INIT_JOYSTICK),
        0,
        "SDL_InitSubSystem: {}",
        p.get_error()
    );
    let mut polled = Vec::new();
    for _ in 0..4 {
        polled.extend(p.poll_event());
    }
    assert!(
        polled.contains(&Event::KeyDown {
            scancode: 40,
            keycode: 13,
            modifiers: 0,
            repeat: false,
        }),
        "polled {polled:?}"
    );
    let mut pad = None;
    for i in 0..p.num_joysticks() {
        if let Some(j) = p.joystick_open(i) {
            if p.joystick_name(j).as_deref() == Some("Script Pad") {
                pad = Some(j);
                break;
            }
            p.joystick_close(j);
        }
    }
    let pad = pad.expect("scripted joystick attached at SDL_InitSubSystem");
    assert_eq!(p.joystick_num_buttons(pad), 4);
    p.joystick_update();
    assert_eq!(p.joystick_get_axis(pad, 1), -32768);
    assert_eq!(p.joystick_get_button(pad, 3), 1);
    p.joystick_close(pad);
    p.quit_sub_system(sys::SDL_INIT_VIDEO | sys::SDL_INIT_JOYSTICK);
}

/// SDL's global state is not safe to drive from parallel test threads.
static SDL: Mutex<()> = Mutex::new(());

/// Selects the dummy video and audio drivers, overriding any
/// `SDL_VIDEODRIVER`/`SDL_AUDIODRIVER` already in the environment (a plain
/// `SDL_SetHint` loses to the environment), without an `unsafe` `set_var`.
fn force_dummy_drivers() {
    for (name, value) in [
        (c"SDL_VIDEODRIVER", c"dummy"),
        (c"SDL_AUDIODRIVER", c"dummy"),
    ] {
        // SAFETY: two NUL-terminated strings; SDL copies them.
        let set = unsafe {
            sys::SDL_SetHintWithPriority(
                name.as_ptr(),
                value.as_ptr(),
                sys::SDL_HintPriority::SDL_HINT_OVERRIDE,
            )
        };
        assert!(set == sys::SDL_bool::SDL_TRUE, "SDL_SetHintWithPriority");
    }
}

/// Serialises SDL use and selects the dummy drivers before any subsystem
/// starts.
fn sdl(flags: u32) -> (MutexGuard<'static, ()>, SdlPlatform) {
    let guard = SDL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut p = SdlPlatform::new();
    force_dummy_drivers();
    assert_eq!(p.init(0), 0, "SDL_Init: {}", p.get_error());
    assert_eq!(
        p.init_sub_system(flags),
        0,
        "SDL_InitSubSystem: {}",
        p.get_error()
    );
    (guard, p)
}

fn drain(p: &mut SdlPlatform) {
    while p.poll_event().is_some() {}
}

const RGB888: u32 = sys::SDL_PixelFormatEnum::SDL_PIXELFORMAT_RGB888 as u32;
const STREAMING: i32 = sys::SDL_TextureAccess::SDL_TEXTUREACCESS_STREAMING as i32;

#[test]
fn window_renderer_texture_lifecycle() {
    let (_g, mut p) = sdl(sys::SDL_INIT_VIDEO);
    let window = p
        .create_window("rustyrian test", 0, 0, 640, 400, 0)
        .unwrap_or_else(|| panic!("SDL_CreateWindow: {}", p.get_error()));
    let (mut w, mut h) = (0, 0);
    p.get_window_size(window, Some(&mut w), Some(&mut h));
    assert_eq!((w, h), (640, 400));
    p.set_window_size(window, 960, 600);
    p.get_window_size(window, Some(&mut w), None);
    assert_eq!(w, 960);
    assert!(p.get_num_video_displays() >= 1);
    let display = p.get_window_display_index(window);
    assert!(display >= 0);
    let mut bounds = Rect::default();
    assert_eq!(p.get_display_bounds(display, &mut bounds), 0);
    assert!(bounds.w > 0 && bounds.h > 0);
    let mut untouched = Rect {
        x: 7,
        y: 7,
        w: 7,
        h: 7,
    };
    assert_eq!(p.get_display_bounds(99, &mut untouched), -1);
    assert_eq!(untouched.x, 7, "out-rect written on failure");

    let renderer = p
        .create_renderer(window, -1, 0)
        .unwrap_or_else(|| panic!("SDL_CreateRenderer: {}", p.get_error()));
    let texture = p
        .create_texture(renderer, RGB888, STREAMING, 320, 200)
        .unwrap_or_else(|| panic!("SDL_CreateTexture: {}", p.get_error()));
    let (mut format, mut access, mut tw, mut th) = (0, 0, 0, 0);
    assert_eq!(
        p.query_texture(
            texture,
            Some(&mut format),
            Some(&mut access),
            Some(&mut tw),
            Some(&mut th)
        ),
        0
    );
    assert_eq!((format, access, tw, th), (RGB888, STREAMING, 320, 200));

    assert!(p.probe_locked_texture(texture).is_none());
    let (len, pitch) = {
        let (buf, pitch) = p.lock_texture(texture, None).expect("SDL_LockTexture");
        buf.fill(0xAB);
        (buf.len(), pitch)
    };
    assert!(pitch >= 320 * 4);
    assert_eq!(len, usize::try_from(pitch).unwrap() * 200);
    let seen = p.probe_locked_texture(texture).expect("locked bytes");
    assert_eq!(seen.len(), len);
    assert!(seen.iter().all(|&b| b == 0xAB));
    p.unlock_texture(texture);
    assert!(p.probe_locked_texture(texture).is_none());

    let area = Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
    };
    let (buf, sub_pitch) = p.lock_texture(texture, Some(area)).expect("rect lock");
    assert_eq!(sub_pitch, pitch);
    assert_eq!(buf.len(), usize::try_from(pitch).unwrap() * 39 + 30 * 4);
    p.unlock_texture(texture);

    assert_eq!(p.set_render_draw_color(renderer, 0, 0, 0, 255), 0);
    assert_eq!(p.render_clear(renderer), 0);
    let dst = Rect {
        x: 0,
        y: 0,
        w: 960,
        h: 600,
    };
    assert_eq!(p.render_copy(renderer, texture, None, Some(dst)), 0);
    assert_eq!(p.render_flush(renderer), 0);
    let palette = [0u32; 256];
    let screen = [0u8; 320 * 200];
    p.render_present(
        renderer,
        &rustyrian_core::platform::FrameView {
            screen: &screen,
            palette: &palette,
            sim: None,
            snapshot: None,
        },
    );

    p.destroy_texture(texture);
    assert_eq!(p.query_texture(texture, None, None, None, None), -1);
    p.destroy_renderer(renderer);
    p.destroy_window(window);
    p.quit_sub_system(sys::SDL_INIT_VIDEO);
}

#[test]
fn every_event_round_trips_through_sdl() {
    let (_g, mut p) = sdl(sys::SDL_INIT_VIDEO);
    p.start_text_input();
    drain(&mut p);
    let events = [
        Event::WindowFocusGained,
        Event::WindowFocusLost,
        Event::WindowResized { w: 640, h: 480 },
        Event::KeyDown {
            scancode: 40,
            keycode: 13,
            modifiers: 0x0100,
            repeat: false,
        },
        Event::KeyDown {
            scancode: 4,
            keycode: 97,
            modifiers: 0,
            repeat: true,
        },
        Event::KeyUp {
            scancode: 41,
            keycode: 27,
            modifiers: 0x0001,
            repeat: false,
        },
        Event::TextInput("abc é".to_owned()),
        Event::MouseMotion {
            x: 160,
            y: 100,
            xrel: -3,
            yrel: 4,
        },
        Event::MouseButtonDown {
            button: 1,
            x: 10,
            y: 20,
        },
        Event::MouseButtonUp {
            button: 3,
            x: 30,
            y: 40,
        },
        Event::Quit,
    ];
    // Each poll that finds the queue empty ends at SDL's poll sentinel, so an
    // event pushed between two `while (SDL_PollEvent())` loops is returned by
    // the first poll of the next loop.
    for e in &events {
        assert_eq!(
            p.push_event(e),
            1,
            "SDL_PushEvent({e:?}): {}",
            p.get_error()
        );
        assert_eq!(p.poll_event().as_ref(), Some(e));
        assert_eq!(p.poll_event(), None);
    }
    // Injected events arrive the same way.
    for e in &events {
        p.probe_inject_event(e.clone());
        assert_eq!(p.poll_event().as_ref(), Some(e));
        assert_eq!(p.poll_event(), None);
    }
    // Text is dropped while text input is stopped, as in FakePlatform.
    p.stop_text_input();
    drain(&mut p);
    p.probe_inject_event(Event::TextInput("x".to_owned()));
    assert_eq!(p.poll_event(), None);
    p.quit_sub_system(sys::SDL_INIT_VIDEO);
}

#[test]
fn scancode_names_match_fake_platform() {
    let (_g, mut p) = sdl(0);
    let mut fake = FakePlatform::new(FakeConfig::default());
    let mut names = Vec::new();
    for sc in -1..=512 {
        let real = p.get_scancode_name(sc);
        assert_eq!(
            real,
            fake.get_scancode_name(sc),
            "SDL_GetScancodeName({sc})"
        );
        if !real.is_empty() {
            names.push(real);
        }
    }
    assert!(names.len() > 200);
    let extra = ["", "return", "RETURN", "keypad 7", "NoSuchKey", "Left Ctrl"];
    for name in names.iter().map(String::as_str).chain(extra) {
        assert_eq!(
            p.get_scancode_from_name(name),
            fake.get_scancode_from_name(name),
            "SDL_GetScancodeFromName({name:?})"
        );
        let lower = name.to_lowercase();
        assert_eq!(
            p.get_scancode_from_name(&lower),
            fake.get_scancode_from_name(&lower),
            "SDL_GetScancodeFromName({lower:?})"
        );
    }
}

#[test]
fn audio_device_runs_callback_and_converter() {
    let (_g, mut p) = sdl(sys::SDL_INIT_AUDIO);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let desired = AudioSpec {
        freq: 44_100,
        format: sys::AUDIO_S16LSB as u16,
        channels: 1,
        silence: 0,
        samples: 512,
        size: 0,
    };
    let mut obtained = AudioSpec::default();
    let device = p.open_audio_device(
        None,
        0,
        &desired,
        Box::new(move |buf: &mut [u8]| {
            buf.fill(0);
            seen.fetch_add(1, Ordering::Relaxed);
        }),
        &mut obtained,
        0,
    );
    assert_ne!(device, 0, "SDL_OpenAudioDevice: {}", p.get_error());
    assert_eq!(obtained.freq, 44_100);
    assert_eq!(obtained.channels, 1);
    p.pause_audio_device(device, 0);
    let deadline = Instant::now() + Duration::from_secs(5);
    while calls.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(calls.load(Ordering::Relaxed) > 0, "callback never ran");
    p.lock_audio_device(device);
    p.unlock_audio_device(device);
    p.close_audio_device(device);

    let mut cvt = AudioCvt::default();
    let ret = p.build_audio_cvt(
        &mut cvt,
        sys::AUDIO_S8 as u16,
        1,
        11_025,
        sys::AUDIO_S16LSB as u16,
        1,
        44_100,
    );
    assert_eq!(ret, 1, "SDL_BuildAudioCVT: {}", p.get_error());
    assert_eq!(cvt.needed, 1);
    assert!(cvt.len_mult >= 8);
    let handle = cvt.handle;
    for _ in 0..2 {
        cvt.len = 100;
        let mut buf = vec![0u8; 100 * usize::try_from(cvt.len_mult).unwrap()];
        buf[..100].fill(0x40);
        assert_eq!(p.convert_audio(&mut cvt, &mut buf), 0);
        assert!(cvt.len_cvt > 0 && cvt.len_cvt <= 100 * cvt.len_mult);
        assert_eq!(cvt.handle, handle);
    }
    // Rebuilding reuses the converter slot.
    p.build_audio_cvt(
        &mut cvt,
        sys::AUDIO_S8 as u16,
        1,
        11_025,
        sys::AUDIO_S16LSB as u16,
        1,
        22_050,
    );
    assert_eq!(cvt.handle, handle);
    p.quit_sub_system(sys::SDL_INIT_AUDIO);
}

#[test]
fn virtual_joysticks_follow_the_script() {
    let (_g, mut p) = sdl(0);
    p.probe_joystick(JoystickChange::Declare {
        name: "Fake Pad".to_owned(),
        axes: 2,
        buttons: 4,
        hats: 1,
    });
    // Declared before the subsystem starts: attached by SDL_InitSubSystem.
    p.probe_joystick(JoystickChange::Axis {
        device: 0,
        axis: 1,
        value: -32768,
    });
    for round in 0..2 {
        assert_eq!(p.init_sub_system(sys::SDL_INIT_JOYSTICK), 0);
        let n = p.num_joysticks();
        let mut found = None;
        for i in 0..n {
            if let Some(j) = p.joystick_open(i) {
                if p.joystick_name(j).as_deref() == Some("Fake Pad") {
                    found = Some(j);
                    break;
                }
                p.joystick_close(j);
            }
        }
        let j = found.unwrap_or_else(|| panic!("virtual joystick missing in round {round}"));
        assert_eq!(p.joystick_num_axes(j), 2);
        assert_eq!(p.joystick_num_buttons(j), 4);
        assert_eq!(p.joystick_num_hats(j), 1);
        p.joystick_update();
        assert_eq!(p.joystick_get_axis(j, 1), -32768);
        p.probe_joystick(JoystickChange::Button {
            device: 0,
            button: 2,
            pressed: round == 0,
        });
        p.probe_joystick(JoystickChange::Hat {
            device: 0,
            hat: 0,
            value: 4,
        });
        p.joystick_update();
        assert_eq!(p.joystick_get_button(j, 2), u8::from(round == 0));
        assert_eq!(p.joystick_get_hat(j, 0), 4);
        p.joystick_close(j);
        p.quit_sub_system(sys::SDL_INIT_JOYSTICK);
    }
}

#[test]
fn logging_and_misc_calls() {
    let (_g, mut p) = sdl(sys::SDL_INIT_VIDEO);
    p.log("rustyrian-sdl test: info");
    p.log_debug(0, "rustyrian-sdl test: debug");
    p.log_warn(0, "rustyrian-sdl test: warn");
    p.log_error(0, "rustyrian-sdl test: error %s %d");
    let f = p.log_get_output_function().expect("default log output");
    assert_eq!(p.log_get_output_function(), Some(f));
    p.log_output(f, 0, 6, "rustyrian-sdl test: critical\n");
    assert!(p.get_base_path().is_some());
    let t0 = p.get_ticks();
    p.delay(20);
    assert!(p.get_ticks().wrapping_sub(t0) >= 20);
    assert_eq!(p.show_cursor(-1), p.show_cursor(-1));
    assert_eq!(p.joystick_event_state(-1), p.joystick_event_state(-1));
    p.quit_sub_system(sys::SDL_INIT_VIDEO);
}

#[test]
fn time_and_localtime_are_sane() {
    let mut p = SdlPlatform::new();
    let now = p.time();
    let std_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!((i64::try_from(std_now).unwrap() - now).abs() <= 2);
    let epoch = p.localtime(0);
    // 1970-01-01T00:00Z is 1969-12-31 or 1970-01-01 in every time zone.
    match epoch.year {
        69 => assert_eq!((epoch.mon, epoch.mday, epoch.yday), (11, 31, 364)),
        70 => assert_eq!((epoch.mon, epoch.mday, epoch.yday), (0, 1, 0)),
        y => panic!("localtime(0).tm_year = {y}"),
    }
    assert!((0..7).contains(&epoch.wday));
    let today = p.localtime(now);
    assert!(today.year + 1900 >= 2024);
    assert!((0..12).contains(&today.mon) && (1..=31).contains(&today.mday));
}

// --- networking ---

fn free_ports(n: usize) -> Vec<u16> {
    let sockets: Vec<UdpSocket> = (0..n)
        .map(|_| UdpSocket::bind("0.0.0.0:0").expect("ephemeral port"))
        .collect();
    sockets
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect()
}

/// What upstream's netcode observes of one call, with addresses replaced by
/// their role so that two runs on different ports compare equal.
#[derive(Debug, PartialEq)]
enum Step {
    Resolve(i32, u32, u16),
    Open(bool),
    Bind(i32),
    Send(i32, i32),
    Recv {
        ret: i32,
        len: i32,
        status: i32,
        channel: i32,
        from: &'static str,
        data: Vec<u8>,
    },
}

fn recv_wait<P: Platform>(
    p: &mut P,
    s: UdpSocketId,
    packet: &mut UdpPacket,
    names: &[(IpAddress, &'static str)],
) -> Step {
    let deadline = Instant::now() + Duration::from_secs(5);
    let ret = loop {
        let r = p.net_udp_recv(s, packet);
        if r != 0 || Instant::now() > deadline {
            break r;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let from = names
        .iter()
        .find(|(a, _)| *a == packet.address)
        .map_or("?", |(_, n)| n);
    let len = usize::try_from(packet.len).unwrap_or(0);
    Step::Recv {
        ret,
        len: packet.len,
        status: packet.status,
        channel: packet.channel,
        from,
        data: packet.data[..len].to_vec(),
    }
}

/// Two peers on 127.0.0.1 exchanging packets as upstream's netplay does.
fn exchange<P: Platform>(a: &mut P, b: &mut P, ports: &[u16]) -> Vec<Step> {
    let mut steps = Vec::new();
    assert_eq!(a.net_init(), 0);
    assert_eq!(b.net_init(), 0);
    let mut addr_a = IpAddress::default();
    let mut addr_b = IpAddress::default();
    let r = a.net_resolve_host(&mut addr_b, Some("127.0.0.1"), ports[1]);
    steps.push(Step::Resolve(
        r,
        u32::from_be(addr_b.host),
        u16::from_be(addr_b.port) - ports[1],
    ));
    let r = b.net_resolve_host(&mut addr_a, Some("127.0.0.1"), ports[0]);
    steps.push(Step::Resolve(
        r,
        u32::from_be(addr_a.host),
        u16::from_be(addr_a.port) - ports[0],
    ));
    let mut any = IpAddress::default();
    let r = a.net_resolve_host(&mut any, None, 0);
    steps.push(Step::Resolve(r, any.host, any.port));
    let names = [(addr_a, "a"), (addr_b, "b")];

    let sa = a.net_udp_open(ports[0]);
    let sb = b.net_udp_open(ports[1]);
    steps.push(Step::Open(sa.is_some()));
    steps.push(Step::Open(sb.is_some()));
    let (sa, sb) = (sa.unwrap(), sb.unwrap());
    // A port already in use fails, as SDLNet_UDP_Open does.
    steps.push(Step::Open(a.net_udp_open(ports[0]).is_some()));
    steps.push(Step::Bind(a.net_udp_bind(sa, 0, &addr_b)));
    steps.push(Step::Bind(b.net_udp_bind(sb, 0, &addr_a)));
    steps.push(Step::Bind(b.net_udp_bind(sb, 32, &addr_a)));

    let mut out_a = a.net_alloc_packet(256).unwrap();
    let mut out_b = b.net_alloc_packet(256).unwrap();
    let mut in_a = a.net_alloc_packet(256).unwrap();
    let mut in_b = b.net_alloc_packet(256).unwrap();
    assert_eq!(out_a.maxlen, 256);
    assert_eq!(out_a.data.len(), 256);

    // Nothing pending yet.
    steps.push(Step::Send(b.net_udp_recv(sb, &mut in_b), in_b.len));

    // a → b on channel 0; b sees it on its channel 0 (bound to a).
    out_a.data[..5].copy_from_slice(b"hello");
    out_a.len = 5;
    let r = a.net_udp_send(sa, 0, &mut out_a);
    steps.push(Step::Send(r, out_a.status));
    steps.push(recv_wait(b, sb, &mut in_b, &names));
    steps.push(Step::Send(b.net_udp_recv(sb, &mut in_b), in_b.len));

    // b → a by explicit address (channel −1).
    out_b.address = addr_a;
    out_b.data[..3].copy_from_slice(b"ack");
    out_b.len = 3;
    let r = b.net_udp_send(sb, -1, &mut out_b);
    steps.push(Step::Send(r, out_b.status));
    steps.push(recv_wait(a, sa, &mut in_a, &names));

    // A packet longer than the receiver's buffer is truncated to maxlen.
    let mut small = b.net_alloc_packet(4).unwrap();
    out_a.data[..10].copy_from_slice(b"0123456789");
    out_a.len = 10;
    let r = a.net_udp_send(sa, 0, &mut out_a);
    steps.push(Step::Send(r, out_a.status));
    let step = recv_wait(b, sb, &mut small, &names);
    if cfg!(windows) {
        // Winsock reports WSAEMSGSIZE for truncated datagrams; skip it.
        let _ = step;
    } else {
        steps.push(step);
    }

    // An unbound channel sends nowhere.
    let r = a.net_udp_send(sa, 5, &mut out_a);
    steps.push(Step::Send(r, out_a.status));

    a.net_free_packet(out_a);
    b.net_free_packet(out_b);
    a.net_quit();
    b.net_quit();
    steps
}

#[test]
fn udp_exchange_matches_fake_platform() {
    let ports = free_ports(4);
    let mut sa = SdlPlatform::new();
    let mut sb = SdlPlatform::new();
    let sdl = exchange(&mut sa, &mut sb, &ports[..2]);
    let config = FakeConfig {
        net: NetBackend::Udp,
        ..FakeConfig::default()
    };
    let mut fa = FakePlatform::new(config.clone());
    let mut fb = FakePlatform::new(config);
    let fake = exchange(&mut fa, &mut fb, &ports[2..]);
    assert_eq!(sdl, fake);
    assert!(matches!(
        sdl[11],
        Step::Recv {
            ret: 1,
            channel: 0,
            from: "a",
            ..
        }
    ));
}

#[test]
fn udp_follows_sdl_net_semantics() {
    let mut p = SdlPlatform::new();
    let ports = free_ports(2);
    let s = p.net_udp_open(ports[0]).expect("open");
    let peer = UdpSocket::bind(("127.0.0.1", ports[1])).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut addr = IpAddress::default();
    assert_eq!(
        p.net_resolve_host(&mut addr, Some("127.0.0.1"), ports[1]),
        0
    );
    assert_eq!(addr.port, ports[1].to_be());
    assert_eq!(addr.host, u32::from_ne_bytes([127, 0, 0, 1]));
    let other = IpAddress {
        host: u32::from_ne_bytes([127, 0, 0, 2]),
        port: 1u16.to_be(),
    };

    // Channel −1 picks the first channel with room for another address.
    assert_eq!(p.net_udp_bind(s, -1, &other), 0);
    assert_eq!(p.net_udp_bind(s, -1, &other), 0);
    assert_eq!(p.net_udp_bind(s, 3, &addr), 3);
    assert_eq!(p.net_udp_bind(s, 3, &addr), 3);
    assert_eq!(p.net_udp_bind(s, -1, &other), 0);
    assert_eq!(p.net_udp_bind(s, -1, &other), 0);
    assert_eq!(p.net_udp_bind(s, -1, &other), 1);
    assert_eq!(p.net_udp_bind(s, 0, &other), -1);
    assert_eq!(p.net_get_error(), "No room for new addresses");
    assert_eq!(p.net_udp_bind(s, 32, &other), -1);
    assert_eq!(p.net_get_error(), "Invalid channel");
    assert_eq!(p.net_udp_bind(UdpSocketId(9999), 0, &other), -1);
    assert_eq!(p.net_get_error(), "Passed a NULL socket");

    // Send stores the channel and sends to every address bound to it.
    let mut packet = p.net_alloc_packet(16).unwrap();
    packet.data[..2].copy_from_slice(b"hi");
    packet.len = 2;
    packet.status = 77;
    assert_eq!(p.net_udp_send(s, 3, &mut packet), 2);
    assert_eq!((packet.channel, packet.status), (3, 2));
    let mut buf = [0u8; 16];
    for _ in 0..2 {
        let (n, _) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"hi");
    }
    // A failed send leaves `status` alone.
    packet.len = -1;
    assert_eq!(p.net_udp_send(s, 3, &mut packet), 0);
    assert_eq!(packet.status, 2);

    // Receive reports the highest channel bound to the sender.
    assert_eq!(p.net_udp_bind(s, 7, &addr), 7);
    peer.send_to(b"yo", ("127.0.0.1", ports[0])).unwrap();
    let mut inbox = p.net_alloc_packet(16).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while p.net_udp_recv(s, &mut inbox) == 0 {
        assert!(Instant::now() < deadline, "no datagram");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!((inbox.channel, inbox.len, inbox.status), (7, 2, 2));
    assert_eq!(inbox.address, addr);

    // Resolve failure sets INADDR_NONE but still stores the port.
    let mut none = IpAddress::default();
    // `.invalid` never resolves (RFC 6761); "" resolves to the local host on Windows.
    assert_eq!(
        p.net_resolve_host(&mut none, Some("no.such.host.invalid"), 4242),
        -1
    );
    assert_eq!((none.host, none.port), (u32::MAX, 4242u16.to_be()));
    assert!(p.net_alloc_packet(-1).is_none());
}
