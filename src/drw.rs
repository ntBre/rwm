use std::ffi::{c_char, c_int, c_uint};
use std::ptr::null_mut;

use crate::bindgen;
use crate::bindgen::Cur;
use crate::bindgen::Drw;
use crate::bindgen::Window;
use bindgen::{Clr, Display, Fnt};

pub(crate) fn create(
    dpy: *mut Display,
    screen: c_int,
    root: Window,
    w: c_uint,
    h: c_uint,
) -> *mut Drw {
    unsafe {
        let drw: *mut Drw = crate::util::ecalloc(1, size_of::<Drw>()).cast();
        (*drw).dpy = dpy;
        (*drw).screen = screen;
        (*drw).root = root;
        (*drw).w = w;
        (*drw).h = h;
        (*drw).drawable = bindgen::XCreatePixmap(
            dpy,
            root,
            w,
            h,
            bindgen::XDefaultDepth(dpy, screen) as u32,
        );
        (*drw).gc = bindgen::XCreateGC(dpy, root, 0, std::ptr::null_mut());
        bindgen::XSetLineAttributes(
            dpy,
            (*drw).gc,
            1,
            bindgen::LineSolid as i32,
            bindgen::CapButt as i32,
            bindgen::JoinMiter as i32,
        );
        drw
    }
}

pub(crate) fn rect(
    drw: *mut Drw,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
    filled: c_int,
    invert: c_int,
) {
    unsafe {
        if drw.is_null() || (*drw).scheme.is_null() {
            return;
        }
        bindgen::XSetForeground(
            (*drw).dpy,
            (*drw).gc,
            if invert != 0 {
                (*(*drw).scheme.offset(bindgen::ColBg as isize)).pixel
            } else {
                (*(*drw).scheme.offset(bindgen::ColFg as isize)).pixel
            },
        );
        if filled != 0 {
            bindgen::XFillRectangle(
                (*drw).dpy,
                (*drw).drawable,
                (*drw).gc,
                x,
                y,
                w,
                h,
            );
        } else {
            bindgen::XDrawRectangle(
                (*drw).dpy,
                (*drw).drawable,
                (*drw).gc,
                x,
                y,
                w - 1,
                h - 1,
            );
        }
    }
}

pub(crate) fn cur_create(drw: *mut Drw, shape: c_int) -> *mut Cur {
    if drw.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let cur: *mut Cur = crate::util::ecalloc(1, size_of::<Cur>()).cast();
        if cur.is_null() {
            return std::ptr::null_mut();
        }
        (*cur).cursor = bindgen::XCreateFontCursor((*drw).dpy, shape as c_uint);
        cur
    }
}

pub(crate) fn setscheme(drw: *mut Drw, scm: *mut Clr) {
    if !drw.is_null() {
        unsafe {
            (*drw).scheme = scm;
        }
    }
}

pub(crate) fn fontset_create(
    drw: *mut Drw,
    fonts: &mut [*const c_char],
    fontcount: usize,
) -> *mut Fnt {
    unsafe {
        let mut ret: *mut Fnt = null_mut();

        // since fonts is a & not a *, it can't be null, but it could be empty
        if drw.is_null() || fonts.is_empty() {
            return null_mut();
        }

        for i in 1..=fontcount {
            let cur = xfont_create(drw, fonts[fontcount - i], null_mut());
            if !cur.is_null() {
                (*cur).next = ret;
                ret = cur;
            }
        }
        (*drw).fonts = ret;
        ret
    }
}

// DUMMY
fn xfont_create(
    drw: *mut Drw,
    fontname: *const i8,
    fontpattern: *mut bindgen::FcPattern,
) -> *mut Fnt {
    unsafe { bindgen::xfont_create(drw, fontname, fontpattern) }
}

// DUMMY
pub(crate) fn scm_create(
    drw: *mut Drw,
    clrnames: &mut [*const c_char],
    clrcount: usize,
) -> *mut Clr {
    unsafe { bindgen::drw_scm_create(drw, clrnames.as_mut_ptr(), clrcount) }
}

pub(crate) fn fontset_getwidth(drw: *mut Drw, text: *const c_char) -> c_uint {
    unsafe {
        if drw.is_null() || (*drw).fonts.is_null() || text.is_null() {
            return 0;
        }
    }
    self::text(drw, 0, 0, 0, 0, 0, text, 0) as c_uint
}

// DUMMY
#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    drw: *mut Drw,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
    lpad: c_uint,
    text: *const c_char,
    invert: c_int,
) -> c_int {
    unsafe { bindgen::drw_text(drw, x, y, w, h, lpad, text, invert) }
}

pub(crate) fn map(
    drw: *mut Drw,
    win: Window,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
) {
    if drw.is_null() {
        return;
    }
    unsafe {
        bindgen::XCopyArea(
            (*drw).dpy,
            (*drw).drawable,
            win,
            (*drw).gc,
            x,
            y,
            w,
            h,
            x,
            y,
        );
        bindgen::XSync((*drw).dpy, bindgen::False as i32);
    }
}

pub(crate) fn resize(drw: *mut Drw, w: c_uint, h: c_uint) {
    unsafe {
        if drw.is_null() {
            return;
        }
        (*drw).w = w;
        (*drw).h = h;
        if (*drw).drawable != 0 {
            bindgen::XFreePixmap((*drw).dpy, (*drw).drawable);
        }
        (*drw).drawable = bindgen::XCreatePixmap(
            (*drw).dpy,
            (*drw).root,
            w,
            h,
            bindgen::XDefaultDepth((*drw).dpy, (*drw).screen) as c_uint,
        );
    }
}
