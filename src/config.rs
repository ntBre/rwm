use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::{null, null_mut},
    sync::LazyLock,
};

use libc::c_char;
use x11::xlib::{Button1, Button2, Button3, ControlMask, Mod4Mask, ShiftMask};

use crate::{
    enums::{Clk, Scheme},
    key_handlers::*,
    layouts::{monocle, tile},
    Layout,
};
use rwm::{Arg, Button, Key};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Rule {
    pub class: *const ::std::os::raw::c_char,
    pub instance: *const ::std::os::raw::c_char,
    pub title: *const ::std::os::raw::c_char,
    pub tags: ::std::os::raw::c_uint,
    pub isfloating: ::std::os::raw::c_int,
    pub monitor: ::std::os::raw::c_int,
}

// appearance

/// Border pixel of windows
pub const BORDERPX: c_uint = 3;
// Snap pixel
pub const SNAP: c_uint = 32;
/// 0 means no bar
pub const SHOWBAR: c_int = 1;
/// 0 means bottom bar
pub const TOPBAR: c_int = 1;
pub const FONTS: [&CStr; 1] = [c"monospace:size=12"];
const DMENUFONT: &CStr = c"monospace:size=12";
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
pub const LOCK_FULLSCREEN: c_int = 1;

pub const LAYOUTS: [Layout; 3] = [
    Layout { symbol: c"[]=".as_ptr(), arrange: Some(tile) },
    Layout { symbol: c"><>".as_ptr(), arrange: None },
    Layout { symbol: c"[M]".as_ptr(), arrange: Some(monocle) },
];

// key definitions
pub const MODKEY: c_uint = Mod4Mask;

/// This type is needed just to implement Sync for this raw pointer
pub struct DmenuCmd(pub [*const c_char; 14]);

unsafe impl Sync for DmenuCmd {}

pub static mut DMENUMON: [c_char; 2] = ['0' as c_char, '\0' as c_char];

// commands
pub static DMENUCMD: DmenuCmd = DmenuCmd([
    c"dmenu_run".as_ptr(),
    c"-m".as_ptr(),
    unsafe { DMENUMON.as_ptr() },
    c"-fn".as_ptr(),
    DMENUFONT.as_ptr(),
    c"-nb".as_ptr(),
    COL_GRAY1.as_ptr(),
    c"-nf".as_ptr(),
    COL_GRAY3.as_ptr(),
    c"-sb".as_ptr(),
    COL_CYAN.as_ptr(),
    c"-sf".as_ptr(),
    COL_GRAY4.as_ptr(),
    null_mut(),
]);
pub const TERMCMD: [*const c_char; 2] = [c"st".as_ptr(), null_mut()];

use x11::keysym::{
    XK_Return, XK_Tab, XK_b, XK_c, XK_comma, XK_d, XK_f, XK_h, XK_i, XK_j,
    XK_k, XK_l, XK_m, XK_period, XK_q, XK_space, XK_t, XK_u, XK_0, XK_1, XK_2,
    XK_3, XK_4, XK_5, XK_6, XK_7, XK_8, XK_9,
};

