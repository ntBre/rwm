use std::cmp::max;
use std::ffi::c_int;
use std::process::Command;
use std::ptr::null_mut;

use x11::xlib::{
    ButtonRelease, CWY, ConfigureRequest, CurrentTime, DestroyAll,
    EnterWindowMask, Expose, ExposureMask, False, GrabModeAsync, GrabSuccess,
    MapRequest, MotionNotify, NoEventMask, SubstructureRedirectMask,
    XCheckMaskEvent, XConfigureWindow, XEvent, XGrabPointer, XGrabServer,
    XKillClient, XMaskEvent, XSetCloseDownMode, XSetErrorHandler, XSync,
    XUngrabPointer, XUngrabServer, XWarpPointer, XWindowChanges,
};

use crate::core::{
    HANDLER, MOUSEMASK, XNONE, arrange, attach, attachstack, detach,
    detachstack, drawbar, focus, getrootptr, height, is_visible, nexttiled,
    pop, recttomon, resize, resizebarwin, restack, sendevent, setfullscreen,
    unfocus, updatebarpos, width, xerror, xerrordummy,
};
use crate::enums::WM;
use crate::{Arg, Client, Monitor};
use crate::{State, cfor};

pub(crate) fn togglebar(state: &mut State, _arg: *const Arg) {
    unsafe {
        let monitor = &mut *state.selmon;
        monitor.pertag.showbars[monitor.pertag.curtag] = !monitor.showbar;
        monitor.showbar = monitor.pertag.showbars[monitor.pertag.curtag];
        updatebarpos(state, state.selmon);
        resizebarwin(state, state.selmon);
        if state.config.showsystray {
            let mut wc = XWindowChanges {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                border_width: 0,
                sibling: 0,
                stack_mode: 0,
            };
            if !(monitor).showbar {
                wc.y = -state.bh;
            } else if (monitor).showbar {
                wc.y = 0;
                if !(monitor).topbar {
                    wc.y = (monitor).mh - state.bh;
                }
            }
            XConfigureWindow(
                state.dpy,
                state.systray().win,
                CWY as u32,
                &mut wc,
            );
        }
        arrange(state, state.selmon);
    }
}

pub(crate) fn focusstack(state: &mut State, arg: *const Arg) {
    unsafe {
        let mut c: *mut Client = null_mut();
        let mut i: *mut Client;

        if (*state.selmon).sel.is_null()
            || ((*(*state.selmon).sel).isfullscreen
                && state.config.lock_fullscreen)
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
        let monitor = &mut *state.selmon;
        monitor.pertag.nmasters[monitor.pertag.curtag] =
            std::cmp::max(monitor.nmaster + (*arg).i(), 0);
        monitor.nmaster = monitor.pertag.nmasters[monitor.pertag.curtag];
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
            || (*(*state.selmon).lt[(*state.selmon).sellt])
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
        let monitor = &mut *state.selmon;
        monitor.pertag.mfacts[monitor.pertag.curtag] = f;
        monitor.mfact = monitor.pertag.mfacts[monitor.pertag.curtag];
        arrange(state, state.selmon);
    }
}

