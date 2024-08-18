use std::ffi::c_int;
use std::ptr::null_mut;

use crate::bindgen::{self, bh, dpy, Arg, Client};
use crate::config::LOCK_FULLSCREEN;
use crate::{arrange, focus, is_visible, restack, updatebarpos};

macro_rules! cfor {
    ((; $cond:expr; $step:expr) $body:block ) => {
        cfor!(({}; $cond; $step) $body)
    };
    (($init:expr; $cond:expr; $step:expr) $body:block ) => {
        $init;
        while $cond {
            $body;
            $step;
        }
    };
}

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
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        assert!(!bindgen::selmon.is_null());
        let selmon = &mut *bindgen::selmon;
        if selmon.sel.is_null()
            || ((*selmon.sel).isfullscreen != 0 && LOCK_FULLSCREEN != 0)
        {
            return;
        }
        if (*arg).i > 0 {
            cfor!((c = (*selmon.sel).next; !c.is_null() && !is_visible(c); c = (*c).next) {});
            if c.is_null() {
                cfor!((c = selmon.clients; !c.is_null() && !is_visible(c); c = (*c).next) {});
            }
        } else {
            cfor!((i = selmon.clients; i != selmon.sel; i = (*i).next) {
                if is_visible(i) {
                    c = i;
                }
            });
            if c.is_null() {
                cfor!((; !i.is_null(); i = (*i).next) {
                    if is_visible(i) {
                        c = i;
                    }
                });
            }
        }
        if !c.is_null() {
            focus(c);
            restack(selmon);
        }
    }
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
    unsafe {
        assert!(!bindgen::selmon.is_null());
        let selmon = &mut *bindgen::selmon;

        if arg.is_null()
            || (*selmon.lt[selmon.sellt as usize]).arrange.is_none()
        {
            return;
        }
        let f = if (*arg).f < 1.0 {
            (*arg).f + selmon.mfact
        } else {
            (*arg).f - 1.0
        };
        if f < 0.05 || f > 0.95 {
            return;
        }
        selmon.mfact = f;
        arrange(selmon);
    }
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
