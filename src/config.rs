//! I'll probably read these from a config file at some point, but for now
//! include them in a config source file like in C

use x11::{
    keysym::{
        XK_Return, XK_Tab, XK_b, XK_c, XK_comma, XK_d, XK_f, XK_h, XK_i, XK_j,
        XK_k, XK_l, XK_m, XK_p, XK_period, XK_q, XK_space, XK_t, XK_0, XK_1,
        XK_2, XK_3, XK_4, XK_5, XK_6, XK_7, XK_8, XK_9,
    },
    xlib::{Button1, Button2, Button3, ControlMask, Mod4Mask, ShiftMask},
};

use crate::{
    focusmon, focusstack, incnmaster, killclient,
    layouts::{monocle, tile},
    movemouse, quit, resizemouse, setlayout, setmfact, spawn, tag, tagmon,
    togglebar, togglefloating, toggletag, toggleview, view, zoom, Arg, Button,
    Clk, Key, Layout, Rule,
};

/// border pixel of windows
pub const BORDERPX: i32 = 1;
/// snap pixel
pub const SNAP: i32 = 32;
pub const SHOWBAR: bool = true;
pub const TOPBAR: bool = true;
pub const FONTS: [&str; 1] = ["monospace:size=10"];
const COL_GRAY1: &str = "#222222";
const COL_GRAY2: &str = "#444444";
const COL_GRAY3: &str = "#bbbbbb";
const COL_GRAY4: &str = "#eeeeee";
const COL_CYAN: &str = "#005577";
pub const COLORS: [[&str; 3]; 2] = [
    [COL_GRAY3, COL_GRAY1, COL_GRAY2], // SchemeNorm
    [COL_GRAY4, COL_CYAN, COL_CYAN],   // SchemeSel
];

pub const TAGS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// handling rules for specific programs. use xprop(1) to obtain class,
/// instance, and title information:
///   WM_CLASS(STRING) = instance, class
///   WM_NAME(STRING) = title
pub const RULES: [Rule; 2] = [
    // class, instance, title, tags mask, isfloating, monitor
    Rule::new(Some("Gimp"), None, None, 0, true, -1),
    Rule::new(Some("Firefox"), None, None, 1 << 8, false, -1),
];

pub const MFACT: f64 = 0.5;
pub const NMASTER: i32 = 1;
pub const RESIZEHINTS: bool = true;
/// force focus on the fullscreen window
pub const LOCKFULLSCREEN: bool = true;

pub const LAYOUTS: [Layout; 3] = [
    // tile
    Layout {
        symbol: "[]=",
        arrange: Some(tile),
    },
    // floating
    Layout {
        symbol: "><>",
        arrange: None,
    },
    // monocle
    Layout {
        symbol: "[M]",
        arrange: Some(monocle),
    },
];

pub const MODKEY: u32 = Mod4Mask;

pub const TERMCMD: &str = "st";
pub const DMENUFONT: &str = "monospace:size=10";

pub static DMENUCMD: &[&str] = &[
    "dmenu_run",
    "-m",
    "0",
    "-fn",
    DMENUFONT,
    "-nb",
    COL_GRAY1,
    "-nf",
    COL_GRAY3,
    "-sb",
    COL_CYAN,
    "-sf",
    COL_GRAY4,
];

pub static mut DMENUMON: &&str = &DMENUCMD[2];

use ControlMask as Ctrl;
use ShiftMask as Shift;

