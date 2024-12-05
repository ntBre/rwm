use std::cmp::max;
use std::ffi::c_int;
use std::process::Command;
use std::ptr::null_mut;

use x11::xlib::{
    ButtonRelease, ConfigureRequest, CurrentTime, DestroyAll, EnterWindowMask,
    Expose, ExposureMask, False, GrabModeAsync, GrabSuccess, MapRequest,
    MotionNotify, NoEventMask, SubstructureRedirectMask, XCheckMaskEvent,
    XConfigureWindow, XEvent, XGrabPointer, XGrabServer, XKillClient,
    XMaskEvent, XSetCloseDownMode, XSetErrorHandler, XSync, XUngrabPointer,
    XUngrabServer, XWarpPointer, XWindowChanges, CWY,
};

use crate::config::CONFIG;
use crate::enums::WM;
use crate::{
    arrange, attach, attachstack, detach, detachstack, drawbar, focus,
    getrootptr, height, is_visible, nexttiled, pop, recttomon, resize,
    resizebarwin, restack, sendevent, setfullscreen, unfocus, updatebarpos,
    width, xerror, xerrordummy, DPY, HANDLER, MONS, MOUSEMASK, ROOT,
    SCRATCHTAG, SELMON, SYSTRAY, TAGMASK, XNONE,
};
use rwm::State;
use rwm::{Arg, Client, Monitor};

pub(crate) fn togglebar(state: &State, _arg: *const Arg) {
    unsafe {
        (*(*SELMON).pertag).showbars[(*(*SELMON).pertag).curtag as usize] =
            !((*SELMON).showbar);
        (*SELMON).showbar =
            (*(*SELMON).pertag).showbars[(*(*SELMON).pertag).curtag as usize];
        updatebarpos(state, SELMON);
        resizebarwin(state, SELMON);
        if CONFIG.showsystray {
            let mut wc = XWindowChanges {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                border_width: 0,
                sibling: 0,
                stack_mode: 0,
            };
            if !(*SELMON).showbar {
                wc.y = -state.bh;
            } else if (*SELMON).showbar {
                wc.y = 0;
                if !(*SELMON).topbar {
                    wc.y = (*SELMON).mh - state.bh;
                }
            }
            XConfigureWindow(DPY, (*SYSTRAY).win, CWY as u32, &mut wc);
        }
        arrange(state, SELMON);
    }
}

pub(crate) fn focusstack(_state: &State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        if (*SELMON).sel.is_null()
            || ((*(*SELMON).sel).isfullscreen && CONFIG.lock_fullscreen)
        {
            return;
        }
        if (*arg).i() > 0 {
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
            focus(_state, c);
            restack(_state, SELMON);
        }
    }
}

/// Increase the number of windows in the master area.
pub(crate) fn incnmaster(_state: &State, arg: *const Arg) {
    unsafe {
        (*(*SELMON).pertag).nmasters[(*(*SELMON).pertag).curtag as usize] =
            std::cmp::max((*SELMON).nmaster + (*arg).i(), 0);
        (*SELMON).nmaster =
            (*(*SELMON).pertag).nmasters[(*(*SELMON).pertag).curtag as usize];
        arrange(_state, SELMON);
    }
}

