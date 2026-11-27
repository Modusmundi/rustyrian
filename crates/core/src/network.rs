// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `network.c` / `network.h` (`WITH_NETWORK` is always on,
//! rule R11).

use crate::Game;
use crate::logging::SDL_LOG_CATEGORY_APPLICATION;
use crate::platform::{IpAddress, Platform, UdpPacket, UdpSocketId};
use serde::{Deserialize, Serialize};

pub const PACKET_ACKNOWLEDGE: u16 = 0x00;
pub const PACKET_KEEP_ALIVE: u16 = 0x01;
/// Version, delay, episodes, player_number, name.
pub const PACKET_CONNECT: u16 = 0x10;
/// Episode, difficulty.
pub const PACKET_DETAILS: u16 = 0x11;
pub const PACKET_QUIT: u16 = 0x20;
pub const PACKET_WAITING: u16 = 0x21;
pub const PACKET_BUSY: u16 = 0x22;
pub const PACKET_GAME_QUIT: u16 = 0x30;
pub const PACKET_GAME_PAUSE: u16 = 0x31;
pub const PACKET_GAME_MENU: u16 = 0x32;
/// State id.
pub const PACKET_STATE_RESEND: u16 = 0x40;
/// `<state>` (not acknowledged).
pub const PACKET_STATE: u16 = 0x41;
/// `<xor state>` (not acknowledged).
pub const PACKET_STATE_XOR: u16 = 0x42;

/// Increment whenever networking changes might create incompatibility.
pub const NET_VERSION: u16 = 2;
/// UDP port.
pub const NET_PORT: u16 = 1333;
pub const NET_PACKET_SIZE: i32 = 256;
pub const NET_PACKET_QUEUE: usize = 16;
/// Ticks to wait for packet acknowledgment before resending.
pub const NET_RETRY: u32 = 640;
/// Ticks to wait before requesting an unreceived game packet.
pub const NET_RESEND: u32 = 320;
/// Ticks to wait between keep-alive packets.
pub const NET_KEEP_ALIVE: u32 = 1600;
/// Ticks to wait before considering the connection dead.
pub const NET_TIME_OUT: u32 = 16000;

/// A queue of `UDPpacket *` (`None` is NULL).
pub type PacketQueue = [Option<UdpPacket>; NET_PACKET_QUEUE];

/// Mutable globals of `network.c` (rule R3).
///
/// `char *` globals hold the string bytes without the NUL (`None` is NULL).
/// `network_player_name`/`network_opponent_name` initially point to the
/// static `empty_string`, i.e. `Some(vec![])`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub is_network_game: bool,
    /// Minimum is 1 + 0.
    pub network_delay: i32,
    pub network_opponent_host: Option<Vec<u8>>,
    pub network_player_port: u16,
    pub network_opponent_port: u16,
    pub empty_string: [u8; 1],
    pub network_player_name: Option<Vec<u8>>,
    pub network_opponent_name: Option<Vec<u8>>,
    /// `socket` (`UDPsocket`); `None` is NULL.
    pub socket: Option<UdpSocketId>,
    pub ip: IpAddress,
    pub packet_out_temp: Option<UdpPacket>,
    pub packet_temp: Option<UdpPacket>,
    pub packet_in: PacketQueue,
    pub packet_out: PacketQueue,
    pub last_out_sync: u16,
    pub queue_in_sync: u16,
    pub queue_out_sync: u16,
    pub last_ack_sync: u16,
    pub last_in_tick: u32,
    pub last_out_tick: u32,
    pub packet_state_in: PacketQueue,
    pub packet_state_in_xor: PacketQueue,
    pub packet_state_out: PacketQueue,
    pub last_state_in_sync: u16,
    pub last_state_out_sync: u16,
    pub last_state_in_tick: u32,
    pub net_initialized: bool,
    pub connected: bool,
    pub quit: bool,
    /// Player number on this PC (1 or 2).
    pub this_player_num: u32,
    pub halt_game: bool,
    pub move_ok: bool,
    pub pause_request: bool,
    pub skip_level_request: bool,
    pub help_request: bool,
    pub nort_ship_request: bool,
    pub your_in_game_menu_request: bool,
    pub in_game_menu_request: bool,
    /// `network_check`'s `static Uint32 keep_alive_tick`.
    pub network_check_keep_alive_tick: u32,
    /// `network_state_update`'s `static Uint32 resend_tick`.
    pub network_state_update_resend_tick: u32,
}

