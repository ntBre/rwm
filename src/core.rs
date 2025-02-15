use std::cmp::max;
use std::ffi::{c_int, c_uint, c_ulong, CStr};
use std::io::Read;
use std::mem::{size_of, MaybeUninit};
use std::ptr::null_mut;
use std::sync::LazyLock;

use crate::config::Config;
use crate::drw::{fontset_create, Drw};
use crate::enums::{Clk, Col, Net, Scheme, XEmbed, WM};
use crate::key_handlers::view;
use crate::util::{self, ecalloc};
use crate::xembed::{
    XEMBED_EMBEDDED_VERSION, XEMBED_MAPPED, XEMBED_WINDOW_ACTIVATE,
    XEMBED_WINDOW_DEACTIVATE,
};
use crate::{
    drw, handlers, Arg, Client, Layout, Monitor, Pertag, State, Systray,
    Window, ICONIC_STATE, NORMAL_STATE, WITHDRAWN_STATE,
};
use libc::{c_long, c_uchar, pid_t, sigaction};
use x11::keysym::XK_Num_Lock;
use x11::xlib::{
    self, Above, AnyButton, AnyKey, AnyModifier, BadAccess, BadDrawable,
    BadMatch, BadWindow, Below, ButtonPressMask, ButtonReleaseMask,
    CWBackPixel, CWBackPixmap, CWBorderWidth, CWCursor, CWEventMask, CWHeight,
    CWOverrideRedirect, CWSibling, CWStackMode, CWWidth, ClientMessage,
    ControlMask, CopyFromParent, CurrentTime, Display, EnterWindowMask,
    ExposureMask, False, FocusChangeMask, GrabModeAsync, GrabModeSync,
    InputHint, IsViewable, LeaveWindowMask, LockMask, Mod1Mask, Mod2Mask,
    Mod3Mask, Mod4Mask, Mod5Mask, NoEventMask, PAspect, PBaseSize, PMaxSize,
    PMinSize, PResizeInc, PSize, ParentRelative, PointerMotionMask,
    PointerRoot, PropModeAppend, PropModeReplace, PropertyChangeMask,
    RevertToPointerRoot, ShiftMask, StructureNotifyMask,
    SubstructureNotifyMask, SubstructureRedirectMask, Success, True,
    XChangeProperty, XChangeWindowAttributes, XConfigureWindow,
    XCreateSimpleWindow, XDestroyWindow, XErrorEvent, XFillRectangle, XFree,
    XGetSelectionOwner, XInternAtom, XMapRaised, XMapSubwindows, XMapWindow,
    XMoveResizeWindow, XPropertyEvent, XSelectInput, XSetErrorHandler,
    XSetForeground, XSetSelectionOwner, XSetWindowAttributes, XSync,
    XUnmapWindow, XWindowChanges, CWX, CWY, XA_ATOM, XA_CARDINAL, XA_STRING,
    XA_WINDOW, XA_WM_NAME,
};

#[macro_export]
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

/// function to be called on a startup error
pub extern "C" fn xerrorstart(_: *mut Display, _: *mut XErrorEvent) -> c_int {
    panic!("another window manager is already running")
}

// from Xproto.h
pub const X_SET_INPUT_FOCUS: u8 = 42;
pub const X_POLY_TEXT_8: u8 = 74;
pub const X_POLY_FILL_RECTANGLE: u8 = 70;
pub const X_POLY_SEGMENT: u8 = 66;
pub const X_CONFIGURE_WINDOW: u8 = 12;
pub const X_GRAB_BUTTON: u8 = 28;
pub const X_GRAB_KEY: u8 = 33;
pub const X_COPY_AREA: u8 = 62;

// from cursorfont.h
pub const XC_LEFT_PTR: u8 = 68;
pub const XC_SIZING: u8 = 120;
pub const XC_FLEUR: u8 = 52;

// from X.h
// const BUTTON_RELEASE: i32 = 5;
pub const XNONE: c_long = 0;

/// Unfortunately this can't be packed into the State struct since it needs to
/// be accessed here in `xerror`, where I don't get a chance to pass State.
///
/// What's going on here is that `checkotherwm` calls `XSetErrorHandler` to set
/// the handler temporarily to `xerrorstart`. `XSetErrorHandler` returns the
/// previous handler fn, which we store here. At the end of `checkotherwm`, we
/// then set the error handler to `xerror`, which, via `XERRORLIB`, is just a
/// wrapper around the original X error handler with a little logging and an
/// early return to allow certain kinds of errors.
///
/// Obviously it would be nice to handle this with a closure in `checkotherwm`,
/// but `XSetErrorHandler` requires an `unsafe extern "C" fn`, not any old Fn.
pub static mut XERRORXLIB: Option<
    unsafe extern "C" fn(*mut Display, *mut XErrorEvent) -> i32,
> = None;

/// # Safety
pub unsafe extern "C" fn xerror(
    mdpy: *mut Display,
    ee: *mut XErrorEvent,
) -> c_int {
    unsafe {
        let e = *ee;
        if e.error_code == BadWindow
            || (e.request_code == X_SET_INPUT_FOCUS && e.error_code == BadMatch)
            || (e.request_code == X_POLY_TEXT_8 && e.error_code == BadDrawable)
            || (e.request_code == X_POLY_FILL_RECTANGLE
                && e.error_code == BadDrawable)
            || (e.request_code == X_POLY_SEGMENT && e.error_code == BadDrawable)
            || (e.request_code == X_CONFIGURE_WINDOW
                && e.error_code == BadMatch)
            || (e.request_code == X_GRAB_BUTTON && e.error_code == BadAccess)
            || (e.request_code == X_GRAB_KEY && e.error_code == BadAccess)
            || (e.request_code == X_COPY_AREA && e.error_code == BadDrawable)
        {
            return 0;
        }
        eprintln!(
            "rwm: fatal error: request code={}, error code={}",
            e.request_code, e.error_code
        );
        (XERRORXLIB.unwrap())(mdpy, ee)
    }
}

pub extern "C" fn xerrordummy(
    _dpy: *mut Display,
    _ee: *mut xlib::XErrorEvent,
) -> c_int {
    0
}

const BROKEN: &CStr = c"broken";

type Atom = c_ulong;

pub fn createmon(state: &State) -> *mut Monitor {
    log::trace!("createmon");

    // I thought about trying to create a Monitor directly, followed by
    // Box::into_raw(Box::new(m)), but we use libc::free to free the Monitors
    // later. I'd have to replace that with Box::from_raw and allow it to drop
    // for that to work I think.
    let m: *mut Monitor = ecalloc(1, size_of::<Monitor>()).cast();

    unsafe {
        (*m).tagset[0] = 1;
        (*m).tagset[1] = 1;
        (*m).mfact = state.config.mfact;
        (*m).nmaster = state.config.nmaster;
        (*m).showbar = state.config.showbar;
        (*m).topbar = state.config.topbar;
        (*m).lt[0] = &state.config.layouts[0];
        (*m).lt[1] = &state.config.layouts[1 % state.config.layouts.len()];
        (*m).ltsymbol = state.config.layouts[0].symbol.clone();

        (*m).pertag = Pertag {
            curtag: 1,
            prevtag: 1,
            nmasters: vec![(*m).nmaster; state.config.tags.len() + 1],
            mfacts: vec![(*m).mfact; state.config.tags.len() + 1],
            sellts: vec![(*m).sellt; state.config.tags.len() + 1],
            ltidxs: vec![(*m).lt; state.config.tags.len() + 1],
            showbars: vec![(*m).showbar; state.config.tags.len() + 1],
        };
    }

    m
}

pub fn checkotherwm(dpy: *mut Display) {
    log::trace!("checkotherwm");
    unsafe {
        XERRORXLIB = XSetErrorHandler(Some(xerrorstart));
        xlib::XSelectInput(
            dpy,
            xlib::XDefaultRootWindow(dpy),
            SubstructureRedirectMask,
        );
        XSetErrorHandler(Some(xerror));
        xlib::XSync(dpy, False);
    }
}

