//! Mirror of `rustyrian_core::network::State` <-> the C globals of
//! `network.c`.
//!
//! Mirrored by contents (pointer values are not preserved):
//! - `UDPpacket *` globals: loading copies the packet; storing reuses C's
//!   packet when its `maxlen` matches and otherwise frees it and allocates a
//!   new one the way the `SDLNet_AllocPacket` shim does (so the C code can
//!   still `SDLNet_FreePacket` it). Nothing is recorded on the C session.
//! - `char *` globals: loading copies the NUL-terminated string; storing
//!   frees C's string if it is a live upstream heap block (not `argv` or
//!   `empty_string`), then points an empty string at `empty_string` (the C
//!   initial value) and copies any other into a fresh block from the
//!   upstream allocator.

use super::video::{c_heap_free, c_heap_from, handle_from_c, handle_to_c, sv};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::network::{NET_PACKET_QUEUE, PacketQueue, State};
use rustyrian_core::platform::{IpAddress, UdpPacket, UdpSocketId};
use std::ffi::{CStr, c_char, c_void};

/// Copies a C packet.
///
/// # Safety
/// `p` is NULL or a packet whose `data` holds `maxlen` bytes.
pub unsafe fn packet_from_c(p: *const b::UDPpacket) -> Option<UdpPacket> {
    // SAFETY: per the caller.
    let p = unsafe { p.as_ref() }?;
    let maxlen = usize::try_from(p.maxlen).unwrap_or(0);
    let data = if p.data.is_null() {
        Vec::new()
    } else {
        // SAFETY: per the caller.
        unsafe { std::slice::from_raw_parts(p.data, maxlen) }.to_vec()
    };
    Some(UdpPacket {
        channel: p.channel,
        data,
        len: p.len,
        maxlen: p.maxlen,
        status: p.status,
        address: IpAddress {
            host: p.address.host,
            port: p.address.port,
        },
    })
}

/// Frees a packet allocated by the `SDLNet_AllocPacket` shim or this mirror.
///
/// # Safety
/// `p` is NULL or such a packet, not referenced anywhere else.
unsafe fn free_packet(p: *mut b::UDPpacket) {
    // SAFETY: per the caller.
    if let Some(q) = unsafe { p.as_mut() } {
        // SAFETY: both were allocated with calloc.
        unsafe {
            libc::free(q.data.cast());
            libc::free(p.cast());
        }
    }
}

/// Makes `*slot` a C packet equal to `r`.
///
/// # Safety
/// `*slot` is NULL or a packet as in `free_packet`.
pub unsafe fn packet_to_c(r: Option<&UdpPacket>, slot: &mut *mut b::UDPpacket) {
    let Some(r) = r else {
        // SAFETY: per the caller.
        unsafe { free_packet(*slot) };
        *slot = std::ptr::null_mut();
        return;
    };
    // SAFETY: per the caller.
    let fits = unsafe { slot.as_ref() }.is_some_and(|c| c.maxlen == r.maxlen);
    if !fits {
        // SAFETY: per the caller; fresh calloc allocations as in the shim.
        unsafe {
            free_packet(*slot);
            let p = libc::calloc(1, std::mem::size_of::<b::UDPpacket>()).cast::<b::UDPpacket>();
            let data = libc::calloc(1, r.data.len().max(1)).cast::<u8>();
            assert!(!p.is_null() && !data.is_null(), "out of memory");
            (*p).data = data;
            *slot = p;
        }
    }
    // SAFETY: `*slot` is a valid packet whose data holds `r.data.len()` bytes.
    let c = unsafe { &mut **slot };
    // SAFETY: as above.
    unsafe { std::ptr::copy_nonoverlapping(r.data.as_ptr(), c.data, r.data.len()) };
    c.channel = r.channel;
    c.len = r.len;
    c.maxlen = r.maxlen;
    c.status = r.status;
    c.address = b::IPaddress {
        host: r.address.host,
        port: r.address.port,
    };
}

/// # Safety
/// `getter` returns the address of a `UDPpacket *[NET_PACKET_QUEUE]`.
unsafe fn queue<'a>(
    getter: unsafe extern "C" fn() -> *mut c_void,
) -> &'a mut [*mut b::UDPpacket; NET_PACKET_QUEUE] {
    // SAFETY: per the caller.
    unsafe { sv(getter) }
}

/// # Safety
/// As [`queue`]; every non-NULL entry is a valid packet.
unsafe fn queue_from_c(getter: unsafe extern "C" fn() -> *mut c_void) -> PacketQueue {
    // SAFETY: per the caller.
    let q = unsafe { queue(getter) };
    // SAFETY: per the caller.
    q.map(|p| unsafe { packet_from_c(p) })
}