impl Default for State {
    fn default() -> Self {
        State {
            is_network_game: false,
            network_delay: 1 + 1,
            network_opponent_host: None,
            network_player_port: NET_PORT,
            network_opponent_port: NET_PORT,
            empty_string: [0],
            network_player_name: Some(Vec::new()),
            network_opponent_name: Some(Vec::new()),
            socket: None,
            ip: IpAddress::default(),
            packet_out_temp: None,
            packet_temp: None,
            packet_in: Default::default(),
            packet_out: Default::default(),
            last_out_sync: 0,
            queue_in_sync: 0,
            queue_out_sync: 0,
            last_ack_sync: 0,
            last_in_tick: 0,
            last_out_tick: 0,
            packet_state_in: Default::default(),
            packet_state_in_xor: Default::default(),
            packet_state_out: Default::default(),
            last_state_in_sync: 0,
            last_state_out_sync: 0,
            last_state_in_tick: 0,
            net_initialized: false,
            connected: false,
            quit: false,
            this_player_num: 0,
            halt_game: false,
            move_ok: false,
            pause_request: false,
            skip_level_request: false,
            help_request: false,
            nort_ship_request: false,
            your_in_game_menu_request: false,
            in_game_menu_request: false,
            network_check_keep_alive_tick: 0,
            network_state_update_resend_tick: 0,
        }
    }
}

/// `SDLNet_Write16`: big-endian store into `data[at..at + 2]`.
fn write16(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}

/// The `UDPsocket` handed to SDL_net. A NULL socket becomes id 0, which no
/// platform ever allocates, so the platform answers as SDL_net does for NULL.
fn socket_id(socket: Option<UdpSocketId>) -> UdpSocketId {
    socket.unwrap_or(UdpSocketId(0))
}

/// Logs `"<what>: <SDLNet_GetError()>"` as an error.
fn log_net_error(g: &mut Game, what: &str) {
    let e = g.platform.net_get_error();
    g.platform
        .log_error(SDL_LOG_CATEGORY_APPLICATION, &format!("{what}: {e}"));
}

/// Copies `src` into `dst`: every field except the data buffer, then the first
/// `src.len` data bytes.
pub fn packet_copy(dst: &mut UdpPacket, src: &UdpPacket) {
    dst.channel = src.channel;
    dst.len = src.len;
    dst.maxlen = src.maxlen;
    dst.status = src.status;
    dst.address = src.address;
    // `len` never exceeds the 256-byte buffers in upstream's uses.
    let n = src.len as usize;
    dst.data[..n].copy_from_slice(&src.data[..n]);
}

/// Drops `packet[0]` and moves the first `max_packets` entries one slot towards
/// the front.
pub fn packets_shift_up(
    platform: &mut dyn Platform,
    packet: &mut [Option<UdpPacket>],
    max_packets: i32,
) {
    let max = max_packets as usize;
    if let Some(p) = packet[0].take() {
        platform.net_free_packet(p);
    }
    for i in 0..max - 1 {
        packet[i] = packet[i + 1].take();
    }
    packet[max - 1] = None;
}

/// Drops `packet[max_packets - 1]` and moves the first `max_packets` entries
/// one slot towards the back.
pub fn packets_shift_down(
    platform: &mut dyn Platform,
    packet: &mut [Option<UdpPacket>],
    max_packets: i32,
) {
    let max = max_packets as usize;
    if let Some(p) = packet[max - 1].take() {
        platform.net_free_packet(p);
    }
    for i in (1..max).rev() {
        packet[i] = packet[i - 1].take();
    }
    packet[0] = None;
}

