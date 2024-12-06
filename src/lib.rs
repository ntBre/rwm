use std::ffi::{c_char, c_int, c_uint};

use drw::Drw;
use enums::{Clk, Net, XEmbed, WM};
use x11::{
    xft::XftColor,
    xlib::{self, Atom, Display},
};

pub mod drw;
pub mod enums;
pub mod events;
pub mod util;

pub type Window = u64;
pub type Clr = XftColor;

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
    pub func: Option<fn(&mut State, *const Arg)>,
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
    pub arrange: Option<fn(&mut State, *mut Monitor)>,
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
    pub showbars: Vec<bool>,
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

pub struct Cursors {
    pub normal: Cursor,
    pub resize: Cursor,
    pub move_: Cursor,
}

pub struct State {
    /// Bar height
    pub bh: c_int,
    /// X display screen geometry width
    pub sw: c_int,
    pub wmatom: [Atom; WM::Last as usize],
    pub netatom: [Atom; Net::Last as usize],
    pub xatom: [Atom; XEmbed::Last as usize],
    pub dpy: *mut Display,
    pub drw: Drw,
    pub cursors: Cursors,
    pub SELMON: *mut Monitor,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            // drop cursors
            xlib::XFreeCursor(self.drw.dpy, self.cursors.move_.cursor);
            xlib::XFreeCursor(self.drw.dpy, self.cursors.normal.cursor);
            xlib::XFreeCursor(self.drw.dpy, self.cursors.resize.cursor);

            let fonts = std::mem::take(&mut self.drw.fonts);

            // must drop the fonts before the display they depend on
            drop(fonts);

            drw::free(&mut self.drw);

            xlib::XCloseDisplay(self.dpy);
        }
    }
}
