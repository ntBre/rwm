use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::null,
    sync::LazyLock,
};

use x11::xlib::{Button1, Button2, Button3};

use crate::{bindgen::Layout, enums::Clk};
use crate::{
    bindgen::{
        monocle, movemouse, resizemouse, setlayout, spawn, tag, termcmd, tile,
        togglefloating, toggletag, toggleview, view, zoom, Arg, Button, Rule,
        MODKEY,
    },
    enums::Scheme,
};

// appearance

/// Border pixel of windows
pub const BORDERPX: c_uint = 3;
// Snap pixel
// pub const SNAP: c_uint = 32;
/// 0 means no bar
pub const SHOWBAR: c_int = 1;
/// 0 means bottom bar
pub const TOPBAR: c_int = 1;
pub const FONTS: [&CStr; 1] = [c"monospace:size=12"];
// const DMENUFONT: &str = "monospace:size=12";
const COL_GRAY1: &CStr = c"#222222";
const COL_GRAY2: &CStr = c"#444444";
const COL_GRAY3: &CStr = c"#bbbbbb";
const COL_GRAY4: &CStr = c"#eeeeee";
const COL_CYAN: &CStr = c"#005577";

pub static COLORS: LazyLock<[[&CStr; 3]; 2]> = LazyLock::new(|| {
    let mut ret = [[c""; 3]; 2];
    ret[Scheme::Norm as usize] = [COL_GRAY3, COL_GRAY1, COL_GRAY2];
    ret[Scheme::Sel as usize] = [COL_GRAY4, COL_CYAN, COL_CYAN];
    ret
});

// tagging
pub const TAGS: [&CStr; 9] =
    [c"1", c"2", c"3", c"4", c"5", c"6", c"7", c"8", c"9"];

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

// layouts

/// Factor of master area size [0.05..0.95]
pub const MFACT: c_float = 0.5;
/// Number of clients in master area
pub const NMASTER: c_int = 1;
/// 1 means respect size hints in tiled resizals
pub const RESIZE_HINTS: c_int = 0;
/// 1 will force focus on the fullscreen window
// pub const LOCK_FULLSCREEN: c_int = 1;

pub const LAYOUTS: [Layout; 3] = [
    Layout { symbol: c"[]=".as_ptr(), arrange: Some(tile) },
    Layout { symbol: c"><>".as_ptr(), arrange: None },
    Layout { symbol: c"[M]".as_ptr(), arrange: Some(monocle) },
];

// key definitions

// commands

// button definitions

impl Button {
    const fn new(
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

pub static BUTTONS: [Button; 11] = [
    Button::new(Clk::LtSymbol, 0, Button1, setlayout, Arg { i: 0 }),
    Button::new(
        Clk::LtSymbol,
        0,
        Button3,
        setlayout,
        Arg { v: &LAYOUTS[2] as *const _ as *const _ },
    ),
    Button::new(Clk::WinTitle, 0, Button2, zoom, Arg { i: 0 }),
    Button::new(
        Clk::StatusText,
        0,
        Button2,
        spawn,
        Arg { v: unsafe { termcmd.as_ptr().cast() } },
    ),
    Button::new(Clk::ClientWin, MODKEY, Button1, movemouse, Arg { i: 0 }),
    Button::new(
        Clk::ClientWin,
        MODKEY,
        Button2,
        togglefloating,
        Arg { i: 0 },
    ),
    Button::new(Clk::ClientWin, MODKEY, Button3, resizemouse, Arg { i: 0 }),
    Button::new(Clk::TagBar, 0, Button1, view, Arg { i: 0 }),
    Button::new(Clk::TagBar, 0, Button3, toggleview, Arg { i: 0 }),
    Button::new(Clk::TagBar, MODKEY, Button1, tag, Arg { i: 0 }),
    Button::new(Clk::TagBar, MODKEY, Button3, toggletag, Arg { i: 0 }),
];
