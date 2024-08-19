use std::ffi::c_int;
use std::ptr::null_mut;

use x11::xlib::False;

use crate::bindgen::{self, dpy, mons, wmatom, Arg, Client, Layout, Monitor};
use crate::config::LOCK_FULLSCREEN;
use crate::enums::WM;
use crate::{
    arrange, attach, attachstack, detach, detachstack, drawbar, focus,
    is_visible, nexttiled, pop, resize, restack, sendevent, unfocus,
    updatebarpos, BH, TAGMASK,
};

fn get_selmon() -> &'static mut Monitor {
    unsafe {
        assert!(!bindgen::selmon.is_null());
        &mut *bindgen::selmon
    }
}

pub(crate) unsafe extern "C" fn togglebar(_arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();
        selmon.showbar = (selmon.showbar == 0) as c_int;
        updatebarpos(selmon);
        bindgen::XMoveResizeWindow(
            dpy,
            selmon.barwin,
            selmon.wx,
            selmon.by,
            selmon.ww as u32,
            BH as u32,
        );
        arrange(selmon);
    }
}

pub(crate) unsafe extern "C" fn focusstack(arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        let selmon = get_selmon();
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
        let selmon = get_selmon();
        selmon.nmaster = std::cmp::max(selmon.nmaster + (*arg).i, 0);
        arrange(selmon);
    }
}

/// Set the fraction of the screen occupied by the master window. An `arg.f`
/// greater than 1.0 sets the fraction absolutely, while fractional values add
/// to the current value. Total values are restricted to the range [0.05, 0.95]
/// to leave at least 5% of the screen for other windows.
pub(crate) unsafe extern "C" fn setmfact(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

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
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        selmon.mfact = f;
        arrange(selmon);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) unsafe extern "C" fn zoom(_arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        let mut c = selmon.sel;
        if (*selmon.lt[selmon.sellt as usize]).arrange.is_none()
            || c.is_null()
            || (*c).isfloating != 0
        {
            return;
        }
        if c == nexttiled(selmon.clients) {
            c = nexttiled((*c).next);
            if c.is_null() {
                return;
            }
        }
        pop(c);
    }
}

/// View the tag identified by `arg.ui`.
pub(crate) unsafe extern "C" fn view(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        if (*arg).ui & TAGMASK == selmon.tagset[selmon.seltags as usize] {
            return;
        }
        selmon.seltags ^= 1; // toggle sel tagset
        if (*arg).ui & TAGMASK != 0 {
            selmon.tagset[selmon.seltags as usize] = (*arg).ui & TAGMASK;
        }
        focus(null_mut());
        arrange(selmon);
    }
}

pub(crate) unsafe extern "C" fn killclient(_arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        if selmon.sel.is_null() {
            return;
        }

        if sendevent(selmon.sel, wmatom[WM::Delete as usize]) == 0 {
            bindgen::XGrabServer(dpy);
            bindgen::XSetErrorHandler(Some(bindgen::xerrordummy));
            bindgen::XSetCloseDownMode(dpy, bindgen::DestroyAll as i32);
            bindgen::XKillClient(dpy, (*selmon.sel).win);
            bindgen::XSync(dpy, False);
            bindgen::XSetErrorHandler(Some(bindgen::xerror));
            bindgen::XUngrabServer(dpy);
        }
    }
}

pub(crate) unsafe extern "C" fn setlayout(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();
        if arg.is_null()
            || (*arg).v.is_null()
            || (*arg).v.cast() != selmon.lt[selmon.sellt as usize]
        {
            selmon.sellt ^= 1;
        }
        if !arg.is_null() && !(*arg).v.is_null() {
            selmon.lt[selmon.sellt as usize] = (*arg).v as *mut Layout;
        }
        libc::strncpy(
            selmon.ltsymbol.as_mut_ptr(),
            (*selmon.lt[selmon.sellt as usize]).symbol,
            size_of_val(&selmon.ltsymbol),
        );
        if !selmon.sel.is_null() {
            arrange(selmon);
        } else {
            drawbar(selmon);
        }
    }
}

