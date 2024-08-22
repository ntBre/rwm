use std::ffi::{c_int, c_uint, c_void};

use x11::xlib::KeySym;

use enums::Clk;

pub mod enums;

#[repr(C)]
#[derive(Copy, Clone)]
pub union Arg {
    pub i: c_int,
    pub ui: c_uint,
    pub f: f32,
    pub v: *const c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Button {
    pub click: c_uint,
    pub mask: c_uint,
    pub button: c_uint,
    pub func: Option<unsafe extern "C" fn(arg: *const Arg)>,
    pub arg: Arg,
}

impl Button {
    pub const fn new(
        click: Clk,
        mask: c_uint,
        button: c_uint,
        func: unsafe extern "C" fn(arg: *const Arg),
        arg: Arg,
    ) -> Self {
        Self { click: click as c_uint, mask, button, func: Some(func), arg }
    }
}

unsafe impl Sync for Button {}

pub struct Cursor {
    pub cursor: x11::xlib::Cursor,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Key {
    pub mod_: c_uint,
    pub keysym: KeySym,
    pub func: Option<unsafe extern "C" fn(arg1: *const Arg)>,
    pub arg: Arg,
}

impl Key {
    pub const fn new(
        mod_: c_uint,
        keysym: u32,
        func: unsafe extern "C" fn(arg1: *const Arg),
        arg: Arg,
    ) -> Self {
        Self { mod_, keysym: keysym as KeySym, func: Some(func), arg }
    }
}

unsafe impl Sync for Key {}
