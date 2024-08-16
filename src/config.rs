use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::null,
};

use x11::xlib::{Button1, Button2, Button3};

use crate::bindgen::{self, Monitor};
use crate::{
    bindgen::{Rule, MODKEY},
    button_handlers::*,
    enums::Clk,
    Arg,
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

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Layout {
    pub symbol: &'static CStr,
    pub arrange:
        ::std::option::Option<unsafe extern "C" fn(arg1: *mut Monitor)>,
}

impl Layout {
    const fn new(
        symbol: &'static CStr,
        arrange: Option<unsafe extern "C" fn(arg1: *mut Monitor)>,
    ) -> Self {
        Self { symbol, arrange }
    }
}

#[derive(Clone)]
pub struct Button {
    pub click: ::std::os::raw::c_uint,
    pub mask: ::std::os::raw::c_uint,
    pub button: ::std::os::raw::c_uint,
    pub func: Option<fn(arg: &Arg)>,
    pub arg: Arg,
}

impl Button {
    const fn new(
        click: Clk,
        mask: c_uint,
        button: c_uint,
        func: fn(&Arg),
        arg: Arg,
    ) -> Self {
        Self { click: click as c_uint, mask, button, func: Some(func), arg }
    }
}

const LAYOUTS: [Layout; 3] = [
    Layout::new(c"[]=", Some(bindgen::tile)),
    Layout::new(c"><>", None),
    Layout::new(c"[M]", Some(bindgen::monocle)),
];

const TERMCMD: [&CStr; 1] = [c"st"];

pub static BUTTONS: [Button; 11] = [
    Button::new(Clk::LtSymbol, 0, Button1, setlayout, Arg::None),
    Button::new(
        Clk::LtSymbol,
        0,
        Button3,
        setlayout,
        Arg::Layout(&LAYOUTS[2]),
    ),
    Button::new(Clk::WinTitle, 0, Button2, zoom, Arg::None),
    Button::new(Clk::StatusText, 0, Button2, spawn, Arg::Str(&TERMCMD)),
    Button::new(Clk::ClientWin, MODKEY, Button1, movemouse, Arg::None),
    Button::new(Clk::ClientWin, MODKEY, Button2, togglefloating, Arg::None),
    Button::new(Clk::ClientWin, MODKEY, Button3, resizemouse, Arg::None),
    Button::new(Clk::TagBar, 0, Button1, view, Arg::None),
    Button::new(Clk::TagBar, 0, Button3, toggleview, Arg::None),
    Button::new(Clk::TagBar, MODKEY, Button1, tag, Arg::None),
    Button::new(Clk::TagBar, MODKEY, Button3, toggletag, Arg::None),
];
