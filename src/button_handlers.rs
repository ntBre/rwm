#![allow(unused)]

use std::ptr::null_mut;

use crate::bindgen::{self, selmon};

use crate::{arrange, focus, Arg, TAGMASK};

pub fn setlayout(arg: &Arg) {
    log::trace!("setlayout: {arg:?}");
}

/// Toggles the master and top of the stack, bound to middle click on the title
/// bar by default
pub fn zoom(arg: &Arg) {
    log::trace!("zoom: {arg:?}");
    unsafe {
        let mut c = (*selmon).sel;
        if (*(*selmon).lt[(*selmon).sellt as usize]).arrange.is_none()
            || c.is_null()
            || (*c).isfloating != 0
        {
            return;
        }
        if c == bindgen::nexttiled((*selmon).clients) {
            c = bindgen::nexttiled((*c).next);
            if c.is_null() {
                return;
            }
        }
        bindgen::pop(c);
    }
}

pub fn spawn(arg: &Arg) {
    log::trace!("spawn: {arg:?}");
}

pub fn movemouse(arg: &Arg) {
    log::trace!("movemouse: {arg:?}");
}

pub fn togglefloating(arg: &Arg) {
    log::trace!("togglefloating: {arg:?}");
}

pub fn resizemouse(arg: &Arg) {
    log::trace!("resizemouse: {arg:?}");
}

pub fn view(arg: &Arg) {
    log::trace!("view: {arg:?}");
    let Arg::Uint(ui) = arg else { return };
    unsafe {
        if ui & TAGMASK == (*selmon).tagset[(*selmon).seltags as usize] {
            return;
        }
        (*selmon).tagset[(*selmon).seltags as usize] ^= 1; // toggle sel tagset
        if ui & TAGMASK != 0 {
            (*selmon).tagset[(*selmon).seltags as usize] = ui & TAGMASK;
        }
        focus(null_mut());
        arrange(selmon);
    }
}

pub fn toggleview(arg: &Arg) {
    log::trace!("toggleview: {arg:?}");
}

pub fn tag(arg: &Arg) {
    log::trace!("tag: {arg:?}");
}

pub fn toggletag(arg: &Arg) {
    log::trace!("toggletag: {arg:?}");
}
