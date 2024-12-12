use std::ffi::{c_int, c_uint};

use enums::Clk;
use x11::xft::XftColor;

pub mod drw;
pub mod enums;
pub mod events;
pub mod util;

pub use state::*;
mod state;

pub type Window = u64;
pub type Clr = XftColor;

#[repr(C)]
#[derive(Clone, Debug, serde::Deserialize)]
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

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct ButtonFn(pub Option<fn(&mut State, *const Arg)>);

impl TryFrom<String> for ButtonFn {
    type Error = String;

    fn try_from(_value: String) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[repr(C)]
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Button {
    pub click: c_uint,
    pub mask: c_uint,
    pub button: c_uint,
    pub func: ButtonFn,
    pub arg: Arg,
}

impl Button {
    pub const fn new(
        click: Clk,
        mask: c_uint,
        button: c_uint,
        func: fn(&mut State, *const Arg),
        arg: Arg,
    ) -> Self {
        Self {
            click: click as c_uint,
            mask,
            button,
            func: ButtonFn(Some(func)),
            arg,
        }
    }
}

unsafe impl Sync for Button {}

pub struct Cursor {
    pub cursor: x11::xlib::Cursor,
}

#[repr(C)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rule {
    pub class: String,
    pub instance: String,
    pub title: String,
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

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct LayoutFn(pub Option<fn(&mut State, *mut Monitor)>);

impl TryFrom<String> for LayoutFn {
    type Error = String;

    fn try_from(_value: String) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[repr(C)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Layout {
    pub symbol: String,
    pub arrange: LayoutFn,
}

#[derive(Clone, Debug)]
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
    pub showbars: Vec<bool>,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Monitor {
    pub ltsymbol: String,
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
    pub pertag: Pertag,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Client {
    pub name: String,
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

pub struct Cursors {
    pub normal: Cursor,
    pub resize: Cursor,
    pub move_: Cursor,
}