pub fn setup(dpy: *mut Display) -> State {
    log::trace!("setup");
    unsafe {
        let mut wa = xlib::XSetWindowAttributes {
            background_pixmap: 0,
            background_pixel: 0,
            border_pixmap: 0,
            border_pixel: 0,
            bit_gravity: 0,
            win_gravity: 0,
            backing_store: 0,
            backing_planes: 0,
            backing_pixel: 0,
            save_under: 0,
            event_mask: 0,
            do_not_propagate_mask: 0,
            override_redirect: 0,
            colormap: 0,
            cursor: 0,
        };
        let mut sa = sigaction {
            sa_sigaction: libc::SIG_IGN,
            sa_mask: std::mem::zeroed(),
            sa_flags: libc::SA_NOCLDSTOP
                | libc::SA_NOCLDWAIT
                | libc::SA_RESTART,
            #[cfg(not(target_os = "macos"))]
            sa_restorer: None,
        };
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGCHLD, &sa, null_mut());

        while libc::waitpid(-1, null_mut(), libc::WNOHANG) > 0 {}

        let screen = xlib::XDefaultScreen(dpy);
        let sh = xlib::XDisplayHeight(dpy, screen);
        let root = xlib::XRootWindow(dpy, screen);
        let sw = xlib::XDisplayWidth(dpy, screen);
        let mut drw = drw::create(dpy, screen, root, sw as u32, sh as u32);
        let config = Config::load_home();
        if fontset_create(&mut drw, &config.fonts).is_err()
            || drw.fonts.is_empty()
        {
            panic!("no fonts could be loaded");
        }

        /* init cursors */
        let cursors = crate::Cursors {
            normal: drw::cur_create(&drw, XC_LEFT_PTR as i32),
            resize: drw::cur_create(&drw, XC_SIZING as i32),
            move_: drw::cur_create(&drw, XC_FLEUR as i32),
        };
        let mut state = State {
            bh: drw.fonts[0].h as i32 + 2,
            sw,
            sh,
            cursors,
            wmatom: Default::default(),
            netatom: Default::default(),
            xatom: Default::default(),
            dpy,
            lrpad: drw.fonts[0].h as i32,
            drw,
            selmon: null_mut(),
            mons: null_mut(),
            stext: String::new(),
            scheme: Default::default(),
            screen,
            root,
            wmcheckwin: xlib::XCreateSimpleWindow(
                dpy, root, 0, 0, 1, 1, 0, 0, 0,
            ),
            numlockmask: 0,
            running: true,
            systray: None,
            config,

            #[cfg(target_os = "linux")]
            xcon: null_mut(),
        };

        updategeom(&mut state);

        /* init atoms */
        let utf8string = XInternAtom(state.dpy, c"UTF8_STRING".as_ptr(), False);
        state.wmatom[WM::Protocols as usize] =
            XInternAtom(state.dpy, c"WM_PROTOCOLS".as_ptr(), False);
        state.wmatom[WM::Delete as usize] =
            XInternAtom(state.dpy, c"WM_DELETE_WINDOW".as_ptr(), False);
        state.wmatom[WM::State as usize] =
            XInternAtom(state.dpy, c"WM_STATE".as_ptr(), False);
        state.wmatom[WM::TakeFocus as usize] =
            XInternAtom(state.dpy, c"WM_TAKE_FOCUS".as_ptr(), False);

        state.netatom[Net::ActiveWindow as usize] =
            XInternAtom(state.dpy, c"_NET_ACTIVE_WINDOW".as_ptr(), False);
        state.netatom[Net::Supported as usize] =
            XInternAtom(state.dpy, c"_NET_SUPPORTED".as_ptr(), False);

        state.netatom[Net::SystemTray as usize] =
            XInternAtom(state.dpy, c"_NET_SYSTEM_TRAY_S0".as_ptr(), False);
        state.netatom[Net::SystemTrayOP as usize] =
            XInternAtom(state.dpy, c"_NET_SYSTEM_TRAY_OPCODE".as_ptr(), False);
        state.netatom[Net::SystemTrayOrientation as usize] = XInternAtom(
            state.dpy,
            c"_NET_SYSTEM_TRAY_ORIENTATION".as_ptr(),
            False,
        );
        state.netatom[Net::SystemTrayOrientationHorz as usize] = XInternAtom(
            state.dpy,
            c"_NET_SYSTEM_TRAY_ORIENTATION_HORZ".as_ptr(),
            False,
        );

        state.netatom[Net::WMName as usize] =
            XInternAtom(state.dpy, c"_NET_WM_NAME".as_ptr(), False);
        state.netatom[Net::WMState as usize] =
            XInternAtom(state.dpy, c"_NET_WM_STATE".as_ptr(), False);
        state.netatom[Net::WMCheck as usize] =
            XInternAtom(state.dpy, c"_NET_SUPPORTING_WM_CHECK".as_ptr(), False);
        state.netatom[Net::WMFullscreen as usize] =
            XInternAtom(state.dpy, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), False);
        state.netatom[Net::WMWindowType as usize] =
            XInternAtom(state.dpy, c"_NET_WM_WINDOW_TYPE".as_ptr(), False);
        state.netatom[Net::WMWindowTypeDialog as usize] = XInternAtom(
            state.dpy,
            c"_NET_WM_WINDOW_TYPE_DIALOG".as_ptr(),
            False,
        );
        state.netatom[Net::ClientList as usize] =
            XInternAtom(state.dpy, c"_NET_CLIENT_LIST".as_ptr(), False);

        state.xatom[XEmbed::Manager as usize] =
            XInternAtom(state.dpy, c"MANAGER".as_ptr(), False);
        state.xatom[XEmbed::XEmbed as usize] =
            XInternAtom(state.dpy, c"_XEMBED".as_ptr(), False);
        state.xatom[XEmbed::XEmbedInfo as usize] =
            XInternAtom(state.dpy, c"_XEMBED_INFO".as_ptr(), False);

        /* init appearance */
        for i in 0..state.config.colors.0.len() {
            state.scheme.push(drw::scm_create(
                &state.drw,
                &state.config.colors.0[i],
                3,
            ));
        }

        // init system tray
        updatesystray(&mut state);

        /* init bars */
        updatebars(&mut state);
        updatestatus(&mut state);

        xlib::XChangeProperty(
            state.dpy,
            state.wmcheckwin,
            state.netatom[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            &raw mut state.wmcheckwin as *mut c_uchar,
            1,
        );
        xlib::XChangeProperty(
            state.dpy,
            state.wmcheckwin,
            state.netatom[Net::WMName as usize],
            utf8string,
            8,
            PropModeReplace,
            c"rwm".as_ptr() as *mut c_uchar,
            3,
        );
        xlib::XChangeProperty(
            state.dpy,
            root,
            state.netatom[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            &raw mut state.wmcheckwin as *mut c_uchar,
            1,
        );
        /* EWMH support per view */
        xlib::XChangeProperty(
            state.dpy,
            root,
            state.netatom[Net::Supported as usize],
            XA_ATOM,
            32,
            PropModeReplace,
            &raw mut state.netatom as *mut c_uchar,
            Net::Last as i32,
        );
        xlib::XDeleteProperty(
            state.dpy,
            root,
            state.netatom[Net::ClientList as usize],
        );

        // /* select events */
        wa.cursor = state.cursors.normal.cursor;
        wa.event_mask = SubstructureRedirectMask
            | SubstructureNotifyMask
            | ButtonPressMask
            | PointerMotionMask
            | EnterWindowMask
            | LeaveWindowMask
            | StructureNotifyMask
            | PropertyChangeMask;
        xlib::XChangeWindowAttributes(
            state.dpy,
            root,
            CWEventMask | CWCursor,
            &mut wa,
        );
        xlib::XSelectInput(state.dpy, root, wa.event_mask);
        grabkeys(&mut state);
        focus(&mut state, null_mut());

        state
    }
}

pub unsafe fn focus(state: &mut State, mut c: *mut Client) {
    log::trace!("focus: c = {c:?}");
    unsafe {
        if c.is_null() || !is_visible(c) {
            c = (*state.selmon).stack;
            while !c.is_null() && !is_visible(c) {
                c = (*c).snext;
            }
        }
        if !(*state.selmon).sel.is_null() && (*state.selmon).sel != c {
            unfocus(state, (*state.selmon).sel, false);
        }
        if !c.is_null() {
            if (*c).mon != state.selmon {
                state.selmon = (*c).mon;
            }
            if (*c).isurgent != 0 {
                seturgent(state, c, false);
            }
            detachstack(c);
            attachstack(c);
            grabbuttons(state, c, true);
            let color = state.scheme[(Scheme::Sel, Col::Border)].pixel;
            xlib::XSetWindowBorder(state.dpy, (*c).win, color);
            setfocus(state, c);
        } else {
            xlib::XSetInputFocus(
                state.dpy,
                state.root,
                RevertToPointerRoot,
                CurrentTime,
            );
            xlib::XDeleteProperty(
                state.dpy,
                state.root,
                state.netatom[Net::ActiveWindow as usize],
            );
        }
        (*state.selmon).sel = c;
        drawbars(state);
    }
}

pub fn drawbars(state: &mut State) {
    log::trace!("drawbars");
    unsafe {
        let mut m = state.mons;
        while !m.is_null() {
            drawbar(state, m);
            m = (*m).next;
        }
    }
}