/// Writes the packet header (`type`, `last_out_sync`) into `packet_out_temp`.
pub fn network_prepare(g: &mut Game, type_: u16) {
    let n = &mut g.network;
    let p = n
        .packet_out_temp
        .as_mut()
        .expect("network_init allocated packet_out_temp");
    write16(&mut p.data, 0, type_);
    write16(&mut p.data, 2, n.last_out_sync);
}

/// Sends `packet_out_temp` with length `len` on channel 0, unacknowledged.
pub fn network_send_no_ack(g: &mut Game, len: i32) -> bool {
    let socket = socket_id(g.network.socket);
    let p = g
        .network
        .packet_out_temp
        .as_mut()
        .expect("network_init allocated packet_out_temp");
    p.len = len;

    if g.platform.net_udp_send(socket, 0, p) == 0 {
        log_net_error(g, "SDLNet_UDP_Send");
        return false;
    }

    true
}

/// Sends `packet_out_temp` and queues a copy until the opponent acknowledges it.
pub fn network_send(g: &mut Game, len: i32) -> bool {
    let temp = network_send_no_ack(g, len);

    let i = g
        .network
        .last_out_sync
        .wrapping_sub(g.network.queue_out_sync);
    if usize::from(i) < NET_PACKET_QUEUE {
        let mut p = g
            .platform
            .net_alloc_packet(NET_PACKET_SIZE)
            .expect("SDLNet_AllocPacket");
        packet_copy(
            &mut p,
            g.network
                .packet_out_temp
                .as_ref()
                .expect("network_init allocated packet_out_temp"),
        );
        g.network.packet_out[usize::from(i)] = Some(p);
    } else {
        g.platform.log_error(
            SDL_LOG_CATEGORY_APPLICATION,
            "Overflowed outbound packet queue.",
        );
        return false;
    }

    g.network.last_out_sync = g.network.last_out_sync.wrapping_add(1);

    if network_is_sync(g) {
        g.network.last_out_tick = g.platform.get_ticks();
    }

    temp
}

/// Sends an acknowledgment of packet `sync`.
pub fn network_acknowledge(g: &mut Game, sync: u16) -> i32 {
    let p = g
        .network
        .packet_out_temp
        .as_mut()
        .expect("network_init allocated packet_out_temp");
    write16(&mut p.data, 0, PACKET_ACKNOWLEDGE);
    write16(&mut p.data, 2, sync);
    network_send_no_ack(g, 4);

    0
}

/// Whether anything arrived from the opponent within `NET_TIME_OUT` ticks.
pub fn network_is_alive(g: &mut Game) -> bool {
    g.platform.get_ticks().wrapping_sub(g.network.last_in_tick) < NET_TIME_OUT
        || g.platform
            .get_ticks()
            .wrapping_sub(g.network.last_state_in_tick)
            < NET_TIME_OUT
}

/// Discards the packet being processed (`packet_in[0]`) and advances the
/// inbound queue.
pub fn network_update(g: &mut Game) -> bool {
    if g.network.packet_in[0].is_some() {
        packets_shift_up(
            g.platform.as_mut(),
            &mut g.network.packet_in,
            NET_PACKET_QUEUE as i32,
        );

        g.network.queue_in_sync = g.network.queue_in_sync.wrapping_add(1);

        return true;
    }

    false
}

/// Whether the opponent has acknowledged every packet sent so far.
pub fn network_is_sync(g: &mut Game) -> bool {
    // Both operands promote to `int`: no 16-bit wrap.
    i32::from(g.network.queue_out_sync) - i32::from(g.network.last_ack_sync) == 1
}

/// Starts a new outbound state packet in `packet_state_out[0]`.
pub fn network_state_prepare(g: &mut Game) {
    if g.network.packet_state_out[0].is_some() {
        g.platform.log_warn(
            SDL_LOG_CATEGORY_APPLICATION,
            "Previous state packet has not been sent.",
        );
    } else {
        let mut p = g
            .platform
            .net_alloc_packet(NET_PACKET_SIZE)
            .expect("SDLNet_AllocPacket");
        p.len = 28;
        g.network.packet_state_out[0] = Some(p);
    }

    let n = &mut g.network;
    let p = n.packet_state_out[0].as_mut().expect("allocated above");
    write16(&mut p.data, 0, PACKET_STATE);
    write16(&mut p.data, 2, n.last_state_out_sync);
    p.data[4..28].fill(0);
}

