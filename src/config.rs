use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::null,
};

use x11::xlib::{Button1, Button2, Button3};

use crate::bindgen;
use crate::{
    bindgen::{Rule, MODKEY},
    enums::Clk,
};

/// Border pixel of windows
pub const BORDERPX: c_uint = 3;
// Snap pixel
// pub const SNAP: c_uint = 32;
/// 0 means no bar
pub const SHOWBAR: c_int = 1;
/// 0 means bottom bar
pub const TOPBAR: c_int = 1;

// layouts

/// Factor of master area size [0.05..0.95]
pub const MFACT: c_float = 0.5;
/// Number of clients in master area
pub const NMASTER: c_int = 1;

impl Rule {
    const fn new(
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

pub const RULES: [Rule; 2] = [
    Rule::new(c"Gimp", null(), null(), 0, 1, -1),
    Rule::new(c"Firefox", null(), null(), 1 << 8, 0, -1),
];

impl Button {
    const fn new(
        click: Clk,
        mask: c_uint,
        button: c_uint,
        func: unsafe extern "C" fn(*const Arg),
        arg: Arg,
    ) -> Self {
        Self { click: click as c_uint, mask, button, func: Some(func), arg }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union Arg {
    pub i: ::std::os::raw::c_int,
    pub ui: ::std::os::raw::c_uint,
    pub f: f32,
    pub v: *const ::std::os::raw::c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Button {
    pub click: ::std::os::raw::c_uint,
    pub mask: ::std::os::raw::c_uint,
    pub button: ::std::os::raw::c_uint,
    pub func: ::std::option::Option<unsafe extern "C" fn(arg: *const Arg)>,
    pub arg: Arg,
}

pub static BUTTONS: [Button; 11] = [
    Button::new(
        Clk::LtSymbol,
        0,
        Button1,
        bindgen::setlayout,
        Arg::default(),
    ),
    Button::new(
        Clk::LtSymbol,
        0,
        Button3,
        bindgen::setlayout,
        // supposed to be a *const c_void, cast & to *const Layout then void
        Arg { v: &bindgen::layouts[2] as *const _ as *const _ },
    ),
    Button::new(Clk::WinTitle, 0, Button2, bindgen::zoom, Arg::default()),
    Button::new(
        Clk::StatusText,
        0,
        Button2,
        bindgen::spawn,
        Arg { v: bindgen::termcmd.as_ptr().cast() },
    ),
    Button::new(
        Clk::ClientWin,
        MODKEY,
        Button1,
        bindgen::movemouse,
        Arg::default(),
    ),
    Button::new(
        Clk::ClientWin,
        MODKEY,
        Button2,
        bindgen::togglefloating,
        Arg::default(),
    ),
    Button::new(
        Clk::ClientWin,
        MODKEY,
        Button3,
        bindgen::resizemouse,
        Arg::default(),
    ),
    Button::new(Clk::TagBar, 0, Button1, bindgen::view, Arg::default()),
    Button::new(Clk::TagBar, 0, Button3, bindgen::toggleview, Arg::default()),
    Button::new(Clk::TagBar, MODKEY, Button1, bindgen::tag, Arg::default()),
    Button::new(
        Clk::TagBar,
        MODKEY,
        Button3,
        bindgen::toggletag,
        Arg::default(),
    ),
];
