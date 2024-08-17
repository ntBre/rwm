use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::null,
    sync::LazyLock,
};

use x11::xlib::{Button1, Button2, Button3, ControlMask, Mod4Mask, ShiftMask};

use crate::{
    bindgen::{
        self, monocle, movemouse, resizemouse, setlayout, spawn, tag, termcmd,
        tile, togglefloating, toggletag, toggleview, view, zoom, Arg, Button,
        KeySym, Rule, XK_d,
    },
    enums::Scheme,
};
use crate::{
    bindgen::{Key, Layout},
    enums::Clk,
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
// const DMENUFONT: &CStr = c"monospace:size=12";
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
pub const MODKEY: c_uint = Mod4Mask;

// commands
// pub static DMENUCMD: LazyLock<[&CStr; 13]> = LazyLock::new(|| {
//     [
//         c"dmenu_run",
//         c"-m",
//         // CStr::from_ptr is not yet stable as a const fn
//         unsafe { CStr::from_ptr(crate::bindgen::dmenumon.as_ptr()) },
//         c"-fn",
//         DMENUFONT,
//         c"-nb",
//         COL_GRAY1,
//         c"-nf",
//         COL_GRAY3,
//         c"-sb",
//         COL_CYAN,
//         c"-sf",
//         COL_GRAY4,
//     ]
// });
// pub const TERMCMD: [&CStr; 1] = [c"st"];

impl Key {
    const fn new(
        mod_: c_uint,
        keysym: u32,
        func: unsafe extern "C" fn(arg1: *const Arg),
        arg: Arg,
    ) -> Self {
        Self { mod_, keysym: keysym as KeySym, func: Some(func), arg }
    }
}

unsafe impl Sync for Key {}

use x11::keysym::{
    XK_Return, XK_Tab, XK_b, XK_c, XK_comma, XK_f, XK_h, XK_i, XK_j, XK_k,
    XK_l, XK_m, XK_period, XK_q, XK_space, XK_t, XK_u, XK_0, XK_1, XK_2, XK_3,
    XK_4, XK_5, XK_6, XK_7, XK_8, XK_9,
};

pub static KEYS: [Key; 60] = [
    Key::new(
        MODKEY,
        XK_d,
        spawn,
        Arg { v: unsafe { bindgen::dmenucmd.as_ptr().cast() } },
    ),
    Key::new(
        MODKEY,
        XK_t,
        spawn,
        Arg { v: unsafe { bindgen::termcmd.as_ptr().cast() } },
    ),
    Key::new(MODKEY, XK_b, bindgen::togglebar, Arg { i: 0 }),
    Key::new(MODKEY, XK_j, bindgen::focusstack, Arg { i: 1 }),
    Key::new(MODKEY, XK_k, bindgen::focusstack, Arg { i: -1 }),
    Key::new(MODKEY, XK_i, bindgen::incnmaster, Arg { i: 1 }),
    Key::new(MODKEY, XK_u, bindgen::incnmaster, Arg { i: -1 }),
    Key::new(MODKEY, XK_h, bindgen::setmfact, Arg { f: -0.05 }),
    Key::new(MODKEY, XK_l, bindgen::setmfact, Arg { f: 0.05 }),
    Key::new(MODKEY, XK_Return, bindgen::zoom, Arg { i: 0 }),
    Key::new(MODKEY, XK_Tab, bindgen::view, Arg { i: 0 }),
    Key::new(MODKEY | ShiftMask, XK_c, bindgen::killclient, Arg { i: 0 }),
    Key::new(
        MODKEY | ShiftMask,
        XK_t,
        bindgen::setlayout,
        Arg { v: &LAYOUTS[0] as *const _ as *const _ },
    ),
    Key::new(
        MODKEY,
        XK_f,
        setlayout,
        Arg { v: &LAYOUTS[1] as *const _ as *const _ },
    ),
    Key::new(
        MODKEY,
        XK_m,
        setlayout,
        Arg { v: &LAYOUTS[2] as *const _ as *const _ },
    ),
    Key::new(MODKEY, XK_space, setlayout, Arg { i: 0 }),
    Key::new(MODKEY | ShiftMask, XK_space, togglefloating, Arg { i: 0 }),
    Key::new(MODKEY, XK_0, view, Arg { ui: !0 }),
    Key::new(MODKEY | ShiftMask, XK_0, tag, Arg { ui: !0 }),
    Key::new(MODKEY, XK_comma, bindgen::focusmon, Arg { i: -1 }),
    Key::new(MODKEY, XK_period, bindgen::focusmon, Arg { i: 1 }),
    Key::new(MODKEY | ShiftMask, XK_comma, bindgen::tagmon, Arg { i: -1 }),
    Key::new(MODKEY | ShiftMask, XK_period, bindgen::tagmon, Arg { i: 1 }),
    Key::new(MODKEY, XK_1, bindgen::view, Arg { ui: 1 << 0 }),
    Key::new(
        MODKEY | ControlMask,
        XK_1,
        bindgen::toggleview,
        Arg { ui: 1 << 0 },
    ),
    Key::new(MODKEY | ShiftMask, XK_1, bindgen::tag, Arg { ui: 1 << 0 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_1,
        bindgen::toggletag,
        Arg { ui: 1 << 0 },
    ),
    Key::new(MODKEY, XK_2, bindgen::view, Arg { ui: 1 << 1 }),
    Key::new(
        MODKEY | ControlMask,
        XK_2,
        bindgen::toggleview,
        Arg { ui: 1 << 1 },
    ),
    Key::new(MODKEY | ShiftMask, XK_2, bindgen::tag, Arg { ui: 1 << 1 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_2,
        bindgen::toggletag,
        Arg { ui: 1 << 1 },
    ),
    Key::new(MODKEY, XK_3, bindgen::view, Arg { ui: 1 << 2 }),
    Key::new(
        MODKEY | ControlMask,
        XK_3,
        bindgen::toggleview,
        Arg { ui: 1 << 2 },
    ),
    Key::new(MODKEY | ShiftMask, XK_3, bindgen::tag, Arg { ui: 1 << 2 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_3,
        bindgen::toggletag,
        Arg { ui: 1 << 2 },
    ),
    Key::new(MODKEY, XK_4, bindgen::view, Arg { ui: 1 << 3 }),
    Key::new(
        MODKEY | ControlMask,
        XK_4,
        bindgen::toggleview,
        Arg { ui: 1 << 3 },
    ),
    Key::new(MODKEY | ShiftMask, XK_4, bindgen::tag, Arg { ui: 1 << 3 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_4,
        bindgen::toggletag,
        Arg { ui: 1 << 3 },
    ),
    Key::new(MODKEY, XK_5, bindgen::view, Arg { ui: 1 << 4 }),
    Key::new(
        MODKEY | ControlMask,
        XK_5,
        bindgen::toggleview,
        Arg { ui: 1 << 4 },
    ),
    Key::new(MODKEY | ShiftMask, XK_5, bindgen::tag, Arg { ui: 1 << 4 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_5,
        bindgen::toggletag,
        Arg { ui: 1 << 4 },
    ),
    Key::new(MODKEY, XK_6, bindgen::view, Arg { ui: 1 << 5 }),
    Key::new(
        MODKEY | ControlMask,
        XK_6,
        bindgen::toggleview,
        Arg { ui: 1 << 5 },
    ),
    Key::new(MODKEY | ShiftMask, XK_6, bindgen::tag, Arg { ui: 1 << 5 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_6,
        bindgen::toggletag,
        Arg { ui: 1 << 5 },
    ),
    Key::new(MODKEY, XK_7, bindgen::view, Arg { ui: 1 << 6 }),
    Key::new(
        MODKEY | ControlMask,
        XK_7,
        bindgen::toggleview,
        Arg { ui: 1 << 6 },
    ),
    Key::new(MODKEY | ShiftMask, XK_7, bindgen::tag, Arg { ui: 1 << 6 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_7,
        bindgen::toggletag,
        Arg { ui: 1 << 6 },
    ),
    Key::new(MODKEY, XK_8, bindgen::view, Arg { ui: 1 << 7 }),
    Key::new(
        MODKEY | ControlMask,
        XK_8,
        bindgen::toggleview,
        Arg { ui: 1 << 7 },
    ),
    Key::new(MODKEY | ShiftMask, XK_8, bindgen::tag, Arg { ui: 1 << 7 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_8,
        bindgen::toggletag,
        Arg { ui: 1 << 7 },
    ),
    Key::new(MODKEY, XK_9, bindgen::view, Arg { ui: 1 << 8 }),
    Key::new(
        MODKEY | ControlMask,
        XK_9,
        bindgen::toggleview,
        Arg { ui: 1 << 8 },
    ),
    Key::new(MODKEY | ShiftMask, XK_9, bindgen::tag, Arg { ui: 1 << 8 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_9,
        bindgen::toggletag,
        Arg { ui: 1 << 8 },
    ),
    Key::new(MODKEY | ShiftMask, XK_q, bindgen::quit, Arg { i: 0 }),
];

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
