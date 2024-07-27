use std::ffi::{c_int, c_uint};

use crate::bindgen;
use crate::bindgen::Cur;
use crate::bindgen::Drw;
use crate::bindgen::Window;
use bindgen::{Clr, Display};

pub(crate) fn create(
    dpy: *mut Display,
    screen: c_int,
    root: Window,
    w: c_uint,
    h: c_uint,
) -> *mut Drw {
    unsafe {
        // funny they don't check ecalloc output here but do in other places
        let drw: *mut Drw = bindgen::ecalloc(1, size_of::<Drw>()).cast();
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
        let cur: *mut Cur = bindgen::ecalloc(1, size_of::<Cur>()).cast();
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
