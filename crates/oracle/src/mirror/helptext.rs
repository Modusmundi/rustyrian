//! Mirror of `rustyrian_core::helptext::State` <-> the C globals of
//! `helptext.c` (arrays of `char` strings, copied as bytes).

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::helptext::State;

/// Copies between a C `char` array global and its core `u8` array of the same
/// shape (`char` and `u8` have the same size and layout).
macro_rules! text {
    ($load:expr, $dst:expr, $c:ident) => {
        if $load {
            copy(&raw const b::$c, $dst);
        } else {
            store(&*$dst, &raw mut b::$c);
        }
    };
}

/// `p` as a `*const B`; a size mismatch between the C global and the core
/// array is a compile error.
const fn same<A, B>(p: *const A) -> *const B {
    const { assert!(size_of::<A>() == size_of::<B>()) };
    p.cast()
}

fn copy<T: Copy, C>(src: *const C, dst: &mut T) {
    // SAFETY: C runs only on this thread, inside calls we make; the C global
    // has the same layout as `T` (`char` and `u8` arrays of equal size).
    *dst = *unsafe { global(same::<C, T>(src)) };
}

fn store<T: Copy, C>(src: &T, dst: *mut C) {
    // SAFETY: as in `copy`.
    *unsafe { global_mut(same::<C, T>(dst).cast_mut()) } = *src;
}

fn mirror(s: &mut State, load: bool) {
    let st = s;
    text!(load, &mut *st.help_txt, helpTxt);
    text!(load, &mut st.p_name, pName);
    text!(load, &mut st.misc_text, miscText);
    text!(load, &mut st.misc_text_b, miscTextB);
    text!(load, &mut st.key_name, keyName);
    text!(load, &mut st.menu_text, menuText);
    text!(load, &mut st.outputs, outputs);
    text!(load, &mut st.topic_name, topicName);
    text!(load, &mut st.main_menu_help, mainMenuHelp);
    text!(load, &mut st.in_game_text, inGameText);
    text!(load, &mut st.detail_level, detailLevel);
    text!(load, &mut st.game_speed_text, gameSpeedText);
    text!(load, &mut st.input_devices, inputDevices);
    text!(load, &mut st.network_text, networkText);
    text!(load, &mut st.difficulty_name_b, difficultyNameB);
    text!(load, &mut st.joy_button_names, joyButtonNames);
    text!(load, &mut st.super_ships, superShips);
    text!(load, &mut st.special_name, specialName);
    text!(load, &mut st.destruct_help, destructHelp);
    text!(load, &mut st.weapon_names, weaponNames);
    text!(load, &mut st.destruct_mode_name, destructModeName);
    text!(load, &mut *st.ship_info, shipInfo);
    text!(load, &mut st.menu_int, menuInt);
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        mirror(self, true);
    }

    fn store_to_c(&self) {
        let mut s = self.clone();
        mirror(&mut s, false);
    }
}
