//! Differential tests for `network.c` / `network.h`.
//!
//! Both engines run on fake platforms with the loopback network backend. Each
//! test opens the same socket on both (and, when wanted, binds channel 0 to the
//! same peer), stores one `network` state into both, runs the function on both
//! and compares return values, the whole `network` state and every platform
//! call (sent datagrams are recorded with their contents).

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::network::{self as net, NET_PACKET_QUEUE, NET_PACKET_SIZE, PacketQueue, State};
use rustyrian_core::platform::{IpAddress, Platform, UdpPacket, UdpSocketId};
use rustyrian_harness::{CallLog, test_session};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::network::{packet_from_c, packet_to_c};
use rustyrian_oracle::session;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn packet(k: u8) -> UdpPacket {
    let mut data = vec![0; NET_PACKET_SIZE as usize];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(k);
    }
    UdpPacket {
        channel: i32::from(k % 2),
        data,
        len: 4 + i32::from(k),
        maxlen: NET_PACKET_SIZE,
        status: -i32::from(k),
        address: IpAddress {
            host: 0x0100_007f,
            port: 0x3505 + u16::from(k),
        },
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        is_network_game: true,
        network_delay: 5,
        network_opponent_host: Some(b"localhost".to_vec()),
        network_player_port: 4000,
        network_opponent_port: 4001,
        network_player_name: Some(b"Alice".to_vec()),
        network_opponent_name: Some(Vec::new()),
        socket: Some(UdpSocketId(9)),
        ip: IpAddress {
            host: 0x0201_a8c0,
            port: 0x3505,
        },
        packet_out_temp: Some(packet(1)),
        packet_temp: Some(packet(2)),
        last_out_sync: 11,
        queue_in_sync: 12,
        queue_out_sync: 13,
        last_ack_sync: 14,
        last_in_tick: 15,
        last_out_tick: 16,
        last_state_in_sync: 17,
        last_state_out_sync: 18,
        last_state_in_tick: 19,
        net_initialized: true,
        connected: true,
        quit: true,
        this_player_num: 2,
        halt_game: true,
        move_ok: true,
        pause_request: true,
        skip_level_request: true,
        help_request: true,
        nort_ship_request: true,
        your_in_game_menu_request: true,
        in_game_menu_request: true,
        network_check_keep_alive_tick: 20,
        network_state_update_resend_tick: 21,
        ..State::default()
    };
    s.packet_in[0] = Some(packet(3));
    s.packet_out[15] = Some(packet(4));
    s.packet_state_in[1] = Some(packet(5));
    s.packet_state_in_xor[2] = Some(packet(6));
    s.packet_state_out[3] = Some(packet(7));
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);

    // Storing again over existing C packets (reused in place) and back to
    // the initial state (packets freed, names back at `empty_string`).
    s.packet_in[0] = Some(packet(8));
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

/// The peer channel 0 is bound to in [`Net::new`].
const PEER: IpAddress = IpAddress {
    host: 0x0100_007f,
    port: 0x3605,
};

/// One C engine and one Rust engine with their platform call logs.
struct Net {
    g: Box<Game>,
    c_log: CallLog,
    r_log: CallLog,
}

impl Net {
    /// Fresh engines whose platforms fail the next call to each of `fail`.
    fn bare(fail: &[&str]) -> Net {
        let (mut c, c_log) = test_session();
        let (mut r, r_log) = test_session();
        for f in fail {
            c.inner_mut().fail_next(f);
            r.inner_mut().fail_next(f);
        }
        session::install(c);
        Net {
            g: Game::new(Box::new(r)),
            c_log,
            r_log,
        }
    }

    /// Fresh engines with an open socket (channel 0 bound to [`PEER`] when
    /// `bound`, so sends succeed) and the virtual clock at `ms`. Returns the
    /// state of an initialized network on that socket.
    fn new(bound: bool, ms: u32, fail: &[&str]) -> (Net, State) {
        let mut n = Net::bare(fail);
        let socket = n.both(|p| p.net_udp_open(1333)).expect("socket");
        if bound {
            n.both(|p| p.net_udp_bind(socket, 0, &PEER));
        }
        n.both(|p| p.delay(ms));
        let blank = || {
            Some(UdpPacket {
                data: vec![0; NET_PACKET_SIZE as usize],
                maxlen: NET_PACKET_SIZE,
                ..UdpPacket::default()
            })
        };
        let s = State {
            socket: Some(socket),
            packet_temp: blank(),
            packet_out_temp: blank(),
            net_initialized: true,
            ..State::default()
        };
        (n, s)
    }