/// Set the fraction of the screen occupied by the master window. An `arg.f`
/// greater than 1.0 sets the fraction absolutely, while fractional values add
/// to the current value. Total values are restricted to the range [0.05, 0.95]
/// to leave at least 5% of the screen for other windows.
pub(crate) fn setmfact(_state: &State, arg: *const Arg) {
    unsafe {
        if arg.is_null()
            || (*(*SELMON).lt[(*SELMON).sellt as usize]).arrange.is_none()
        {
            return;
        }
        let f = if (*arg).f() < 1.0 {
            (*arg).f() + (*SELMON).mfact
        } else {
            (*arg).f() - 1.0
        };
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        (*(*SELMON).pertag).mfacts[(*(*SELMON).pertag).curtag as usize] = f;
        (*SELMON).mfact =
            (*(*SELMON).pertag).mfacts[(*(*SELMON).pertag).curtag as usize];
        arrange(_state, SELMON);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) fn zoom(_state: &State, _arg: *const Arg) {
    unsafe {
        let mut c = (*SELMON).sel;
        if (*(*SELMON).lt[(*SELMON).sellt as usize]).arrange.is_none()
            || c.is_null()
            || (*c).isfloating
        {
            return;
        }
        if c == nexttiled((*SELMON).clients) {
            c = nexttiled((*c).next);
            if c.is_null() {
                return;
            }
        }
        pop(_state, c);
    }
}

/// View the tag identified by `arg.ui`.
pub(crate) fn view(_state: &State, arg: *const Arg) {
    log::trace!("view");
    unsafe {
        if (*arg).ui() & *TAGMASK
            == (*SELMON).tagset[(*SELMON).seltags as usize]
        {
            return;
        }
        (*SELMON).seltags ^= 1; // toggle sel tagset

        // Safety: we were gonna dereference it anyway
        let pertag = &mut *(*SELMON).pertag;
        if ((*arg).ui() & *TAGMASK) != 0 {
            (*SELMON).tagset[(*SELMON).seltags as usize] =
                (*arg).ui() & *TAGMASK;
            pertag.prevtag = pertag.curtag;

            if (*arg).ui() == !0 {
                pertag.curtag = 0;
            } else {
                let mut i;
                cfor!((i = 0; ((*arg).ui() & 1 << i) == 0; i += 1) {});
                pertag.curtag = i + 1;
            }
        } else {
            std::mem::swap(&mut pertag.prevtag, &mut pertag.curtag);
        }

        (*SELMON).nmaster = pertag.nmasters[pertag.curtag as usize];
        (*SELMON).mfact = pertag.mfacts[pertag.curtag as usize];
        (*SELMON).sellt = pertag.sellts[pertag.curtag as usize];
        (*SELMON).lt[(*SELMON).sellt as usize] =
            pertag.ltidxs[pertag.curtag as usize][(*SELMON).sellt as usize];
        (*SELMON).lt[((*SELMON).sellt ^ 1) as usize] = pertag.ltidxs
            [pertag.curtag as usize][((*SELMON).sellt ^ 1) as usize];

        if (*SELMON).showbar != pertag.showbars[pertag.curtag as usize] {
            togglebar(_state, null_mut());
        }

        focus(_state, null_mut());
        arrange(_state, SELMON);
    }
}

pub(crate) fn killclient(state: &State, _arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }

        if sendevent(
            state,
            (*(*SELMON).sel).win,
            state.wmatom[WM::Delete as usize],
            NoEventMask as i32,
            state.wmatom[WM::Delete as usize] as i64,
            CurrentTime as i64,
            0,
            0,
            0,
        ) == 0
        {
            XGrabServer(DPY);
            XSetErrorHandler(Some(xerrordummy));
            XSetCloseDownMode(DPY, DestroyAll);
            XKillClient(DPY, (*(*SELMON).sel).win);
            XSync(DPY, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(DPY);
        }
    }
}

pub(crate) fn setlayout(_state: &State, arg: *const Arg) {
    log::trace!("setlayout: {arg:?}");
    unsafe {
        if arg.is_null()
            || (*arg).l().is_none()
            || !std::ptr::eq(
                &CONFIG.layouts[(*arg).l().unwrap()],
                (*SELMON).lt[(*SELMON).sellt as usize],
            )
        {
            (*(*SELMON).pertag).sellts[(*(*SELMON).pertag).curtag as usize] ^=
                1;
            (*SELMON).sellt =
                (*(*SELMON).pertag).sellts[(*(*SELMON).pertag).curtag as usize];
        }
        if !arg.is_null() && (*arg).l().is_some() {
            (*(*SELMON).pertag).ltidxs[(*(*SELMON).pertag).curtag as usize]
                [(*SELMON).sellt as usize] =
                &CONFIG.layouts[(*arg).l().unwrap()];
            (*SELMON).lt[(*SELMON).sellt as usize] = (*(*SELMON).pertag).ltidxs
                [(*(*SELMON).pertag).curtag as usize]
                [(*SELMON).sellt as usize];
        }
        libc::strncpy(
            (*SELMON).ltsymbol.as_mut_ptr(),
            (*(*SELMON).lt[(*SELMON).sellt as usize]).symbol,
            size_of_val(&(*SELMON).ltsymbol),
        );
        if !(*SELMON).sel.is_null() {
            arrange(_state, SELMON);
        } else {
            drawbar(_state, SELMON);
        }
    }
}

