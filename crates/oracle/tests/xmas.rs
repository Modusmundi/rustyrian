//! Differential tests for `xmas.c` / `xmas.h`.

use proptest::prelude::*;
use rustyrian_core::xmas::{self, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

mod net_support;
use net_support::Rig;
mod menus_support;
use menus_support::{Act, any_act, output_rect, run_capped, script};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State { xmas: true };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// `xmas_time` on the fake clock (a September date).
#[test]
fn xmas_time_matches() {
    let mut d = Diff::new();
    // SAFETY: plain C call.
    let c = unsafe { b::xmas_time() };
    assert_eq!(c, xmas::xmas_time(&mut d.game));
    assert!(!c);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Runs `xmas_prompt` on both engines with `acts`; compares the answer,
/// every call and the whole state.
fn run_prompt(acts: &[(Act, u32)], shop_loaded: bool) -> bool {
    let mut rig = Rig::new(&script(output_rect(), 300, acts));
    rig.video();
    if shop_loaded {
        rustyrian_core::sprite::je_load_comp_shapes(
            &mut rig.g,
            rustyrian_core::varz::Sprite2Ref::ShopSpriteSheet,
            b'1',
        )
        .expect("shop sprites");
        rig.store();
    }
    rig.clear();
    // SAFETY: plain C call inside the guard.
    let r = rig.run(|| unsafe { b::xmas_prompt() }, xmas::xmas_prompt);
    rig.check();
    r.expect("no exit")
}

/// `xmas_prompt`: snow animation, keyboard and mouse choice, answering or
/// cancelling. Each case runs the whole prompt on both engines (about 0.4 s),
/// so the case count is capped at 48 whatever `PROPTEST_CASES` says.
#[test]
fn xmas_prompt_matches() {
    let acts = prop::collection::vec(
        (
            any_act(
                vec![(140, 126), (180, 126)],
                vec![
                    Act::Key("LEFT"),
                    Act::Key("RIGHT"),
                    Act::Key("SPACE"),
                    Act::Key("RETURN"),
                    Act::Key("ESCAPE"),
                    Act::Key("A"),
                ],
            ),
            20u32..400,
        ),
        0..8,
    );
    run_capped(48, (acts, any::<bool>()), |(acts, shop_loaded)| {
        run_prompt(&acts, shop_loaded);
        Ok(())
    });
}

/// Both answers by keyboard and by mouse.
#[test]
fn xmas_prompt_answers_match() {
    assert!(run_prompt(&[(Act::Key("RETURN"), 100)], false));
    assert!(!run_prompt(
        &[(Act::Key("RIGHT"), 100), (Act::Key("SPACE"), 100)],
        false
    ));
    assert!(!run_prompt(
        &[(Act::Key("LEFT"), 100), (Act::Key("RETURN"), 100)],
        false
    ));
    assert!(run_prompt(
        &[(Act::Move(140, 126), 100), (Act::Click(1), 100)],
        false
    ));
    assert!(!run_prompt(
        &[(Act::Move(180, 126), 100), (Act::Click(1), 100)],
        false
    ));
    assert!(!run_prompt(&[(Act::Click(3), 100)], false));
    assert!(run_prompt(
        &[
            (Act::Key("RIGHT"), 100),
            (Act::Key("LEFT"), 100),
            (Act::Key("RETURN"), 100)
        ],
        false
    ));
}