    /// Runs `f` on both platforms (unlogged); returns the Rust result after
    /// checking that C's is equal.
    fn both<R: PartialEq + std::fmt::Debug>(&mut self, f: impl Fn(&mut dyn Platform) -> R) -> R {
        let c = session::with(|s| f(s));
        let r = f(self.g.platform.as_mut());
        assert_eq!(c, r);
        self.c_log.take();
        self.r_log.take();
        r
    }

    /// Makes `s` the network state of both engines.
    fn store(&mut self, s: State) {
        s.store_to_c();
        self.g.network = s;
    }

    /// Asserts equal network states and platform calls since the last check.
    fn check(&self) {
        assert_eq!(diff::load::<State>(), self.g.network);
        assert_eq!(self.c_log.take(), self.r_log.take());
    }
}

fn any_packet() -> impl Strategy<Value = UdpPacket> {
    (
        prop::collection::vec(any::<u8>(), NET_PACKET_SIZE as usize),
        0..=NET_PACKET_SIZE,
        -1..3i32,
        any::<i32>(),
        any::<u32>(),
        any::<u16>(),
    )
        .prop_map(|(data, len, channel, status, host, port)| UdpPacket {
            channel,
            data,
            len,
            maxlen: NET_PACKET_SIZE,
            status,
            address: IpAddress { host, port },
        })
}

fn any_queue() -> impl Strategy<Value = PacketQueue> {
    prop::collection::vec(prop::option::of(any_packet()), NET_PACKET_QUEUE)
        .prop_map(|v| v.try_into().expect("NET_PACKET_QUEUE entries"))
}

/// A queue whose first `k` entries are packets and the rest NULL.
fn contiguous_queue() -> impl Strategy<Value = PacketQueue> {
    (
        prop::collection::vec(any_packet(), NET_PACKET_QUEUE),
        0..=NET_PACKET_QUEUE,
    )
        .prop_map(|(v, k)| {
            let mut q = PacketQueue::default();
            for (slot, p) in q.iter_mut().zip(v).take(k) {
                *slot = Some(p);
            }
            q
        })
}

/// A `u16` near `base` (within a queue length either way) or anywhere.
fn near(base: u16) -> impl Strategy<Value = u16> {
    prop_oneof![
        (0u16..40).prop_map(move |d| base.wrapping_add(d).wrapping_sub(20)),
        any::<u16>(),
    ]
}

/// A tick within `NET_TIME_OUT`-ish of `ms`, or anything.
fn tick_near(ms: u32) -> impl Strategy<Value = u32> {
    prop_oneof![
        (0u32..40_000).prop_map(move |d| ms.wrapping_sub(d)),
        any::<u32>(),
    ]
}

/// The C packet queue `getter` addresses, as `UDPpacket **`.
fn c_queue(getter: unsafe extern "C" fn() -> *mut std::ffi::c_void) -> *mut *mut b::UDPpacket {
    // SAFETY: the getter returns the address of a `UDPpacket *[16]` static.
    unsafe { getter() }.cast()
}