pub static KEYS: [Key; 60] = [
    Key::new(MODKEY, XK_d, spawn, Arg { v: DMENUCMD.0.as_ptr().cast() }),
    Key::new(MODKEY, XK_t, spawn, Arg { v: TERMCMD.as_ptr().cast() }),
    Key::new(MODKEY, XK_b, togglebar, Arg { i: 0 }),
    Key::new(MODKEY, XK_j, focusstack, Arg { i: 1 }),
    Key::new(MODKEY, XK_k, focusstack, Arg { i: -1 }),
    Key::new(MODKEY, XK_i, incnmaster, Arg { i: 1 }),
    Key::new(MODKEY, XK_u, incnmaster, Arg { i: -1 }),
    Key::new(MODKEY, XK_h, setmfact, Arg { f: -0.05 }),
    Key::new(MODKEY, XK_l, setmfact, Arg { f: 0.05 }),
    Key::new(MODKEY, XK_Return, zoom, Arg { i: 0 }),
    Key::new(MODKEY, XK_Tab, view, Arg { i: 0 }),
    Key::new(MODKEY | ShiftMask, XK_c, killclient, Arg { i: 0 }),
    Key::new(
        MODKEY | ShiftMask,
        XK_t,
        setlayout,
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
    Key::new(MODKEY, XK_comma, focusmon, Arg { i: -1 }),
    Key::new(MODKEY, XK_period, focusmon, Arg { i: 1 }),
    Key::new(MODKEY | ShiftMask, XK_comma, tagmon, Arg { i: -1 }),
    Key::new(MODKEY | ShiftMask, XK_period, tagmon, Arg { i: 1 }),
    Key::new(MODKEY, XK_1, view, Arg { ui: 1 << 0 }),
    Key::new(MODKEY | ControlMask, XK_1, toggleview, Arg { ui: 1 << 0 }),
    Key::new(MODKEY | ShiftMask, XK_1, tag, Arg { ui: 1 << 0 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_1,
        toggletag,
        Arg { ui: 1 << 0 },
    ),
    Key::new(MODKEY, XK_2, view, Arg { ui: 1 << 1 }),
    Key::new(MODKEY | ControlMask, XK_2, toggleview, Arg { ui: 1 << 1 }),
    Key::new(MODKEY | ShiftMask, XK_2, tag, Arg { ui: 1 << 1 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_2,
        toggletag,
        Arg { ui: 1 << 1 },
    ),
    Key::new(MODKEY, XK_3, view, Arg { ui: 1 << 2 }),
    Key::new(MODKEY | ControlMask, XK_3, toggleview, Arg { ui: 1 << 2 }),
    Key::new(MODKEY | ShiftMask, XK_3, tag, Arg { ui: 1 << 2 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_3,
        toggletag,
        Arg { ui: 1 << 2 },
    ),
    Key::new(MODKEY, XK_4, view, Arg { ui: 1 << 3 }),
    Key::new(MODKEY | ControlMask, XK_4, toggleview, Arg { ui: 1 << 3 }),
    Key::new(MODKEY | ShiftMask, XK_4, tag, Arg { ui: 1 << 3 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_4,
        toggletag,
        Arg { ui: 1 << 3 },
    ),
    Key::new(MODKEY, XK_5, view, Arg { ui: 1 << 4 }),
    Key::new(MODKEY | ControlMask, XK_5, toggleview, Arg { ui: 1 << 4 }),
    Key::new(MODKEY | ShiftMask, XK_5, tag, Arg { ui: 1 << 4 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_5,
        toggletag,
        Arg { ui: 1 << 4 },
    ),
    Key::new(MODKEY, XK_6, view, Arg { ui: 1 << 5 }),
    Key::new(MODKEY | ControlMask, XK_6, toggleview, Arg { ui: 1 << 5 }),
    Key::new(MODKEY | ShiftMask, XK_6, tag, Arg { ui: 1 << 5 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_6,
        toggletag,
        Arg { ui: 1 << 5 },
    ),
    Key::new(MODKEY, XK_7, view, Arg { ui: 1 << 6 }),
    Key::new(MODKEY | ControlMask, XK_7, toggleview, Arg { ui: 1 << 6 }),
    Key::new(MODKEY | ShiftMask, XK_7, tag, Arg { ui: 1 << 6 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_7,
        toggletag,
        Arg { ui: 1 << 6 },
    ),
    Key::new(MODKEY, XK_8, view, Arg { ui: 1 << 7 }),
    Key::new(MODKEY | ControlMask, XK_8, toggleview, Arg { ui: 1 << 7 }),
    Key::new(MODKEY | ShiftMask, XK_8, tag, Arg { ui: 1 << 7 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_8,
        toggletag,
        Arg { ui: 1 << 7 },
    ),
    Key::new(MODKEY, XK_9, view, Arg { ui: 1 << 8 }),
    Key::new(MODKEY | ControlMask, XK_9, toggleview, Arg { ui: 1 << 8 }),
    Key::new(MODKEY | ShiftMask, XK_9, tag, Arg { ui: 1 << 8 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_9,
        toggletag,
        Arg { ui: 1 << 8 },
    ),
    Key::new(MODKEY | ShiftMask, XK_q, quit, Arg { i: 0 }),
];

// button definitions

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
        Arg { v: TERMCMD.as_ptr().cast() },
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
