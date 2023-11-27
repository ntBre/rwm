use std::cmp::min;

use crate::{height, is_visible, nexttiled, resize, Display, Monitor};

pub fn tile(dpy: &Display, m: *mut Monitor) {
    let mut n = 0;
    unsafe {
        let mut c = nexttiled((*m).clients);
        while !c.is_null() {
            c = nexttiled((*c).next);
            n += 1;
        }
        if n == 0 {
            return;
        }

        let mw = if n > (*m).nmaster {
            if (*m).nmaster > 0 {
                // no casts in dwm, not really sure which conversions are
                // expected, but mw is an unsigned int
                ((*m).ww as f64 * (*m).mfact) as i16
            } else {
                0
            }
        } else {
            (*m).ww
        };
        let mut i = 0;
        let mut my = 0;
        let mut ty = 0;
        let mut c = nexttiled((*m).clients);
        while !c.is_null() {
            if i < (*m).nmaster {
                let h = ((*m).wh - my) as i32 / (min(n, (*m).nmaster) - i);
                resize(
                    dpy,
                    c,
                    (*m).wx as i32,
                    ((*m).wy + my) as i32,
                    mw as i32 - (2 * (*c).bw) as i32,
                    h - (2 * (*c).bw),
                    false,
                );
                if my + (height(c) as i16) < (*m).wh {
                    my += height(c) as i16;
                }
            } else {
                let h = ((*m).wh - ty) / (n - i) as i16;
                resize(
                    dpy,
                    c,
                    ((*m).wx + mw) as i32,
                    ((*m).wy + ty) as i32,
                    ((*m).ww - mw as i16 - (2 * (*c).bw) as i16) as i32,
                    h as i32 - (2 * (*c).bw),
                    false,
                );
                if ty + (height(c) as i16) < (*m).wh {
                    ty += height(c) as i16;
                }
            }
            c = nexttiled((*c).next);
            i += 1;
        }
    }
}

pub fn monocle(dpy: &Display, m: *mut Monitor) {
    let mut n = 0;
    unsafe {
        let mut c = (*m).clients;
        while !c.is_null() {
            if is_visible(c) {
                n += 1;
            }
            c = (*c).next;
        }
        if n > 0 {
            // override layout symbol
            (*m).ltsymbol = format!("[{n}]");
        }
        let mut c = nexttiled((*m).clients);
        while !c.is_null() {
            resize(
                dpy,
                c,
                (*m).wx as i32,
                (*m).wy as i32,
                (*m).ww as i32 - (2 * (*c).bw) as i32,
                (*m).wh as i32 - (2 * (*c).bw) as i32,
                false,
            );
            c = nexttiled((*c).next);
        }
    }
}