pub fn setfocus(state: &mut State, c: *mut Client) {
    log::trace!("setfocus");
    unsafe {
        if (*c).neverfocus == 0 {
            xlib::XSetInputFocus(
                state.dpy,
                (*c).win,
                RevertToPointerRoot,
                CurrentTime,
            );
            xlib::XChangeProperty(
                state.dpy,
                state.root,
                state.netatom[Net::ActiveWindow as usize],
                XA_WINDOW,
                32,
                PropModeReplace,
                (&mut (*c).win) as *mut u64 as *mut c_uchar,
                1,
            );
        }
        sendevent(
            state,
            (*c).win,
            state.wmatom[WM::TakeFocus as usize],
            NoEventMask as i32,
            state.wmatom[WM::TakeFocus as usize] as i64,
            CurrentTime as i64,
            0,
            0,
            0,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sendevent(
    state: &mut State,
    w: Window,
    proto: Atom,
    mask: c_int,
    d0: c_long,
    d1: c_long,
    d2: c_long,
    d3: c_long,
    d4: c_long,
) -> c_int {
    log::trace!("sendevent");
    let mut n = 0;
    let mut protocols = std::ptr::null_mut();
    let mt;
    let mut exists = 0;
    unsafe {
        if proto == state.wmatom[WM::TakeFocus as usize]
            || proto == state.wmatom[WM::Delete as usize]
        {
            mt = state.wmatom[WM::Protocols as usize];
            if xlib::XGetWMProtocols(state.dpy, w, &mut protocols, &mut n) != 0
            {
                while exists == 0 && n > 0 {
                    n -= 1;
                    exists = (*protocols.offset(n as isize) == proto) as c_int;
                }
                XFree(protocols.cast());
            }
        } else {
            exists = 1;
            mt = proto;
        }
        if exists != 0 {
            let mut ev = xlib::XEvent { type_: ClientMessage };
            ev.client_message.window = w;
            ev.client_message.message_type = mt;
            ev.client_message.format = 32;
            ev.client_message.data.set_long(0, d0);
            ev.client_message.data.set_long(1, d1);
            ev.client_message.data.set_long(2, d2);
            ev.client_message.data.set_long(3, d3);
            ev.client_message.data.set_long(4, d4);
            xlib::XSendEvent(state.dpy, w, False, mask as i64, &mut ev);
        }
        exists
    }
}

pub fn grabbuttons(state: &mut State, c: *mut Client, focused: bool) {
    log::trace!("grabbuttons");
    unsafe {
        updatenumlockmask(state);
        let modifiers =
            [0, LockMask, state.numlockmask, state.numlockmask | LockMask];
        xlib::XUngrabButton(state.dpy, AnyButton as u32, AnyModifier, (*c).win);
        if !focused {
            xlib::XGrabButton(
                state.dpy,
                AnyButton as u32,
                AnyModifier,
                (*c).win,
                False,
                BUTTONMASK as u32,
                GrabModeSync,
                GrabModeSync,
                XNONE as u64,
                XNONE as u64,
            );
        }
        for button in &state.config.buttons {
            if button.click == Clk::ClientWin as u32 {
                for mod_ in modifiers {
                    xlib::XGrabButton(
                        state.dpy,
                        button.button,
                        button.mask | mod_,
                        (*c).win,
                        False,
                        BUTTONMASK as u32,
                        GrabModeAsync,
                        GrabModeSync,
                        XNONE as u64,
                        XNONE as u64,
                    );
                }
            }
        }
    }
}

pub fn arrange(state: &mut State, mut m: *mut Monitor) {
    log::trace!("arrange");
    unsafe {
        if !m.is_null() {
            showhide(state, (*m).stack);
        } else {
            m = state.mons;
            while !m.is_null() {
                showhide(state, (*m).stack);
                m = (*m).next;
            }
        }

        if !m.is_null() {
            arrangemon(state, m);
            restack(state, m);
        } else {
            m = state.mons;
            while !m.is_null() {
                arrangemon(state, m);
                m = (*m).next;
            }
        }
    }
}

pub fn arrangemon(state: &mut State, m: *mut Monitor) {
    log::trace!("arrangemon");
    unsafe {
        (*m).ltsymbol = (*(*m).lt[(*m).sellt as usize]).symbol.clone();
        let arrange = &(*(*m).lt[(*m).sellt as usize]).arrange;
        if let Some(arrange) = arrange {
            (arrange.0)(state, m);
        }
    }
}

pub fn restack(state: &mut State, m: *mut Monitor) {
    log::trace!("restack");
    drawbar(state, m);
    unsafe {
        if (*m).sel.is_null() {
            return;
        }
        if (*(*m).sel).isfloating
            || (*(*m).lt[(*m).sellt as usize]).arrange.is_none()
        {
            xlib::XRaiseWindow(state.dpy, (*(*m).sel).win);
        }
        if (*(*m).lt[(*m).sellt as usize]).arrange.is_some() {
            let mut wc = xlib::XWindowChanges {
                stack_mode: Below,
                sibling: (*m).barwin,
                x: Default::default(),
                y: Default::default(),
                width: Default::default(),
                height: Default::default(),
                border_width: Default::default(),
            };
            let mut c = (*m).stack;
            while !c.is_null() {
                if !(*c).isfloating && is_visible(c) {
                    xlib::XConfigureWindow(
                        state.dpy,
                        (*c).win,
                        (CWSibling | CWStackMode) as c_uint,
                        &mut wc as *mut _,
                    );
                    wc.sibling = (*c).win;
                }
                c = (*c).snext;
            }
        }
        xlib::XSync(state.dpy, False);
        let mut ev = xlib::XEvent { type_: 0 };
        while xlib::XCheckMaskEvent(state.dpy, EnterWindowMask, &mut ev) != 0 {}
    }
}

pub fn showhide(state: &mut State, c: *mut Client) {
    log::trace!("showhide");
    unsafe {
        if c.is_null() {
            return;
        }
        if is_visible(c) {
            // show clients top down
            xlib::XMoveWindow(state.dpy, (*c).win, (*c).x, (*c).y);
            if ((*(*(*c).mon).lt[(*(*c).mon).sellt as usize])
                .arrange
                .is_none()
                || (*c).isfloating)
                && !(*c).isfullscreen
            {
                resize(state, c, (*c).x, (*c).y, (*c).w, (*c).h, 0);
            }
            showhide(state, (*c).snext);
        } else {
            // hide clients bottom up
            showhide(state, (*c).snext);
            xlib::XMoveWindow(state.dpy, (*c).win, width(c) * -2, (*c).y);
        }
    }
}

pub fn resize(
    state: &mut State,
    c: *mut Client,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    interact: c_int,
) {
    log::trace!("resize");
    if applysizehints(state, c, &mut x, &mut y, &mut w, &mut h, interact) != 0 {
        resizeclient(state, c, x, y, w, h);
    }
}

pub fn resizeclient(
    state: &mut State,
    c: *mut Client,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    log::trace!("resizeclient");
    unsafe {
        (*c).oldx = (*c).x;
        (*c).oldy = (*c).y;
        (*c).oldw = (*c).w;
        (*c).oldh = (*c).h;
        (*c).x = x;
        (*c).y = y;
        (*c).w = w;
        (*c).h = h;
        let mut wc = xlib::XWindowChanges {
            x,
            y,
            width: w,
            height: h,
            border_width: (*c).bw,
            sibling: 0,
            stack_mode: 0,
        };
        xlib::XConfigureWindow(
            state.dpy,
            (*c).win,
            (CWX | CWY | CWWidth | CWHeight | CWBorderWidth) as u32,
            &mut wc,
        );
        configure(state, c);
        xlib::XSync(state.dpy, False);
    }
}

pub fn resizebarwin(state: &mut State, m: *mut Monitor) {
    unsafe {
        let mut w = (*m).ww;
        if state.config.showsystray
            && m == systraytomon(state, m)
            && !state.config.systrayonleft
        {
            w -= getsystraywidth(state) as i32;
        }
        XMoveResizeWindow(
            state.dpy,
            (*m).barwin,
            (*m).wx,
            (*m).by,
            w as u32,
            state.bh as u32,
        );
    }
}

pub fn configure(state: &mut State, c: *mut Client) {
    log::trace!("configure");
    unsafe {
        let mut ce = xlib::XConfigureEvent {
            type_: x11::xlib::ConfigureNotify,
            serial: 0,
            send_event: 0,
            display: state.dpy,
            event: (*c).win,
            window: (*c).win,
            x: (*c).x,
            y: (*c).y,
            width: (*c).w,
            height: (*c).h,
            border_width: (*c).bw,
            above: XNONE as u64,
            override_redirect: False,
        };
        xlib::XSendEvent(
            state.dpy,
            (*c).win,
            False,
            StructureNotifyMask,
            &mut ce as *mut xlib::XConfigureEvent as *mut xlib::XEvent,
        );
    }
}

pub fn applysizehints(
    state: &mut State,
    c: *mut Client,
    x: &mut i32,
    y: &mut i32,
    w: &mut i32,
    h: &mut i32,
    interact: c_int,
) -> c_int {
    log::trace!("applysizehints");
    unsafe {
        let m = (*c).mon;
        let interact = interact != 0;
        // set minimum possible
        *w = 1.max(*w);
        *h = 1.max(*h);
        if interact {
            if *x > state.sw {
                *x = state.sw - width(c);
            }
            if *y > state.sh {
                *y = state.sh - height(c);
            }
            if *x + *w + 2 * (*c).bw < 0 {
                *x = 0;
            }
            if *y + *h + 2 * (*c).bw < 0 {
                *y = 0;
            }
        } else {
            if *x >= ((*m).wx + (*m).ww) {
                *x = (*m).wx + (*m).ww - width(c);
            }
            if *y >= ((*m).wy + (*m).wh) {
                *y = (*m).wy + (*m).wh - height(c);
            }
            if *x + *w + 2 * (*c).bw <= (*m).wx {
                *x = (*m).wx;
            }
            if *y + *h + 2 * (*c).bw <= (*m).wy {
                *y = (*m).wy;
            }
        }
        if *h < state.bh {
            *h = state.bh;
        }
        if *w < state.bh {
            *w = state.bh;
        }
        if state.config.resize_hints
            || (*c).isfloating
            || (*(*(*c).mon).lt[(*(*c).mon).sellt as usize])
                .arrange
                .is_none()
        {
            if (*c).hintsvalid == 0 {
                updatesizehints(state, c);
            }
            /* see last two sentences in ICCCM 4.1.2.3 */
            let baseismin = (*c).basew == (*c).minw && (*c).baseh == (*c).minh;
            if !baseismin {
                /* temporarily remove base dimensions */
                *w -= (*c).basew;
                *h -= (*c).baseh;
            }
            /* adjust for aspect limits */
            if (*c).mina > 0.0 && (*c).maxa > 0.0 {
                if (*c).maxa < *w as f32 / *h as f32 {
                    *w = (*h as f32 * (*c).maxa + 0.5) as i32;
                } else if (*c).mina < *h as f32 / *w as f32 {
                    *h = (*w as f32 * (*c).mina + 0.5) as i32;
                }
            }
            if baseismin {
                /* increment calculation requires this */
                *w -= (*c).basew;
                *h -= (*c).baseh;
            }
            /* adjust for increment value */
            if (*c).incw != 0 {
                *w -= *w % (*c).incw;
            }
            if (*c).inch != 0 {
                *h -= *h % (*c).inch;
            }
            /* restore base dimensions */
            *w = max(*w + (*c).basew, (*c).minw);
            *h = max(*h + (*c).baseh, (*c).minh);
            if (*c).maxw != 0 {
                *w = std::cmp::min(*w, (*c).maxw);
            }
            if (*c).maxh != 0 {
                *h = std::cmp::min(*h, (*c).maxh);
            }
        }
        (*x != (*c).x || *y != (*c).y || *w != (*c).w || *h != (*c).h) as c_int
    }
}

pub fn updatesizehints(state: &mut State, c: *mut Client) {
    log::trace!("updatesizehints");
    let mut msize: i64 = 0;
    let mut size = xlib::XSizeHints {
        flags: Default::default(),
        x: Default::default(),
        y: Default::default(),
        width: Default::default(),
        height: Default::default(),
        min_width: Default::default(),
        min_height: Default::default(),
        max_width: Default::default(),
        max_height: Default::default(),
        width_inc: Default::default(),
        height_inc: Default::default(),
        min_aspect: xlib::AspectRatio { x: 0, y: 0 },
        max_aspect: xlib::AspectRatio { x: 0, y: 0 },
        base_width: Default::default(),
        base_height: Default::default(),
        win_gravity: Default::default(),
    };
    unsafe {
        if xlib::XGetWMNormalHints(state.dpy, (*c).win, &mut size, &mut msize)
            == 0
        {
            /* size is uninitialized, ensure that size.flags aren't used */
            size.flags = PSize;
        }
        if size.flags & PBaseSize != 0 {
            (*c).basew = size.base_width;
            (*c).baseh = size.base_height;
        } else if size.flags & PMinSize != 0 {
            (*c).basew = size.min_width;
            (*c).baseh = size.min_height;
        } else {
            (*c).basew = 0;
            (*c).baseh = 0;
        }

        if size.flags & PResizeInc != 0 {
            (*c).incw = size.width_inc;
            (*c).inch = size.height_inc;
        } else {
            (*c).incw = 0;
            (*c).inch = 0;
        }

        if size.flags & PMaxSize != 0 {
            (*c).maxw = size.max_width;
            (*c).maxh = size.max_height;
        } else {
            (*c).maxw = 0;
            (*c).maxh = 0;
        }

        if size.flags & PMinSize != 0 {
            (*c).minw = size.min_width;
            (*c).minh = size.min_height;
        } else if size.flags & PBaseSize != 0 {
            (*c).minw = size.base_width;
            (*c).minh = size.base_height;
        } else {
            (*c).minw = 0;
            (*c).minh = 0;
        }

        if size.flags & PAspect != 0 {
            (*c).mina = size.min_aspect.y as f32 / size.min_aspect.x as f32;
            (*c).maxa = size.max_aspect.x as f32 / size.max_aspect.y as f32;
        } else {
            (*c).mina = 0.0;
            (*c).maxa = 0.0;
        }

        (*c).isfixed = ((*c).maxw != 0
            && (*c).maxh != 0
            && (*c).maxw == (*c).minw
            && (*c).maxh == (*c).minh) as c_int;
        (*c).hintsvalid = 1;
    }
}

pub fn pop(state: &mut State, c: *mut Client) {
    log::trace!("pop");
    detach(c);
    attach(c);
    unsafe {
        focus(state, c);
        arrange(state, (*c).mon);
    }
}

pub fn detach(c: *mut Client) {
    log::trace!("detach");
    unsafe {
        let mut tc: *mut *mut Client = &mut (*(*c).mon).clients;
        while !(*tc).is_null() && *tc != c {
            tc = &mut (*(*tc)).next;
        }
        *tc = (*c).next;
    }
}

pub fn nexttiled(mut c: *mut Client) -> *mut Client {
    log::trace!("nexttiled");
    unsafe {
        while !c.is_null() && ((*c).isfloating || !is_visible(c)) {
            c = (*c).next;
        }
        c
    }
}

pub fn grabkeys(state: &mut State) {
    log::trace!("grabkeys");
    unsafe {
        updatenumlockmask(state);
        let modifiers =
            [0, LockMask, state.numlockmask, state.numlockmask | LockMask];
        let (mut start, mut end, mut skip): (i32, i32, i32) = (0, 0, 0);
        xlib::XUngrabKey(state.dpy, AnyKey, AnyModifier, state.root);
        xlib::XDisplayKeycodes(state.dpy, &mut start, &mut end);
        let syms = xlib::XGetKeyboardMapping(
            state.dpy,
            start as u8,
            end - start + 1,
            &mut skip,
        );
        if syms.is_null() {
            return;
        }
        for k in start..=end {
            for key in &state.config.keys {
                // skip modifier codes, we do that ourselves
                if key.keysym
                    == (*syms.offset(((k - start) * skip) as isize)) as u64
                {
                    for m in modifiers {
                        xlib::XGrabKey(
                            state.dpy,
                            k,
                            key.mod_ | m,
                            state.root,
                            True,
                            GrabModeAsync,
                            GrabModeAsync,
                        );
                    }
                }
            }
        }
        XFree(syms.cast());
    }
}

pub fn updatenumlockmask(state: &mut State) {
    log::trace!("updatenumlockmask");
    unsafe {
        state.numlockmask = 0;
        let modmap = xlib::XGetModifierMapping(state.dpy);
        for i in 0..8 {
            for j in 0..(*modmap).max_keypermod {
                if *(*modmap)
                    .modifiermap
                    .offset((i * (*modmap).max_keypermod + j) as isize)
                    == xlib::XKeysymToKeycode(state.dpy, XK_Num_Lock as u64)
                {
                    state.numlockmask = 1 << i;
                }
            }
        }
        xlib::XFreeModifiermap(modmap);
    }
}

pub fn seturgent(state: &mut State, c: *mut Client, urg: bool) {
    log::trace!("seturgent");
    unsafe {
        (*c).isurgent = urg as c_int;
        let wmh = xlib::XGetWMHints(state.dpy, (*c).win);
        if wmh.is_null() {
            return;
        }
        (*wmh).flags = if urg {
            (*wmh).flags | xlib::XUrgencyHint
        } else {
            (*wmh).flags & !{ xlib::XUrgencyHint }
        };
        xlib::XSetWMHints(state.dpy, (*c).win, wmh);
        XFree(wmh.cast());
    }
}

pub fn unfocus(state: &mut State, c: *mut Client, setfocus: bool) {
    log::trace!("unfocus");
    if c.is_null() {
        return;
    }
    grabbuttons(state, c, false);
    unsafe {
        // scheme[SchemeNorm][ColBorder].pixel
        let color = state.scheme[(Scheme::Norm, Col::Border)].pixel;
        xlib::XSetWindowBorder(state.dpy, (*c).win, color);
        if setfocus {
            xlib::XSetInputFocus(
                state.dpy,
                state.root,
                RevertToPointerRoot,
                CurrentTime,
            );
            xlib::XDeleteProperty(
                state.dpy,
                state.root,
                state.netatom[Net::ActiveWindow as usize],
            );
        }
    }
}

pub fn updatestatus(state: &mut State) {
    log::trace!("updatestatus");
    if gettextprop(state.dpy, state.root, XA_WM_NAME, &mut state.stext) == 0 {
        state.stext = "rwm-1.0".to_string();
    }
    drawbar(state, state.selmon);
    updatesystray(state);
}

pub fn updatesystrayicongeom(
    state: &mut State,
    i: *mut Client,
    w: c_int,
    h: c_int,
) {
    if i.is_null() {
        return;
    }
    unsafe {
        let i = &mut *i;
        i.h = state.bh;
        if w == h {
            i.w = state.bh;
        } else if h == state.bh {
            i.w = w;
        } else {
            i.w = (state.bh as f32 * (w as f32 / h as f32)) as i32;
        }
        applysizehints(state, i, &mut i.x, &mut i.y, &mut i.w, &mut i.h, False);
        // force icons into the systray dimensions if they don't want to
        if i.h > state.bh {
            if i.w == i.h {
                i.w = state.bh;
            } else {
                i.w = (state.bh as f32 * (i.w as f32 / i.h as f32)) as i32;
            }
            i.h = state.bh;
        }
    }
}

pub fn updatesystrayiconstate(
    state: &mut State,
    i: *mut Client,
    ev: *mut XPropertyEvent,
) {
    unsafe {
        let mut flags: Atom = 0;
        let code;
        if !state.config.showsystray
            || i.is_null()
            || (*ev).atom != state.xatom[XEmbed::XEmbedInfo as usize]
        {
            flags =
                getatomprop(state, i, state.xatom[XEmbed::XEmbedInfo as usize]);
            if flags == 0 {
                return;
            }
        }
        let i = &mut *i;
        if flags & XEMBED_MAPPED != 0 && i.tags == 0 {
            i.tags = 1;
            code = XEMBED_WINDOW_ACTIVATE;
            XMapRaised(state.dpy, i.win);
            setclientstate(state, i, NORMAL_STATE);
        } else if (flags & XEMBED_MAPPED) == 0 && i.tags != 0 {
            i.tags = 0;
            code = XEMBED_WINDOW_DEACTIVATE;
            XUnmapWindow(state.dpy, i.win);
            setclientstate(state, i, WITHDRAWN_STATE);
        } else {
            return;
        }
        sendevent(
            state,
            i.win,
            state.xatom[XEmbed::XEmbed as usize],
            StructureNotifyMask as i32,
            CurrentTime as i64,
            code as i64,
            0,
            state.systray().win as i64,
            XEMBED_EMBEDDED_VERSION as i64,
        );
    }
}

pub const fn default_window_attributes() -> XSetWindowAttributes {
    XSetWindowAttributes {
        background_pixmap: 0,
        background_pixel: 0,
        border_pixmap: 0,
        border_pixel: 0,
        bit_gravity: 0,
        win_gravity: 0,
        backing_store: 0,
        backing_planes: 0,
        backing_pixel: 0,
        save_under: 0,
        event_mask: 0,
        do_not_propagate_mask: 0,
        override_redirect: 0,
        colormap: 0,
        cursor: 0,
    }
}

pub fn updatesystray(state: &mut State) {
    unsafe {
        let mut wa = default_window_attributes();
        let mut wc: XWindowChanges;
        let mut i: *mut Client;
        let m: *mut Monitor = systraytomon(state, null_mut());
        let mut x: c_int = (*m).mx + (*m).mw;
        let sw = textw(&mut state.drw, &state.stext, 0)
            + state.config.systrayspacing as i32;
        let mut w = 1;

        if !state.config.showsystray {
            return;
        }
        if state.config.systrayonleft {
            x -= sw + state.lrpad / 2;
        }
        if state.systray.is_none() {
            // init systray
            let win = XCreateSimpleWindow(
                state.dpy,
                state.root,
                x,
                (*m).by,
                w,
                state.bh as u32,
                0,
                0,
                state.scheme[(Scheme::Sel, Col::Bg)].pixel,
            );
            wa.event_mask = ButtonPressMask | ExposureMask;
            wa.override_redirect = True;
            wa.background_pixel = state.scheme[(Scheme::Norm, Col::Bg)].pixel;
            XSelectInput(state.dpy, win, SubstructureNotifyMask);
            XChangeProperty(
                state.dpy,
                win,
                state.netatom[Net::SystemTrayOrientation as usize],
                XA_CARDINAL,
                32,
                PropModeReplace,
                &state.netatom[Net::SystemTrayOrientationHorz as usize]
                    as *const _ as *const _,
                1,
            );
            XChangeWindowAttributes(
                state.dpy,
                win,
                CWEventMask | CWOverrideRedirect | CWBackPixel,
                &mut wa,
            );
            XMapRaised(state.dpy, win);
            XSetSelectionOwner(
                state.dpy,
                state.netatom[Net::SystemTray as usize],
                win,
                CurrentTime,
            );
            if XGetSelectionOwner(
                state.dpy,
                state.netatom[Net::SystemTray as usize],
            ) == win
            {
                sendevent(
                    state,
                    state.root,
                    state.xatom[XEmbed::Manager as usize],
                    StructureNotifyMask as i32,
                    CurrentTime as i64,
                    state.netatom[Net::SystemTray as usize] as i64,
                    win as i64,
                    0_i64,
                    0_i64,
                );
                XSync(state.dpy, False);
                state.systray = Some(Systray { win, icons: null_mut() });
            } else {
                log::error!("unable to obtain system tray");
                return;
            }
        } // end if !SYSTRAY
        cfor!(((w, i) = (0, state.systray().icons);
        !i.is_null();
        i = (*i).next) {
            // make sure the background color stays the same
            wa.background_pixel = state.scheme[(Scheme::Norm , Col::Bg )].pixel;
            XChangeWindowAttributes(state.dpy, (*i).win, CWBackPixel, &mut wa);
            XMapRaised(state.dpy, (*i).win);
            w += state.config.systrayspacing;
            (*i).x = w as i32;
            XMoveResizeWindow(state.dpy, (*i).win, (*i).x, 0, (*i).w as u32, (*i).h as u32);
            w += (*i).w as u32;
            if (*i).mon != m {
                (*i).mon = m;
            }
        });
        w = if w != 0 { w + state.config.systrayspacing } else { 1 };
        x -= w as i32;
        XMoveResizeWindow(
            state.dpy,
            state.systray().win,
            x,
            (*m).by,
            w,
            state.bh as u32,
        );
        wc = XWindowChanges {
            x,
            y: (*m).by,
            width: w as i32,
            height: state.bh,
            border_width: 0,
            sibling: (*m).barwin,
            stack_mode: Above,
        };
        XConfigureWindow(
            state.dpy,
            state.systray().win,
            (CWX | CWY | CWWidth | CWHeight | CWSibling | CWStackMode) as u32,
            &mut wc,
        );
        XMapWindow(state.dpy, state.systray().win);
        XMapSubwindows(state.dpy, state.systray().win);
        // redraw background
        XSetForeground(
            state.dpy,
            state.drw.gc,
            state.scheme[(Scheme::Norm, Col::Bg)].pixel,
        );
        XFillRectangle(
            state.dpy,
            state.systray().win,
            state.drw.gc,
            0,
            0,
            w,
            state.bh as u32,
        );
        XSync(state.dpy, False);
    } // end unsafe
}

pub fn wintosystrayicon(state: &State, w: Window) -> *mut Client {
    unsafe {
        let mut i = null_mut();
        if !state.config.showsystray || w == 0 {
            return i;
        }
        cfor!((i = state.systray().icons; !i.is_null() && (*i).win != w;
            i = (*i).next) {});

        i
    }
}

pub fn systraytomon(state: &State, m: *mut Monitor) -> *mut Monitor {
    unsafe {
        let mut t: *mut Monitor;
        let mut i;
        let mut n;
        if state.config.systraypinning == 0 {
            if m.is_null() {
                return state.selmon;
            }
            if m == state.selmon {
                return m;
            } else {
                return null_mut();
            }
        }
        cfor!(((n, t) = (1, state.mons);
            !t.is_null() && !(*t).next.is_null();
            (n, t) = (n+1, (*t).next)) {});
        cfor!(((i, t) = (1, state.mons);
            !t.is_null() && !(*t).next.is_null() && i < state.config.systraypinning;
            (i, t) = (i+1, (*t).next)) {});
        if state.config.systraypinningfailfirst
            && n < state.config.systraypinning
        {
            return state.mons;
        }

        t
    }
}

pub fn textw(drw: &mut Drw, x: &str, lrpad: c_int) -> c_int {
    log::trace!("textw");
    unsafe { drw::fontset_getwidth(drw, x) as c_int + lrpad }
}

pub fn drawbar(state: &mut State, m: *mut Monitor) {
    log::trace!("drawbar");
    unsafe {
        let mut tw = 0;
        let mut stw = 0;
        let boxs = state.drw.fonts[0].h / 9;
        let boxw = state.drw.fonts[0].h / 6 + 2;
        let (mut occ, mut urg) = (0, 0);

        if state.config.showsystray
            && m == systraytomon(state, m)
            && !state.config.systrayonleft
        {
            stw = getsystraywidth(state);
        }

        if !(*m).showbar {
            return;
        }

        // draw status first so it can be overdrawn by tags later
        if m == state.selmon {
            // status is only drawn on selected monitor
            drw::setscheme(&mut state.drw, state.scheme[Scheme::Norm].clone());
            tw = textw(&mut state.drw, &state.stext, state.lrpad / 2) + 2; // 2px right padding
            log::trace!("drawbar: text");
            drw::text(
                &mut state.drw,
                (*m).ww - tw - stw as i32,
                0,
                tw as u32,
                state.bh as u32,
                (state.lrpad / 2 - 2) as u32,
                &state.stext,
                0,
            );
        }

        resizebarwin(state, m);

        let mut c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent != 0 {
                urg |= (*c).tags;
            }
            c = (*c).next;
        }

        let mut x = 0;
        for (i, tag) in state.config.tags.iter().enumerate() {
            let text = tag.to_owned();
            let w = textw(&mut state.drw, &text, state.lrpad);
            drw::setscheme(
                &mut state.drw,
                state.scheme[if ((*m).tagset[(*m).seltags as usize] & (1 << i))
                    != 0
                {
                    Scheme::Sel
                } else {
                    Scheme::Norm
                }]
                .clone(),
            );
            log::trace!("drawbar: text 2");
            drw::text(
                &mut state.drw,
                x,
                0,
                w as u32,
                state.bh as u32,
                state.lrpad as u32 / 2,
                &text,
                (urg as i32) & (1 << i),
            );

            if (occ & (1 << i)) != 0 {
                drw::rect(
                    &mut state.drw,
                    x + boxs as i32,
                    boxs as i32,
                    boxw,
                    boxw,
                    (m == state.selmon
                        && !(*state.selmon).sel.is_null()
                        && ((*(*state.selmon).sel).tags & (1 << i)) != 0)
                        as c_int,
                    (urg & (1 << i)) != 0,
                );
            }
            x += w as i32;
        }

        let w = textw(&mut state.drw, &(*m).ltsymbol, state.lrpad);
        drw::setscheme(&mut state.drw, state.scheme[Scheme::Norm].clone());
        log::trace!("drawbar: text 3");
        x = drw::text(
            &mut state.drw,
            x,
            0,
            w as u32,
            state.bh as u32,
            state.lrpad as u32 / 2,
            &(*m).ltsymbol,
            0,
        ) as i32;
        log::trace!("finished drawbar text 3");

        let w = (*m).ww - tw - stw as i32 - x;
        if w > state.bh {
            if !(*m).sel.is_null() {
                drw::setscheme(
                    &mut state.drw,
                    state.scheme[if m == state.selmon {
                        Scheme::Sel
                    } else {
                        Scheme::Norm
                    }]
                    .clone(),
                );
                log::trace!("drawbar: text 4");
                drw::text(
                    &mut state.drw,
                    x,
                    0,
                    w as u32,
                    state.bh as u32,
                    state.lrpad as u32 / 2,
                    &(*(*m).sel).name,
                    0,
                );
                if (*(*m).sel).isfloating {
                    drw::rect(
                        &mut state.drw,
                        x + boxs as i32,
                        boxs as i32,
                        boxw,
                        boxw,
                        (*(*m).sel).isfixed,
                        false,
                    );
                }
            } else {
                drw::setscheme(
                    &mut state.drw,
                    state.scheme[Scheme::Norm].clone(),
                );
                drw::rect(
                    &mut state.drw,
                    x,
                    0,
                    w as u32,
                    state.bh as u32,
                    1,
                    true,
                );
            }
        }
        drw::map(
            &state.drw,
            (*m).barwin,
            0,
            0,
            (*m).ww as u32 - stw,
            state.bh as u32,
        );
    }
}

pub fn gettextprop(
    dpy: *mut Display,
    w: Window,
    atom: Atom,
    text: &mut String,
) -> c_int {
    log::trace!("gettextprop");
    unsafe {
        let mut name = xlib::XTextProperty {
            value: std::ptr::null_mut(),
            encoding: 0,
            format: 0,
            nitems: 0,
        };
        let c = xlib::XGetTextProperty(dpy, w, &mut name, atom);
        if c == 0 || name.nitems == 0 {
            return 0;
        }

        let mut n = 0;
        let mut list: *mut *mut i8 = std::ptr::null_mut();
        if name.encoding == XA_STRING {
            let name_val = CStr::from_ptr(name.value.cast());
            *text = name_val.to_string_lossy().to_string();
        } else if xlib::XmbTextPropertyToTextList(
            dpy,
            &name,
            &mut list,
            &mut n as *mut _,
        ) >= Success as i32
            && n > 0
            && !(*list).is_null()
        {
            // TODO handle this properly. *list is a "string" in some encoding I
            // don't understand. the main test case I noticed an issue with was
            // a browser tab with a ·, which was initially taking the value -73
            // as an i8, which is the correct character 183 as a u8. This
            // solution works for characters like that that fit in a u8 but
            // doesn't work for larger characters like ў (cyrillic short u).
            // actually `list` doesn't even contain the right characters for the
            // short u. it just starts at the space after it, as demonstrated by
            // using libc::printf to try to print it.
            //
            // Looks like my encoding is different. Getting 238 in Rust vs 287
            // in C. Using XGetAtomName shows 238 is UTF8_STRING, while 287 is
            // _NET_WM_WINDOW_TYPE_POPUP_MENU (??). In dwm in the VM, 287 is
            // also UTF8_STRING
            *text = String::new();
            let mut c = *list;
            while *c != 0 {
                text.push(char::from(*c as u8));
                c = c.offset(1);
            }
            xlib::XFreeStringList(list);
        }
        xlib::XFree(name.value as *mut _);
    }
    1
}

pub fn updatebars(state: &mut State) {
    log::trace!("updatebars");
    let mut wa = xlib::XSetWindowAttributes {
        override_redirect: True,
        background_pixmap: ParentRelative as u64,
        event_mask: ButtonPressMask | ExposureMask,
        // everything else should be uninit I guess
        background_pixel: 0,
        border_pixmap: 0,
        border_pixel: 0,
        bit_gravity: 0,
        win_gravity: 0,
        backing_store: 0,
        backing_planes: 0,
        backing_pixel: 0,
        save_under: 0,
        do_not_propagate_mask: 0,
        colormap: 0,
        cursor: 0,
    };
    let mut ch = xlib::XClassHint {
        res_name: c"rwm".as_ptr().cast_mut(),
        res_class: c"rwm".as_ptr().cast_mut(),
    };

    unsafe {
        let mut m = state.mons;
        while !m.is_null() {
            if (*m).barwin != 0 {
                continue;
            }
            let mut w = (*m).ww;
            if state.config.showsystray && m == systraytomon(state, m) {
                w -= getsystraywidth(state) as i32;
            }
            (*m).barwin = xlib::XCreateWindow(
                state.dpy,
                state.root,
                (*m).wx as c_int,
                (*m).by as c_int,
                w as c_uint,
                state.bh as c_uint,
                0,
                xlib::XDefaultDepth(state.dpy, state.screen),
                CopyFromParent as c_uint,
                xlib::XDefaultVisual(state.dpy, state.screen),
                CWOverrideRedirect | CWBackPixmap | CWEventMask,
                &mut wa,
            );
            xlib::XDefineCursor(
                state.dpy,
                (*m).barwin,
                state.cursors.normal.cursor,
            );
            if state.config.showsystray && m == systraytomon(state, m) {
                xlib::XMapRaised(state.dpy, state.systray().win);
            }
            xlib::XMapRaised(state.dpy, (*m).barwin);
            xlib::XSetClassHint(state.dpy, (*m).barwin, &mut ch);
            m = (*m).next;
        }
    }
}

pub fn updategeom(state: &mut State) -> i32 {
    log::trace!("updategeom");
    unsafe {
        let mut dirty = 0;
        if x11::xinerama::XineramaIsActive(state.dpy) != 0 {
            log::trace!("updategeom: xinerama active");

            let mut nn = 0;
            let info = x11::xinerama::XineramaQueryScreens(
                state.dpy as *mut _,
                &mut nn,
            );

            let mut n = 0;
            let mut m = state.mons;
            while !m.is_null() {
                m = (*m).next;
                n += 1;
            }

            // only consider unique geometries as separate screens
            let unique: *mut x11::xinerama::XineramaScreenInfo = ecalloc(
                nn as usize,
                size_of::<x11::xinerama::XineramaScreenInfo>(),
            )
            .cast();
            // Safety: we obviously just constructed this with this size. don't
            // forget to free it later!
            let unique = std::slice::from_raw_parts_mut(unique, nn as usize);
            let mut i = 0;
            let mut j = 0;
            while i < nn {
                if isuniquegeom(unique, j, info.offset(i as isize) as *mut _)
                    != 0
                {
                    libc::memcpy(
                        (&mut unique[j]) as *mut _ as *mut _,
                        info.offset(i as isize).cast(),
                        size_of::<x11::xinerama::XineramaScreenInfo>(),
                    );
                    j += 1;
                }
                i += 1;
            }
            xlib::XFree(info.cast());
            nn = j as i32;

            // new monitors if nn > n
            if nn > n {
                log::trace!("updategeom: adding monitors");
            }
            for _ in n..nn {
                let mut m = state.mons;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }
                if !m.is_null() {
                    (*m).next = createmon(state);
                } else {
                    state.mons = createmon(state);
                }
            }

            let mut i = 0;
            let mut m = state.mons;
            while i < nn && !m.is_null() {
                if i >= n
                    || unique[i as usize].x_org != (*m).mx as i16
                    || unique[i as usize].y_org != (*m).my as i16
                    || unique[i as usize].width != (*m).mw as i16
                    || unique[i as usize].height != (*m).mh as i16
                {
                    dirty = 1;
                    (*m).num = i;

                    (*m).mx = unique[i as usize].x_org as i32;
                    (*m).wx = unique[i as usize].x_org as i32;

                    (*m).my = unique[i as usize].y_org as i32;
                    (*m).wy = unique[i as usize].y_org as i32;

                    (*m).mw = unique[i as usize].width as i32;
                    (*m).ww = unique[i as usize].width as i32;

                    (*m).mh = unique[i as usize].height as i32;
                    (*m).wh = unique[i as usize].height as i32;

                    updatebarpos(state, m);
                }
                m = (*m).next;
                i += 1;
            }

            // removed monitors if n > nn
            if n > nn {
                log::trace!("updategeom: removing monitors");
            }
            for _ in nn..n {
                let mut m = state.mons;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }
                let mut c = (*m).clients;
                while !c.is_null() {
                    dirty = 1;
                    (*m).clients = (*c).next;
                    detachstack(c);
                    (*c).mon = state.mons;
                    attach(c);
                    attachstack(c);
                    c = (*m).clients;
                }
                if m == state.selmon {
                    state.selmon = state.mons;
                }
                cleanupmon(m, state);
            }
            libc::free(unique.as_mut_ptr().cast());
        } else {
            log::trace!("updategeom: default monitor setup");

            // default monitor setup
            if state.mons.is_null() {
                state.mons = createmon(state);
            }
            if (*state.mons).mw != state.sw || (*state.mons).mh != state.sh {
                dirty = 1;
                (*state.mons).mw = state.sw;
                (*state.mons).ww = state.sw;
                (*state.mons).mh = state.sh;
                (*state.mons).wh = state.sh;
                updatebarpos(state, state.mons);
            }
        }
        if dirty != 0 {
            state.selmon = state.mons;
            state.selmon = wintomon(state, state.root);
        }
        dirty
    }
}

