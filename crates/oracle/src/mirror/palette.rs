//! Mirror of `rustyrian_core::palette::State` <-> the C globals of `palette.c`.

use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::palette::{Color, PALETTES_MAX, Palette, State};

/// Converts a C `Palette`.
pub fn palette_from_c(c: &b::Palette) -> Palette {
    c.map(|x| Color {
        r: x.r,
        g: x.g,
        b: x.b,
        a: x.a,
    })
}

/// Converts a core `Palette`.
pub fn palette_to_c(r: &Palette) -> b::Palette {
    r.map(|x| b::SDL_Color {
        r: x.r,
        g: x.g,
        b: x.b,
        a: x.a,
    })
}

type Diff = [[i32; 3]; 256];

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make. `palettes`
        // is declared `Palette palettes[]` in the header and defined with
        // `PALETTES_MAX` entries in `palette.c`.
        unsafe {
            let palettes = sv::<[b::Palette; PALETTES_MAX]>(b::ot_sv_palette_palettes);
            for (d, s) in self.palettes.iter_mut().zip(palettes.iter()) {
                *d = palette_from_c(s);
            }
            self.palettes_count = *global(&raw const b::palettesCount);
            self.palette = palette_from_c(sv::<b::Palette>(b::ot_sv_palette_palette));
            self.rgb_palette = *global(&raw const b::rgb_palette);
            self.yuv_palette = *global(&raw const b::yuv_palette);
            self.colors = palette_from_c(global(&raw const b::colors));
            self.fade_palette_diff = *sv::<Diff>(b::ot_sv_palette_fade_palette_diff);
            self.fade_solid_diff = *sv::<Diff>(b::ot_sv_palette_fade_solid_diff);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            let palettes = sv::<[b::Palette; PALETTES_MAX]>(b::ot_sv_palette_palettes);
            for (d, s) in palettes.iter_mut().zip(self.palettes.iter()) {
                *d = palette_to_c(s);
            }
            *global_mut(&raw mut b::palettesCount) = self.palettes_count;
            *sv::<b::Palette>(b::ot_sv_palette_palette) = palette_to_c(&self.palette);
            *global_mut(&raw mut b::rgb_palette) = self.rgb_palette;
            *global_mut(&raw mut b::yuv_palette) = self.yuv_palette;
            *global_mut(&raw mut b::colors) = palette_to_c(&self.colors);
            *sv::<Diff>(b::ot_sv_palette_fade_palette_diff) = self.fade_palette_diff;
            *sv::<Diff>(b::ot_sv_palette_fade_solid_diff) = self.fade_solid_diff;
        }
    }
}
