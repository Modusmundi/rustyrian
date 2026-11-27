//! Netplay: an arbitrary sequence of received datagrams, queue updates, state
//! sends and clock steps through the network state machine (D-SEC-2: no
//! datagram may panic or hang it).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::Game;
use rustyrian_core::network as net;
use rustyrian_core::platform::IpAddress;
use rustyrian_harness::fake::Datagram;

const PEER: IpAddress = IpAddress {
    host: 0x0100_007f,
    port: 0x3605,
};

fuzz_target!(|data: &[u8]| {
    let (session, _log, fake) = rustyrian_harness::test_session_with_handle();
    let mut g = Game::new(Box::new(session));
    if net::network_init(&mut g) != 0 {
        return;
    }
    let Some(socket) = g.network.socket else {
        return;
    };
    g.platform.net_udp_bind(socket, 0, &PEER);
    // Already quitting: the timeout and QUIT paths, which end the game
    // through its video teardown, are not part of this target.
    g.network.quit = true;
    g.network.connected = true;
    g.network.is_network_game = true;

    let mut rest = data;
    while let Some((&op, tail)) = rest.split_first() {
        rest = tail;
        match op % 5 {
            // One datagram: a length byte, then its bytes.
            0 => {
                let Some((&len, tail)) = rest.split_first() else {
                    break;
                };
                let n = usize::from(len).min(tail.len());
                fake.push_datagram(Datagram {
                    socket,
                    address: PEER,
                    data: tail[..n].to_vec(),
                });
                rest = &tail[n..];
                let _ = net::network_check(&mut g);
            }
            1 => {
                net::network_update(&mut g);
            }
            2 => {
                net::network_state_prepare(&mut g);
                net::network_state_send(&mut g);
            }
            // The next state only when it cannot wait forever.
            3 => {
                if !net::network_state_is_reset(&mut g) && g.network.packet_state_in[1].is_some() {
                    let _ = net::network_state_update(&mut g);
                }
            }
            _ => {
                let ms = rest.first().map_or(0, |&b| u32::from(b) * 20);
                g.platform.delay(ms);
                rest = rest.get(1..).unwrap_or(&[]);
            }
        }
    }
});