pub fn wintomon(state: &mut State, w: Window) -> *mut Monitor {
    log::trace!("wintomon");
    unsafe {
        let mut x = 0;
        let mut y = 0;
        if w == state.root && getrootptr(state, &mut x, &mut y) != 0 {
            return recttomon(state, x, y, 1, 1);
        }
        let mut m = state.mons;
        while !m.is_null() {
            if w == (*m).barwin {
                return m;
            }
            m = (*m).next;
        }
        let c = wintoclient(state, w);
        if !c.is_null() {
            return (*c).mon;
        }
        state.selmon
    }
}

pub fn wintoclient(state: &mut State, w: u64) -> *mut Client {
    log::trace!("wintoclient");
    unsafe {
        let mut m = state.mons;
        while !m.is_null() {
            let mut c = (*m).clients;
            while !c.is_null() {
                if (*c).win == w {
                    return c;
                }
                c = (*c).next;
            }
            m = (*m).next;
        }
    }
    std::ptr::null_mut()
}

pub fn recttomon(
    state: &State,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
) -> *mut Monitor {
    log::trace!("recttomon");
    unsafe {
        let mut r = state.selmon;
        let mut area = 0;
        let mut m = state.mons;
        while !m.is_null() {
            let a = intersect(x, y, w, h, m);
            if a > area {
                area = a;
                r = m;
            }
            m = (*m).next;
        }
        r
    }
}