proptest! {
    #[test]
    fn packet_copy_matches(dst in any_packet(), src in any_packet()) {
        let _d = Diff::new();
        let (mut cd, mut cs) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: fresh slots; the packets live until the end of the test.
        unsafe {
            packet_to_c(Some(&dst), &mut cd);
            packet_to_c(Some(&src), &mut cs);
            b::ot_s_network_packet_copy(cd, cs);
        }
        let mut r = dst;
        net::packet_copy(&mut r, &src);
        // SAFETY: valid C packets.
        unsafe {
            prop_assert_eq!(packet_from_c(cd), Some(r));
            prop_assert_eq!(packet_from_c(cs), Some(src));
        }
    }

    #[test]
    fn packets_shift_up_matches(q in any_queue(), max in 1..=NET_PACKET_QUEUE as i32) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State { packet_in: q, ..s });
        // SAFETY: the C queue holds valid packets.
        unsafe { b::ot_s_network_packets_shift_up(c_queue(b::ot_sv_network_packet_in), max) };
        net::packets_shift_up(n.g.platform.as_mut(), &mut n.g.network.packet_in, max);
        n.check();
    }

    #[test]
    fn packets_shift_down_matches(q in any_queue(), max in 1..=NET_PACKET_QUEUE as i32) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State { packet_state_out: q, ..s });
        // SAFETY: the C queue holds valid packets.
        unsafe {
            b::ot_s_network_packets_shift_down(c_queue(b::ot_sv_network_packet_state_out), max);
        }
        net::packets_shift_down(n.g.platform.as_mut(), &mut n.g.network.packet_state_out, max);
        n.check();
    }

    #[test]
    fn prepare_and_send_match(
        bound: bool,
        ms in 0u32..100_000,
        last_out_sync: u16,
        k in 0u16..20,
        ack in 0u16..3,
        packet_out in any_queue(),
        temp in any_packet(),
        type_: u16,
        len in 0..=NET_PACKET_SIZE,
        last_out_tick: u32,
    ) {
        let (mut n, s) = Net::new(bound, ms, &[]);
        let queue_out_sync = last_out_sync.wrapping_sub(k);
        n.store(State {
            last_out_sync,
            queue_out_sync,
            last_ack_sync: queue_out_sync.wrapping_sub(ack),
            last_out_tick,
            packet_out,
            packet_out_temp: Some(temp),
            ..s
        });
        // SAFETY: an initialized C network state.
        unsafe { b::network_prepare(type_) };
        net::network_prepare(&mut n.g, type_);
        n.check();
        // SAFETY: as above.
        let c = unsafe { b::network_send(len) };
        prop_assert_eq!(c, net::network_send(&mut n.g, len));
        n.check();
    }

    #[test]
    fn send_no_ack_and_acknowledge_match(
        bound: bool,
        temp in any_packet(),
        len in 0..=NET_PACKET_SIZE,
        sync: u16,
    ) {
        let (mut n, s) = Net::new(bound, 0, &[]);
        n.store(State { packet_out_temp: Some(temp), ..s });
        // SAFETY: an initialized C network state.
        let c = unsafe { b::ot_s_network_network_send_no_ack(len) };
        prop_assert_eq!(c, net::network_send_no_ack(&mut n.g, len));
        n.check();
        // SAFETY: as above.
        let c = unsafe { b::ot_s_network_network_acknowledge(sync) };
        prop_assert_eq!(c, net::network_acknowledge(&mut n.g, sync));
        n.check();
    }

    #[test]
    fn network_is_alive_matches(
        (ms, last_in_tick, last_state_in_tick) in (0u32..200_000)
            .prop_flat_map(|ms| (Just(ms), tick_near(ms), tick_near(ms))),
    ) {
        let (mut n, s) = Net::new(true, ms, &[]);
        n.store(State { last_in_tick, last_state_in_tick, ..s });
        // SAFETY: an initialized C network state.
        let c = unsafe { b::ot_s_network_network_is_alive() };
        prop_assert_eq!(c, net::network_is_alive(&mut n.g));
        n.check();
    }

    #[test]
    fn network_update_matches(packet_in in any_queue(), queue_in_sync: u16) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State { packet_in, queue_in_sync, ..s });
        // SAFETY: an initialized C network state.
        let c = unsafe { b::network_update() };
        prop_assert_eq!(c, net::network_update(&mut n.g));
        n.check();
    }

    #[test]
    fn network_is_sync_matches(
        (queue_out_sync, last_ack_sync) in any::<u16>().prop_flat_map(|q| (Just(q), near(q))),
    ) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State { queue_out_sync, last_ack_sync, ..s });
        // SAFETY: an initialized C network state.
        let c = unsafe { b::network_is_sync() };
        prop_assert_eq!(c, net::network_is_sync(&mut n.g));
        n.check();
    }

    #[test]
    fn network_state_prepare_matches(slot in prop::option::of(any_packet()), sync: u16) {
        let (mut n, s) = Net::new(true, 0, &[]);
        let mut packet_state_out = PacketQueue::default();
        packet_state_out[0] = slot;
        n.store(State { packet_state_out, last_state_out_sync: sync, ..s });
        // SAFETY: an initialized C network state.
        unsafe { b::network_state_prepare() };
        net::network_state_prepare(&mut n.g);
        n.check();
    }

    #[test]
    fn network_state_send_matches(
        bound: bool,
        fail: bool,
        delay in -1..=7i32,
        sync in prop_oneof![0u16..64, any::<u16>()],
        mut queue in contiguous_queue(),
        first in any_packet(),
    ) {
        let fails: &[&str] = if fail { &["SDLNet_UDP_Send"] } else { &[] };
        let (mut n, s) = Net::new(bound, 0, fails);
        queue[0] = Some(first);
        n.store(State {
            network_delay: delay,
            last_state_out_sync: sync,
            packet_state_out: queue,
            ..s
        });
        // SAFETY: an initialized C network state with a prepared state packet.
        let c = unsafe { b::network_state_send() };
        prop_assert_eq!(c, net::network_state_send(&mut n.g));
        n.check();
    }

    #[test]
    fn network_state_is_reset_matches(
        (delay, sync) in prop_oneof![-2..20i32, any::<i32>()]
            .prop_flat_map(|d| (Just(d), near(d as u16))),
    ) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State { network_delay: delay, last_state_out_sync: sync, ..s });
        // SAFETY: an initialized C network state.
        let c = unsafe { b::network_state_is_reset() };
        prop_assert_eq!(c, net::network_state_is_reset(&mut n.g));
        n.check();
    }

    #[test]
    fn network_state_reset_matches(
        ms in 0u32..100_000,
        packet_state_in in any_queue(),
        packet_state_in_xor in any_queue(),
        packet_state_out in any_queue(),
        last_state_in_sync: u16,
        last_state_out_sync: u16,
        last_state_in_tick: u32,
    ) {
        let (mut n, s) = Net::new(true, ms, &[]);
        n.store(State {
            packet_state_in,
            packet_state_in_xor,
            packet_state_out,
            last_state_in_sync,
            last_state_out_sync,
            last_state_in_tick,
            ..s
        });
        // SAFETY: an initialized C network state.
        unsafe { b::network_state_reset() };
        net::network_state_reset(&mut n.g);
        n.check();
    }

    #[test]
    fn network_init_matches(
        delay in prop_oneof![-2..12i32, any::<i32>()],
        port: u16,
        fail in prop_oneof![
            Just(None),
            Just(Some("SDLNet_Init")),
            Just(Some("SDLNet_UDP_Open")),
            Just(Some("SDLNet_AllocPacket")),
        ],
    ) {
        let mut n = Net::bare(fail.as_slice());
        n.store(State { network_delay: delay, network_player_port: port, ..State::default() });
        // SAFETY: plain C call on the test thread.
        let c = unsafe { b::network_init() };
        prop_assert_eq!(c, net::network_init(&mut n.g));
        n.check();
    }

    #[test]
    fn je_clear_special_requests_matches(flags: [bool; 7]) {
        let (mut n, s) = Net::new(true, 0, &[]);
        n.store(State {
            pause_request: flags[0],
            in_game_menu_request: flags[1],
            skip_level_request: flags[2],
            help_request: flags[3],
            nort_ship_request: flags[4],
            your_in_game_menu_request: flags[5],
            halt_game: flags[6],
            ..s
        });
        // SAFETY: plain C call on the test thread.
        unsafe { b::JE_clearSpecialRequests() };
        net::je_clear_special_requests(&mut n.g);
        n.check();
    }
}

