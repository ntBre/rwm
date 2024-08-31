use std::ffi::{c_char, c_int, c_uint, c_void};

use x11::xlib::KeySym;

use enums::Clk;

pub mod enums;

pub type Window = u64;

#[repr(C)]
#[derive(Copy, Clone)]
pub union Arg {
    pub i: c_int,
    pub ui: c_uint,
    pub f: f32,
    pub v: *const c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Button {
    pub click: c_uint,
    pub mask: c_uint,
    pub button: c_uint,
    pub func: Option<unsafe extern "C" fn(arg: *const Arg)>,
    pub arg: Arg,
}

impl Button {
    pub const fn new(
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

pub struct Cursor {
    pub cursor: x11::xlib::Cursor,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Key {
    pub mod_: c_uint,
    pub keysym: KeySym,
    pub func: Option<unsafe extern "C" fn(arg1: *const Arg)>,
    pub arg: Arg,
}

impl Key {
    pub const fn new(
        mod_: c_uint,
        keysym: u32,
        func: unsafe extern "C" fn(arg1: *const Arg),
        arg: Arg,
    ) -> Self {
        Self { mod_, keysym: keysym as KeySym, func: Some(func), arg }
    }
}

unsafe impl Sync for Key {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Rule {
    pub class: *const c_char,
    pub instance: *const c_char,
    pub title: *const c_char,
    pub tags: c_uint,
    pub isfloating: bool,
    pub isterminal: bool,
    pub noswallow: bool,
    pub monitor: c_int,
}

pub struct Systray {
    pub win: Window,
    pub icons: *mut Client,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Layout {
    pub symbol: *const c_char,
    pub arrange: Option<unsafe extern "C" fn(arg1: *mut Monitor)>,
}

pub struct Pertag {
    /// Current tag
    pub curtag: c_uint,
    /// Previous tag
    pub prevtag: c_uint,
    /// Number of windows in master area
    pub nmasters: Vec<c_int>,
    /// Proportion of monitor for master area
    pub mfacts: Vec<f32>,
    /// Selected layouts
    pub sellts: Vec<c_uint>,
    /// Matrix of tag and layout indices
    pub ltidxs: Vec<[*const Layout; 2]>,
    /// Whether to display the bar
    pub showbars: Vec<c_int>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Monitor {
    pub ltsymbol: [c_char; 16usize],
    pub mfact: f32,
    pub nmaster: c_int,
    pub num: c_int,
    pub by: c_int,
    pub mx: c_int,
    pub my: c_int,
    pub mw: c_int,
    pub mh: c_int,
    pub wx: c_int,
    pub wy: c_int,
    pub ww: c_int,
    pub wh: c_int,
    pub seltags: c_uint,
    pub sellt: c_uint,
    pub tagset: [c_uint; 2usize],
    pub showbar: c_int,
    pub topbar: c_int,
    pub clients: *mut Client,
    pub sel: *mut Client,
    pub stack: *mut Client,
    pub next: *mut Monitor,
    pub barwin: Window,
    pub lt: [*const Layout; 2usize],
    pub pertag: *mut Pertag,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Client {
    pub name: [c_char; 256usize],
    pub mina: f32,
    pub maxa: f32,
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
    pub oldx: c_int,
    pub oldy: c_int,
    pub oldw: c_int,
    pub oldh: c_int,
    pub basew: c_int,
    pub baseh: c_int,
    pub incw: c_int,
    pub inch: c_int,
    pub maxw: c_int,
    pub maxh: c_int,
    pub minw: c_int,
    pub minh: c_int,
    pub hintsvalid: c_int,
    pub bw: c_int,
    pub oldbw: c_int,
    pub tags: c_uint,
    pub isfixed: c_int,
    pub isfloating: bool,
    pub isurgent: c_int,
    pub neverfocus: c_int,
    pub oldstate: bool,
    pub isfullscreen: bool,
    pub isterminal: bool,
    pub noswallow: bool,
    pub pid: libc::pid_t,
    pub next: *mut Client,
    pub snext: *mut Client,
    pub swallowing: *mut Client,
    pub mon: *mut Monitor,
    pub win: Window,
}