pub fn removesystrayicon(state: &mut State, i: *mut Client) {
    unsafe {
        if !state.config.showsystray || i.is_null() {
            return;
        }
        let mut ii: *mut *mut Client;
        cfor!((
            ii = &mut state.systray_mut().icons as *mut _;
            !ii.is_null() && *ii != i;
            ii = &mut (*(*ii)).next) {});
        if !ii.is_null() {
            *ii = (*i).next;
        }
        libc::free(i.cast());
    }
}

// "macros"

#[inline]
pub fn intersect(
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    m: *mut Monitor,
) -> c_int {
    use std::cmp::{max, min};
    unsafe {
        max(0, min((x) + (w), (*m).wx + (*m).ww) - max(x, (*m).wx))
            * max(0, min((y) + (h), (*m).wy + (*m).wh) - max(y, (*m).wy))
    }
}

#[inline]
pub fn width(x: *mut Client) -> i32 {
    unsafe { (*x).w + 2 * (*x).bw }
}

#[inline]
pub fn height(x: *mut Client) -> i32 {
    unsafe { (*x).h + 2 * (*x).bw }
}

#[inline]
pub fn cleanmask(state: &State, mask: u32) -> u32 {
    mask & !(state.numlockmask | LockMask)
        & (ShiftMask
            | ControlMask
            | Mod1Mask
            | Mod2Mask
            | Mod3Mask
            | Mod4Mask
            | Mod5Mask)
}

