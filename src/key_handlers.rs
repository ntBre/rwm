use std::cmp::max;
use std::ffi::c_int;
use std::ptr::null_mut;

use libc::{c_char, sigaction, SIGCHLD, SIG_DFL};
use x11::xlib::{
    ConfigureRequest, CurrentTime, DestroyAll, EnterWindowMask, Expose,
    ExposureMask, False, GrabModeAsync, GrabSuccess, MapRequest, MotionNotify,
    SubstructureRedirectMask,
};

use crate::bindgen::{
    self, dmenucmd, dmenumon, mons, root, wmatom, Arg, ButtonRelease, Client,
    Layout, Monitor, XEvent,
};
use crate::config::{LOCK_FULLSCREEN, SNAP};
use crate::enums::WM;
use crate::util::die;
use crate::{
    arrange, attach, attachstack, detach, detachstack, drawbar, focus,
    getrootptr, height, is_visible, nexttiled, pop, recttomon, resize, restack,
    sendevent, unfocus, updatebarpos, width, BH, DPY, HANDLER, MOUSEMASK,
    SELMON, TAGMASK, XNONE,
};

pub(crate) unsafe extern "C" fn togglebar(_arg: *const Arg) {
    unsafe {
        (*SELMON).showbar = ((*SELMON).showbar == 0) as c_int;
        updatebarpos(SELMON);
        bindgen::XMoveResizeWindow(
            DPY,
            (*SELMON).barwin,
            (*SELMON).wx,
            (*SELMON).by,
            (*SELMON).ww as u32,
            BH as u32,
        );
        arrange(SELMON);
    }
}

pub(crate) unsafe extern "C" fn focusstack(arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        if (*SELMON).sel.is_null()
            || ((*(*SELMON).sel).isfullscreen != 0 && LOCK_FULLSCREEN != 0)
        {
            return;
        }
        if (*arg).i > 0 {
            cfor!((c = (*(*SELMON).sel).next; !c.is_null() && !is_visible(c); c = (*c).next) {});
            if c.is_null() {
                cfor!((c = (*SELMON).clients; !c.is_null() && !is_visible(c); c = (*c).next) {});
            }
        } else {
            cfor!((i = (*SELMON).clients; i != (*SELMON).sel; i = (*i).next) {
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
            restack(SELMON);
        }
    }
}

/// Increase the number of windows in the master area.
pub(crate) unsafe extern "C" fn incnmaster(arg: *const Arg) {
    unsafe {
        (*SELMON).nmaster = std::cmp::max((*SELMON).nmaster + (*arg).i, 0);
        arrange(SELMON);
    }
}

/// Set the fraction of the screen occupied by the master window. An `arg.f`
/// greater than 1.0 sets the fraction absolutely, while fractional values add
/// to the current value. Total values are restricted to the range [0.05, 0.95]
/// to leave at least 5% of the screen for other windows.
pub(crate) unsafe extern "C" fn setmfact(arg: *const Arg) {
    unsafe {
        if arg.is_null()
            || (*(*SELMON).lt[(*SELMON).sellt as usize]).arrange.is_none()
        {
            return;
        }
        let f = if (*arg).f < 1.0 {
            (*arg).f + (*SELMON).mfact
        } else {
            (*arg).f - 1.0
        };
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        (*SELMON).mfact = f;
        arrange(SELMON);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) unsafe extern "C" fn zoom(_arg: *const Arg) {
    unsafe {
        let mut c = (*SELMON).sel;
        if (*(*SELMON).lt[(*SELMON).sellt as usize]).arrange.is_none()
            || c.is_null()
            || (*c).isfloating != 0
        {
            return;
        }
        if c == nexttiled((*SELMON).clients) {
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
        if (*arg).ui & TAGMASK == (*SELMON).tagset[(*SELMON).seltags as usize] {
            return;
        }
        (*SELMON).seltags ^= 1; // toggle sel tagset
        if (*arg).ui & TAGMASK != 0 {
            (*SELMON).tagset[(*SELMON).seltags as usize] = (*arg).ui & TAGMASK;
        }
        focus(null_mut());
        arrange(SELMON);
    }
}

pub(crate) unsafe extern "C" fn killclient(_arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }

        if sendevent((*SELMON).sel, wmatom[WM::Delete as usize]) == 0 {
            bindgen::XGrabServer(DPY);
            bindgen::XSetErrorHandler(Some(bindgen::xerrordummy));
            bindgen::XSetCloseDownMode(DPY, DestroyAll);
            bindgen::XKillClient(DPY, (*(*SELMON).sel).win);
            bindgen::XSync(DPY, False);
            bindgen::XSetErrorHandler(Some(bindgen::xerror));
            bindgen::XUngrabServer(DPY);
        }
    }
}

pub(crate) unsafe extern "C" fn setlayout(arg: *const Arg) {
    unsafe {
        if arg.is_null()
            || (*arg).v.is_null()
            || (*arg).v.cast() != (*SELMON).lt[(*SELMON).sellt as usize]
        {
            (*SELMON).sellt ^= 1;
        }
        if !arg.is_null() && !(*arg).v.is_null() {
            (*SELMON).lt[(*SELMON).sellt as usize] = (*arg).v as *mut Layout;
        }
        libc::strncpy(
            (*SELMON).ltsymbol.as_mut_ptr(),
            (*(*SELMON).lt[(*SELMON).sellt as usize]).symbol,
            size_of_val(&(*SELMON).ltsymbol),
        );
        if !(*SELMON).sel.is_null() {
            arrange(SELMON);
        } else {
            drawbar(SELMON);
        }
    }
}

pub(crate) unsafe extern "C" fn togglefloating(_arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        if (*(*SELMON).sel).isfullscreen != 0 {
            // no support for fullscreen windows
            return;
        }
        (*(*SELMON).sel).isfloating = ((*(*SELMON).sel).isfloating == 0
            || (*(*SELMON).sel).isfixed != 0)
            as c_int;
        if (*(*SELMON).sel).isfloating != 0 {
            let sel = &mut *(*SELMON).sel;
            resize(sel, sel.x, sel.y, sel.w, sel.h, 0);
        }
        arrange(SELMON);
    }
}

