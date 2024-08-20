use std::{cmp::min, ffi::c_uint};

use libc::c_int;

use crate::bindgen::Monitor;
use crate::{height, is_visible, nexttiled, resize};

pub(crate) unsafe extern "C" fn monocle(m: *mut Monitor) {
    unsafe {
        let mut n = 0;
        let mut c;
        cfor!((c = (*m).clients; !c.is_null(); c = (*c).next) {
            if is_visible(c) {
                n += 1;
            }
        });
        if n > 0 {
            // override layout symbol
            libc::snprintf(
                (*m).ltsymbol.as_mut_ptr(),
                size_of_val(&(*m).ltsymbol),
                c"[%d]".as_ptr(),
                n,
            );
        }
        cfor!((c = nexttiled((*m).clients); !c.is_null(); c = nexttiled((*c).next)) {
            resize(c, (*m).wx, (*m).wy, (*m).ww - 2 * (*c).bw, (*m).wh - 2 * (*c).bw, 0);
        });
    }
}

pub(crate) unsafe extern "C" fn tile(m: *mut Monitor) {
    unsafe {
        let mut i;
        let mut n;
        let mut h;
        let mut c;
        let mut my;
        let mut ty;

        cfor!((
            (n, c) = (0, nexttiled((*m).clients));
            !c.is_null();
            (n, c) = (n + 1, nexttiled((*c).next))
        ) {});

        if n == 0 {
            return;
        }

        let mw = if n > (*m).nmaster {
            if (*m).nmaster > 0 {
                ((*m).ww as f32 * (*m).mfact) as c_uint
            } else {
                0
            }
        } else {
            (*m).ww as c_uint
        };

        cfor!((
            (i, my, ty, c) = (0, 0, 0, nexttiled((*m).clients));
            !c.is_null();
            (c, i) = (nexttiled((*c).next), i+1)
        ) {
            if i < (*m).nmaster {
                h = ((*m).wh - my) / (min(n, (*m).nmaster) - i);
                resize(
                    c,
                    (*m).wx,
                    (*m).wy + my,
                    mw as c_int - (2*(*c).bw),
                    h - (2*(*c).bw),
                    0
                );
                if my + height(c) < (*m).wh {
                    my += height(c);
                }

            } else {
                h = ((*m).wh - ty) / (n - i);
                resize(
                    c,
                    (*m).wx + mw as c_int,
                    (*m).wy + ty,
                    (*m).ww - mw as c_int - (2*(*c).bw),
                    h - (2*(*c).bw),
                    0
                );
                if ty + height(c) < (*m).wh {
                    ty += height(c);
                }
            }
        });
    }
}
