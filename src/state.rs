use std::{
    ffi::{c_int, c_uint},
    ops::Index,
};

use x11::xlib::{self, Atom, Display};

#[cfg(target_os = "linux")]
use xcb::Connection;

use crate::{
    config::Config,
    drw::{self, Drw},
    enums::{Col, Net, Scheme, XEmbed, WM},
    Clr, Cursors, Monitor, Systray, Window,
};

/// A color scheme.
#[derive(Default)]
pub struct ClrScheme(Vec<Vec<Clr>>);

impl Index<(Scheme, Col)> for ClrScheme {
    type Output = Clr;

    fn index(&self, index: (Scheme, Col)) -> &Self::Output {
        &self.0[index.0 as usize][index.1 as usize]
    }
}

impl Index<Scheme> for ClrScheme {
    type Output = Vec<Clr>;

    fn index(&self, index: Scheme) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl ClrScheme {
    pub fn push(&mut self, clr: Vec<Clr>) {
        self.0.push(clr);
    }
}

pub struct State {
    /// Bar height
    pub bh: c_int,
    /// X display screen geometry width
    pub sw: c_int,
    /// X display screen geometry height
    pub sh: c_int,
    pub wmatom: [Atom; WM::Last as usize],
    pub netatom: [Atom; Net::Last as usize],
    pub xatom: [Atom; XEmbed::Last as usize],
    pub dpy: *mut Display,
    pub drw: Drw,
    pub cursors: Cursors,
    pub selmon: *mut Monitor,
    pub mons: *mut Monitor,
    pub stext: String,
    pub scheme: ClrScheme,
    pub screen: c_int,
    pub root: Window,
    /// sum of left and right padding for text
    pub lrpad: c_int,
    pub systray: Option<Systray>,
    /// Supporting window for NetWMCheck
    pub wmcheckwin: Window,
    pub running: bool,
    pub numlockmask: c_uint,
    pub CONFIG: Config,

    #[cfg(target_os = "linux")]
    pub xcon: *mut Connection,
}

impl State {
    pub fn systray(&self) -> &Systray {
        self.systray.as_ref().unwrap()
    }

    pub fn systray_mut(&mut self) -> &mut Systray {
        self.systray.as_mut().unwrap()
    }

    pub fn tagmask(&self) -> u32 {
        (1 << self.CONFIG.tags.len()) - 1
    }

    pub fn scratchtag(&self) -> u32 {
        1 << self.CONFIG.tags.len()
    }
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
