use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

use x11::xlib::KeySym;

use enums::Clk;

pub mod enums;

pub type Window = u64;

#[repr(C)]
#[derive(Copy, Clone)]
pub union Arg {
    pub i: c_int,
    pub ui: c_uint,
    pub f: f32,
    pub v: *const c_void,
}

unsafe impl Send for Arg {}

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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Rule {
    pub class: *const c_char,
    pub instance: *const c_char,
    pub title: *const c_char,
    pub tags: c_uint,
    pub isfloating: c_int,
    pub monitor: c_int,
}

impl Rule {
    pub const fn new(
        class: &'static CStr,
        instance: *const i8,
        title: *const i8,
        tags: c_uint,
        isfloating: c_int,
        monitor: c_int,
    ) -> Self {
        Self {
            class: class.as_ptr(),
            instance,
            title,
            tags,
            isfloating,
            monitor,
        }
    }
}
