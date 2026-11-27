//! Differential tests for `palette.c` / `palette.h`.

use rustyrian_core::palette::{Color, State};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn color(i: usize, salt: usize) -> Color {
    let v = i.wrapping_mul(31).wrapping_add(salt);
    Color {
        r: v as u8,
        g: (v >> 1) as u8,
        b: (v >> 2) as u8,
        a: (v >> 3) as u8,
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (p, pal) in s.palettes.iter_mut().enumerate() {
        for (i, c) in pal.iter_mut().enumerate() {
            *c = color(i, p);
        }
    }
    s.palettes_count = 23;
    for i in 0..256 {
        s.palette[i] = color(i, 100);
        s.colors[i] = color(i, 200);
        s.rgb_palette[i] = (i as u32) * 0x0101_0101;
        s.yuv_palette[i] = !(i as u32);
        s.fade_palette_diff[i] = [i as i32, -(i as i32), 7];
        s.fade_solid_diff[i] = [-1, i as i32 * 3, -(i as i32)];
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
