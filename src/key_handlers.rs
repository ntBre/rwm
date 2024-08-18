use std::ffi::c_int;

use crate::bindgen::{self, bh, dpy, Arg};
use crate::{arrange, updatebarpos};

pub(crate) unsafe extern "C" fn togglebar(_arg: *const Arg) {
    unsafe {
        assert!(!bindgen::selmon.is_null());
        let selmon = &mut *bindgen::selmon;
        selmon.showbar = (selmon.showbar == 0) as c_int;
        updatebarpos(selmon);
        bindgen::XMoveResizeWindow(
            dpy,
            selmon.barwin,
            selmon.wx,
            selmon.by,
            selmon.ww as u32,
            bh as u32,
        );
        arrange(selmon);
    }
}

pub(crate) unsafe extern "C" fn focusstack(arg: *const Arg) {
    unsafe { bindgen::focusstack(arg) }
}

/// Increase the number of windows in the master area.
pub(crate) unsafe extern "C" fn incnmaster(arg: *const Arg) {
    unsafe {
        assert!(!bindgen::selmon.is_null());
        let selmon = &mut *bindgen::selmon;
        selmon.nmaster = std::cmp::max(selmon.nmaster + (*arg).i, 0);
        arrange(selmon);
    }
}

pub(crate) unsafe extern "C" fn setmfact(arg: *const Arg) {
    unsafe { bindgen::setmfact(arg) }
}

pub(crate) unsafe extern "C" fn zoom(arg: *const Arg) {
    unsafe { bindgen::zoom(arg) }
}

pub(crate) unsafe extern "C" fn view(arg: *const Arg) {
    unsafe { bindgen::view(arg) }
}

pub(crate) unsafe extern "C" fn killclient(arg: *const Arg) {
    unsafe { bindgen::killclient(arg) }
}

pub(crate) unsafe extern "C" fn setlayout(arg: *const Arg) {
    unsafe { bindgen::setlayout(arg) }
}

pub(crate) unsafe extern "C" fn togglefloating(arg: *const Arg) {
    unsafe { bindgen::togglefloating(arg) }
}

pub(crate) unsafe extern "C" fn tag(arg: *const Arg) {
    unsafe { bindgen::tag(arg) }
}

pub(crate) unsafe extern "C" fn focusmon(arg: *const Arg) {
    unsafe { bindgen::focusmon(arg) }
}

pub(crate) unsafe extern "C" fn tagmon(arg: *const Arg) {
    unsafe { bindgen::tagmon(arg) }
}

pub(crate) unsafe extern "C" fn toggleview(arg: *const Arg) {
    unsafe { bindgen::toggleview(arg) }
}

pub(crate) unsafe extern "C" fn quit(arg: *const Arg) {
    unsafe { bindgen::quit(arg) }
}