/// Sends `packet_state_out[0]`, plus the xor of the last `network_delay` state
/// packets when one is due, then shifts the outbound state queue.
pub fn network_state_send(g: &mut Game) -> i32 {
    let socket = socket_id(g.network.socket);
    let p = g.network.packet_state_out[0]
        .as_mut()
        .expect("network_state_prepare allocated packet_state_out[0]");
    if g.platform.net_udp_send(socket, 0, p) == 0 {
        log_net_error(g, "SDLNet_UDP_Send");
        return -1;
    }

    let delay = g.network.network_delay;
    if delay > 1
        && (i32::from(g.network.last_state_out_sync) + 1) % delay == 0
        && g.network.packet_state_out[(delay - 1) as usize].is_some()
    {
        let n = &mut g.network;
        let temp = n
            .packet_temp
            .as_mut()
            .expect("network_init allocated packet_temp");
        packet_copy(temp, n.packet_state_out[0].as_ref().expect("sent above"));
        write16(&mut temp.data, 0, PACKET_STATE_XOR);
        for i in 1..delay {
            let src = n.packet_state_out[i as usize]
                .as_ref()
                .expect("state packets are queued contiguously");
            for j in 4..temp.len {
                temp.data[j as usize] ^= src.data[j as usize];
            }
        }

        if g.platform.net_udp_send(socket, 0, temp) == 0 {
            log_net_error(g, "SDLNet_UDP_Send");
            return -1;
        }
    }

    packets_shift_down(
        g.platform.as_mut(),
        &mut g.network.packet_state_out,
        NET_PACKET_QUEUE as i32,
    );

    g.network.last_state_out_sync = g.network.last_state_out_sync.wrapping_add(1);

    0
}

/// Whether the level is still within its first `network_delay` states.
pub fn network_state_is_reset(g: &mut Game) -> bool {
    i32::from(g.network.last_state_out_sync) < g.network.network_delay
}

/// Empties every state queue for a new level.
pub fn network_state_reset(g: &mut Game) {
    g.network.last_state_out_sync = 0;
    g.network.last_state_in_sync = 0;

    let n = &mut g.network;
    for queue in [
        &mut n.packet_state_in,
        &mut n.packet_state_in_xor,
        &mut n.packet_state_out,
    ] {
        for slot in queue.iter_mut() {
            if let Some(p) = slot.take() {
                g.platform.net_free_packet(p);
            }
        }
    }

    g.network.last_state_in_tick = g.platform.get_ticks();
}

/// Initializes SDL_net, opens the local socket and allocates the work packets.
pub fn network_init(g: &mut Game) -> i32 {
    g.platform.log("Initializing network...");

    if g.network.network_delay.wrapping_mul(2) > NET_PACKET_QUEUE as i32 - 2 {
        g.platform.log_error(
            SDL_LOG_CATEGORY_APPLICATION,
            "Network delay would overflow packet queue.",
        );
        return -4;
    }

    if g.platform.net_init() != 0 {
        log_net_error(g, "SDLNet_Init");
        return -1;
    }

    g.network.socket = g.platform.net_udp_open(g.network.network_player_port);
    if g.network.socket.is_none() {
        log_net_error(g, "SDLNet_UDP_Open");
        return -2;
    }

    g.network.packet_temp = g.platform.net_alloc_packet(NET_PACKET_SIZE);
    g.network.packet_out_temp = g.platform.net_alloc_packet(NET_PACKET_SIZE);

    if g.network.packet_temp.is_none() || g.network.packet_out_temp.is_none() {
        log_net_error(g, "SDLNet_AllocPacket");
        return -3;
    }

    g.network.net_initialized = true;

    0
}

/// Clears the special requests raised during a frame.
#[doc(alias = "JE_clearSpecialRequests")]
pub fn je_clear_special_requests(g: &mut Game) {
    g.network.pause_request = false;
    g.network.in_game_menu_request = false;
    g.network.skip_level_request = false;
    g.network.help_request = false;
    g.network.nort_ship_request = false;
}
