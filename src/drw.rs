use std::ffi::{c_int, c_uint};

use crate::bindgen;
use crate::bindgen::Drw;

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
