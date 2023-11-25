//! I'll probably read these from a config file at some point, but for now
//! include them in a config source file like in C

use x11::xlib::{Button1, Button2, Button3, Mod1Mask};

use crate::{
    movemouse, resizemouse, setlayout, spawn, tag, togglefloating, toggletag,
    toggleview, view, zoom, Arg, Button, Clk, Layout,
};

pub const FONTS: [&str; 1] = ["monospace:size=10"];
pub const MFACT: f64 = 0.5;
pub const NMASTER: i32 = 1;
pub const SHOWBAR: bool = true;
pub const TOPBAR: bool = true;
pub const LAYOUTS: [Layout; 3] = [
    // tile
    Layout {
        symbol: "[]=",
        arrange: |_mon| todo!(),
    },
    // floating
    Layout {
        symbol: "><>",
        arrange: |_| {},
    },
    // monocle
    Layout {
        symbol: "[M]",
        arrange: |_mon| todo!(),
    },
];

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

pub const MODKEY: u32 = Mod1Mask;

pub const TERMCMD: &str = "st";

#[rustfmt::skip]
pub const BUTTONS: [Button; 11] = [
    Button { click: Clk::LtSymbol,   mask:      0, button: Button1, func: setlayout,      arg: Arg::Uint(0)             },
    Button { click: Clk::LtSymbol,   mask:      0, button: Button3, func: setlayout,      arg: Arg::Layout(&LAYOUTS[2]) },
    Button { click: Clk::WinTitle,   mask:      0, button: Button2, func: zoom,           arg: Arg::Uint(0)             },
    Button { click: Clk::StatusText, mask:      0, button: Button2, func: spawn,          arg: Arg::Str(TERMCMD)        },
    Button { click: Clk::ClientWin,  mask: MODKEY, button: Button1, func: movemouse,      arg: Arg::Uint(0)             },
    Button { click: Clk::ClientWin,  mask: MODKEY, button: Button2, func: togglefloating, arg: Arg::Uint(0)             },
    Button { click: Clk::ClientWin,  mask: MODKEY, button: Button3, func: resizemouse,    arg: Arg::Uint(0)             },
    Button { click: Clk::TagBar,     mask:      0, button: Button1, func: view,           arg: Arg::Uint(0)             },
    Button { click: Clk::TagBar,     mask:      0, button: Button3, func: toggleview,     arg: Arg::Uint(0)             },
    Button { click: Clk::TagBar,     mask: MODKEY, button: Button1, func: tag,            arg: Arg::Uint(0)             },
    Button { click: Clk::TagBar,     mask: MODKEY, button: Button3, func: toggletag,      arg: Arg::Uint(0)             },
];