pub fn getrootptr(state: &mut State, x: *mut c_int, y: *mut c_int) -> c_int {
    unsafe {
        let mut di = 0;
        let mut dui = 0;
        let mut dummy = 0;
        xlib::XQueryPointer(
            state.dpy, state.root, &mut dummy, &mut dummy, x, y, &mut di,
            &mut di, &mut dui,
        )
    }
}

/// remove `mon` from the linked list of `Monitor`s in `state.MONS` and free it.
pub fn cleanupmon(mon: *mut Monitor, state: &mut State) {
    unsafe {
        if mon == state.mons {
            state.mons = (*state.mons).next;
        } else {
            let mut m = state.mons;
            while !m.is_null() && (*m).next != mon {
                m = (*m).next;
            }
            (*m).next = (*mon).next;
        }
        xlib::XUnmapWindow(state.dpy, (*mon).barwin);
        xlib::XDestroyWindow(state.dpy, (*mon).barwin);
        libc::free(mon.cast());
    }
}

pub fn attachstack(c: *mut Client) {
    log::trace!("attachstack");
    unsafe {
        (*c).snext = (*(*c).mon).stack;
        (*(*c).mon).stack = c;
    }
}

pub fn attach(c: *mut Client) {
    log::trace!("attach");
    unsafe {
        (*c).next = (*(*c).mon).clients;
        (*(*c).mon).clients = c;
    }
}

pub fn detachstack(c: *mut Client) {
    log::trace!("detachstack");
    unsafe {
        let mut tc: *mut *mut Client = &mut (*(*c).mon).stack;
        while !(*tc).is_null() && *tc != c {
            tc = &mut (*(*tc)).snext;
        }
        *tc = (*c).snext;

        if c == (*(*c).mon).sel {
            let mut t = (*(*c).mon).stack;
            while !t.is_null() && !is_visible(t) {
                t = (*t).snext;
            }
            (*(*c).mon).sel = t;
        }
    }
}

/// this is actually a macro in the C code, but an inline function is probably
/// as close as I can get
#[inline]
pub fn is_visible(c: *const Client) -> bool {
    unsafe {
        ((*c).tags & (*(*c).mon).tagset[(*(*c).mon).seltags as usize]) != 0
    }
}

pub fn updatebarpos(state: &mut State, m: *mut Monitor) {
    log::trace!("updatebarpos");

    unsafe {
        (*m).wy = (*m).my;
        (*m).wh = (*m).mh;
        if (*m).showbar {
            (*m).wh -= state.bh;
            (*m).by = if (*m).topbar { (*m).wy } else { (*m).wy + (*m).wh };
            (*m).wy = if (*m).topbar { (*m).wy + state.bh } else { (*m).wy };
        } else {
            (*m).by = -state.bh;
        }
    }
}

pub fn isuniquegeom(
    unique: &mut [x11::xinerama::XineramaScreenInfo],
    mut n: usize,
    info: *mut x11::xinerama::XineramaScreenInfo,
) -> c_int {
    unsafe {
        assert!(!info.is_null());
        let info = &*info;
        while n > 0 {
            n -= 1; // pretty sure this happens immediately in `while (n--)`

            if unique[n].x_org == info.x_org
                && unique[n].y_org == info.y_org
                && unique[n].width == info.width
                && unique[n].height == info.height
            {
                return 0;
            }
        }
        1
    }
}

pub fn cleanup(mut state: State) {
    log::trace!("entering cleanup");

    unsafe {
        let a = Arg::Ui(!0);
        view(&mut state, &a);
        (*state.selmon).lt[(*state.selmon).sellt as usize] =
            &Layout { symbol: String::new(), arrange: None };

        let mut m = state.mons;
        while !m.is_null() {
            while !(*m).stack.is_null() {
                unmanage(&mut state, (*m).stack, 0);
            }
            m = (*m).next;
        }

        xlib::XUngrabKey(state.dpy, AnyKey, AnyModifier, state.root);

        while !state.mons.is_null() {
            cleanupmon(state.mons, &mut state);
        }

        if state.config.showsystray {
            XUnmapWindow(state.dpy, state.systray().win);
            XDestroyWindow(state.dpy, state.systray().win);
        }

        xlib::XDestroyWindow(state.dpy, state.wmcheckwin);
        xlib::XSync(state.dpy, False);
        xlib::XSetInputFocus(
            state.dpy,
            PointerRoot as u64,
            RevertToPointerRoot,
            CurrentTime,
        );
        xlib::XDeleteProperty(
            state.dpy,
            state.root,
            state.netatom[Net::ActiveWindow as usize],
        );

        #[cfg(target_os = "linux")]
        drop(Box::from_raw(state.xcon));

        drop(state);
    }

    log::trace!("finished cleanup");
}