pub(crate) unsafe extern "C" fn togglefloating(_arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        if selmon.sel.is_null() {
            return;
        }
        if (*selmon.sel).isfullscreen != 0 {
            // no support for fullscreen windows
            return;
        }
        (*selmon.sel).isfloating = ((*selmon.sel).isfloating == 0
            || (*selmon.sel).isfixed != 0)
            as c_int;
        if (*selmon.sel).isfloating != 0 {
            let sel = &mut *selmon.sel;
            resize(sel, sel.x, sel.y, sel.w, sel.h, 0);
        }
        arrange(selmon);
    }
}

pub(crate) unsafe extern "C" fn tag(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        if !selmon.sel.is_null() && (*arg).ui & TAGMASK != 0 {
            (*selmon.sel).tags = (*arg).ui & TAGMASK;
            focus(null_mut());
            arrange(selmon);
        }
    }
}

fn dirtomon(dir: i32) -> *mut Monitor {
    unsafe {
        let mut m;

        if dir > 0 {
            m = (*bindgen::selmon).next;
            if m.is_null() {
                m = mons;
            }
        } else if bindgen::selmon == mons {
            cfor!((m = mons; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = mons; (*m).next != bindgen::selmon; m = (*m).next) {});
        }
        m
    }
}

pub(crate) unsafe extern "C" fn focusmon(arg: *const Arg) {
    unsafe {
        if (*mons).next.is_null() {
            return;
        }
        let m = dirtomon((*arg).i);
        if m == bindgen::selmon {
            return;
        }
        unfocus((*bindgen::selmon).sel, false);
        bindgen::selmon = m;
        focus(null_mut());
    }
}

fn sendmon(c: *mut Client, m: *mut Monitor) {
    unsafe {
        if (*c).mon == m {
            return;
        }

        unfocus(c, true);
        detach(c);
        detachstack(c);
        (*c).mon = m;
        // assign tags of target monitor
        (*c).tags = (*m).tagset[(*m).seltags as usize];
        attach(c);
        attachstack(c);
        focus(null_mut());
        arrange(null_mut());
    }
}

pub(crate) unsafe extern "C" fn tagmon(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        if selmon.sel.is_null() || (*mons).next.is_null() {
            return;
        }
        sendmon(selmon.sel, dirtomon((*arg).i));
    }
}

pub(crate) unsafe extern "C" fn toggleview(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();

        let newtagset =
            selmon.tagset[selmon.seltags as usize] ^ ((*arg).ui & TAGMASK);

        if newtagset != 0 {
            selmon.tagset[selmon.seltags as usize] = newtagset;
            focus(null_mut());
            arrange(selmon);
        }
    }
}

pub(crate) unsafe extern "C" fn quit(_arg: *const Arg) {
    unsafe {
        crate::running = 0;
    }
}

pub(crate) unsafe extern "C" fn movemouse(arg: *const Arg) {
    unsafe { bindgen::movemouse(arg) }
}

pub(crate) unsafe extern "C" fn resizemouse(arg: *const Arg) {
    unsafe { bindgen::resizemouse(arg) }
}

pub(crate) unsafe extern "C" fn spawn(arg: *const Arg) {
    unsafe { bindgen::spawn(arg) }
}

/// Move the current window to the tag specified by `arg.ui`.
pub(crate) unsafe extern "C" fn toggletag(arg: *const Arg) {
    unsafe {
        let selmon = get_selmon();
        if selmon.sel.is_null() {
            return;
        }
        let newtags = (*selmon.sel).tags ^ ((*arg).ui & TAGMASK);
        if newtags != 0 {
            (*selmon.sel).tags = newtags;
            focus(null_mut());
            arrange(selmon);
        }
    }
}
