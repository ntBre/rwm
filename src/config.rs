use std::{
    ffi::{c_float, c_int, c_uint, CStr},
    ptr::null,
};

use crate::bindgen::Rule;

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