/// # Safety
/// As [`queue_from_c`].
unsafe fn queue_to_c(r: &PacketQueue, getter: unsafe extern "C" fn() -> *mut c_void) {
    // SAFETY: per the caller.
    let q = unsafe { queue(getter) };
    for (r, slot) in r.iter().zip(q.iter_mut()) {
        // SAFETY: per the caller.
        unsafe { packet_to_c(r.as_ref(), slot) };
    }
}

/// # Safety
/// `p` is NULL or NUL-terminated.
unsafe fn string_from_c(p: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: per the caller.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

/// Replaces the C string in `*slot` with `r`.
///
/// # Safety
/// `*slot` is NULL, a string not on the upstream heap, or a live upstream
/// heap block referenced nowhere else.
unsafe fn string_to_c(r: Option<&Vec<u8>>, empty: *mut c_char, slot: &mut *mut c_char) {
    // SAFETY: per the caller.
    unsafe { c_heap_free(*slot) };
    *slot = match r {
        None => std::ptr::null_mut(),
        Some(s) if s.is_empty() => empty,
        Some(s) => {
            let mut z = s.clone();
            z.push(0);
            c_heap_from(&z).cast()
        }
    };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            self.is_network_game = *global(&raw const b::isNetworkGame);
            self.network_delay = *global(&raw const b::network_delay);
            self.network_opponent_host =
                string_from_c(*global(&raw const b::network_opponent_host));
            self.network_player_port = *global(&raw const b::network_player_port);
            self.network_opponent_port = *global(&raw const b::network_opponent_port);
            self.empty_string = *sv::<[u8; 1]>(b::ot_sv_network_empty_string);
            self.network_player_name = string_from_c(*global(&raw const b::network_player_name));
            self.network_opponent_name =
                string_from_c(*global(&raw const b::network_opponent_name));
            self.socket =
                handle_from_c(*sv::<b::UDPsocket>(b::ot_sv_network_socket)).map(UdpSocketId);
            let ip = sv::<b::IPaddress>(b::ot_sv_network_ip);
            self.ip = IpAddress {
                host: ip.host,
                port: ip.port,
            };
            self.packet_out_temp = packet_from_c(*global(&raw const b::packet_out_temp));
            self.packet_temp =
                packet_from_c(*sv::<*mut b::UDPpacket>(b::ot_sv_network_packet_temp));
            self.packet_in = queue_from_c(b::ot_sv_network_packet_in);
            self.packet_out = queue_from_c(b::ot_sv_network_packet_out);
            self.last_out_sync = *sv::<u16>(b::ot_sv_network_last_out_sync);
            self.queue_in_sync = *sv::<u16>(b::ot_sv_network_queue_in_sync);
            self.queue_out_sync = *sv::<u16>(b::ot_sv_network_queue_out_sync);
            self.last_ack_sync = *sv::<u16>(b::ot_sv_network_last_ack_sync);
            self.last_in_tick = *sv::<u32>(b::ot_sv_network_last_in_tick);
            self.last_out_tick = *sv::<u32>(b::ot_sv_network_last_out_tick);
            self.packet_state_in = queue_from_c(b::ot_sv_network_packet_state_in);
            self.packet_state_in_xor = queue_from_c(b::ot_sv_network_packet_state_in_xor);
            self.packet_state_out = queue_from_c(b::ot_sv_network_packet_state_out);
            self.last_state_in_sync = *sv::<u16>(b::ot_sv_network_last_state_in_sync);
            self.last_state_out_sync = *sv::<u16>(b::ot_sv_network_last_state_out_sync);
            self.last_state_in_tick = *sv::<u32>(b::ot_sv_network_last_state_in_tick);
            self.net_initialized = *sv::<bool>(b::ot_sv_network_net_initialized);
            self.connected = *sv::<bool>(b::ot_sv_network_connected);
            self.quit = *sv::<bool>(b::ot_sv_network_quit);
            self.this_player_num = *global(&raw const b::thisPlayerNum);
            self.halt_game = *global(&raw const b::haltGame);
            self.move_ok = *global(&raw const b::moveOk);
            self.pause_request = *global(&raw const b::pauseRequest);
            self.skip_level_request = *global(&raw const b::skipLevelRequest);
            self.help_request = *global(&raw const b::helpRequest);
            self.nort_ship_request = *global(&raw const b::nortShipRequest);
            self.your_in_game_menu_request = *global(&raw const b::yourInGameMenuRequest);
            self.in_game_menu_request = *global(&raw const b::inGameMenuRequest);
            self.network_check_keep_alive_tick =
                *sv::<u32>(b::ot_sv_network_network_check_keep_alive_tick);
            self.network_state_update_resend_tick =
                *sv::<u32>(b::ot_sv_network_network_state_update_resend_tick);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`; packets were allocated by the shim or
        // by this mirror.
        unsafe {
            let empty = sv::<[c_char; 1]>(b::ot_sv_network_empty_string);
            *empty = self.empty_string.map(|c| c as c_char);
            let empty: *mut c_char = empty.as_mut_ptr();
            *global_mut(&raw mut b::isNetworkGame) = self.is_network_game;
            *global_mut(&raw mut b::network_delay) = self.network_delay;
            string_to_c(
                self.network_opponent_host.as_ref(),
                empty,
                global_mut(&raw mut b::network_opponent_host),
            );
            *global_mut(&raw mut b::network_player_port) = self.network_player_port;
            *global_mut(&raw mut b::network_opponent_port) = self.network_opponent_port;
            string_to_c(
                self.network_player_name.as_ref(),
                empty,
                global_mut(&raw mut b::network_player_name),
            );
            string_to_c(
                self.network_opponent_name.as_ref(),
                empty,
                global_mut(&raw mut b::network_opponent_name),
            );
            *sv::<b::UDPsocket>(b::ot_sv_network_socket) = handle_to_c(self.socket.map(|h| h.0));
            *sv::<b::IPaddress>(b::ot_sv_network_ip) = b::IPaddress {
                host: self.ip.host,
                port: self.ip.port,
            };
            packet_to_c(
                self.packet_out_temp.as_ref(),
                global_mut(&raw mut b::packet_out_temp),
            );
            packet_to_c(
                self.packet_temp.as_ref(),
                sv::<*mut b::UDPpacket>(b::ot_sv_network_packet_temp),
            );
            queue_to_c(&self.packet_in, b::ot_sv_network_packet_in);
            queue_to_c(&self.packet_out, b::ot_sv_network_packet_out);
            *sv::<u16>(b::ot_sv_network_last_out_sync) = self.last_out_sync;
            *sv::<u16>(b::ot_sv_network_queue_in_sync) = self.queue_in_sync;
            *sv::<u16>(b::ot_sv_network_queue_out_sync) = self.queue_out_sync;
            *sv::<u16>(b::ot_sv_network_last_ack_sync) = self.last_ack_sync;
            *sv::<u32>(b::ot_sv_network_last_in_tick) = self.last_in_tick;
            *sv::<u32>(b::ot_sv_network_last_out_tick) = self.last_out_tick;
            queue_to_c(&self.packet_state_in, b::ot_sv_network_packet_state_in);
            queue_to_c(
                &self.packet_state_in_xor,
                b::ot_sv_network_packet_state_in_xor,
            );
            queue_to_c(&self.packet_state_out, b::ot_sv_network_packet_state_out);
            *sv::<u16>(b::ot_sv_network_last_state_in_sync) = self.last_state_in_sync;
            *sv::<u16>(b::ot_sv_network_last_state_out_sync) = self.last_state_out_sync;
            *sv::<u32>(b::ot_sv_network_last_state_in_tick) = self.last_state_in_tick;
            *sv::<bool>(b::ot_sv_network_net_initialized) = self.net_initialized;
            *sv::<bool>(b::ot_sv_network_connected) = self.connected;
            *sv::<bool>(b::ot_sv_network_quit) = self.quit;
            *global_mut(&raw mut b::thisPlayerNum) = self.this_player_num;
            *global_mut(&raw mut b::haltGame) = self.halt_game;
            *global_mut(&raw mut b::moveOk) = self.move_ok;
            *global_mut(&raw mut b::pauseRequest) = self.pause_request;
            *global_mut(&raw mut b::skipLevelRequest) = self.skip_level_request;
            *global_mut(&raw mut b::helpRequest) = self.help_request;
            *global_mut(&raw mut b::nortShipRequest) = self.nort_ship_request;
            *global_mut(&raw mut b::yourInGameMenuRequest) = self.your_in_game_menu_request;
            *global_mut(&raw mut b::inGameMenuRequest) = self.in_game_menu_request;
            *sv::<u32>(b::ot_sv_network_network_check_keep_alive_tick) =
                self.network_check_keep_alive_tick;
            *sv::<u32>(b::ot_sv_network_network_state_update_resend_tick) =
                self.network_state_update_resend_tick;
        }
    }
}
