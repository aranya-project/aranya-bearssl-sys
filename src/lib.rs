#![allow(
    clippy::missing_safety_doc,
    clippy::redundant_static_lifetimes,
    clippy::too_many_arguments,
    clippy::unreadable_literal,
    clippy::upper_case_acronyms,
    improper_ctypes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_imports
)]
#![no_std]

use core::{
    convert::TryInto,
    ffi::{c_char, c_int, c_uint, c_ulong, c_void},
};

#[allow(clippy::useless_transmute, clippy::derive_partial_eq_without_eq)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub use generated::*;

/// A wrapper for the `br_sha256_update` macro.
///
/// # Safety
///
/// See the `br_xxx_update` docs.
pub unsafe fn br_sha256_update(ctx: *mut br_sha256_context, data: *const c_void, len: usize) {
    br_sha224_update(ctx, data, len)
}

/// A wrapper for the `br_sha512_update` macro.
///
/// # Safety
///
/// See the `br_xxx_update` docs.
pub unsafe fn br_sha512_update(ctx: *mut br_sha512_context, data: *const c_void, len: usize) {
    br_sha384_update(ctx, data, len)
}
