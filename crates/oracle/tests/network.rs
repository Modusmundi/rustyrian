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

mod net_support;
use net_support::{Rig, store_net, taps};

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

// ---------------------------------------------------------------------------
// network_check, network_state_update, network_connect, network_tyrian_halt
// ---------------------------------------------------------------------------

/// A packet: header (`type`, `sync`) then `body`.
fn pkt(type_: u16, sync: u16, body: &[u8]) -> Vec<u8> {
    let mut v = type_.to_be_bytes().to_vec();
    v.extend_from_slice(&sync.to_be_bytes());
    v.extend_from_slice(body);
    v
}

/// A CONNECT packet as `network_connect` sends it.
fn connect_pkt(
    sync: u16,
    version: u16,
    delay: u16,
    episodes: u16,
    player: u16,
    name: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    for v in [version, delay, episodes, player] {
        body.extend_from_slice(&v.to_be_bytes());
    }
    body.extend_from_slice(name);
    body.push(0);
    pkt(net::PACKET_CONNECT, sync, &body)
}

/// A printable name of up to 20 bytes from `seed`.
fn name_of(seed: &[u8]) -> Vec<u8> {
    seed.iter().take(20).map(|b| b'A' + b % 26).collect()
}

/// One inbound datagram for the `network_check` tests.
#[derive(Clone, Debug)]
enum Inbound {
    /// A well-formed packet of the given type (`sync` relative to the
    /// counter it is checked against).
    Typed(u16, i16),
    /// Shorter than a header.
    Short(usize),
    /// From an address no channel is bound to.
    Stranger,
    Unknown(u16),
}

const TYPES: [u16; 13] = [
    net::PACKET_ACKNOWLEDGE,
    net::PACKET_KEEP_ALIVE,
    net::PACKET_CONNECT,
    net::PACKET_DETAILS,
    net::PACKET_WAITING,
    net::PACKET_BUSY,
    net::PACKET_GAME_QUIT,
    net::PACKET_GAME_PAUSE,
    net::PACKET_GAME_MENU,
    net::PACKET_QUIT,
    net::PACKET_STATE_RESEND,
    net::PACKET_STATE,
    net::PACKET_STATE_XOR,
];

fn any_inbound() -> impl Strategy<Value = Inbound> {
    prop_oneof![
        8 => (prop::sample::select(TYPES.to_vec()), -4i16..20).prop_map(|(t, o)| Inbound::Typed(t, o)),
        1 => (0usize..4).prop_map(Inbound::Short),
        1 => Just(Inbound::Stranger),
        1 => (0x50u16..0x60).prop_map(Inbound::Unknown),
    ]
}

/// The bytes of `inbound` for the engines' current state.
fn inbound_bytes(s: &State, inbound: &Inbound, body: &[u8]) -> Vec<u8> {
    match *inbound {
        Inbound::Typed(t, off) => {
            let base = match t {
                net::PACKET_ACKNOWLEDGE => s.queue_out_sync,
                net::PACKET_STATE | net::PACKET_STATE_XOR => s.last_state_in_sync.wrapping_sub(1),
                net::PACKET_STATE_RESEND => s.last_state_out_sync,
                _ => s.queue_in_sync,
            };
            let sync = if t == net::PACKET_STATE_RESEND {
                base.wrapping_sub(off as u16)
            } else {
                base.wrapping_add(off as u16)
            };
            match t {
                net::PACKET_CONNECT => connect_pkt(
                    sync,
                    net::NET_VERSION,
                    s.network_delay as u16,
                    u16::from(body[0]),
                    2,
                    &name_of(&body[1..1 + usize::from(body[2] % 21)]),
                ),
                net::PACKET_DETAILS => pkt(t, sync, &body[..4]),
                net::PACKET_STATE | net::PACKET_STATE_XOR => pkt(t, sync, &body[..24]),
                _ => pkt(t, sync, &[]),
            }
        }
        Inbound::Short(n) => body[..n].to_vec(),
        Inbound::Stranger => pkt(net::PACKET_KEEP_ALIVE, 0, &[]),
        Inbound::Unknown(t) => pkt(t, 0, &body[..2]),
    }
}

