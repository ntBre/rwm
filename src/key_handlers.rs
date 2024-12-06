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
    width, xerror, xerrordummy, HANDLER, MONS, MOUSEMASK, ROOT, SCRATCHTAG,
    SYSTRAY, TAGMASK, XNONE,
};
use rwm::State;
use rwm::{Arg, Client, Monitor};

pub(crate) fn togglebar(state: &mut State, _arg: *const Arg) {
    unsafe {
        (*(*state.SELMON).pertag).showbars
            [(*(*state.SELMON).pertag).curtag as usize] =
            !((*state.SELMON).showbar);
        (*state.SELMON).showbar = (*(*state.SELMON).pertag).showbars
            [(*(*state.SELMON).pertag).curtag as usize];
        updatebarpos(state, state.SELMON);
        resizebarwin(state, state.SELMON);
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
            if !(*state.SELMON).showbar {
                wc.y = -state.bh;
            } else if (*state.SELMON).showbar {
                wc.y = 0;
                if !(*state.SELMON).topbar {
                    wc.y = (*state.SELMON).mh - state.bh;
                }
            }
            XConfigureWindow(state.dpy, (*SYSTRAY).win, CWY as u32, &mut wc);
        }
        arrange(state, state.SELMON);
    }
}

pub(crate) fn focusstack(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        if (*state.SELMON).sel.is_null()
            || ((*(*state.SELMON).sel).isfullscreen && CONFIG.lock_fullscreen)
        {
            return;
        }
        if (*arg).i() > 0 {
            cfor!((c = (*(*state.SELMON).sel).next; !c.is_null() && !is_visible(c); c = (*c).next) {});
            if c.is_null() {
                cfor!((c = (*state.SELMON).clients; !c.is_null() && !is_visible(c); c = (*c).next) {});
            }
        } else {
            cfor!((i = (*state.SELMON).clients; i != (*state.SELMON).sel; i = (*i).next) {
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
            focus(state, c);
            restack(state, state.SELMON);
        }
    }
}

/// Increase the number of windows in the master area.
pub(crate) fn incnmaster(state: &mut State, arg: *const Arg) {
    unsafe {
        (*(*state.SELMON).pertag).nmasters
            [(*(*state.SELMON).pertag).curtag as usize] =
            std::cmp::max((*state.SELMON).nmaster + (*arg).i(), 0);
        (*state.SELMON).nmaster = (*(*state.SELMON).pertag).nmasters
            [(*(*state.SELMON).pertag).curtag as usize];
        arrange(state, state.SELMON);
    }
}

/// Set the fraction of the screen occupied by the master window. An `arg.f`
/// greater than 1.0 sets the fraction absolutely, while fractional values add
/// to the current value. Total values are restricted to the range [0.05, 0.95]
/// to leave at least 5% of the screen for other windows.
pub(crate) fn setmfact(state: &mut State, arg: *const Arg) {
    unsafe {
        if arg.is_null()
            || (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
                .arrange
                .is_none()
        {
            return;
        }
        let f = if (*arg).f() < 1.0 {
            (*arg).f() + (*state.SELMON).mfact
        } else {
            (*arg).f() - 1.0
        };
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        (*(*state.SELMON).pertag).mfacts
            [(*(*state.SELMON).pertag).curtag as usize] = f;
        (*state.SELMON).mfact = (*(*state.SELMON).pertag).mfacts
            [(*(*state.SELMON).pertag).curtag as usize];
        arrange(state, state.SELMON);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) fn zoom(state: &mut State, _arg: *const Arg) {
    unsafe {
        let mut c = (*state.SELMON).sel;
        if (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
            .arrange
            .is_none()
            || c.is_null()
            || (*c).isfloating
        {
            return;
        }
        if c == nexttiled((*state.SELMON).clients) {
            c = nexttiled((*c).next);
            if c.is_null() {
                return;
            }
        }
        pop(state, c);
    }
}

/// View the tag identified by `arg.ui`.
pub(crate) fn view(state: &mut State, arg: *const Arg) {
    log::trace!("view");
    unsafe {
        if (*arg).ui() & *TAGMASK
            == (*state.SELMON).tagset[(*state.SELMON).seltags as usize]
        {
            return;
        }
        (*state.SELMON).seltags ^= 1; // toggle sel tagset

        // Safety: we were gonna dereference it anyway
        let pertag = &mut *(*state.SELMON).pertag;
        if ((*arg).ui() & *TAGMASK) != 0 {
            (*state.SELMON).tagset[(*state.SELMON).seltags as usize] =
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

        (*state.SELMON).nmaster = pertag.nmasters[pertag.curtag as usize];
        (*state.SELMON).mfact = pertag.mfacts[pertag.curtag as usize];
        (*state.SELMON).sellt = pertag.sellts[pertag.curtag as usize];
        (*state.SELMON).lt[(*state.SELMON).sellt as usize] = pertag.ltidxs
            [pertag.curtag as usize][(*state.SELMON).sellt as usize];
        (*state.SELMON).lt[((*state.SELMON).sellt ^ 1) as usize] = pertag
            .ltidxs[pertag.curtag as usize]
            [((*state.SELMON).sellt ^ 1) as usize];

        if (*state.SELMON).showbar != pertag.showbars[pertag.curtag as usize] {
            togglebar(state, null_mut());
        }

        focus(state, null_mut());
        arrange(state, state.SELMON);
    }
}

pub(crate) fn killclient(state: &mut State, _arg: *const Arg) {
    unsafe {
        if (*state.SELMON).sel.is_null() {
            return;
        }

        if sendevent(
            state,
            (*(*state.SELMON).sel).win,
            state.wmatom[WM::Delete as usize],
            NoEventMask as i32,
            state.wmatom[WM::Delete as usize] as i64,
            CurrentTime as i64,
            0,
            0,
            0,
        ) == 0
        {
            XGrabServer(state.dpy);
            XSetErrorHandler(Some(xerrordummy));
            XSetCloseDownMode(state.dpy, DestroyAll);
            XKillClient(state.dpy, (*(*state.SELMON).sel).win);
            XSync(state.dpy, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(state.dpy);
        }
    }
}

pub(crate) fn setlayout(state: &mut State, arg: *const Arg) {
    log::trace!("setlayout: {arg:?}");
    unsafe {
        if arg.is_null()
            || (*arg).l().is_none()
            || !std::ptr::eq(
                &CONFIG.layouts[(*arg).l().unwrap()],
                (*state.SELMON).lt[(*state.SELMON).sellt as usize],
            )
        {
            (*(*state.SELMON).pertag).sellts
                [(*(*state.SELMON).pertag).curtag as usize] ^= 1;
            (*state.SELMON).sellt = (*(*state.SELMON).pertag).sellts
                [(*(*state.SELMON).pertag).curtag as usize];
        }
        if !arg.is_null() && (*arg).l().is_some() {
            (*(*state.SELMON).pertag).ltidxs
                [(*(*state.SELMON).pertag).curtag as usize]
                [(*state.SELMON).sellt as usize] =
                &CONFIG.layouts[(*arg).l().unwrap()];
            (*state.SELMON).lt[(*state.SELMON).sellt as usize] =
                (*(*state.SELMON).pertag).ltidxs
                    [(*(*state.SELMON).pertag).curtag as usize]
                    [(*state.SELMON).sellt as usize];
        }
        libc::strncpy(
            (*state.SELMON).ltsymbol.as_mut_ptr(),
            (*(*state.SELMON).lt[(*state.SELMON).sellt as usize]).symbol,
            size_of_val(&(*state.SELMON).ltsymbol),
        );
        if !(*state.SELMON).sel.is_null() {
            arrange(state, state.SELMON);
        } else {
            drawbar(state, state.SELMON);
        }
    }
}

pub(crate) fn togglefloating(state: &mut State, _arg: *const Arg) {
    log::trace!("togglefloating: {_arg:?}");
    unsafe {
        if (*state.SELMON).sel.is_null() {
            return;
        }
        if (*(*state.SELMON).sel).isfullscreen {
            // no support for fullscreen windows
            return;
        }
        (*(*state.SELMON).sel).isfloating = !(*(*state.SELMON).sel).isfloating
            || (*(*state.SELMON).sel).isfixed != 0;
        if (*(*state.SELMON).sel).isfloating {
            let sel = &mut *(*state.SELMON).sel;
            resize(state, sel, sel.x, sel.y, sel.w, sel.h, 0);
        }
        arrange(state, state.SELMON);
    }
}

/// Push clients up (`Arg::I(+N)`) and down (`Arg::I(-N)`) the stack.
///
/// From the [stacker patch](https://dwm.suckless.org/patches/stacker/). This
/// should only be called with an ISINC arg, in their parlance, so also inline
/// their stackpos function, in the branch where this is true
pub(crate) fn pushstack(state: &mut State, arg: *const Arg) {
    fn modulo(n: c_int, m: c_int) -> c_int {
        if n % m < 0 {
            (n % m) + m
        } else {
            n % m
        }
    }
    unsafe {
        // begin stackpos
        if (*state.SELMON).clients.is_null() {
            return;
        }
        if (*state.SELMON).sel.is_null() {
            return;
        }
        let mut i;
        let mut c;
        cfor!((
            (i, c) = (0, (*state.SELMON).clients);
            c != (*state.SELMON).sel;
            (i, c) = (i + is_visible(c) as c_int, (*c).next)) {});
        let mut n;
        cfor!((n = i; !c.is_null(); (n, c) = (n + is_visible(c) as c_int, (*c).next)) {});
        let mut stackpos = modulo(i + (*arg).i(), n);
        // end stackpos

        let sel = (*state.SELMON).sel;
        match stackpos.cmp(&0) {
            std::cmp::Ordering::Less => return,
            std::cmp::Ordering::Equal => {
                detach(sel);
                attach(sel);
            }
            std::cmp::Ordering::Greater => {
                let mut p;
                cfor!((
                (p, c) = (null_mut(), (*state.SELMON).clients);
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
        arrange(state, state.SELMON);
    }
}

pub(crate) fn tag(state: &mut State, arg: *const Arg) {
    unsafe {
        if !(*state.SELMON).sel.is_null() && (*arg).ui() & *TAGMASK != 0 {
            (*(*state.SELMON).sel).tags = (*arg).ui() & *TAGMASK;
            focus(state, null_mut());
            arrange(state, state.SELMON);
        }
    }
}

fn dirtomon(state: &State, dir: i32) -> *mut Monitor {
    unsafe {
        let mut m;

        if dir > 0 {
            m = (*state.SELMON).next;
            if m.is_null() {
                m = MONS;
            }
        } else if state.SELMON == MONS {
            cfor!((m = MONS; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = MONS; (*m).next != state.SELMON; m = (*m).next) {});
        }
        m
    }
}

pub(crate) fn focusmon(state: &mut State, arg: *const Arg) {
    unsafe {
        if (*MONS).next.is_null() {
            return;
        }
        let m = dirtomon(state, (*arg).i());
        if m == state.SELMON {
            return;
        }
        unfocus(state, (*state.SELMON).sel, false);
        state.SELMON = m;
        focus(state, null_mut());
    }
}

fn sendmon(state: &mut State, c: *mut Client, m: *mut Monitor) {
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

pub(crate) fn tagmon(state: &mut State, arg: *const Arg) {
    unsafe {
        if (*state.SELMON).sel.is_null() || (*MONS).next.is_null() {
            return;
        }
        sendmon(state, (*state.SELMON).sel, dirtomon(state, (*arg).i()));
    }
}

pub(crate) fn toggleview(state: &mut State, arg: *const Arg) {
    unsafe {
        let newtagset = (*state.SELMON).tagset
            [(*state.SELMON).seltags as usize]
            ^ ((*arg).ui() & *TAGMASK);

        if newtagset != 0 {
            (*state.SELMON).tagset[(*state.SELMON).seltags as usize] =
                newtagset;

            if newtagset == !0 {
                (*(*state.SELMON).pertag).prevtag =
                    (*(*state.SELMON).pertag).curtag;
                (*(*state.SELMON).pertag).curtag = 0;
            }

            // test if the user did not select the same tag
            if (newtagset & 1 << ((*(*state.SELMON).pertag).curtag - 1)) == 0 {
                (*(*state.SELMON).pertag).prevtag =
                    (*(*state.SELMON).pertag).curtag;
                let mut i;
                cfor!((i = 0; (newtagset & 1 << i) == 0; i += 1) {});
                (*(*state.SELMON).pertag).curtag = i + 1;
            }

            // apply settings for this view
            (*state.SELMON).nmaster = (*(*state.SELMON).pertag).nmasters
                [(*(*state.SELMON).pertag).curtag as usize];
            (*state.SELMON).mfact = (*(*state.SELMON).pertag).mfacts
                [(*(*state.SELMON).pertag).curtag as usize];
            (*state.SELMON).sellt = (*(*state.SELMON).pertag).sellts
                [(*(*state.SELMON).pertag).curtag as usize];
            (*state.SELMON).lt[(*state.SELMON).sellt as usize] =
                (*(*state.SELMON).pertag).ltidxs
                    [(*(*state.SELMON).pertag).curtag as usize]
                    [(*state.SELMON).sellt as usize];
            (*state.SELMON).lt[((*state.SELMON).sellt ^ 1) as usize] =
                (*(*state.SELMON).pertag).ltidxs
                    [(*(*state.SELMON).pertag).curtag as usize]
                    [((*state.SELMON).sellt ^ 1) as usize];

            if (*state.SELMON).showbar
                != (*(*state.SELMON).pertag).showbars
                    [(*(*state.SELMON).pertag).curtag as usize]
            {
                togglebar(state, null_mut());
            }

            focus(state, null_mut());
            arrange(state, state.SELMON);
        }
    }
}

pub(crate) fn quit(_state: &mut State, _arg: *const Arg) {
    unsafe {
        crate::RUNNING = false;
    }
}

// these are shared between movemouse and resizemouse
const CONFIGURE_REQUEST: i32 = ConfigureRequest;
const EXPOSE: i32 = Expose;
const MAP_REQUEST: i32 = MapRequest;
const MOTION_NOTIFY: i32 = MotionNotify;

pub(crate) fn movemouse(state: &mut State, _arg: *const Arg) {
    log::trace!("movemouse");
    unsafe {
        let c = (*state.SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c; // reborrow now that it's not null
        if c.isfullscreen {
            return; // no support for moving fullscreen windows with mouse
        }
        restack(state, state.SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if XGrabPointer(
            state.dpy,
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
        if getrootptr(state, &mut x, &mut y) == 0 {
            return;
        }
        // nil init?
        let mut ev = XEvent { type_: 0 };
        let mut lasttime = 0;

        // emulating do-while
        loop {
            XMaskEvent(
                state.dpy,
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
                    if ((*state.SELMON).wx - nx).abs() < CONFIG.snap as c_int {
                        nx = (*state.SELMON).wx;
                    } else if (((*state.SELMON).wx + (*state.SELMON).ww)
                        - (nx + width(c)))
                    .abs()
                        < CONFIG.snap as c_int
                    {
                        nx = (*state.SELMON).wx + (*state.SELMON).ww - width(c);
                    }
                    if ((*state.SELMON).wy - ny).abs() < CONFIG.snap as c_int {
                        ny = (*state.SELMON).wy;
                    } else if (((*state.SELMON).wy + (*state.SELMON).wh)
                        - (ny + height(c)))
                    .abs()
                        < CONFIG.snap as c_int
                    {
                        ny =
                            (*state.SELMON).wy + (*state.SELMON).wh - height(c);
                    }
                    if !c.isfloating
                        && (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nx - c.x).abs() > CONFIG.snap as c_int
                            || (ny - c.y).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
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
        XUngrabPointer(state.dpy, CurrentTime);
        let m = recttomon(state, c.x, c.y, c.w, c.h);
        if m != state.SELMON {
            sendmon(state, c, m);
            state.SELMON = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn resizemouse(state: &mut State, _arg: *const Arg) {
    log::trace!("resizemouse");
    unsafe {
        let c = (*state.SELMON).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c;
        if c.isfullscreen {
            return; // no support for resizing fullscreen window with mouse
        }
        restack(state, state.SELMON);
        let ocx = c.x;
        let ocy = c.y;
        if XGrabPointer(
            state.dpy,
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
            state.dpy,
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
                state.dpy,
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
                    if (*c.mon).wx + nw >= (*state.SELMON).wx
                        && (*c.mon).wx + nw
                            <= (*state.SELMON).wx + (*state.SELMON).ww
                        && (*c.mon).wy + nh >= (*state.SELMON).wy
                        && (*c.mon).wy + nh
                            <= (*state.SELMON).wy + (*state.SELMON).wh
                        && !c.isfloating
                        && (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nw - c.w).abs() > CONFIG.snap as c_int
                            || (nh - c.h).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.SELMON).lt[(*state.SELMON).sellt as usize])
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
            state.dpy,
            XNONE as u64,
            c.win,
            0,
            0,
            0,
            0,
            c.w + c.bw - 1,
            c.h + c.bw - 1,
        );
        XUngrabPointer(state.dpy, CurrentTime);
        while XCheckMaskEvent(state.dpy, EnterWindowMask, &mut ev) != 0 {}
        let m = recttomon(state, c.x, c.y, c.w, c.h);
        if m != state.SELMON {
            sendmon(state, c, m);
            state.SELMON = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn spawn(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut argv = (*arg).v();
        if argv == *CONFIG.dmenucmd {
            log::trace!("spawn: dmenucmd on monitor {}", (*state.SELMON).num);
            argv.push("-m".into());
            argv.push((*state.SELMON).num.to_string());
        }

        (*state.SELMON).tagset[(*state.SELMON).seltags as usize] &=
            !*SCRATCHTAG;

        let mut cmd = Command::new(argv[0].clone());
        let cmd = if argv.len() > 1 { cmd.args(&argv[1..]) } else { &mut cmd };

        let Ok(_) = cmd.spawn() else {
            panic!("rwm: spawn '{:?}' failed", argv[0]);
        };
    }
}

/// Move the current window to the tag specified by `arg.ui`.
pub(crate) fn toggletag(state: &mut State, arg: *const Arg) {
    unsafe {
        if (*state.SELMON).sel.is_null() {
            return;
        }
        let newtags = (*(*state.SELMON).sel).tags ^ ((*arg).ui() & *TAGMASK);
        if newtags != 0 {
            (*(*state.SELMON).sel).tags = newtags;
            focus(state, null_mut());
            arrange(state, state.SELMON);
        }
    }
}

/// Toggle fullscreen for a window.
///
/// adapted from: https://old.reddit.com/r/dwm/comments/avhkgb/fullscreen_mode/
/// for fixing problems with steam games
pub(crate) fn fullscreen(state: &mut State, _: *const Arg) {
    unsafe {
        if (*state.SELMON).sel.is_null() {
            return;
        }
        setfullscreen(
            state,
            (*state.SELMON).sel,
            !(*(*state.SELMON).sel).isfullscreen,
        )
    }
}

pub(crate) fn togglescratch(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client;
        let mut found = false;
        cfor!((
        c = (*state.SELMON).clients;
        !c.is_null();
        c = (*c).next) {
            found = ((*c).tags & *SCRATCHTAG) != 0;
            if found {
                break;
            }
        });
        if found {
            let newtagset = (*state.SELMON).tagset
                [(*state.SELMON).seltags as usize]
                ^ *SCRATCHTAG;
            if newtagset != 0 {
                (*state.SELMON).tagset[(*state.SELMON).seltags as usize] =
                    newtagset;
                focus(state, null_mut());
                arrange(state, state.SELMON);
            }
            if is_visible(c) {
                focus(state, c);
                restack(state, state.SELMON);
            }
        } else {
            spawn(state, arg);
        }
    }
}