pub(crate) fn togglefloating(_state: &State, _arg: *const Arg) {
    log::trace!("togglefloating: {_arg:?}");
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        if (*(*SELMON).sel).isfullscreen {
            // no support for fullscreen windows
            return;
        }
        (*(*SELMON).sel).isfloating =
            !(*(*SELMON).sel).isfloating || (*(*SELMON).sel).isfixed != 0;
        if (*(*SELMON).sel).isfloating {
            let sel = &mut *(*SELMON).sel;
            resize(_state, sel, sel.x, sel.y, sel.w, sel.h, 0);
        }
        arrange(_state, SELMON);
    }
}

/// Push clients up (`Arg::I(+N)`) and down (`Arg::I(-N)`) the stack.
///
/// From the [stacker patch](https://dwm.suckless.org/patches/stacker/). This
/// should only be called with an ISINC arg, in their parlance, so also inline
/// their stackpos function, in the branch where this is true
pub(crate) fn pushstack(_state: &State, arg: *const Arg) {
    fn modulo(n: c_int, m: c_int) -> c_int {
        if n % m < 0 {
            (n % m) + m
        } else {
            n % m
        }
    }
    unsafe {
        // begin stackpos
        if (*SELMON).clients.is_null() {
            return;
        }
        if (*SELMON).sel.is_null() {
            return;
        }
        let mut i;
        let mut c;
        cfor!((
            (i, c) = (0, (*SELMON).clients);
            c != (*SELMON).sel;
            (i, c) = (i + is_visible(c) as c_int, (*c).next)) {});
        let mut n;
        cfor!((n = i; !c.is_null(); (n, c) = (n + is_visible(c) as c_int, (*c).next)) {});
        let mut stackpos = modulo(i + (*arg).i(), n);
        // end stackpos

        let sel = (*SELMON).sel;
        match stackpos.cmp(&0) {
            std::cmp::Ordering::Less => return,
            std::cmp::Ordering::Equal => {
                detach(sel);
                attach(sel);
            }
            std::cmp::Ordering::Greater => {
                let mut p;
                cfor!((
                (p, c) = (null_mut(), (*SELMON).clients);
                !c.is_null();
                (p, c) = (c, (*c).next)) {
                    stackpos -= (is_visible(c) && c != sel) as c_int;
                    if stackpos == 0 {
                        break;
                    }
                });
                let c = if !c.is_null() { c } else { p };
                detach(sel);
                (*sel).next = (*c).next;
                (*c).next = sel;
            }
        }
        arrange(_state, SELMON);
    }
}

pub(crate) fn tag(_state: &State, arg: *const Arg) {
    unsafe {
        if !(*SELMON).sel.is_null() && (*arg).ui() & *TAGMASK != 0 {
            (*(*SELMON).sel).tags = (*arg).ui() & *TAGMASK;
            focus(_state, null_mut());
            arrange(_state, SELMON);
        }
    }
}

fn dirtomon(dir: i32) -> *mut Monitor {
    unsafe {
        let mut m;

        if dir > 0 {
            m = (*SELMON).next;
            if m.is_null() {
                m = MONS;
            }
        } else if SELMON == MONS {
            cfor!((m = MONS; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = MONS; (*m).next != SELMON; m = (*m).next) {});
        }
        m
    }
}

pub(crate) fn focusmon(state: &State, arg: *const Arg) {
    unsafe {
        if (*MONS).next.is_null() {
            return;
        }
        let m = dirtomon((*arg).i());
        if m == SELMON {
            return;
        }
        unfocus(state, (*SELMON).sel, false);
        SELMON = m;
        focus(state, null_mut());
    }
}

fn sendmon(state: &State, c: *mut Client, m: *mut Monitor) {
    unsafe {
        if (*c).mon == m {
            return;
        }

        unfocus(state, c, true);
        detach(c);
        detachstack(c);
        (*c).mon = m;
        // assign tags of target monitor
        (*c).tags = (*m).tagset[(*m).seltags as usize];
        attach(c);
        attachstack(c);
        focus(state, null_mut());
        arrange(state, null_mut());
    }
}

pub(crate) fn tagmon(_state: &State, arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() || (*MONS).next.is_null() {
            return;
        }
        sendmon(_state, (*SELMON).sel, dirtomon((*arg).i()));
    }
}