/// A queue whose first `k` slots hold copies of `p`.
fn filled(k: usize, p: &UdpPacket) -> PacketQueue {
    let mut q = PacketQueue::default();
    for slot in q.iter_mut().take(k) {
        *slot = Some(p.clone());
    }
    q
}

/// A sent state packet (28 bytes) with `sync`.
fn state_packet(sync: u16, fill: u8) -> UdpPacket {
    let mut data = vec![fill; NET_PACKET_SIZE as usize];
    data[..4].copy_from_slice(&pkt(net::PACKET_STATE, sync, &[]));
    UdpPacket {
        data,
        len: 28,
        maxlen: NET_PACKET_SIZE,
        ..UdpPacket::default()
    }
}

proptest! {
    /// Well-formed packet sequences through `network_check`, without the
    /// paths that end the game.
    #[test]
    fn network_check_sequences_match(
        ms in 0u32..60_000,
        connected: bool,
        quit: bool,
        initialized in prop::bool::weighted(0.95),
        keep_alive_due: bool,
        retry_due: bool,
        (queue_out_sync, pending) in (any::<u16>(), 0usize..6),
        queue_in_sync: u16,
        last_state_in_sync: u16,
        (last_state_out_sync, state_out) in (any::<u16>(), 0usize..16),
        inbound in prop::collection::vec((any_inbound(), prop::collection::vec(any::<u8>(), 28)), 0..24),
        fail in prop_oneof![
            Just(None),
            (0u32..4).prop_map(|k| Some(("SDLNet_UDP_Recv", k))),
            (0u32..4).prop_map(|k| Some(("SDLNet_UDP_Send", k))),
        ],
    ) {
        let mut rig = Rig::new("ots 1\n");
        rig.net();
        rig.both(|p| p.delay(ms));
        let mut s = rig.g.network.clone();
        s.net_initialized = initialized;
        s.connected = connected;
        s.quit = quit;
        s.last_in_tick = ms;
        s.last_state_in_tick = ms;
        s.network_check_keep_alive_tick = if keep_alive_due { ms.wrapping_sub(5000) } else { ms };
        s.last_out_tick = if retry_due { ms.wrapping_sub(1000) } else { ms };
        s.queue_out_sync = queue_out_sync;
        s.last_ack_sync = queue_out_sync.wrapping_sub(1);
        s.last_out_sync = queue_out_sync.wrapping_add(pending as u16);
        s.packet_out = filled(pending, &state_packet(queue_out_sync, 7));
        s.queue_in_sync = queue_in_sync;
        s.last_state_in_sync = last_state_in_sync;
        s.last_state_out_sync = last_state_out_sync;
        s.packet_state_out = filled(state_out, &state_packet(last_state_out_sync, 9));
        store_net(&mut rig, s);
        rig.clear();
        for (i, body) in &inbound {
            // QUIT ends the game unless already quitting.
            if matches!(i, Inbound::Typed(net::PACKET_QUIT, _)) && !quit {
                continue;
            }
            let bytes = inbound_bytes(&rig.g.network, i, body);
            match i {
                Inbound::Stranger => rig.push_from(IpAddress { host: 0x0200_007f, port: 9 }, &bytes),
                _ => rig.push(&bytes),
            }
        }
        if let Some((name, k)) = fail {
            rig.c_fake.fail_after(name, k);
            rig.r_fake.fail_after(name, k);
        }
        for _ in 0..inbound.len() + 2 {
            // SAFETY: plain C call (inside the guard).
            let r = rig.run(|| unsafe { b::network_check() }, net::network_check);
            prop_assert!(r.is_ok());
        }
        rig.check();
    }
}

/// The script for runs that end the game: RETURN taps for the final wait.
fn halt_script() -> String {
    taps(1000, 120_000)
}

