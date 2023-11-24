//! I'll probably read these from a config file at some point, but for now
//! include them in a config source file like in C

use crate::Layout;

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