pub fn unmanage(state: &mut State, c: *mut Client, destroyed: c_int) {
    log::trace!("unmanage");
    unsafe {
        let m = (*c).mon;
        let mut wc = xlib::XWindowChanges {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        if !(*c).swallowing.is_null() {
            unswallow(state, c);
            return;
        }

        let s = swallowingclient(state, (*c).win);
        if !s.is_null() {
            libc::free((*s).swallowing.cast());
            (*s).swallowing = null_mut();
            arrange(state, m);
            focus(state, null_mut());
            return;
        }

        detach(c);
        detachstack(c);
        if destroyed == 0 {
            wc.border_width = (*c).oldbw;
            xlib::XGrabServer(state.dpy); /* avoid race conditions */
            xlib::XSetErrorHandler(Some(xerrordummy));
            xlib::XSelectInput(state.dpy, (*c).win, NoEventMask);
            xlib::XConfigureWindow(
                state.dpy,
                (*c).win,
                CWBorderWidth as u32,
                &mut wc,
            ); /* restore border */
            xlib::XUngrabButton(
                state.dpy,
                AnyButton as u32,
                AnyModifier,
                (*c).win,
            );
            setclientstate(state, c, WITHDRAWN_STATE);
            xlib::XSync(state.dpy, False);
            xlib::XSetErrorHandler(Some(xerror));
            xlib::XUngrabServer(state.dpy);
        }
        libc::free(c.cast());

        if s.is_null() {
            arrange(state, m);
            focus(state, null_mut());
            updateclientlist(state);
        }
    }
}

/// I'm just using the OpenBSD version of the code in the patch rather than the
/// Linux version that uses XCB
pub fn winpid(state: &mut State, w: Window) -> pid_t {
    #[cfg(target_os = "linux")]
    unsafe {
        log::trace!("winpid linux");

        let mut result = 0;

        let spec = xcb::res::ClientIdSpec {
            client: w as u32,
            mask: xcb::res::ClientIdMask::LOCAL_CLIENT_PID,
        };
        assert!(!state.xcon.is_null(), "xcon is null");
        let xcon = &*state.xcon;
        let cookie =
            xcon.send_request(&xcb::res::QueryClientIds { specs: &[spec] });
        let Ok(r) = xcon.wait_for_reply(cookie) else {
            return 0;
        };

        for id in r.ids() {
            let spec = id.spec();
            if !(spec.mask & xcb::res::ClientIdMask::LOCAL_CLIENT_PID)
                .is_empty()
            {
                result = id.value()[0] as i32;
            }
        }

        result
    }

    #[cfg(not(target_os = "linux"))]
    unsafe {
        use x11::xlib::{AnyPropertyType, XGetWindowProperty};

        let mut type_: Atom = 0;
        let mut format: c_int = 0;
        let mut len: c_ulong = 0;
        let mut bytes: c_ulong = 0;
        let mut prop: *mut c_uchar = null_mut();
        if XGetWindowProperty(
            state.dpy,
            w,
            XInternAtom(state.dpy, c"_NET_WM_PID".as_ptr(), 0),
            0,
            1,
            False,
            AnyPropertyType as u64,
            &mut type_,
            &mut format,
            &mut len,
            &mut bytes,
            &mut prop,
        ) != Success as i32
            || prop.is_null()
        {
            return 0;
        }
        let ret = *(prop as *mut pid_t);
        XFree(prop.cast());

        ret
    }
}

/// this looks insane... rust has std::os::unix::process::parent_id, but it
/// doesn't take any arguments. we need to get the parent of a specific process
/// here, so we read from /proc
pub fn getparentprocess(p: pid_t) -> pid_t {
    let filename = format!("/proc/{p}/stat");
    let Ok(mut f) = std::fs::File::open(filename) else {
        return 0;
    };
    let mut buf = Vec::new();
    let Ok(_) = f.read_to_end(&mut buf) else {
        return 0;
    };
    let Ok(s) = String::from_utf8(buf) else {
        return 0;
    };
    // trying to emulate fscanf(f, "%*u %*s %*c %u", &v); which should give the
    // 3rd field
    match s.split_ascii_whitespace().nth(3).map(str::parse) {
        Some(Ok(p)) => p,
        _ => 0,
    }
}

pub fn isdescprocess(p: pid_t, mut c: pid_t) -> pid_t {
    while p != c && c != 0 {
        c = getparentprocess(c);
    }
    c
}

pub fn termforwin(state: &mut State, w: *const Client) -> *mut Client {
    unsafe {
        let w = &*w;

        if w.pid == 0 || w.isterminal {
            return null_mut();
        }

        let mut c;
        let mut m;

        cfor!((m = state.mons; !m.is_null(); m = (*m).next) {
            cfor!((c = (*m).clients; !c.is_null(); c = (*c).next) {
                if (*c).isterminal && (*c).swallowing.is_null()
                && (*c).pid != 0 && isdescprocess((*c).pid, w.pid) != 0 {
                    return c;
                }
            });
        });
    }

    null_mut()
}

pub fn swallowingclient(state: &State, w: Window) -> *mut Client {
    unsafe {
        let mut c;
        let mut m;

        cfor!((m = state.mons; !m.is_null(); m = (*m).next) {
            cfor!((c = (*m).clients; !c.is_null(); c = (*c).next) {
                if !(*c).swallowing.is_null() && (*(*c).swallowing).win == w {
                    return c;
                }
            });
        });

        null_mut()
    }
}

pub fn updateclientlist(state: &mut State) {
    unsafe {
        xlib::XDeleteProperty(
            state.dpy,
            state.root,
            state.netatom[Net::ClientList as usize],
        );
        let mut m = state.mons;
        while !m.is_null() {
            let mut c = (*m).clients;
            while !c.is_null() {
                xlib::XChangeProperty(
                    state.dpy,
                    state.root,
                    state.netatom[Net::ClientList as usize],
                    XA_WINDOW,
                    32,
                    PropModeAppend,
                    &((*c).win) as *const u64 as *const c_uchar,
                    1,
                );
                c = (*c).next;
            }
            m = (*m).next;
        }
    }
}

pub fn setclientstate(s: &mut State, c: *mut Client, state: usize) {
    let mut data: [c_long; 2] = [state as c_long, XNONE as c_long];
    let ptr: *mut c_uchar = data.as_mut_ptr().cast();
    unsafe {
        xlib::XChangeProperty(
            s.dpy,
            (*c).win,
            s.wmatom[WM::State as usize],
            s.wmatom[WM::State as usize],
            32,
            PropModeReplace,
            ptr,
            2,
        );
    }
}

type HandlerFn = fn(&mut State, *mut xlib::XEvent);

pub static HANDLER: LazyLock<[HandlerFn; x11::xlib::LASTEvent as usize]> =
    LazyLock::new(|| {
        pub fn dh(_state: &mut State, _ev: *mut xlib::XEvent) {}
        let mut ret = [dh as fn(state: &mut State, *mut xlib::XEvent);
            x11::xlib::LASTEvent as usize];
        ret[x11::xlib::ButtonPress as usize] = handlers::buttonpress;
        ret[x11::xlib::ClientMessage as usize] = handlers::clientmessage;
        ret[x11::xlib::ConfigureRequest as usize] = handlers::configurerequest;
        ret[x11::xlib::ConfigureNotify as usize] = handlers::configurenotify;
        ret[x11::xlib::DestroyNotify as usize] = handlers::destroynotify;
        ret[x11::xlib::EnterNotify as usize] = handlers::enternotify;
        ret[x11::xlib::Expose as usize] = handlers::expose;
        ret[x11::xlib::FocusIn as usize] = handlers::focusin;
        ret[x11::xlib::KeyPress as usize] = handlers::keypress;
        ret[x11::xlib::MappingNotify as usize] = handlers::mappingnotify;
        ret[x11::xlib::MapRequest as usize] = handlers::maprequest;
        ret[x11::xlib::MotionNotify as usize] = handlers::motionnotify;
        ret[x11::xlib::PropertyNotify as usize] = handlers::propertynotify;
        ret[x11::xlib::ResizeRequest as usize] = handlers::resizerequest;
        ret[x11::xlib::UnmapNotify as usize] = handlers::unmapnotify;
        ret
    });

/// main event loop
pub fn run(state: &mut State) {
    unsafe {
        xlib::XSync(state.dpy, False);
        let mut ev: MaybeUninit<xlib::XEvent> = MaybeUninit::uninit();
        while state.running && xlib::XNextEvent(state.dpy, ev.as_mut_ptr()) == 0
        {
            let mut ev: xlib::XEvent = ev.assume_init();
            if let Some(handler) = HANDLER.get(ev.type_ as usize) {
                handler(state, &mut ev);
            }
        }
    }
}

pub fn scan(state: &mut State) {
    let mut num = 0;
    let mut d1 = 0;
    let mut d2 = 0;
    let mut wins: *mut Window = std::ptr::null_mut();
    let mut wa: MaybeUninit<xlib::XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        if xlib::XQueryTree(
            state.dpy,
            state.root,
            &mut d1,
            &mut d2,
            &mut wins as *mut _,
            &mut num,
        ) != 0
        {
            for i in 0..num {
                if xlib::XGetWindowAttributes(
                    state.dpy,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                    || (*wa.as_mut_ptr()).override_redirect != 0
                    || xlib::XGetTransientForHint(
                        state.dpy,
                        *wins.offset(i as isize),
                        &mut d1,
                    ) != 0
                {
                    continue;
                }
                if (*wa.as_mut_ptr()).map_state == IsViewable
                    || getstate(state, *wins.offset(i as isize))
                        == ICONIC_STATE as i64
                {
                    manage(state, *wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            for i in 0..num {
                // now the transients
                if xlib::XGetWindowAttributes(
                    state.dpy,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                {
                    continue;
                }
                if xlib::XGetTransientForHint(
                    state.dpy,
                    *wins.offset(i as isize),
                    &mut d1,
                ) != 0
                    && ((*wa.as_mut_ptr()).map_state == IsViewable
                        || getstate(state, *wins.offset(i as isize))
                            == ICONIC_STATE as i64)
                {
                    manage(state, *wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            if !wins.is_null() {
                XFree(wins.cast());
            }
        }
    }
}

pub fn manage(state: &mut State, w: Window, wa: *mut xlib::XWindowAttributes) {
    log::trace!("manage");
    let mut trans = 0;
    unsafe {
        let wa = *wa;
        let c: *mut Client = util::ecalloc(1, size_of::<Client>()) as *mut _;
        (*c).win = w;
        (*c).pid = winpid(state, w);
        (*c).x = wa.x;
        (*c).oldx = wa.x;
        (*c).y = wa.y;
        (*c).oldy = wa.y;
        (*c).w = wa.width;
        (*c).oldw = wa.width;
        (*c).h = wa.height;
        (*c).oldh = wa.height;
        (*c).oldbw = wa.border_width;
        (*c).name = String::new();

        let mut term: *mut Client = null_mut();

        updatetitle(state, c);
        log::trace!("manage: XGetTransientForHint");
        if xlib::XGetTransientForHint(state.dpy, w, &mut trans) != 0 {
            let t = wintoclient(state, trans);
            if !t.is_null() {
                (*c).mon = (*t).mon;
                (*c).tags = (*t).tags;
            } else {
                // NOTE must keep in sync with else below
                (*c).mon = state.selmon;
                applyrules(state, c);
                term = termforwin(state, c);
            }
        } else {
            // copied else case from above because the condition is supposed
            // to be xgettransientforhint && (t = wintoclient)
            (*c).mon = state.selmon;
            applyrules(state, c);
            term = termforwin(state, c);
        }
        if (*c).x + width(c) > ((*(*c).mon).wx + (*(*c).mon).ww) as i32 {
            (*c).x = ((*(*c).mon).wx + (*(*c).mon).ww) as i32 - width(c);
        }
        if (*c).y + height(c) > ((*(*c).mon).wy + (*(*c).mon).wh) as i32 {
            (*c).y = ((*(*c).mon).wy + (*(*c).mon).wh) as i32 - height(c);
        }
        (*c).x = max((*c).x, (*(*c).mon).wx as i32);
        (*c).y = max((*c).y, (*(*c).mon).wy as i32);
        (*c).bw = state.config.borderpx as i32;

        // TODO pretty sure this doesn't work with pertags, which explains some
        // behavior I saw before in dwm. probably need to operate on
        // selmon.pertag.tags[selmon.pertag.curtag].
        //
        // TODO I'm also pretty sure this is _not_ the right way to be handling
        // this. checking the name of the window and applying these rules seems
        // like something meant to be handled by RULES
        (*state.selmon).tagset[(*state.selmon).seltags as usize] &=
            !state.scratchtag();
        if (*c).name == state.config.scratchpadname {
            (*c).tags = state.scratchtag();
            (*(*c).mon).tagset[(*(*c).mon).seltags as usize] |= (*c).tags;
            (*c).isfloating = true;
            (*c).x = (*(*c).mon).wx + (*(*c).mon).ww / 2 - width(c) / 2;
            (*c).y = (*(*c).mon).wy + (*(*c).mon).wh / 2 - height(c) / 2;
        }

        log::trace!("manage: XWindowChanges");
        let mut wc = xlib::XWindowChanges {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            border_width: (*c).bw,
            sibling: 0,
            stack_mode: 0,
        };
        log::trace!("manage: XConfigureWindow");
        xlib::XConfigureWindow(state.dpy, w, CWBorderWidth as u32, &mut wc);
        log::trace!(
            "manage: XSetWindowBorder with state.dpy = {:?} and w = {w:?}",
            &raw const state.dpy
        );
        log::trace!("scheme: {:?}", &raw const state.scheme);
        let scheme_norm = &state.scheme[Scheme::Norm];
        log::trace!("scheme[SchemeNorm]: {scheme_norm:?}");
        let border = scheme_norm[Col::Border as usize];
        log::trace!("scheme[SchemeNorm][ColBorder]: {border:?}");
        let pixel = border.pixel;
        log::trace!("pixel = {pixel:?}");
        xlib::XSetWindowBorder(state.dpy, w, pixel);
        configure(state, c); // propagates border width, if size doesn't change
        updatewindowtype(state, c);
        updatesizehints(state, c);
        updatewmhints(state, c);
        xlib::XSelectInput(
            state.dpy,
            w,
            EnterWindowMask
                | FocusChangeMask
                | PropertyChangeMask
                | StructureNotifyMask,
        );
        grabbuttons(state, c, false);
        if !(*c).isfloating {
            (*c).oldstate = trans != 0 || (*c).isfixed != 0;
            (*c).isfloating = (*c).oldstate;
        }
        if (*c).isfloating {
            xlib::XRaiseWindow(state.dpy, (*c).win);
        }
        attach(c);
        attachstack(c);
        xlib::XChangeProperty(
            state.dpy,
            state.root,
            state.netatom[Net::ClientList as usize],
            XA_WINDOW,
            32,
            PropModeAppend,
            &((*c).win as c_uchar),
            1,
        );
        // some windows require this
        xlib::XMoveResizeWindow(
            state.dpy,
            (*c).win,
            (*c).x + 2 * state.sw,
            (*c).y,
            (*c).w as u32,
            (*c).h as u32,
        );
        setclientstate(state, c, NORMAL_STATE);
        if (*c).mon == state.selmon {
            unfocus(state, (*state.selmon).sel, false);
        }
        (*(*c).mon).sel = c;
        arrange(state, (*c).mon);
        xlib::XMapWindow(state.dpy, (*c).win);
        if !term.is_null() {
            swallow(state, term, c);
        }
        focus(state, std::ptr::null_mut());
    }
}

pub fn updatewmhints(state: &mut State, c: *mut Client) {
    log::trace!("updatewmhints");
    const URGENT: i64 = xlib::XUrgencyHint;
    unsafe {
        let wmh = xlib::XGetWMHints(state.dpy, (*c).win);
        if !wmh.is_null() {
            if c == (*state.selmon).sel && (*wmh).flags & URGENT != 0 {
                (*wmh).flags &= !URGENT;
                xlib::XSetWMHints(state.dpy, (*c).win, wmh);
            } else {
                (*c).isurgent = ((*wmh).flags & URGENT != 0) as bool as c_int;
            }
            if (*wmh).flags & InputHint != 0 {
                (*c).neverfocus = ((*wmh).input == 0) as c_int;
            } else {
                (*c).neverfocus = 0;
            }
            xlib::XFree(wmh.cast());
        }
    }
}

pub fn updatewindowtype(state: &mut State, c: *mut Client) {
    log::trace!("updatewindowtype");
    unsafe {
        let s = getatomprop(state, c, state.netatom[Net::WMState as usize]);
        let wtype =
            getatomprop(state, c, state.netatom[Net::WMWindowType as usize]);
        if s == state.netatom[Net::WMFullscreen as usize] {
            setfullscreen(state, c, true);
        }
        if wtype == state.netatom[Net::WMWindowTypeDialog as usize] {
            (*c).isfloating = true;
        }
    }
}

pub fn setfullscreen(state: &mut State, c: *mut Client, fullscreen: bool) {
    unsafe {
        if fullscreen && !(*c).isfullscreen {
            xlib::XChangeProperty(
                state.dpy,
                (*c).win,
                state.netatom[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                // trying to emulate (unsigned char*)&netatom[NetWMFullscreen],
                // so take a reference and then cast
                &state.netatom[Net::WMFullscreen as usize] as *const u64
                    as *const c_uchar,
                1,
            );
            (*c).isfullscreen = true;
            (*c).oldstate = (*c).isfloating;
            (*c).oldbw = (*c).bw;
            (*c).bw = 0;
            (*c).isfloating = true;
            resizeclient(
                state,
                c,
                (*(*c).mon).mx,
                (*(*c).mon).my,
                (*(*c).mon).mw,
                (*(*c).mon).mh,
            );
            xlib::XRaiseWindow(state.dpy, (*c).win);
        } else if !fullscreen && (*c).isfullscreen {
            xlib::XChangeProperty(
                state.dpy,
                (*c).win,
                state.netatom[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                std::ptr::null_mut::<c_uchar>(),
                0,
            );
            (*c).isfullscreen = false;
            (*c).isfloating = (*c).oldstate;
            (*c).bw = (*c).oldbw;
            (*c).x = (*c).oldx;
            (*c).y = (*c).oldy;
            (*c).w = (*c).oldw;
            (*c).h = (*c).oldh;
            resizeclient(state, c, (*c).x, (*c).y, (*c).w, (*c).h);
            arrange(state, (*c).mon);
        }
    }
}

pub fn getatomprop(state: &mut State, c: *mut Client, prop: Atom) -> Atom {
    let mut di = 0;
    let mut dl = 0;
    let mut p = std::ptr::null_mut();
    let mut da = 0;
    let mut atom: Atom = 0;
    unsafe {
        // FIXME (systray author) getatomprop should return the number of items
        // and a pointer to the stored data instead of this workaround
        let mut req = XA_ATOM;
        if prop == state.xatom[XEmbed::XEmbedInfo as usize] {
            req = state.xatom[XEmbed::XEmbedInfo as usize];
        }
        if xlib::XGetWindowProperty(
            state.dpy,
            (*c).win,
            prop,
            0,
            std::mem::size_of::<Atom>() as i64,
            False,
            req,
            &mut da,
            &mut di,
            &mut dl,
            &mut dl,
            &mut p,
        ) == Success as i32
            && !p.is_null()
        {
            // the C code is *(Atom *)p. is that different from (Atom) *p?
            // that's closer to what I had before
            atom = *(p as *mut Atom);
            if da == state.xatom[XEmbed::XEmbedInfo as usize] && dl == 2 {
                atom = *(p as *mut Atom).add(1);
            }
            XFree(p.cast());
        }
    }
    atom
}

// TODO this should really just be a method on Systray and called like
// state.systray.width()
pub fn getsystraywidth(state: &State) -> c_uint {
    unsafe {
        let mut w = 0;
        let mut i;
        if state.config.showsystray {
            cfor!((
                i = state.systray().icons;
            !i.is_null();
            (w, i) = (w + (*i).w + state.config.systrayspacing as i32, (*i).next))
            {});
        }
        if w != 0 {
            w as c_uint + state.config.systrayspacing
        } else {
            1
        }
    }
}

pub fn applyrules(state: &mut State, c: *mut Client) {
    log::trace!("applyrules");
    unsafe {
        let mut ch = xlib::XClassHint {
            res_name: std::ptr::null_mut(),
            res_class: std::ptr::null_mut(),
        };
        // rule matching
        (*c).isfloating = false;
        (*c).tags = 0;
        xlib::XGetClassHint(state.dpy, (*c).win, &mut ch);
        let class = if !ch.res_class.is_null() {
            CStr::from_ptr(ch.res_class)
        } else {
            BROKEN
        };
        let instance = if !ch.res_name.is_null() {
            CStr::from_ptr(ch.res_name)
        } else {
            BROKEN
        };

        for r in &state.config.rules {
            if (r.title.is_empty() || (*c).name.contains(&r.title))
                && (r.class.is_empty()
                    || class.to_string_lossy().contains(&r.class))
                && (r.instance.is_empty()
                    || instance.to_string_lossy().contains(&r.instance))
            {
                (*c).isterminal = r.isterminal;
                (*c).noswallow = r.noswallow;
                (*c).isfloating = r.isfloating;
                (*c).tags |= r.tags;
                let mut m = state.mons;
                while !m.is_null() && (*m).num != r.monitor {
                    m = (*m).next;
                }
                if !m.is_null() {
                    (*c).mon = m;
                }
            }
        }
        if !ch.res_class.is_null() {
            xlib::XFree(ch.res_class.cast());
        }
        if !ch.res_name.is_null() {
            xlib::XFree(ch.res_name.cast());
        }
        (*c).tags = if (*c).tags & state.tagmask() != 0 {
            (*c).tags & state.tagmask()
        } else {
            (*(*c).mon).tagset[(*(*c).mon).seltags as usize]
        };
    }
}

pub fn swallow(state: &mut State, p: *mut Client, c: *mut Client) {
    unsafe {
        let c = &mut *c;
        if c.noswallow || c.isterminal {
            return;
        }
        if c.noswallow && !state.config.swallowfloating && c.isfloating {
            return;
        }
        detach(c);
        detachstack(c);

        setclientstate(state, c, WITHDRAWN_STATE);
        let p = &mut *p;
        XUnmapWindow(state.dpy, p.win);
        p.swallowing = c;
        c.mon = p.mon;

        std::mem::swap(&mut p.win, &mut c.win);
        updatetitle(state, p);
        XMoveResizeWindow(state.dpy, p.win, p.x, p.y, p.w as u32, p.h as u32);
        arrange(state, p.mon);
        configure(state, p);
        updateclientlist(state);
    }
}

pub fn unswallow(state: &mut State, c: *mut Client) {
    unsafe {
        let c = &mut *c;

        c.win = (*c.swallowing).win;

        libc::free(c.swallowing.cast());
        c.swallowing = null_mut();

        // unfullscreen the client
        setfullscreen(state, c, false);
        updatetitle(state, c);
        arrange(state, c.mon);
        XMapWindow(state.dpy, c.win);
        XMoveResizeWindow(state.dpy, c.win, c.x, c.y, c.w as u32, c.h as u32);
        setclientstate(state, c, NORMAL_STATE);
        focus(state, null_mut());
        arrange(state, c.mon);
    }
}

pub const BUTTONMASK: i64 = ButtonPressMask | ButtonReleaseMask;
pub const MOUSEMASK: i64 = BUTTONMASK | PointerMotionMask;

pub fn updatetitle(state: &mut State, c: *mut Client) {
    log::trace!("updatetitle");
    unsafe {
        if gettextprop(
            state.dpy,
            (*c).win,
            state.netatom[Net::WMName as usize],
            &mut (*c).name,
        ) == 0
        {
            gettextprop(state.dpy, (*c).win, XA_WM_NAME, &mut (*c).name);
        }
        if (*c).name.is_empty() {
            /* hack to mark broken clients */
            (*c).name = BROKEN.to_string_lossy().to_string();
        }
    }
}

pub fn getstate(state: &mut State, w: Window) -> c_long {
    let mut format = 0;
    let mut result: c_long = -1;
    let mut p: *mut c_uchar = std::ptr::null_mut();
    let mut n = 0;
    let mut extra = 0;
    let mut real = 0;
    unsafe {
        let cond = xlib::XGetWindowProperty(
            state.dpy,
            w,
            state.wmatom[WM::State as usize],
            0,
            2,
            False,
            state.wmatom[WM::State as usize],
            &mut real,
            &mut format,
            &mut n,
            &mut extra,
            (&mut p) as *mut *mut c_uchar,
        );
        if cond != Success as i32 {
            return -1;
        }
        if n != 0 {
            result = *p as c_long;
        }
        XFree(p.cast());
        result
    }
}
