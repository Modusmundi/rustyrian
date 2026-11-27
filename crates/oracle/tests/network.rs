//! Differential tests for `network.c` / `network.h`.

use rustyrian_core::network::{NET_PACKET_SIZE, State};
use rustyrian_core::platform::{IpAddress, UdpPacket, UdpSocketId};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

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