pub(crate) unsafe extern "C" fn tag(arg: *const Arg) {
    unsafe {
        if !(*SELMON).sel.is_null() && (*arg).ui & TAGMASK != 0 {
            (*(*SELMON).sel).tags = (*arg).ui & TAGMASK;
            focus(null_mut());
            arrange(SELMON);
        }
    }
}

fn dirtomon(dir: i32) -> *mut Monitor {
    unsafe {
        let mut m;

        if dir > 0 {
            m = (*SELMON).next;
            if m.is_null() {
                m = mons;
            }
        } else if SELMON == mons {
            cfor!((m = mons; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = mons; (*m).next != SELMON; m = (*m).next) {});
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
        if m == SELMON {
            return;
        }
        unfocus((*SELMON).sel, false);
        SELMON = m;
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
        if (*SELMON).sel.is_null() || (*mons).next.is_null() {
            return;
        }
        sendmon((*SELMON).sel, dirtomon((*arg).i));
    }
}

pub(crate) unsafe extern "C" fn toggleview(arg: *const Arg) {
    unsafe {
        let newtagset = (*SELMON).tagset[(*SELMON).seltags as usize]
            ^ ((*arg).ui & TAGMASK);

        if newtagset != 0 {
            (*SELMON).tagset[(*SELMON).seltags as usize] = newtagset;
            focus(null_mut());
            arrange(SELMON);
        }
    }
}

pub(crate) unsafe extern "C" fn quit(_arg: *const Arg) {
    unsafe {
        crate::running = 0;
    }
}

// these are shared between movemouse and resizemouse
const CONFIGURE_REQUEST: i32 = ConfigureRequest;
const EXPOSE: i32 = Expose;
const MAP_REQUEST: i32 = MapRequest;
const MOTION_NOTIFY: i32 = MotionNotify;

pub(crate) unsafe extern "C" fn movemouse(_arg: *const Arg) {
    log::trace!("movemouse");
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c; // reborrow now that it's not null
        if c.isfullscreen != 0 {
            return; // no support for moving fullscreen windows with mouse
        }
        restack(SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if bindgen::XGrabPointer(
            DPY,
            root,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            XNONE as u64,
            (*bindgen::cursor[bindgen::CurMove as usize]).cursor,
            CurrentTime,
        ) != GrabSuccess
        {
            return;
        }
        let mut x = 0;
        let mut y = 0;
        if getrootptr(&mut x, &mut y) == 0 {
            return;
        }
        // nil init?
        let mut ev = XEvent { type_: 0 };
        let mut lasttime = 0;

        // emulating do-while
        loop {
            bindgen::XMaskEvent(
                DPY,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                &mut ev,
            );
            match ev.type_ {
                CONFIGURE_REQUEST | EXPOSE | MAP_REQUEST => {
                    HANDLER[ev.type_ as usize](&mut ev);
                }
                MOTION_NOTIFY => {
                    if ev.xmotion.time - lasttime <= 1000 / 60 {
                        continue;
                    }
                    lasttime = ev.xmotion.time;
                    let mut nx = ocx + (ev.xmotion.x - x);
                    let mut ny = ocy + (ev.xmotion.y - y);
                    if ((*SELMON).wx - nx).abs() < SNAP as c_int {
                        nx = (*SELMON).wx;
                    } else if (((*SELMON).wx + (*SELMON).ww) - (nx + width(c)))
                        .abs()
                        < SNAP as c_int
                    {
                        nx = (*SELMON).wx + (*SELMON).ww - width(c);
                    }
                    if ((*SELMON).wy - ny).abs() < SNAP as c_int {
                        ny = (*SELMON).wy;
                    } else if (((*SELMON).wy + (*SELMON).wh) - (ny + height(c)))
                        .abs()
                        < SNAP as c_int
                    {
                        ny = (*SELMON).wy + (*SELMON).wh - height(c);
                    }
                    if c.isfloating == 0
                        && (*(*SELMON).lt[(*SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nx - c.x).abs() > SNAP as c_int
                            || (ny - c.y).abs() > SNAP as c_int)
                    {
                        togglefloating(null_mut());
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt as usize])
                        .arrange
                        .is_none()
                        || c.isfloating != 0
                    {
                        resize(c, nx, ny, c.w, c.h, 1);
                    }
                }
                _ => {}
            }
            if ev.type_ == ButtonRelease as i32 {
                break;
            }
        }
        bindgen::XUngrabPointer(DPY, CurrentTime);
        let m = recttomon(c.x, c.y, c.w, c.h);
        if m != SELMON {
            sendmon(c, m);
            SELMON = m;
            focus(null_mut());
        }
    }
}

pub(crate) unsafe extern "C" fn resizemouse(_arg: *const Arg) {
    log::trace!("resizemouse");
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c;
        if c.isfullscreen != 0 {
            return; // no support for resizing fullscreen window with mouse
        }
        restack(SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if bindgen::XGrabPointer(
            DPY,
            root,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            XNONE as u64,
            (*bindgen::cursor[bindgen::CurResize as usize]).cursor,
            CurrentTime,
        ) != GrabSuccess
        {
            return;
        }
        bindgen::XWarpPointer(
            DPY,
            XNONE as u64,
            c.win,
            0,
            0,
            0,
            0,
            c.w + c.bw - 1,
            c.h + c.bw - 1,
        );

        let mut ev = XEvent { type_: 0 };
        let mut lasttime = 0;

        // do-while
        loop {
            bindgen::XMaskEvent(
                DPY,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                &mut ev,
            );
            match ev.type_ {
                CONFIGURE_REQUEST | EXPOSE | MAP_REQUEST => {
                    HANDLER[ev.type_ as usize](&mut ev);
                }
                MOTION_NOTIFY => {
                    if ev.xmotion.time - lasttime <= 1000 / 60 {
                        continue;
                    }
                    lasttime = ev.xmotion.time;
                    let nw = max(ev.xmotion.x - ocx - 2 * c.bw + 1, 1);
                    let nh = max(ev.xmotion.y - ocy - 2 * c.bw + 1, 1);
                    if (*c.mon).wx + nw >= (*SELMON).wx
                        && (*c.mon).wx + nw <= (*SELMON).wx + (*SELMON).ww
                        && (*c.mon).wy + nh >= (*SELMON).wy
                        && (*c.mon).wy + nh <= (*SELMON).wy + (*SELMON).wh
                        && c.isfloating == 0
                        && (*(*SELMON).lt[(*SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nw - c.w).abs() > SNAP as c_int
                            || (nh - c.h).abs() > SNAP as c_int)
                    {
                        togglefloating(null_mut());
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt as usize])
                        .arrange
                        .is_none()
                        || c.isfloating != 0
                    {
                        resize(c, c.x, c.y, nw, nh, 1);
                    }
                }
                _ => {}
            }
            if ev.type_ == ButtonRelease as i32 {
                break;
            }
        }

        bindgen::XWarpPointer(
            DPY,
            XNONE as u64,
            c.win,
            0,
            0,
            0,
            0,
            c.w + c.bw - 1,
            c.h + c.bw - 1,
        );
        bindgen::XUngrabPointer(DPY, CurrentTime);
        while bindgen::XCheckMaskEvent(DPY, EnterWindowMask, &mut ev) != 0 {}
        let m = recttomon(c.x, c.y, c.w, c.h);
        if m != SELMON {
            sendmon(c, m);
            SELMON = m;
            focus(null_mut());
        }
    }
}

pub(crate) unsafe extern "C" fn spawn(arg: *const Arg) {
    unsafe {
        if (*arg).v.cast() == dmenucmd.as_ptr() {
            log::trace!("spawn: dmenucmd on monitor {}", (*SELMON).num);
            dmenumon[0] = '0' as c_char + (*SELMON).num as c_char;
        }
        if libc::fork() == 0 {
            if !DPY.is_null() {
                libc::close(bindgen::XConnectionNumber(DPY));
            }
            libc::setsid();

            let mut sa = sigaction {
                sa_sigaction: SIG_DFL,
                // this is probably not strictly safe, but I'd rather not
                // MaybeUninit the whole sigaction if I can avoid it
                sa_mask: std::mem::zeroed(),
                sa_flags: 0,
                sa_restorer: None,
            };
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(SIGCHLD, &sa, null_mut());

            // trying to emulate ((char **)arg->v)[0]: casting arg->v to a
            // char ** and then accessing the first string (char *)
            libc::execvp(
                *(((*arg).v as *const *const c_char).offset(0)),
                (*arg).v as *const *const c_char,
            );
            die(&format!(
                "dwm: execvp '{:?}' failed:",
                *(((*arg).v as *const *const c_char).offset(0)),
            ));
        }
    }
}

/// Move the current window to the tag specified by `arg.ui`.
pub(crate) unsafe extern "C" fn toggletag(arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        let newtags = (*(*SELMON).sel).tags ^ ((*arg).ui & TAGMASK);
        if newtags != 0 {
            (*(*SELMON).sel).tags = newtags;
            focus(null_mut());
            arrange(SELMON);
        }
    }
}
