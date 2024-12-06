use std::ffi::{c_char, c_int};

use x11::xlib::{self, Atom, Display};

use super::{
    drw::{self, Drw},
    enums::{Net, XEmbed, WM},
    Cursors, Monitor,
};

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
    pub selmon: *mut Monitor,
    pub mons: *mut Monitor,
    pub stext: [c_char; 256],
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
