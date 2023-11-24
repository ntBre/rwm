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
        arrange: |mon| todo!(),
    },
    // floating
    Layout {
        symbol: "><>",
        arrange: |_| {},
    },
    // monocle
    Layout {
        symbol: "[M]",
        arrange: |mon| todo!(),
    },
];
