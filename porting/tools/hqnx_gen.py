#!/usr/bin/env python3
"""One-off translator: vendor/opentyrian/src/video_scale_hqNx.c -> crates/core/src/video_scale_hqnx.rs.

Mechanically expands the PIXELxx_yy macros inside the three `switch (pattern)`
tables into Rust `match` arms. Everything outside the switch tables (interp*,
diff, the scan loops) is hand-written in the HEADER / PROLOGUE / EPILOGUE
templates below, statement for statement after the C.

usage: hqnx_gen.py <repo root>
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
src = (root / "vendor/opentyrian/src/video_scale_hqNx.c").read_text().splitlines()

# ---- macros -------------------------------------------------------------
macros = {}
re_def = re.compile(r"^#define\s+(PIXEL\w+)\s+(.*?)\s*$")
re_dst = r"(?:dst|\(dst(?: \+ (?P<off>[^)]*?))?\s*\))"
re_store = re.compile(r"^\*\(Uint32 \*\)" + re_dst + r"\s*=\s*c\[(?P<v>\d)\];$")
re_interp = re.compile(r"^(?P<f>interp\d+)\(\(Uint32 \*\)" + re_dst + r", (?P<args>c\[\d\](?:, c\[\d\])*)\);$")


def rust_off(off):
    if off is None:
        return "dst"
    terms = []
    for t in off.split(" + "):
        t = t.strip()
        m = re.fullmatch(r"(?:(\d+) \* )?(dst_Bpp|dst_pitch)", t)
        assert m, (off, t)
        name = "DST_BPP" if m.group(2) == "dst_Bpp" else "dst_pitch"
        terms.append(f"{m.group(1)} * {name}" if m.group(1) else name)
    return "dst + " + " + ".join(terms)


for line in src:
    m = re_def.match(line)
    if not m:
        continue
    name, body = m.groups()
    assert name not in macros, name
    if (s := re_store.match(body)):
        macros[name] = f"store(&mut buf[{rust_off(s.group('off'))}..], c[{s.group('v')}]);"
    elif (s := re_interp.match(body)):
        args = ", ".join(a.strip() for a in s.group("args").split(","))
        macros[name] = f"{s.group('f')}(&mut buf[{rust_off(s.group('off'))}..], {args});"
    else:
        raise SystemExit(f"unparsed macro {name}: {body}")

# ---- switch tables ------------------------------------------------------
re_case = re.compile(r"^case (\d+):$")
re_if = re.compile(r"^if \(diff\(w\[(\d)\], w\[(\d)\]\)\)$")


def tokens_of_switch(start):
    """Tokens of the switch body that opens after line index `start`."""
    toks = []
    depth = 0
    i = start + 1
    while True:
        for t in re.findall(r"case \d+:|if \(diff\(w\[\d\], w\[\d\]\)\)|else|break;|\{|\}|PIXEL\w+|\S+", src[i].split("//")[0]):
            toks.append(t)
            if t == "{":
                depth += 1
            elif t == "}":
                depth -= 1
                if depth == 0:
                    return toks, i
        i += 1


def parse_block(toks, pos, ind):
    """Parses `{ stmts }` from toks[pos] == '{'; returns (lines, pos after, broke)."""
    assert toks[pos] == "{", toks[pos]
    pos += 1
    out = []
    broke = False
    while toks[pos] != "}":
        t = toks[pos]
        assert not broke, "statement after break"
        if t in macros:
            out.append(ind + macros[t])
            pos += 1
        elif (m := re_if.match(t)):
            then, pos, b = parse_block(toks, pos + 1, ind + "    ")
            assert not b
            out.append(f"{ind}if diff(yuv_palette, w[{m.group(1)}], w[{m.group(2)}]) {{")
            out += then
            if toks[pos] == "else":
                els, pos, b = parse_block(toks, pos + 1, ind + "    ")
                assert not b
                out.append(f"{ind}}} else {{")
                out += els
            out.append(ind + "}")
        elif t == "break;":
            broke = True
            pos += 1
        else:
            raise SystemExit(f"unexpected token {t!r}")
    return out, pos + 1, broke


def gen_switch(start, ind):
    toks, end = tokens_of_switch(start)
    assert toks[0] == "{" and toks[-1] == "}"
    pos = 1
    arms = []
    seen = set()
    while pos < len(toks) - 1:
        labels = []
        while (m := re_case.match(toks[pos])):
            labels.append(int(m.group(1)))
            pos += 1
        assert labels, toks[pos]
        body, pos, broke = parse_block(toks, pos, ind + "        ")
        assert broke, f"case {labels} falls through"
        assert not seen & set(labels)
        seen |= set(labels)
        pat = " | ".join(map(str, labels))
        arms.append(f"{ind}    {pat} => {{")
        arms += body
        arms.append(f"{ind}    }}")
    assert seen == set(range(256)), sorted(set(range(256)) - seen)
    lines = [f"{ind}match pattern {{"] + arms
    # C has no `default:`; every value 0..=255 has a label (checked above) and
    # `pattern` never exceeds 255.
    lines += [f"{ind}    _ => {{}}", f"{ind}}}"]
    return lines, end


switch_lines = [i for i, l in enumerate(src) if l.strip() == "switch (pattern)"]
assert len(switch_lines) == 3
tables = [gen_switch(i, " " * 12)[0] for i in switch_lines]

HEADER = '''// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) 2003 MaxSt ( maxst@hiend3d.com )
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale_hqNx.c`: the hq2x, hq3x and hq4x scalers.
//!
//! The `switch (pattern)` tables of the three scalers are a mechanical
//! translation of upstream's (each `PIXELxy_z` macro expanded in place, case
//! labels and arm order kept); everything else follows the C statement for
//! statement.
//!
//! The C `Uint32 *pc` / `Uint8 *dst` pointers into the locked texture are byte
//! offsets into the locked buffer (rule R8); `pc` arguments are the buffer
//! suffix starting at the pointed-to pixel. Pixels are stored in native byte
//! order, as C's `*(Uint32 *)` stores are.

use crate::Game;
use crate::platform::TextureId;
use crate::video::{ScreenId, VGA_HEIGHT, VGA_WIDTH};
use serde::{Deserialize, Serialize};

/// `Ymask`.
pub const YMASK: i32 = 0x00FF_0000;
/// `Umask`.
pub const UMASK: i32 = 0x0000_FF00;
/// `Vmask`.
pub const VMASK: i32 = 0x0000_00FF;
/// `trY`.
pub const TR_Y: i32 = 0x0030_0000;
/// `trU`.
pub const TR_U: i32 = 0x0000_0700;
/// `trV`.
pub const TR_V: i32 = 0x0000_0006;

/// Mutable globals of `video_scale_hqNx.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `YUV1`.
    pub yuv1: i32,
    /// `YUV2`.
    pub yuv2: i32,
}

/// `*(Uint32 *)pc = v`.
fn store(pc: &mut [u8], v: u32) {
    pc[..4].copy_from_slice(&v.to_ne_bytes());
}

pub fn interp1(pc: &mut [u8], c1: u32, c2: u32) {
    store(pc, c1.wrapping_mul(3).wrapping_add(c2) >> 2);
}

pub fn interp2(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(pc, c1.wrapping_mul(2).wrapping_add(c2).wrapping_add(c3) >> 2);
}

pub fn interp3(pc: &mut [u8], c1: u32, c2: u32) {
    store(
        pc,
        (((c1 & 0x00FF00).wrapping_mul(7).wrapping_add(c2 & 0x00FF00) & 0x0007_F800)
            .wrapping_add((c1 & 0xFF00FF).wrapping_mul(7).wrapping_add(c2 & 0xFF00FF) & 0x07F8_07F8))
            >> 3,
    );
}

pub fn interp4(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(2)
            .wrapping_add((c2 & 0x00FF00).wrapping_add(c3 & 0x00FF00).wrapping_mul(7))
            & 0x000F_F000)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(2)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_add(c3 & 0xFF00FF).wrapping_mul(7))
                    & 0x0FF0_0FF0,
            ))
            >> 4,
    );
}

pub fn interp5(pc: &mut [u8], c1: u32, c2: u32) {
    store(pc, c1.wrapping_add(c2) >> 1);
}

pub fn interp6(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(5)
            .wrapping_add((c2 & 0x00FF00).wrapping_mul(2))
            .wrapping_add(c3 & 0x00FF00)
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(5)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_mul(2))
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp7(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(6)
            .wrapping_add(c2 & 0x00FF00)
            .wrapping_add(c3 & 0x00FF00)
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(6)
                    .wrapping_add(c2 & 0xFF00FF)
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp8(pc: &mut [u8], c1: u32, c2: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(5)
            .wrapping_add((c2 & 0x00FF00).wrapping_mul(3))
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(5)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_mul(3))
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp9(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(2)
            .wrapping_add((c2 & 0x00FF00).wrapping_add(c3 & 0x00FF00).wrapping_mul(3))
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(2)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_add(c3 & 0xFF00FF).wrapping_mul(3))
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp10(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(14)
            .wrapping_add(c2 & 0x00FF00)
            .wrapping_add(c3 & 0x00FF00)
            & 0x000F_F000)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(14)
                    .wrapping_add(c2 & 0xFF00FF)
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x0FF0_0FF0,
            ))
            >> 4,
    );
}

/// Takes `yuv_palette` (`palette.c`) rather than the `Game`: the scalers call
/// it while the locked texture borrows `g.platform`.
pub fn diff(yuv_palette: &[u32; 256], w1: u32, w2: u32) -> bool {
    let yuv1 = yuv_palette[w1 as usize];
    let yuv2 = yuv_palette[w2 as usize];
    ((yuv1 & YMASK as u32) as i32 - (yuv2 & YMASK as u32) as i32).abs() > TR_Y
        || ((yuv1 & UMASK as u32) as i32 - (yuv2 & UMASK as u32) as i32).abs() > TR_U
        || ((yuv1 & VMASK as u32) as i32 - (yuv2 & VMASK as u32) as i32).abs() > TR_V
}
'''

PROLOGUE = '''
pub fn hq{n}x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {{
    let src_surface = g.video.screen(src_surface);
    let pixels = &src_surface.pixels;
    let mut src: usize = 0;

    let src_pitch = src_surface.pitch;

    const DST_BPP: usize = 4; // dst_surface->format->BytesPerPixel
    let height = VGA_HEIGHT; // src_surface->h
    let width = VGA_WIDTH; // src_surface->w

    let Some((buf, dst_pitch)) = g.platform.lock_texture(dst_texture, None) else {{
        // D-UB-SCALER-LOCK: C ignores the failure and writes through the
        // unset pixel pointer.
        g.platform.unlock_texture(dst_texture);
        return;
    }};
    let dst_pitch = dst_pitch as usize;
    let mut dst: usize = 0;

    let yuv_palette = &g.palette.yuv_palette;
    let rgb_palette = &g.palette.rgb_palette;
    let st = &mut g.video_scale_hqnx;

    let mut w = [0u32; 10];
    let mut c = [0u32; 10];

    //   +----+----+----+
    //   |    |    |    |
    //   | w1 | w2 | w3 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w4 | w5 | w6 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w7 | w8 | w9 |
    //   +----+----+----+

    for j in 0..height {{
        let src_temp = src;
        let dst_temp = dst;

        let prevline: isize = if j > 0 {{ -(width as isize) }} else {{ 0 }};
        let nextline: isize = if j < height - 1 {{ width as isize }} else {{ 0 }};

        for i in 0..width {{
            w[2] = u32::from(pixels[src.wrapping_add_signed(prevline)]);
            w[5] = u32::from(pixels[src]);
            w[8] = u32::from(pixels[src.wrapping_add_signed(nextline)]);

            if i > 0 {{
                w[1] = u32::from(pixels[src.wrapping_add_signed(prevline - 1)]);
                w[4] = u32::from(pixels[src - 1]);
                w[7] = u32::from(pixels[src.wrapping_add_signed(nextline - 1)]);
            }} else {{
                w[1] = w[2];
                w[4] = w[5];
                w[7] = w[8];
            }}

            if i < width - 1 {{
                w[3] = u32::from(pixels[src.wrapping_add_signed(prevline + 1)]);
                w[6] = u32::from(pixels[src + 1]);
                w[9] = u32::from(pixels[src.wrapping_add_signed(nextline + 1)]);
            }} else {{
                w[3] = w[2];
                w[6] = w[5];
                w[9] = w[8];
            }}

            let mut pattern: i32 = 0;
            let mut flag: i32 = 1;

            st.yuv1 = yuv_palette[w[5] as usize] as i32;

            for k in 1..=9 {{
                if k == 5 {{
                    continue;
                }}

                if w[k] != w[5] {{
                    st.yuv2 = yuv_palette[w[k] as usize] as i32;
                    if ((st.yuv1 & YMASK) - (st.yuv2 & YMASK)).abs() > TR_Y
                        || ((st.yuv1 & UMASK) - (st.yuv2 & UMASK)).abs() > TR_U
                        || ((st.yuv1 & VMASK) - (st.yuv2 & VMASK)).abs() > TR_V
                    {{
                        pattern |= flag;
                    }}
                }}
                flag <<= 1;
            }}

            for k in 1..=9 {{
                // hq{n}x has a nasty inability to accept more than 6 bits for each component
                c[k] = rgb_palette[w[k] as usize] & 0xfcfc_fcfc;
            }}

'''

EPILOGUE = '''
            src += 1;
            dst += {n} * DST_BPP;
        }}

        src = src_temp.wrapping_add_signed(src_pitch as isize);
        dst = dst_temp + {n} * dst_pitch;
    }}

    g.platform.unlock_texture(dst_texture);
}}
'''

out = HEADER
for n, table in zip((2, 3, 4), tables):
    out += PROLOGUE.format(n=n) + "\n".join(table) + "\n" + EPILOGUE.format(n=n)

(root / "crates/core/src/video_scale_hqnx.rs").write_text(out)
print("ok", {k: len(v) for k, v in zip((2, 3, 4), tables)})
