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
        (*(*state.selmon).pertag).showbars
            [(*(*state.selmon).pertag).curtag as usize] =
            !((*state.selmon).showbar);
        (*state.selmon).showbar = (*(*state.selmon).pertag).showbars
            [(*(*state.selmon).pertag).curtag as usize];
        updatebarpos(state, state.selmon);
        resizebarwin(state, state.selmon);
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
            if !(*state.selmon).showbar {
                wc.y = -state.bh;
            } else if (*state.selmon).showbar {
                wc.y = 0;
                if !(*state.selmon).topbar {
                    wc.y = (*state.selmon).mh - state.bh;
                }
            }
            XConfigureWindow(state.dpy, (*SYSTRAY).win, CWY as u32, &mut wc);
        }
        arrange(state, state.selmon);
    }
}

pub(crate) fn focusstack(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        if (*state.selmon).sel.is_null()
            || ((*(*state.selmon).sel).isfullscreen && CONFIG.lock_fullscreen)
        {
            return;
        }
        if (*arg).i() > 0 {
            cfor!((c = (*(*state.selmon).sel).next; !c.is_null() && !is_visible(c); c = (*c).next) {});
            if c.is_null() {
                cfor!((c = (*state.selmon).clients; !c.is_null() && !is_visible(c); c = (*c).next) {});
            }
        } else {
            cfor!((i = (*state.selmon).clients; i != (*state.selmon).sel; i = (*i).next) {
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
            restack(state, state.selmon);
        }
    }
}

/// Increase the number of windows in the master area.
pub(crate) fn incnmaster(state: &mut State, arg: *const Arg) {
    unsafe {
        (*(*state.selmon).pertag).nmasters
            [(*(*state.selmon).pertag).curtag as usize] =
            std::cmp::max((*state.selmon).nmaster + (*arg).i(), 0);
        (*state.selmon).nmaster = (*(*state.selmon).pertag).nmasters
            [(*(*state.selmon).pertag).curtag as usize];
        arrange(state, state.selmon);
    }
}

/// Set the fraction of the screen occupied by the master window. An `arg.f`
/// greater than 1.0 sets the fraction absolutely, while fractional values add
/// to the current value. Total values are restricted to the range [0.05, 0.95]
/// to leave at least 5% of the screen for other windows.
pub(crate) fn setmfact(state: &mut State, arg: *const Arg) {
    unsafe {
        if arg.is_null()
            || (*(*state.selmon).lt[(*state.selmon).sellt as usize])
                .arrange
                .is_none()
        {
            return;
        }
        let f = if (*arg).f() < 1.0 {
            (*arg).f() + (*state.selmon).mfact
        } else {
            (*arg).f() - 1.0
        };
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        (*(*state.selmon).pertag).mfacts
            [(*(*state.selmon).pertag).curtag as usize] = f;
        (*state.selmon).mfact = (*(*state.selmon).pertag).mfacts
            [(*(*state.selmon).pertag).curtag as usize];
        arrange(state, state.selmon);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) fn zoom(state: &mut State, _arg: *const Arg) {
    unsafe {
        let mut c = (*state.selmon).sel;
        if (*(*state.selmon).lt[(*state.selmon).sellt as usize])
            .arrange
            .is_none()
            || c.is_null()
            || (*c).isfloating
        {
            return;
        }
        if c == nexttiled((*state.selmon).clients) {
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
            == (*state.selmon).tagset[(*state.selmon).seltags as usize]
        {
            return;
        }
        (*state.selmon).seltags ^= 1; // toggle sel tagset

        // Safety: we were gonna dereference it anyway
        let pertag = &mut *(*state.selmon).pertag;
        if ((*arg).ui() & *TAGMASK) != 0 {
            (*state.selmon).tagset[(*state.selmon).seltags as usize] =
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

        (*state.selmon).nmaster = pertag.nmasters[pertag.curtag as usize];
        (*state.selmon).mfact = pertag.mfacts[pertag.curtag as usize];
        (*state.selmon).sellt = pertag.sellts[pertag.curtag as usize];
        (*state.selmon).lt[(*state.selmon).sellt as usize] = pertag.ltidxs
            [pertag.curtag as usize][(*state.selmon).sellt as usize];
        (*state.selmon).lt[((*state.selmon).sellt ^ 1) as usize] = pertag
            .ltidxs[pertag.curtag as usize]
            [((*state.selmon).sellt ^ 1) as usize];

        if (*state.selmon).showbar != pertag.showbars[pertag.curtag as usize] {
            togglebar(state, null_mut());
        }

        focus(state, null_mut());
        arrange(state, state.selmon);
    }
}

pub(crate) fn killclient(state: &mut State, _arg: *const Arg) {
    unsafe {
        if (*state.selmon).sel.is_null() {
            return;
        }

        if sendevent(
            state,
            (*(*state.selmon).sel).win,
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
            XKillClient(state.dpy, (*(*state.selmon).sel).win);
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
                (*state.selmon).lt[(*state.selmon).sellt as usize],
            )
        {
            (*(*state.selmon).pertag).sellts
                [(*(*state.selmon).pertag).curtag as usize] ^= 1;
            (*state.selmon).sellt = (*(*state.selmon).pertag).sellts
                [(*(*state.selmon).pertag).curtag as usize];
        }
        if !arg.is_null() && (*arg).l().is_some() {
            (*(*state.selmon).pertag).ltidxs
                [(*(*state.selmon).pertag).curtag as usize]
                [(*state.selmon).sellt as usize] =
                &CONFIG.layouts[(*arg).l().unwrap()];
            (*state.selmon).lt[(*state.selmon).sellt as usize] =
                (*(*state.selmon).pertag).ltidxs
                    [(*(*state.selmon).pertag).curtag as usize]
                    [(*state.selmon).sellt as usize];
        }
        libc::strncpy(
            (*state.selmon).ltsymbol.as_mut_ptr(),
            (*(*state.selmon).lt[(*state.selmon).sellt as usize]).symbol,
            size_of_val(&(*state.selmon).ltsymbol),
        );
        if !(*state.selmon).sel.is_null() {
            arrange(state, state.selmon);
        } else {
            drawbar(state, state.selmon);
        }
    }
}

pub(crate) fn togglefloating(state: &mut State, _arg: *const Arg) {
    log::trace!("togglefloating: {_arg:?}");
    unsafe {
        if (*state.selmon).sel.is_null() {
            return;
        }
        if (*(*state.selmon).sel).isfullscreen {
            // no support for fullscreen windows
            return;
        }
        (*(*state.selmon).sel).isfloating = !(*(*state.selmon).sel).isfloating
            || (*(*state.selmon).sel).isfixed != 0;
        if (*(*state.selmon).sel).isfloating {
            let sel = &mut *(*state.selmon).sel;
            resize(state, sel, sel.x, sel.y, sel.w, sel.h, 0);
        }
        arrange(state, state.selmon);
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
        if (*state.selmon).clients.is_null() {
            return;
        }
        if (*state.selmon).sel.is_null() {
            return;
        }
        let mut i;
        let mut c;
        cfor!((
            (i, c) = (0, (*state.selmon).clients);
            c != (*state.selmon).sel;
            (i, c) = (i + is_visible(c) as c_int, (*c).next)) {});
        let mut n;
        cfor!((n = i; !c.is_null(); (n, c) = (n + is_visible(c) as c_int, (*c).next)) {});
        let mut stackpos = modulo(i + (*arg).i(), n);
        // end stackpos

        let sel = (*state.selmon).sel;
        match stackpos.cmp(&0) {
            std::cmp::Ordering::Less => return,
            std::cmp::Ordering::Equal => {
                detach(sel);
                attach(sel);
            }
            std::cmp::Ordering::Greater => {
                let mut p;
                cfor!((
                (p, c) = (null_mut(), (*state.selmon).clients);
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
        arrange(state, state.selmon);
    }
}

pub(crate) fn tag(state: &mut State, arg: *const Arg) {
    unsafe {
        if !(*state.selmon).sel.is_null() && (*arg).ui() & *TAGMASK != 0 {
            (*(*state.selmon).sel).tags = (*arg).ui() & *TAGMASK;
            focus(state, null_mut());
            arrange(state, state.selmon);
        }
    }
}

fn dirtomon(state: &State, dir: i32) -> *mut Monitor {
    unsafe {
        let mut m;

        if dir > 0 {
            m = (*state.selmon).next;
            if m.is_null() {
                m = MONS;
            }
        } else if state.selmon == MONS {
            cfor!((m = MONS; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = MONS; (*m).next != state.selmon; m = (*m).next) {});
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
        if m == state.selmon {
            return;
        }
        unfocus(state, (*state.selmon).sel, false);
        state.selmon = m;
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
        if (*state.selmon).sel.is_null() || (*MONS).next.is_null() {
            return;
        }
        sendmon(state, (*state.selmon).sel, dirtomon(state, (*arg).i()));
    }
}

pub(crate) fn toggleview(state: &mut State, arg: *const Arg) {
    unsafe {
        let newtagset = (*state.selmon).tagset
            [(*state.selmon).seltags as usize]
            ^ ((*arg).ui() & *TAGMASK);

        if newtagset != 0 {
            (*state.selmon).tagset[(*state.selmon).seltags as usize] =
                newtagset;

            if newtagset == !0 {
                (*(*state.selmon).pertag).prevtag =
                    (*(*state.selmon).pertag).curtag;
                (*(*state.selmon).pertag).curtag = 0;
            }

            // test if the user did not select the same tag
            if (newtagset & 1 << ((*(*state.selmon).pertag).curtag - 1)) == 0 {
                (*(*state.selmon).pertag).prevtag =
                    (*(*state.selmon).pertag).curtag;
                let mut i;
                cfor!((i = 0; (newtagset & 1 << i) == 0; i += 1) {});
                (*(*state.selmon).pertag).curtag = i + 1;
            }

            // apply settings for this view
            (*state.selmon).nmaster = (*(*state.selmon).pertag).nmasters
                [(*(*state.selmon).pertag).curtag as usize];
            (*state.selmon).mfact = (*(*state.selmon).pertag).mfacts
                [(*(*state.selmon).pertag).curtag as usize];
            (*state.selmon).sellt = (*(*state.selmon).pertag).sellts
                [(*(*state.selmon).pertag).curtag as usize];
            (*state.selmon).lt[(*state.selmon).sellt as usize] =
                (*(*state.selmon).pertag).ltidxs
                    [(*(*state.selmon).pertag).curtag as usize]
                    [(*state.selmon).sellt as usize];
            (*state.selmon).lt[((*state.selmon).sellt ^ 1) as usize] =
                (*(*state.selmon).pertag).ltidxs
                    [(*(*state.selmon).pertag).curtag as usize]
                    [((*state.selmon).sellt ^ 1) as usize];

            if (*state.selmon).showbar
                != (*(*state.selmon).pertag).showbars
                    [(*(*state.selmon).pertag).curtag as usize]
            {
                togglebar(state, null_mut());
            }

            focus(state, null_mut());
            arrange(state, state.selmon);
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
        let c = (*state.selmon).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c; // reborrow now that it's not null
        if c.isfullscreen {
            return; // no support for moving fullscreen windows with mouse
        }
        restack(state, state.selmon);
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
                    if ((*state.selmon).wx - nx).abs() < CONFIG.snap as c_int {
                        nx = (*state.selmon).wx;
                    } else if (((*state.selmon).wx + (*state.selmon).ww)
                        - (nx + width(c)))
                    .abs()
                        < CONFIG.snap as c_int
                    {
                        nx = (*state.selmon).wx + (*state.selmon).ww - width(c);
                    }
                    if ((*state.selmon).wy - ny).abs() < CONFIG.snap as c_int {
                        ny = (*state.selmon).wy;
                    } else if (((*state.selmon).wy + (*state.selmon).wh)
                        - (ny + height(c)))
                    .abs()
                        < CONFIG.snap as c_int
                    {
                        ny =
                            (*state.selmon).wy + (*state.selmon).wh - height(c);
                    }
                    if !c.isfloating
                        && (*(*state.selmon).lt[(*state.selmon).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nx - c.x).abs() > CONFIG.snap as c_int
                            || (ny - c.y).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.selmon).lt[(*state.selmon).sellt as usize])
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
        if m != state.selmon {
            sendmon(state, c, m);
            state.selmon = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn resizemouse(state: &mut State, _arg: *const Arg) {
    log::trace!("resizemouse");
    unsafe {
        let c = (*state.selmon).sel;
        if c.is_null() {
            return;
        }
        let c = &mut *c;
        if c.isfullscreen {
            return; // no support for resizing fullscreen window with mouse
        }
        restack(state, state.selmon);
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
                    if (*c.mon).wx + nw >= (*state.selmon).wx
                        && (*c.mon).wx + nw
                            <= (*state.selmon).wx + (*state.selmon).ww
                        && (*c.mon).wy + nh >= (*state.selmon).wy
                        && (*c.mon).wy + nh
                            <= (*state.selmon).wy + (*state.selmon).wh
                        && !c.isfloating
                        && (*(*state.selmon).lt[(*state.selmon).sellt as usize])
                            .arrange
                            .is_some()
                        && ((nw - c.w).abs() > CONFIG.snap as c_int
                            || (nh - c.h).abs() > CONFIG.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.selmon).lt[(*state.selmon).sellt as usize])
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
        if m != state.selmon {
            sendmon(state, c, m);
            state.selmon = m;
            focus(state, null_mut());
        }
    }
}

pub(crate) fn spawn(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut argv = (*arg).v();
        if argv == *CONFIG.dmenucmd {
            log::trace!("spawn: dmenucmd on monitor {}", (*state.selmon).num);
            argv.push("-m".into());
            argv.push((*state.selmon).num.to_string());
        }

        (*state.selmon).tagset[(*state.selmon).seltags as usize] &=
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
        if (*state.selmon).sel.is_null() {
            return;
        }
        let newtags = (*(*state.selmon).sel).tags ^ ((*arg).ui() & *TAGMASK);
        if newtags != 0 {
            (*(*state.selmon).sel).tags = newtags;
            focus(state, null_mut());
            arrange(state, state.selmon);
        }
    }
}

/// Toggle fullscreen for a window.
///
/// adapted from: https://old.reddit.com/r/dwm/comments/avhkgb/fullscreen_mode/
/// for fixing problems with steam games
pub(crate) fn fullscreen(state: &mut State, _: *const Arg) {
    unsafe {
        if (*state.selmon).sel.is_null() {
            return;
        }
        setfullscreen(
            state,
            (*state.selmon).sel,
            !(*(*state.selmon).sel).isfullscreen,
        )
    }
}

pub(crate) fn togglescratch(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client;
        let mut found = false;
        cfor!((
        c = (*state.selmon).clients;
        !c.is_null();
        c = (*c).next) {
            found = ((*c).tags & *SCRATCHTAG) != 0;
            if found {
                break;
            }
        });
        if found {
            let newtagset = (*state.selmon).tagset
                [(*state.selmon).seltags as usize]
                ^ *SCRATCHTAG;
            if newtagset != 0 {
                (*state.selmon).tagset[(*state.selmon).seltags as usize] =
                    newtagset;
                focus(state, null_mut());
                arrange(state, state.selmon);
            }
            if is_visible(c) {
                focus(state, c);
                restack(state, state.selmon);
            }
        } else {
            spawn(state, arg);
        }
    }
}
