use std::ffi::{c_char, c_int, c_uint, CStr};

use enums::Clk;

pub mod enums;

pub type Window = u64;

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Arg {
    I(c_int),
    Ui(c_uint),
    F(f32),
    /// Argument for execvp in spawn
    V(Vec<String>),
    /// CONFIG.layouts index for setlayout
    L(Option<usize>),
}

macro_rules! arg_getters {
    ($($field:ident => $fn:ident => $ty:ty$(,)*)*) => {
        $(pub fn $fn(&self) -> $ty {
            if let Self::$field(x) = self {
                return x.clone();
            }
            panic!("{self:?}");
        })*
    }
}

impl Arg {
    arg_getters! {
        I => i => c_int,
        Ui => ui => c_uint,
        F => f => f32,
        V => v => Vec<String>,
        L => l => Option<usize>,
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct Button {
    pub click: c_uint,
    pub mask: c_uint,
    pub button: c_uint,
    pub func: Option<fn(*const Arg)>,
    pub arg: Arg,
}

impl Button {
    pub const fn new(
        click: Clk,
        mask: c_uint,
        button: c_uint,
        func: fn(*const Arg),
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
#[derive(Debug, Copy, Clone)]
pub struct Rule {
    pub class: *const c_char,
    pub instance: *const c_char,
    pub title: *const c_char,
    pub tags: c_uint,
    pub isfloating: c_int,
    pub monitor: c_int,
}

impl Rule {
    pub const fn new(
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
    pub showbar: bool,
    pub topbar: bool,
    pub clients: *mut Client,
    pub sel: *mut Client,
    pub stack: *mut Client,
    pub next: *mut Monitor,
    pub barwin: Window,
    pub lt: [*const Layout; 2usize],
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
    pub isfloating: c_int,
    pub isurgent: c_int,
    pub neverfocus: c_int,
    pub oldstate: c_int,
    pub isfullscreen: c_int,
    pub next: *mut Client,
    pub snext: *mut Client,
    pub mon: *mut Monitor,
    pub win: Window,
}