pub(crate) fn toggleview(_state: &State, arg: *const Arg) {
    unsafe {
        let newtagset = (*SELMON).tagset[(*SELMON).seltags as usize]
            ^ ((*arg).ui() & *TAGMASK);

        if newtagset != 0 {
            (*SELMON).tagset[(*SELMON).seltags as usize] = newtagset;

            if newtagset == !0 {
                (*(*SELMON).pertag).prevtag = (*(*SELMON).pertag).curtag;
                (*(*SELMON).pertag).curtag = 0;
            }

            // test if the user did not select the same tag
            if (newtagset & 1 << ((*(*SELMON).pertag).curtag - 1)) == 0 {
                (*(*SELMON).pertag).prevtag = (*(*SELMON).pertag).curtag;
                let mut i;
                cfor!((i = 0; (newtagset & 1 << i) == 0; i += 1) {});
                (*(*SELMON).pertag).curtag = i + 1;
            }

            // apply settings for this view
            (*SELMON).nmaster = (*(*SELMON).pertag).nmasters
                [(*(*SELMON).pertag).curtag as usize];
            (*SELMON).mfact =
                (*(*SELMON).pertag).mfacts[(*(*SELMON).pertag).curtag as usize];
            (*SELMON).sellt =
                (*(*SELMON).pertag).sellts[(*(*SELMON).pertag).curtag as usize];
            (*SELMON).lt[(*SELMON).sellt as usize] = (*(*SELMON).pertag).ltidxs
                [(*(*SELMON).pertag).curtag as usize]
                [(*SELMON).sellt as usize];
            (*SELMON).lt[((*SELMON).sellt ^ 1) as usize] = (*(*SELMON).pertag)
                .ltidxs[(*(*SELMON).pertag).curtag as usize]
                [((*SELMON).sellt ^ 1) as usize];

            if (*SELMON).showbar
                != (*(*SELMON).pertag).showbars
                    [(*(*SELMON).pertag).curtag as usize]
            {
                togglebar(_state, null_mut());
            }

            focus(_state, null_mut());
            arrange(_state, SELMON);
        }
    }
}

pub(crate) fn quit(_state: &State, _arg: *const Arg) {
    unsafe {
        crate::RUNNING = false;
    }
}

// these are shared between movemouse and resizemouse
const CONFIGURE_REQUEST: i32 = ConfigureRequest;
const EXPOSE: i32 = Expose;
const MAP_REQUEST: i32 = MapRequest;
const MOTION_NOTIFY: i32 = MotionNotify;

