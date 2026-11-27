//! `newsh$.shp` (custom ships, user directory): `loadExtraShapes` on a
//! sprite sheet followed by the ship table, given as plaintext and encrypted
//! here so its checksums hold. When the ships are available, each custom
//! ship's graphic is looked up (`JE_SGr`) and drawn as the game draws ships
//! (`blit_sprite2x2`), and every sprite of the sheet is drawn once.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::sprite as s;
use rustyrian_core::varz::{self, Sprite2Ref};
use rustyrian_core::video::ScreenId;

const V: ScreenId = ScreenId::VgaScreenSeg;

fuzz_target!(|data: &[u8]| {
    let file = rustyrian_fuzz::encrypt_extra_ships(data);
    let mut g = rustyrian_fuzz::game_with_user_file("newsh$.shp", &file);
    rustyrian_fuzz::with_screens(&mut g);
    rustyrian_core::editship::load_extra_shapes(&mut g);
    if !g.editship.extra_avail {
        return;
    }

    // The table holds 10 ships of 15 bytes.
    for ship in 1..=10 {
        let mut sheet = None;
        let graphic = u32::from(varz::je_s_gr(&mut g, ship, &mut sheet));
        if let Some(sheet) = sheet {
            // The ship's banking frames, two columns apart.
            for frame in [graphic, graphic + 2, graphic.wrapping_sub(2)] {
                s::blit_sprite2x2(&mut g, V, 140, 80, sheet, frame);
            }
        }
    }

    let entries = g
        .editship
        .extra_shapes
        .data
        .as_deref()
        .and_then(|d| d.get(..2))
        .map_or(0, |b| u32::from(u16::from_le_bytes([b[0], b[1]])) / 2);
    for index in 0..=entries.min(1024) + 1 {
        s::blit_sprite2(&mut g, V, -4, 195, Sprite2Ref::ExtraShapes, index);
    }
});