/// Move the selected window to the master area. The current master is pushed to
/// the top of the stack.
pub(crate) fn zoom(state: &mut State, _arg: *const Arg) {
    unsafe {
        let mut c = (*state.selmon).sel;
        if (*(*state.selmon).lt[(*state.selmon).sellt])
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
        if (*arg).ui() & state.tagmask()
            == (*state.selmon).tagset[(*state.selmon).seltags]
        {
            return;
        }
        (*state.selmon).seltags ^= 1; // toggle sel tagset

        // Safety: we were gonna dereference it anyway
        let pertag = &mut (*state.selmon).pertag;
        if ((*arg).ui() & state.tagmask()) != 0 {
            (*state.selmon).tagset[(*state.selmon).seltags] =
                (*arg).ui() & state.tagmask();
            pertag.prevtag = pertag.curtag;

            if (*arg).ui() == !0 {
                pertag.curtag = 0;
            } else {
                let mut i;
                cfor!((i = 0; ((*arg).ui() & (1 << i)) == 0; i += 1) {});
                pertag.curtag = i + 1;
            }
        } else {
            std::mem::swap(&mut pertag.prevtag, &mut pertag.curtag);
        }

        (*state.selmon).nmaster = pertag.nmasters[pertag.curtag];
        (*state.selmon).mfact = pertag.mfacts[pertag.curtag];
        (*state.selmon).sellt = pertag.sellts[pertag.curtag];
        (*state.selmon).lt[(*state.selmon).sellt] =
            pertag.ltidxs[pertag.curtag][(*state.selmon).sellt];
        (*state.selmon).lt[(*state.selmon).sellt ^ 1] =
            pertag.ltidxs[pertag.curtag][(*state.selmon).sellt ^ 1];

        if (*state.selmon).showbar != pertag.showbars[pertag.curtag] {
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

        if !sendevent(
            state,
            (*(*state.selmon).sel).win,
            state.wmatom[WM::Delete as usize],
            NoEventMask as i32,
            state.wmatom[WM::Delete as usize] as i64,
            CurrentTime as i64,
            0,
            0,
            0,
        ) {
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
        let monitor = &mut *state.selmon;
        if arg.is_null()
            || (*arg).l().is_none()
            || !std::ptr::eq(
                &state.config.layouts[(*arg).l().unwrap()],
                monitor.lt[monitor.sellt],
            )
        {
            monitor.pertag.sellts[monitor.pertag.curtag] ^= 1;
            monitor.sellt = monitor.pertag.sellts[monitor.pertag.curtag];
        }
        if !arg.is_null() && (*arg).l().is_some() {
            monitor.pertag.ltidxs[monitor.pertag.curtag][monitor.sellt] =
                &state.config.layouts[(*arg).l().unwrap()];
            monitor.lt[monitor.sellt] =
                monitor.pertag.ltidxs[monitor.pertag.curtag][monitor.sellt];
        }
        monitor.ltsymbol = (*monitor.lt[monitor.sellt]).symbol.clone();
        if !monitor.sel.is_null() {
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
            || (*(*state.selmon).sel).isfixed;
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
        if n % m < 0 { (n % m) + m } else { n % m }
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
        if !(*state.selmon).sel.is_null() && (*arg).ui() & state.tagmask() != 0
        {
            (*(*state.selmon).sel).tags = (*arg).ui() & state.tagmask();
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
                m = state.mons;
            }
        } else if state.selmon == state.mons {
            cfor!((m = state.mons; !(*m).next.is_null(); m = (*m).next) {});
        } else {
            cfor!((m = state.mons; (*m).next != state.selmon; m = (*m).next) {});
        }
        m
    }
}

pub(crate) fn focusmon(state: &mut State, arg: *const Arg) {
    unsafe {
        if (*state.mons).next.is_null() {
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
        (*c).tags = (*m).tagset[(*m).seltags];
        attach(c);
        attachstack(c);
        focus(state, null_mut());
        arrange(state, null_mut());
    }
}

pub(crate) fn tagmon(state: &mut State, arg: *const Arg) {
    unsafe {
        if (*state.selmon).sel.is_null() || (*state.mons).next.is_null() {
            return;
        }
        sendmon(state, (*state.selmon).sel, dirtomon(state, (*arg).i()));
    }
}

pub(crate) fn toggleview(state: &mut State, arg: *const Arg) {
    unsafe {
        let monitor = &mut *state.selmon;
        let newtagset =
            monitor.tagset[monitor.seltags] ^ ((*arg).ui() & state.tagmask());

        if newtagset != 0 {
            monitor.tagset[monitor.seltags] = newtagset;

            if newtagset == !0 {
                monitor.pertag.prevtag = monitor.pertag.curtag;
                monitor.pertag.curtag = 0;
            }

            // test if the user did not select the same tag
            if (newtagset & (1 << (monitor.pertag.curtag - 1))) == 0 {
                monitor.pertag.prevtag = monitor.pertag.curtag;
                let mut i;
                cfor!((i = 0; (newtagset & (1 << i)) == 0; i += 1) {});
                monitor.pertag.curtag = i + 1;
            }

            // apply settings for this view
            monitor.nmaster = monitor.pertag.nmasters[monitor.pertag.curtag];
            monitor.mfact = monitor.pertag.mfacts[monitor.pertag.curtag];
            monitor.sellt = monitor.pertag.sellts[monitor.pertag.curtag];
            monitor.lt[monitor.sellt] =
                monitor.pertag.ltidxs[monitor.pertag.curtag][monitor.sellt];
            monitor.lt[monitor.sellt ^ 1] =
                monitor.pertag.ltidxs[monitor.pertag.curtag][monitor.sellt ^ 1];

            if monitor.showbar != monitor.pertag.showbars[monitor.pertag.curtag]
            {
                togglebar(state, null_mut());
            }

            focus(state, null_mut());
            arrange(state, state.selmon);
        }
    }
}

pub(crate) fn quit(state: &mut State, _arg: *const Arg) {
    state.running = false;
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
            state.root,
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
                    if ((*state.selmon).wx - nx).abs()
                        < state.config.snap as c_int
                    {
                        nx = (*state.selmon).wx;
                    } else if (((*state.selmon).wx + (*state.selmon).ww)
                        - (nx + width(c)))
                    .abs()
                        < state.config.snap as c_int
                    {
                        nx = (*state.selmon).wx + (*state.selmon).ww - width(c);
                    }
                    if ((*state.selmon).wy - ny).abs()
                        < state.config.snap as c_int
                    {
                        ny = (*state.selmon).wy;
                    } else if (((*state.selmon).wy + (*state.selmon).wh)
                        - (ny + height(c)))
                    .abs()
                        < state.config.snap as c_int
                    {
                        ny =
                            (*state.selmon).wy + (*state.selmon).wh - height(c);
                    }
                    if !c.isfloating
                        && (*(*state.selmon).lt[(*state.selmon).sellt])
                            .arrange
                            .is_some()
                        && ((nx - c.x).abs() > state.config.snap as c_int
                            || (ny - c.y).abs() > state.config.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.selmon).lt[(*state.selmon).sellt])
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
            state.root,
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
                        && (*(*state.selmon).lt[(*state.selmon).sellt])
                            .arrange
                            .is_some()
                        && ((nw - c.w).abs() > state.config.snap as c_int
                            || (nh - c.h).abs() > state.config.snap as c_int)
                    {
                        togglefloating(state, null_mut());
                    }
                    if (*(*state.selmon).lt[(*state.selmon).sellt])
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
        if argv == *state.config.dmenucmd {
            log::trace!("spawn: dmenucmd on monitor {}", (*state.selmon).num);
            argv.push("-m".into());
            argv.push((*state.selmon).num.to_string());
        }

        (*state.selmon).tagset[(*state.selmon).seltags] &= !state.scratchtag();

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
        let newtags =
            (*(*state.selmon).sel).tags ^ ((*arg).ui() & state.tagmask());
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
            found = ((*c).tags & state.scratchtag()) != 0;
            if found {
                break;
            }
        });
        if found {
            let newtagset = (*state.selmon).tagset[(*state.selmon).seltags]
                ^ state.scratchtag();
            if newtagset != 0 {
                (*state.selmon).tagset[(*state.selmon).seltags] = newtagset;
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