pub static KEYS: [Key; 60] = [
    Key::new(MODKEY, XK_p, spawn, Arg::Str(DMENUCMD)),
    Key::new(MODKEY | Shift, XK_Return, spawn, Arg::Str(&[TERMCMD])),
    Key::new(MODKEY, XK_b, togglebar, Arg::Uint(0)),
    Key::new(MODKEY, XK_j, focusstack, Arg::Int(1)),
    Key::new(MODKEY, XK_k, focusstack, Arg::Int(-1)),
    Key::new(MODKEY, XK_i, incnmaster, Arg::Int(1)),
    Key::new(MODKEY, XK_d, incnmaster, Arg::Int(-1)),
    Key::new(MODKEY, XK_h, setmfact, Arg::Float(-0.05)),
    Key::new(MODKEY, XK_l, setmfact, Arg::Float(0.05)),
    Key::new(MODKEY, XK_Return, zoom, Arg::Uint(0)),
    Key::new(MODKEY, XK_Tab, view, Arg::Uint(0)),
    Key::new(MODKEY | Shift, XK_c, killclient, Arg::Uint(0)),
    Key::new(MODKEY, XK_t, setlayout, Arg::Layout(&LAYOUTS[0])),
    Key::new(MODKEY, XK_f, setlayout, Arg::Layout(&LAYOUTS[1])),
    Key::new(MODKEY, XK_m, setlayout, Arg::Layout(&LAYOUTS[2])),
    Key::new(MODKEY, XK_space, setlayout, Arg::Uint(0)),
    Key::new(MODKEY | Shift, XK_space, togglefloating, Arg::Uint(0)),
    Key::new(MODKEY, XK_0, view, Arg::Uint(!0)),
    Key::new(MODKEY | Shift, XK_0, tag, Arg::Uint(!0)),
    Key::new(MODKEY, XK_comma, focusmon, Arg::Int(-1)),
    Key::new(MODKEY, XK_period, focusmon, Arg::Int(1)),
    Key::new(MODKEY | Shift, XK_comma, tagmon, Arg::Int(-1)),
    Key::new(MODKEY | Shift, XK_period, tagmon, Arg::Int(1)),
    // start TAGKEYS
    Key::new(MODKEY, XK_1, view, Arg::Uint(1 << 0)),
    Key::new(MODKEY | Ctrl, XK_1, toggleview, Arg::Uint(1 << 0)),
    Key::new(MODKEY | Shift, XK_1, tag, Arg::Uint(1 << 0)),
    Key::new(MODKEY | Ctrl | Shift, XK_1, toggletag, Arg::Uint(1 << 0)),
    Key::new(MODKEY, XK_2, view, Arg::Uint(1 << 1)),
    Key::new(MODKEY | Ctrl, XK_2, toggleview, Arg::Uint(1 << 1)),
    Key::new(MODKEY | Shift, XK_2, tag, Arg::Uint(1 << 1)),
    Key::new(MODKEY | Ctrl | Shift, XK_2, toggletag, Arg::Uint(1 << 1)),
    Key::new(MODKEY, XK_3, view, Arg::Uint(1 << 2)),
    Key::new(MODKEY | Ctrl, XK_3, toggleview, Arg::Uint(1 << 2)),
    Key::new(MODKEY | Shift, XK_3, tag, Arg::Uint(1 << 2)),
    Key::new(MODKEY | Ctrl | Shift, XK_3, toggletag, Arg::Uint(1 << 2)),
    Key::new(MODKEY, XK_4, view, Arg::Uint(1 << 3)),
    Key::new(MODKEY | Ctrl, XK_4, toggleview, Arg::Uint(1 << 3)),
    Key::new(MODKEY | Shift, XK_4, tag, Arg::Uint(1 << 3)),
    Key::new(MODKEY | Ctrl | Shift, XK_4, toggletag, Arg::Uint(1 << 3)),
    Key::new(MODKEY, XK_5, view, Arg::Uint(1 << 4)),
    Key::new(MODKEY | Ctrl, XK_5, toggleview, Arg::Uint(1 << 4)),
    Key::new(MODKEY | Shift, XK_5, tag, Arg::Uint(1 << 4)),
    Key::new(MODKEY | Ctrl | Shift, XK_5, toggletag, Arg::Uint(1 << 4)),
    Key::new(MODKEY, XK_6, view, Arg::Uint(1 << 5)),
    Key::new(MODKEY | Ctrl, XK_6, toggleview, Arg::Uint(1 << 5)),
    Key::new(MODKEY | Shift, XK_6, tag, Arg::Uint(1 << 5)),
    Key::new(MODKEY | Ctrl | Shift, XK_6, toggletag, Arg::Uint(1 << 5)),
    Key::new(MODKEY, XK_7, view, Arg::Uint(1 << 6)),
    Key::new(MODKEY | Ctrl, XK_7, toggleview, Arg::Uint(1 << 6)),
    Key::new(MODKEY | Shift, XK_7, tag, Arg::Uint(1 << 6)),
    Key::new(MODKEY | Ctrl | Shift, XK_7, toggletag, Arg::Uint(1 << 6)),
    Key::new(MODKEY, XK_8, view, Arg::Uint(1 << 7)),
    Key::new(MODKEY | Ctrl, XK_8, toggleview, Arg::Uint(1 << 7)),
    Key::new(MODKEY | Shift, XK_8, tag, Arg::Uint(1 << 7)),
    Key::new(MODKEY | Ctrl | Shift, XK_8, toggletag, Arg::Uint(1 << 7)),
    Key::new(MODKEY, XK_9, view, Arg::Uint(1 << 8)),
    Key::new(MODKEY | Ctrl, XK_9, toggleview, Arg::Uint(1 << 8)),
    Key::new(MODKEY | Shift, XK_9, tag, Arg::Uint(1 << 8)),
    Key::new(MODKEY | Ctrl | Shift, XK_9, toggletag, Arg::Uint(1 << 8)),
    // end TAGKEYS
    Key::new(MODKEY | Shift, XK_q, quit, Arg::Uint(0)),
];

use Clk as C;
pub const BUTTONS: [Button; 11] = [
    // click, mask, button, func, arg
    Button::new(C::LtSymbol, 0, Button1, setlayout, Arg::Uint(0)),
    Button::new(C::LtSymbol, 0, Button3, setlayout, Arg::Layout(&LAYOUTS[2])),
    Button::new(C::WinTitle, 0, Button2, zoom, Arg::Uint(0)),
    Button::new(C::StatusText, 0, Button2, spawn, Arg::Str(&[TERMCMD])),
    Button::new(C::ClientWin, MODKEY, Button1, movemouse, Arg::Uint(0)),
    Button::new(C::ClientWin, MODKEY, Button2, togglefloating, Arg::Uint(0)),
    Button::new(C::ClientWin, MODKEY, Button3, resizemouse, Arg::Uint(0)),
    Button::new(C::TagBar, 0, Button1, view, Arg::Uint(0)),
    Button::new(C::TagBar, 0, Button3, toggleview, Arg::Uint(0)),
    Button::new(C::TagBar, MODKEY, Button1, tag, Arg::Uint(0)),
    Button::new(C::TagBar, MODKEY, Button3, toggletag, Arg::Uint(0)),
];
