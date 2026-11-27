//! Mirror of `rustyrian_core::video_scale_hqnx::State` <-> the C globals of
//! `video_scale_hqNx.c`.

use super::Mirror;
use super::video::sv;
use crate::bindings as b;
use rustyrian_core::video_scale_hqnx::State;
use std::ffi::c_int;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; both
        // statics are `int`.
        unsafe {
            self.yuv1 = *sv::<c_int>(b::ot_sv_video_scale_hqNx_YUV1);
            self.yuv2 = *sv::<c_int>(b::ot_sv_video_scale_hqNx_YUV2);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *sv::<c_int>(b::ot_sv_video_scale_hqNx_YUV1) = self.yuv1;
            *sv::<c_int>(b::ot_sv_video_scale_hqNx_YUV2) = self.yuv2;
        }
    }
}
