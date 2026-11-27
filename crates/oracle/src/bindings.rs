//! Raw bindgen output for the upstream headers, generated thunks/getters and
//! the shim signature typedefs.
#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unnecessary_transmutes,
    clippy::all,
    clippy::undocumented_unsafe_blocks
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