/// `network_send` with a full outbound queue logs the overflow and queues
/// nothing; a queue that just fits stores the copy in its last slot.
#[test]
fn network_send_queue_edges_match() {
    for k in [15u16, 16, 17, u16::MAX] {
        let (mut n, s) = Net::new(true, 5000, &[]);
        n.store(State {
            last_out_sync: k,
            queue_out_sync: 0,
            last_ack_sync: u16::MAX,
            ..s
        });
        // SAFETY: an initialized C network state.
        let c = unsafe {
            b::network_prepare(net::PACKET_GAME_PAUSE);
            b::network_send(4)
        };
        net::network_prepare(&mut n.g, net::PACKET_GAME_PAUSE);
        assert_eq!(c, net::network_send(&mut n.g, 4));
        n.check();
    }
}

/// A sent packet reaches the bound peer through the loopback network with the
/// prepared header.
#[test]
fn network_send_reaches_loopback_peer() {
    let (mut n, s) = Net::new(true, 0, &[]);
    n.store(State {
        last_out_sync: 0x1234,
        queue_out_sync: 0x1234,
        last_ack_sync: 0x1233,
        ..s
    });
    // SAFETY: an initialized C network state.
    let c = unsafe {
        b::network_prepare(net::PACKET_DETAILS);
        b::network_send(6)
    };
    assert!(c);
    let sent = session::with(|s| s.inner().handle().take_sent());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].address, PEER);
    assert_eq!(sent[0].data[..4], [0x00, 0x11, 0x12, 0x34]);
    net::network_prepare(&mut n.g, net::PACKET_DETAILS);
    assert!(net::network_send(&mut n.g, 6));
    n.check();
}
