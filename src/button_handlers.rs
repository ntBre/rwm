#![allow(unused)]

use std::ptr::null_mut;

use crate::bindgen::selmon;

use crate::{arrange, focus, Arg, TAGMASK};

pub fn setlayout(arg: &Arg) {
    log::trace!("setlayout");
}

pub fn zoom(arg: &Arg) {
    log::trace!("zoom");
}

pub fn spawn(arg: &Arg) {
    log::trace!("spawn");
}

pub fn movemouse(arg: &Arg) {
    log::trace!("movemouse");
}

pub fn togglefloating(arg: &Arg) {
    log::trace!("togglefloating");
}

pub fn resizemouse(arg: &Arg) {
    log::trace!("resizemouse");
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
    log::trace!("toggleview");
}

pub fn tag(arg: &Arg) {
    log::trace!("tag");
}

pub fn toggletag(arg: &Arg) {
    log::trace!("toggletag");
}