pub(crate) fn movemouse(state: &State, _arg: *const Arg) {
    log::trace!("movemouse");
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c; // reborrow now that it's not null
        if c.isfullscreen {
            return; // no support for moving fullscreen windows with mouse
        }
        restack(state, SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if XGrabPointer(
            DPY,
            ROOT,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            XNONE as u64,
            state.cursors.move_.cursor,
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
            XMaskEvent(
                DPY,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                &mut ev,
            );
            match ev.type_ {
                CONFIGURE_REQUEST | EXPOSE | MAP_REQUEST => {
                    HANDLER[ev.type_ as usize](state, &mut ev);
                }
                MOTION_NOTIFY => {
                    if ev.motion.time - lasttime <= 1000 / 60 {
                        continue;
                    }
                    lasttime = ev.motion.time;
                    let mut nx = ocx + (ev.motion.x - x);
                    let mut ny = ocy + (ev.motion.y - y);
                    if ((*SELMON).wx - nx).abs() < CONFIG.snap as c_int {
                        nx = (*SELMON).wx;
                    } else if (((*SELMON).wx + (*SELMON).ww) - (nx + width(c)))
                        .abs()
                        < CONFIG.snap as c_int
                    {
                        nx = (*SELMON).wx + (*SELMON).ww - width(c);
                    }
                    if ((*SELMON).wy - ny).abs() < CONFIG.snap as c_int {
                        ny = (*SELMON).wy;
                    } else if (((*SELMON).wy + (*SELMON).wh) - (ny + height(c)))
                        .abs()
                        < CONFIG.snap as c_int
                    {
                        ny = (*SELMON).wy + (*SELMON).wh - height(c);
                    }
                    if !c.isfloating
                        && (*(*SELMON).lt[(*SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nx - c.x).abs() > CONFIG.snap as c_int
                            || (ny - c.y).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt as usize])
                        .arrange
                        .is_none()
                        || c.isfloating
                    {
                        resize(state, c, nx, ny, c.w, c.h, 1);
                    }
                }
                _ => {}
            }
            if ev.type_ == ButtonRelease {
                break;
            }
        }
        XUngrabPointer(DPY, CurrentTime);
        let m = recttomon(c.x, c.y, c.w, c.h);
        if m != SELMON {
            sendmon(state, c, m);
            SELMON = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn resizemouse(state: &State, _arg: *const Arg) {
    log::trace!("resizemouse");
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c;
        if c.isfullscreen {
            return; // no support for resizing fullscreen window with mouse
        }
        restack(state, SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if XGrabPointer(
            DPY,
            ROOT,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            XNONE as u64,
            state.cursors.resize.cursor,
            CurrentTime,
        ) != GrabSuccess
        {
            return;
        }
        XWarpPointer(
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
            XMaskEvent(
                DPY,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                &mut ev,
            );
            match ev.type_ {
                CONFIGURE_REQUEST | EXPOSE | MAP_REQUEST => {
                    HANDLER[ev.type_ as usize](state, &mut ev);
                }
                MOTION_NOTIFY => {
                    if ev.motion.time - lasttime <= 1000 / 60 {
                        continue;
                    }
                    lasttime = ev.motion.time;
                    let nw = max(ev.motion.x - ocx - 2 * c.bw + 1, 1);
                    let nh = max(ev.motion.y - ocy - 2 * c.bw + 1, 1);
                    if (*c.mon).wx + nw >= (*SELMON).wx
                        && (*c.mon).wx + nw <= (*SELMON).wx + (*SELMON).ww
                        && (*c.mon).wy + nh >= (*SELMON).wy
                        && (*c.mon).wy + nh <= (*SELMON).wy + (*SELMON).wh
                        && !c.isfloating
                        && (*(*SELMON).lt[(*SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nw - c.w).abs() > CONFIG.snap as c_int
                            || (nh - c.h).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt as usize])
                        .arrange
                        .is_none()
                        || c.isfloating
                    {
                        resize(state, c, c.x, c.y, nw, nh, 1);
                    }
                }
                _ => {}
            }
            if ev.type_ == ButtonRelease {
                break;
            }
        }

        XWarpPointer(
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
        XUngrabPointer(DPY, CurrentTime);
        while XCheckMaskEvent(DPY, EnterWindowMask, &mut ev) != 0 {}
        let m = recttomon(c.x, c.y, c.w, c.h);
        if m != SELMON {
            sendmon(state, c, m);
            SELMON = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn spawn(_state: &State, arg: *const Arg) {
    unsafe {
        let mut argv = (*arg).v();
        if argv == *CONFIG.dmenucmd {
            log::trace!("spawn: dmenucmd on monitor {}", (*SELMON).num);
            argv.push("-m".into());
            argv.push((*SELMON).num.to_string());
        }

        (*SELMON).tagset[(*SELMON).seltags as usize] &= !*SCRATCHTAG;

        let mut cmd = Command::new(argv[0].clone());
        let cmd = if argv.len() > 1 { cmd.args(&argv[1..]) } else { &mut cmd };

        let Ok(_) = cmd.spawn() else {
            panic!("rwm: spawn '{:?}' failed", argv[0]);
        };
    }
}

/// Move the current window to the tag specified by `arg.ui`.
pub(crate) fn toggletag(_state: &State, arg: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        let newtags = (*(*SELMON).sel).tags ^ ((*arg).ui() & *TAGMASK);
        if newtags != 0 {
            (*(*SELMON).sel).tags = newtags;
            focus(_state, null_mut());
            arrange(_state, SELMON);
        }
    }
}

/// Toggle fullscreen for a window.
///
/// adapted from: https://old.reddit.com/r/dwm/comments/avhkgb/fullscreen_mode/
/// for fixing problems with steam games
pub(crate) fn fullscreen(_state: &State, _: *const Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        setfullscreen(_state, (*SELMON).sel, !(*(*SELMON).sel).isfullscreen)
    }
}

pub(crate) fn togglescratch(_state: &State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client;
        let mut found = false;
        cfor!((
        c = (*SELMON).clients;
        !c.is_null();
        c = (*c).next) {
            found = ((*c).tags & *SCRATCHTAG) != 0;
            if found {
                break;
            }
        });
        if found {
            let newtagset =
                (*SELMON).tagset[(*SELMON).seltags as usize] ^ *SCRATCHTAG;
            if newtagset != 0 {
                (*SELMON).tagset[(*SELMON).seltags as usize] = newtagset;
                focus(_state, null_mut());
                arrange(_state, SELMON);
            }
            if is_visible(c) {
                focus(_state, c);
                restack(_state, SELMON);
            }
        } else {
            spawn(_state, arg);
        }
    }
}