/// A rig with video, shape tables and an initialized, connected network.
fn connected_rig(script: &str) -> Rig {
    let mut rig = Rig::new(script);
    rig.video();
    rig.net();
    let mut s = rig.g.network.clone();
    s.connected = true;
    s.is_network_game = true;
    store_net(&mut rig, s);
    rig
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    /// The two paths of `network_check` that end the game: the connection
    /// timing out and the opponent quitting.
    #[test]
    fn network_check_halts_match(quit_packet: bool, pending in 0usize..3, sync_ok: bool) {
        let mut rig = connected_rig(&halt_script());
        rig.both(|p| p.delay(20_000));
        let mut s = rig.g.network.clone();
        let now = 20_000;
        let (last_in, last_state_in) = if quit_packet { (now, now) } else { (now - 17_000, now - 16_500) };
        s.last_in_tick = last_in;
        s.last_state_in_tick = last_state_in;
        s.network_check_keep_alive_tick = now;
        s.last_out_tick = now;
        s.queue_out_sync = 5;
        s.last_ack_sync = if sync_ok { 4 } else { 3 };
        s.last_out_sync = 5 + pending as u16;
        s.packet_out = filled(pending, &state_packet(5, 1));
        store_net(&mut rig, s);
        rig.clear();
        if quit_packet {
            rig.push(&pkt(net::PACKET_QUIT, 0, &[]));
        }
        // SAFETY: plain C call (inside the guard).
        let r = rig.run(|| unsafe { b::network_check() }, net::network_check);
        prop_assert_eq!(r, Err(0));
        rig.check_halted();
    }

    /// `network_tyrian_halt` for every message, with and without waiting for
    /// the opponent's acknowledgments.
    #[test]
    fn network_tyrian_halt_matches(err in 0u32..9, attempt_sync: bool, sync_ok: bool, alive: bool) {
        let mut rig = connected_rig(&halt_script());
        rig.both(|p| p.delay(20_000));
        let mut s = rig.g.network.clone();
        let tick = if alive { 20_000 } else { 1000 };
        s.last_in_tick = tick;
        s.last_state_in_tick = tick;
        s.network_check_keep_alive_tick = 20_000;
        s.last_out_tick = 20_000;
        s.queue_out_sync = 1;
        s.last_ack_sync = if sync_ok { 0 } else { 1 };
        store_net(&mut rig, s);
        rig.clear();
        let r = rig.run(
            // SAFETY: plain C call (inside the guard).
            || unsafe { b::network_tyrian_halt(err, attempt_sync) },
            |g| net::network_tyrian_halt(g, err, attempt_sync),
        );
        prop_assert_eq!(r, Err(0));
        rig.check_halted();
    }

    /// `JE_tyrianHalt`: teardown, saved configuration and saves, exit code.
    #[test]
    fn je_tyrian_halt_matches(code: u8, trent_win: bool, difficulty in 0i8..10) {
        let mut rig = Rig::new("ots 1\n");
        rig.g.config.trent_win = trent_win;
        rig.g.config.difficulty_level = difficulty;
        rig.store();
        rig.video();
        let r = rig.run(
            // SAFETY: plain C call (inside the guard).
            || unsafe { b::JE_tyrianHalt(code) },
            |g| rustyrian_core::varz::je_tyrian_halt(g, code),
        );
        prop_assert_eq!(r, Err(if code == 5 { 0 } else { i32::from(code) }));
        rig.check_halted();
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// `network_state_update` waiting for, receiving or rebuilding the next
    /// state packet (or timing out).
    #[test]
    fn network_state_update_matches(
        delay in 1i32..=4,
        reset: bool,
        lsis in 1u16..2000,
        prefill_in in 0usize..6,
        prefill_xor in prop::collection::vec(prop::option::of(any::<bool>()), 6),
        deliveries in prop::collection::vec((prop::bool::weighted(0.7), -1i16..6, any::<u8>()), 0..8),
        stale_resend: bool,
    ) {
        let mut rig = connected_rig(&halt_script());
        rig.both(|p| p.delay(30_000));
        let now = 30_000u32;
        let mut s = rig.g.network.clone();
        s.network_delay = delay;
        s.last_state_out_sync = if reset { (delay - 1) as u16 } else { delay as u16 + 3 };
        s.last_state_in_sync = lsis;
        s.last_in_tick = now - 15_000;
        s.last_state_in_tick = now - 1000;
        s.network_check_keep_alive_tick = now;
        s.last_out_tick = now;
        s.network_state_update_resend_tick = if stale_resend { now - 1000 } else { now };
        // Slot i of the inbound queues holds the state `lsis - 1 + i`.
        for i in 1..=prefill_in {
            s.packet_state_in[i] = Some(state_packet(lsis.wrapping_add(i as u16 - 1), i as u8 * 3));
        }
        for (i, x) in prefill_xor.iter().enumerate() {
            if let Some(is_xor) = x {
                let mut p = state_packet(lsis.wrapping_add(i as u16), 0x55);
                if *is_xor {
                    p.data[..2].copy_from_slice(&net::PACKET_STATE_XOR.to_be_bytes());
                }
                s.packet_state_in_xor[i + 1] = Some(p);
            }
        }
        store_net(&mut rig, s);
        rig.clear();
        for (state, off, fill) in &deliveries {
            let t = if *state { net::PACKET_STATE } else { net::PACKET_STATE_XOR };
            rig.push(&pkt(t, lsis.wrapping_add(*off as u16), &[*fill; 24]));
        }
        // SAFETY: plain C call (inside the guard).
        let r = rig.run(|| unsafe { b::network_state_update() }, net::network_state_update);
        match r {
            Ok(_) => rig.check(),
            Err(code) => {
                prop_assert_eq!(code, 0);
                rig.check_halted();
            }
        }
    }
}

/// `network_state_update` with no state packet coming: it requests a resend,
/// keeps polling, and the connection times out.
#[test]
fn network_state_update_waits_then_times_out() {
    let mut rig = connected_rig(&halt_script());
    rig.both(|p| p.delay(30_000));
    let now = 30_000u32;
    let mut s = rig.g.network.clone();
    s.network_delay = 2;
    s.last_state_out_sync = 5;
    s.last_state_in_sync = 3;
    s.last_in_tick = now - 15_000;
    s.last_state_in_tick = now - 1000;
    s.network_check_keep_alive_tick = now;
    s.last_out_tick = now;
    s.network_state_update_resend_tick = now - 1000;
    store_net(&mut rig, s);
    rig.clear();
    // SAFETY: plain C call (inside the guard).
    let r = rig.run(
        || unsafe { b::network_state_update() },
        net::network_state_update,
    );
    assert_eq!(r, Err(0));
    rig.check_halted();
}

/// What the opponent sends during `network_connect`.
#[derive(Clone, Debug)]
enum Peer {
    Connect,
    BadVersion,
    BadDelay,
    SamePlayer,
    Ack(u16),
    KeepAlive,
}

fn any_peer() -> impl Strategy<Value = Peer> {
    prop_oneof![
        6 => Just(Peer::Connect),
        1 => Just(Peer::BadVersion),
        1 => Just(Peer::BadDelay),
        1 => Just(Peer::SamePlayer),
        4 => (0u16..3).prop_map(Peer::Ack),
        1 => Just(Peer::KeepAlive),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// `network_connect` against opponent packet sequences: success, every
    /// mismatch, duplicate CONNECTs, lost acknowledgments (resend) and ESC.
    #[test]
    fn network_connect_matches(
        peer in prop::collection::vec(any_peer(), 0..6),
        esc_at in 500u32..4000,
        sync: u16,
        delay in 1i32..=4,
        player in 1u32..=2,
        name_seed in prop::collection::vec(any::<u8>(), 0..30),
        opponent_seed in prop::collection::vec(any::<u8>(), 0..21),
        avail: [bool; 5],
        episodes: u16,
        host in prop::sample::select(vec!["127.0.0.1", "10.0.0.7", "256.1.1.1"]),
        retry_fails: bool,
    ) {
        let script = format!(
            "ots 1\n@{esc_at} key-down ESCAPE\n@{} key-up ESCAPE\n{}",
            esc_at + 45,
            taps(esc_at + 2000, 120_000).trim_start_matches("ots 1\n"),
        );
        let mut rig = Rig::new(&script);
        rig.video();
        rig.net();
        let mut s = rig.g.network.clone();
        s.is_network_game = true;
        s.network_delay = delay;
        s.this_player_num = player;
        s.network_opponent_host = Some(host.as_bytes().to_vec());
        let mut own = name_of(&name_seed);
        own.extend(name_seed.iter().skip(20).map(|b| b'a' + b % 26));
        s.network_player_name = Some(own);
        store_net(&mut rig, s);
        rig.g.episodes.episode_avail = avail;
        rig.store();
        rig.clear();
        let other = (3 - player) as u16;
        let name = name_of(&opponent_seed);
        for p in &peer {
            let bytes = match *p {
                Peer::Connect => connect_pkt(sync, net::NET_VERSION, delay as u16, episodes, other, &name),
                Peer::BadVersion => connect_pkt(sync, 1, delay as u16, episodes, other, &name),
                Peer::BadDelay => connect_pkt(sync, net::NET_VERSION, delay as u16 + 1, episodes, other, &name),
                Peer::SamePlayer => connect_pkt(sync, net::NET_VERSION, delay as u16, episodes, player as u16, &name),
                Peer::Ack(n) => pkt(net::PACKET_ACKNOWLEDGE, n, &[]),
                Peer::KeepAlive => pkt(net::PACKET_KEEP_ALIVE, 0, &[]),
            };
            rig.push(&bytes);
        }
        if retry_fails {
            // Every send after our first CONNECT fails, so the resend in
            // `network_check` never refreshes `last_out_tick` and the
            // handshake restarts after NET_RETRY.
            for k in 1..3000 {
                rig.c_fake.fail_after("SDLNet_UDP_Send", k);
                rig.r_fake.fail_after("SDLNet_UDP_Send", k);
            }
        } else {
            // While our CONNECT is unacknowledged `network_check` keeps
            // resending it, which postpones the handshake's own retry forever
            // (upstream waits for the opponent); acknowledge it eventually.
            rig.push(&pkt(net::PACKET_ACKNOWLEDGE, 0, &[]));
            rig.push(&pkt(net::PACKET_ACKNOWLEDGE, 1, &[]));
        }
        // SAFETY: plain C call (inside the guard).
        let r = rig.run(|| unsafe { b::network_connect() }, net::network_connect);
        match r {
            Ok(v) => {
                prop_assert_eq!(v, 0);
                rig.check();
            }
            Err(code) => {
                prop_assert_eq!(code, 0);
                rig.check_halted();
            }
        }
    }
}

/// `network_state_send` when the state packet goes out but the xor packet
/// sent right after it fails.
#[test]
fn network_state_send_xor_failure_matches() {
    for delay in 2..=4i32 {
        let mut rig = Rig::new("ots 1\n");
        rig.net();
        let mut s = rig.g.network.clone();
        s.network_delay = delay;
        s.last_state_out_sync = (delay - 1) as u16;
        for i in 0..delay as usize {
            s.packet_state_out[i] = Some(state_packet(i as u16, i as u8 + 1));
        }
        store_net(&mut rig, s);
        rig.clear();
        rig.c_fake.fail_after("SDLNet_UDP_Send", 1);
        rig.r_fake.fail_after("SDLNet_UDP_Send", 1);
        let r = rig.run(
            // SAFETY: plain C call (inside the guard).
            || unsafe { b::network_state_send() },
            |g| Ok(net::network_state_send(g)),
        );
        assert_eq!(r, Ok(-1));
        rig.check();
    }
}

/// D-SEC-2: CONNECT packets shorter than 12 bytes are dropped (Rust only).
#[test]
fn short_connect_packets_are_dropped() {
    for len in 4..12 {
        let mut rig = Rig::new("ots 1\n");
        rig.net();
        let before = rig.g.network.clone();
        let bytes = connect_pkt(3, net::NET_VERSION, 2, 1, 2, b"");
        rig.push(&bytes[..len]);
        assert_eq!(net::network_check(&mut rig.g), Ok(0), "len {len}");
        assert_eq!(rig.g.network.packet_in, before.packet_in);
        assert_eq!(rig.g.network.queue_in_sync, before.queue_in_sync);
        assert!(rig.r_fake.take_sent().is_empty(), "no acknowledgment");
    }
}

/// D-SEC-2: every packet type shorter than the fields read for it is dropped
/// without effect; at its minimum length it is accepted (Rust only).
#[test]
fn packets_shorter_than_their_type_are_dropped() {
    for (t, min) in [
        (net::PACKET_CONNECT, 12),
        (net::PACKET_DETAILS, 8),
        (net::PACKET_STATE, 28),
        (net::PACKET_STATE_XOR, 28),
    ] {
        for len in 4..min {
            let mut rig = Rig::new("ots 1\n");
            rig.net();
            let mut before = rig.g.network.clone();
            rig.push(&pkt(t, 0, &vec![1; len - 4]));
            assert_eq!(
                net::network_check(&mut rig.g),
                Ok(0),
                "type {t:#x} len {len}"
            );
            // Only the receive buffer changed.
            before.packet_temp = rig.g.network.packet_temp.clone();
            assert_eq!(rig.g.network, before, "type {t:#x} len {len}");
            assert!(rig.r_fake.take_sent().is_empty());
        }
        let mut rig = Rig::new("ots 1\n");
        rig.net();
        rig.push(&pkt(t, 0, &vec![1; min - 4]));
        assert_eq!(
            net::network_check(&mut rig.g),
            Ok(1),
            "type {t:#x} at minimum"
        );
    }
}

// ---------------------------------------------------------------------------
// net_session: two state machines against a scripted opponent
// ---------------------------------------------------------------------------

/// Address the opponent's channel 0 is bound to (the engines under test).
const US: IpAddress = IpAddress {
    host: 0x0100_007f,
    port: 0x3505,
};

/// Writes `payload` into C's `packet_state_out[0]` after the header.
fn c_write_state_payload(payload: &[u8]) {
    let q = c_queue(b::ot_sv_network_packet_state_out);
    // SAFETY: `network_state_prepare` allocated `packet_state_out[0]` with
    // 256 data bytes; `payload` is 24 bytes.
    unsafe {
        let p = *q;
        std::ptr::copy_nonoverlapping(payload.as_ptr(), (*p).data.add(4), payload.len());
    }
}

/// One frame of the session: the opponent's and our state payloads, whether
/// one of the opponent's packets is lost, whether each side also sends an
/// acknowledged packet, and whether the opponent's packets arrive reversed.
#[derive(Clone, Debug)]
struct Frame {
    theirs: Vec<u8>,
    ours: Vec<u8>,
    lose: bool,
    they_send: Option<u16>,
    we_send: Option<u16>,
    swap: bool,
}

fn any_frame() -> impl Strategy<Value = Frame> {
    let reliable = prop::sample::select(vec![
        net::PACKET_GAME_PAUSE,
        net::PACKET_WAITING,
        net::PACKET_BUSY,
        net::PACKET_GAME_MENU,
    ]);
    (
        prop::collection::vec(any::<u8>(), 24),
        prop::collection::vec(any::<u8>(), 24),
        prop::bool::weighted(0.2),
        prop::option::weighted(0.2, reliable.clone()),
        prop::option::weighted(0.2, reliable),
        any::<bool>(),
    )
        .prop_map(|(theirs, ours, lose, they_send, we_send, swap)| Frame {
            theirs,
            ours,
            lose,
            they_send,
            we_send,
            swap,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// M3 exit test: an opponent (a third, Rust engine) and both engines under
    /// test exchange state, xor and acknowledged packets over the loopback
    /// network for whole sessions, with lost and reordered packets; the
    /// engines' states, platform calls and sent datagrams stay equal.
    #[test]
    fn net_session(delay in 1i32..=4, frames in prop::collection::vec(any_frame(), 4..40)) {
        let mut rig = connected_rig(&halt_script());
        let (peer_platform, _peer_log, peer_fake) = rustyrian_harness::test_session_with_handle();
        let mut peer = Game::new(Box::new(peer_platform));
        prop_assert_eq!(net::network_init(&mut peer), 0);
        let peer_socket = peer.network.socket.expect("socket");
        peer.platform.net_udp_bind(peer_socket, 0, &US);

        let mut s = rig.g.network.clone();
        s.network_delay = delay;
        store_net(&mut rig, s);
        peer.network.network_delay = delay;
        peer.network.connected = true;
        peer.network.is_network_game = true;
        // SAFETY: plain C call.
        unsafe { b::network_state_reset() };
        net::network_state_reset(&mut rig.g);
        net::network_state_reset(&mut peer);
        rig.check_io();

        // At most one of the opponent's state packets is lost per xor group.
        let d = delay as u16;
        let mut lost_group = None;
        for f in &frames {
            // The opponent's frame.
            net::network_state_prepare(&mut peer);
            peer.network.packet_state_out[0].as_mut().expect("prepared").data[4..28]
                .copy_from_slice(&f.theirs);
            let state_sync = peer.network.last_state_out_sync;
            prop_assert_eq!(net::network_state_send(&mut peer), 0);
            if let Some(t) = f.they_send {
                net::network_prepare(&mut peer, t);
                prop_assert!(net::network_send(&mut peer, 4));
            }
            let mut theirs = peer_fake.take_sent();
            let group = state_sync / d;
            if d > 1 && f.lose && lost_group != Some(group) && !theirs.is_empty() {
                lost_group = Some(group);
                theirs.remove(0);
            }
            if f.swap {
                theirs.reverse();
            }
            for dg in &theirs {
                rig.push(&dg.data);
            }

            // Our frame, on both engines.
            rig.run(
                // SAFETY: plain C call (inside the guard).
                || unsafe { b::network_state_prepare() },
                |g| {
                    net::network_state_prepare(g);
                    Ok(())
                },
            )
            .expect("no exit");
            c_write_state_payload(&f.ours);
            rig.g.network.packet_state_out[0].as_mut().expect("prepared").data[4..28]
                .copy_from_slice(&f.ours);
            prop_assert_eq!(
                // SAFETY: plain C call (inside the guard).
                rig.run(|| unsafe { b::network_state_send() }, |g| Ok(net::network_state_send(g))),
                Ok(0)
            );
            if let Some(t) = f.we_send {
                rig.run(
                    // SAFETY: plain C calls (inside the guard).
                    || unsafe {
                        b::network_prepare(t);
                        b::network_send(4)
                    },
                    |g| {
                        net::network_prepare(g, t);
                        Ok(net::network_send(g, 4))
                    },
                )
                .expect("no exit");
            }
            // SAFETY: plain C call (inside the guard).
            let r = rig.run(|| unsafe { b::network_state_update() }, net::network_state_update);
            prop_assert!(r.is_ok());
            // Drain what is left of the inbox.
            // SAFETY: plain C call (inside the guard).
            while rig.run(|| unsafe { b::network_check() }, net::network_check).expect("no exit") == 1 {}
            // SAFETY: plain C call (inside the guard).
            rig.run(|| unsafe { b::network_update() }, |g| Ok(net::network_update(g)))
                .expect("no exit");
            rig.both(|p| p.delay(14));
            let ours = rig.check_io();
            prop_assert_eq!(&diff::load::<State>(), &rig.g.network);

            // The opponent receives our packets and runs its frame.
            for dg in ours {
                peer_fake.push_datagram(rustyrian_harness::fake::Datagram {
                    socket: peer_socket,
                    address: US,
                    data: dg.data,
                });
            }
            while net::network_check(&mut peer).expect("no exit") == 1 {}
            prop_assert!(net::network_state_update(&mut peer).is_ok());
            peer.platform.delay(14);
        }
        rig.check();
    }
}
