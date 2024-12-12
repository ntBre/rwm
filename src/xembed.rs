use std::ffi::c_int;

pub const SYSTEM_TRAY_REQUEST_DOCK: c_int = 0;

/* XEMBED messages */
pub const XEMBED_EMBEDDED_NOTIFY: c_int = 0;
pub const XEMBED_FOCUS_IN: c_int = 4;
pub const XEMBED_MODALITY_ON: c_int = 10;

pub const XEMBED_MAPPED: u64 = 1 << 0;
pub const XEMBED_WINDOW_ACTIVATE: c_int = 1;
pub const XEMBED_WINDOW_DEACTIVATE: c_int = 2;

pub const VERSION_MAJOR: c_int = 0;
pub const VERSION_MINOR: c_int = 0;
pub const XEMBED_EMBEDDED_VERSION: c_int =
    (VERSION_MAJOR << 16) | VERSION_MINOR;
