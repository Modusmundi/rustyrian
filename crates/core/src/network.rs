// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `network.c` / `network.h` (`WITH_NETWORK` is always on,
//! rule R11).

use crate::platform::{IpAddress, UdpPacket, UdpSocketId};
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
