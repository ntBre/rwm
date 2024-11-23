//! tiling window manager based on dwm

use std::cmp::max;
use std::ffi::{c_char, c_int, c_uint, c_ulong, CStr};
use std::io::Read;
use std::mem::size_of_val;
use std::mem::{size_of, MaybeUninit};
use std::ptr::{addr_of, addr_of_mut, null_mut};
use std::sync::LazyLock;

use key_handlers::view;
use libc::{c_long, c_uchar, pid_t, sigaction};
use rwm::enums::XEmbed;
use x11::keysym::XK_Num_Lock;
use x11::xft::XftColor;
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

#[cfg(target_os = "linux")]
use xcb::Connection;

use rwm::{Arg, Client, Cursor, Layout, Monitor, Pertag, Systray, Window};

use config::CONFIG;
use drw::Drw;
use enums::{Clk, Col, Cur, Net, Scheme, WM};
use util::{die, ecalloc};

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
extern "C" fn xerrorstart(_: *mut Display, _: *mut XErrorEvent) -> c_int {
    panic!("another window manager is already running")
}

// from Xproto.h
const X_SET_INPUT_FOCUS: u8 = 42;
const X_POLY_TEXT_8: u8 = 74;
const X_POLY_FILL_RECTANGLE: u8 = 70;
const X_POLY_SEGMENT: u8 = 66;
const X_CONFIGURE_WINDOW: u8 = 12;
const X_GRAB_BUTTON: u8 = 28;
const X_GRAB_KEY: u8 = 33;
const X_COPY_AREA: u8 = 62;

// from cursorfont.h
const XC_LEFT_PTR: u8 = 68;
const XC_SIZING: u8 = 120;
const XC_FLEUR: u8 = 52;

// from X.h
// const BUTTON_RELEASE: i32 = 5;
const XNONE: c_long = 0;

// from Xutil.h
/// for windows that are not mapped
const WITHDRAWN_STATE: usize = 0;
/// most applications want to start this way
const NORMAL_STATE: usize = 1;
/// application wants to start as an icon
const ICONIC_STATE: usize = 3;

#[cfg(target_os = "linux")]
static mut XCON: *mut Connection = null_mut();

extern "C" fn xerror(mdpy: *mut Display, ee: *mut XErrorEvent) -> c_int {
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

extern "C" fn xerrordummy(
    _dpy: *mut Display,
    _ee: *mut xlib::XErrorEvent,
) -> c_int {
    0
}

/// I hate to start using globals already, but I'm not sure how else to do it.
/// maybe we can pack this stuff into a struct eventually
static mut XERRORXLIB: Option<
    unsafe extern "C" fn(*mut Display, *mut XErrorEvent) -> i32,
> = None;

static mut WMATOM: [Atom; WM::Last as usize] = [0; WM::Last as usize];
static mut NETATOM: [Atom; Net::Last as usize] = [0; Net::Last as usize];
static mut XATOM: [Atom; XEmbed::Last as usize] = [0; XEmbed::Last as usize];

static mut DPY: *mut Display = null_mut();

static mut DRW: *mut Drw = std::ptr::null_mut();

static mut SELMON: *mut Monitor = std::ptr::null_mut();
static mut MONS: *mut Monitor = null_mut();

static mut CURSOR: [*mut Cursor; Cur::Last as usize] =
    [null_mut(); Cur::Last as usize];

static mut SCHEME: *mut *mut Clr = null_mut();

fn get_scheme_color(scheme: *mut *mut Clr, i: usize, j: usize) -> Clr {
    unsafe { *(*scheme.add(i)).add(j) }
}

static mut SCREEN: c_int = 0;

static mut SYSTRAY: *mut Systray = null_mut();

const BROKEN: &CStr = c"broken";

static mut STEXT: [c_char; 256] = ['\0' as c_char; 256];

/// bar height
static mut BH: c_int = 0;

/// X display screen geometry width
static mut SW: c_int = 0;

/// X display screen geometry height
static mut SH: c_int = 0;

static mut ROOT: Window = 0;

static mut WMCHECKWIN: Window = 0;

static mut RUNNING: bool = true;

/// sum of left and right padding for text
static mut LRPAD: c_int = 0;

static mut NUMLOCKMASK: c_uint = 0;

type Atom = c_ulong;
type Clr = XftColor;

fn createmon() -> *mut Monitor {
    log::trace!("createmon");

    // I thought about trying to create a Monitor directly, followed by
    // Box::into_raw(Box::new(m)), but we use libc::free to free the Monitors
    // later. I'd have to replace that with Box::from_raw and allow it to drop
    // for that to work I think.
    let m: *mut Monitor = ecalloc(1, size_of::<Monitor>()).cast();

    unsafe {
        (*m).tagset[0] = 1;
        (*m).tagset[1] = 1;
        (*m).mfact = CONFIG.mfact;
        (*m).nmaster = CONFIG.nmaster;
        (*m).showbar = CONFIG.showbar;
        (*m).topbar = CONFIG.topbar;
        (*m).lt[0] = &CONFIG.layouts[0];
        (*m).lt[1] = &CONFIG.layouts[1 % CONFIG.layouts.len()];
        libc::strncpy(
            &mut (*m).ltsymbol as *mut _,
            CONFIG.layouts[0].symbol,
            size_of_val(&(*m).ltsymbol),
        );

        // NOTE: using this instead of ecalloc because it feels weird to
        // allocate a Vec that way, even though it worked in a separate test
        // program. remember to free with Box::from_raw instead of libc::free
        let pertag = Pertag {
            curtag: 1,
            prevtag: 1,
            nmasters: vec![(*m).nmaster; CONFIG.tags.len() + 1],
            mfacts: vec![(*m).mfact; CONFIG.tags.len() + 1],
            sellts: vec![(*m).sellt; CONFIG.tags.len() + 1],
            ltidxs: vec![(*m).lt; CONFIG.tags.len() + 1],
            showbars: vec![(*m).showbar; CONFIG.tags.len() + 1],
        };
        (*m).pertag = Box::into_raw(Box::new(pertag));
    }

    m
}

fn checkotherwm() {
    log::trace!("checkotherwm");
    unsafe {
        XERRORXLIB = XSetErrorHandler(Some(xerrorstart));
        xlib::XSelectInput(
            DPY,
            xlib::XDefaultRootWindow(DPY),
            SubstructureRedirectMask,
        );
        XSetErrorHandler(Some(xerror));
        xlib::XSync(DPY, False);
    }
}

fn setup() {
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

        SCREEN = xlib::XDefaultScreen(DPY);
        SW = xlib::XDisplayWidth(DPY, SCREEN);
        SH = xlib::XDisplayHeight(DPY, SCREEN);
        ROOT = xlib::XRootWindow(DPY, SCREEN);
        DRW = drw::create(DPY, SCREEN, ROOT, SW as u32, SH as u32);
        if drw::fontset_create(DRW, &CONFIG.fonts).is_null() {
            panic!("no fonts could be loaded");
        }
        LRPAD = (*(*DRW).fonts).h as i32;
        BH = (*(*DRW).fonts).h as i32 + 2;
        updategeom();

        /* init atoms */
        let utf8string = XInternAtom(DPY, c"UTF8_STRING".as_ptr(), False);
        WMATOM[WM::Protocols as usize] =
            XInternAtom(DPY, c"WM_PROTOCOLS".as_ptr(), False);
        WMATOM[WM::Delete as usize] =
            XInternAtom(DPY, c"WM_DELETE_WINDOW".as_ptr(), False);
        WMATOM[WM::State as usize] =
            XInternAtom(DPY, c"WM_STATE".as_ptr(), False);
        WMATOM[WM::TakeFocus as usize] =
            XInternAtom(DPY, c"WM_TAKE_FOCUS".as_ptr(), False);

        NETATOM[Net::ActiveWindow as usize] =
            XInternAtom(DPY, c"_NET_ACTIVE_WINDOW".as_ptr(), False);
        NETATOM[Net::Supported as usize] =
            XInternAtom(DPY, c"_NET_SUPPORTED".as_ptr(), False);

        NETATOM[Net::SystemTray as usize] =
            XInternAtom(DPY, c"_NET_SYSTEM_TRAY_S0".as_ptr(), False);
        NETATOM[Net::SystemTrayOP as usize] =
            XInternAtom(DPY, c"_NET_SYSTEM_TRAY_OPCODE".as_ptr(), False);
        NETATOM[Net::SystemTrayOrientation as usize] =
            XInternAtom(DPY, c"_NET_SYSTEM_TRAY_ORIENTATION".as_ptr(), False);
        NETATOM[Net::SystemTrayOrientationHorz as usize] = XInternAtom(
            DPY,
            c"_NET_SYSTEM_TRAY_ORIENTATION_HORZ".as_ptr(),
            False,
        );

        NETATOM[Net::WMName as usize] =
            XInternAtom(DPY, c"_NET_WM_NAME".as_ptr(), False);
        NETATOM[Net::WMState as usize] =
            XInternAtom(DPY, c"_NET_WM_STATE".as_ptr(), False);
        NETATOM[Net::WMCheck as usize] =
            XInternAtom(DPY, c"_NET_SUPPORTING_WM_CHECK".as_ptr(), False);
        NETATOM[Net::WMFullscreen as usize] =
            XInternAtom(DPY, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), False);
        NETATOM[Net::WMWindowType as usize] =
            XInternAtom(DPY, c"_NET_WM_WINDOW_TYPE".as_ptr(), False);
        NETATOM[Net::WMWindowTypeDialog as usize] =
            XInternAtom(DPY, c"_NET_WM_WINDOW_TYPE_DIALOG".as_ptr(), False);
        NETATOM[Net::ClientList as usize] =
            XInternAtom(DPY, c"_NET_CLIENT_LIST".as_ptr(), False);

        XATOM[XEmbed::Manager as usize] =
            XInternAtom(DPY, c"MANAGER".as_ptr(), False);
        XATOM[XEmbed::XEmbed as usize] =
            XInternAtom(DPY, c"_XEMBED".as_ptr(), False);
        XATOM[XEmbed::XEmbedInfo as usize] =
            XInternAtom(DPY, c"_XEMBED_INFO".as_ptr(), False);

        /* init cursors */
        CURSOR[Cur::Normal as usize] = drw::cur_create(DRW, XC_LEFT_PTR as i32);
        CURSOR[Cur::Resize as usize] = drw::cur_create(DRW, XC_SIZING as i32);
        CURSOR[Cur::Move as usize] = drw::cur_create(DRW, XC_FLEUR as i32);

        /* init appearance */
        SCHEME =
            util::ecalloc(CONFIG.colors.len(), size_of::<*mut Clr>()).cast();
        for i in 0..CONFIG.colors.len() {
            *SCHEME.add(i) = drw::scm_create(DRW, &CONFIG.colors[i], 3);
        }

        // init system tray
        updatesystray();

        /* init bars */
        updatebars();
        updatestatus();

        /* supporting window for NetWMCheck */
        WMCHECKWIN = xlib::XCreateSimpleWindow(DPY, ROOT, 0, 0, 1, 1, 0, 0, 0);
        xlib::XChangeProperty(
            DPY,
            WMCHECKWIN,
            NETATOM[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            addr_of_mut!(WMCHECKWIN) as *mut c_uchar,
            1,
        );
        xlib::XChangeProperty(
            DPY,
            WMCHECKWIN,
            NETATOM[Net::WMName as usize],
            utf8string,
            8,
            PropModeReplace,
            c"rwm".as_ptr() as *mut c_uchar,
            3,
        );
        xlib::XChangeProperty(
            DPY,
            ROOT,
            NETATOM[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            addr_of_mut!(WMCHECKWIN) as *mut c_uchar,
            1,
        );
        /* EWMH support per view */
        xlib::XChangeProperty(
            DPY,
            ROOT,
            NETATOM[Net::Supported as usize],
            XA_ATOM,
            32,
            PropModeReplace,
            &raw mut NETATOM as *mut c_uchar,
            Net::Last as i32,
        );
        xlib::XDeleteProperty(DPY, ROOT, NETATOM[Net::ClientList as usize]);

        // /* select events */
        wa.cursor = (*CURSOR[Cur::Normal as usize]).cursor;
        wa.event_mask = SubstructureRedirectMask
            | SubstructureNotifyMask
            | ButtonPressMask
            | PointerMotionMask
            | EnterWindowMask
            | LeaveWindowMask
            | StructureNotifyMask
            | PropertyChangeMask;
        xlib::XChangeWindowAttributes(
            DPY,
            ROOT,
            CWEventMask | CWCursor,
            &mut wa,
        );
        xlib::XSelectInput(DPY, ROOT, wa.event_mask);
        grabkeys();
        focus(null_mut());
    }
}

fn focus(mut c: *mut Client) {
    log::trace!("focus: c = {c:?}");
    unsafe {
        if c.is_null() || !is_visible(c) {
            c = (*SELMON).stack;
            while !c.is_null() && !is_visible(c) {
                c = (*c).snext;
            }
        }
        if !(*SELMON).sel.is_null() && (*SELMON).sel != c {
            unfocus((*SELMON).sel, false);
        }
        if !c.is_null() {
            if (*c).mon != SELMON {
                SELMON = (*c).mon;
            }
            if (*c).isurgent != 0 {
                seturgent(c, false);
            }
            detachstack(c);
            attachstack(c);
            grabbuttons(c, true);
            let color = (*(*SCHEME.offset(Scheme::Sel as isize))
                .offset(Col::Border as isize))
            .pixel;
            xlib::XSetWindowBorder(DPY, (*c).win, color);
            setfocus(c);
        } else {
            xlib::XSetInputFocus(DPY, ROOT, RevertToPointerRoot, CurrentTime);
            xlib::XDeleteProperty(
                DPY,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
            );
        }
        (*SELMON).sel = c;
        drawbars();
    }
}

fn drawbars() {
    log::trace!("drawbars");
    unsafe {
        let mut m = MONS;
        while !m.is_null() {
            drawbar(m);
            m = (*m).next;
        }
    }
}

fn setfocus(c: *mut Client) {
    log::trace!("setfocus");
    unsafe {
        if (*c).neverfocus == 0 {
            xlib::XSetInputFocus(
                DPY,
                (*c).win,
                RevertToPointerRoot,
                CurrentTime,
            );
            xlib::XChangeProperty(
                DPY,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
                XA_WINDOW,
                32,
                PropModeReplace,
                (&mut (*c).win) as *mut u64 as *mut c_uchar,
                1,
            );
        }
        sendevent(
            (*c).win,
            WMATOM[WM::TakeFocus as usize],
            NoEventMask as i32,
            WMATOM[WM::TakeFocus as usize] as i64,
            CurrentTime as i64,
            0,
            0,
            0,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn sendevent(
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
        if proto == WMATOM[WM::TakeFocus as usize]
            || proto == WMATOM[WM::Delete as usize]
        {
            mt = WMATOM[WM::Protocols as usize];
            if xlib::XGetWMProtocols(DPY, w, &mut protocols, &mut n) != 0 {
                while exists == 0 && n > 0 {
                    exists = (*protocols.offset(n as isize) == proto) as c_int;
                    n -= 1;
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
            xlib::XSendEvent(DPY, w, False, mask as i64, &mut ev);
        }
        exists
    }
}

fn grabbuttons(c: *mut Client, focused: bool) {
    log::trace!("grabbuttons");
    unsafe {
        updatenumlockmask();
        let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
        xlib::XUngrabButton(DPY, AnyButton as u32, AnyModifier, (*c).win);
        if !focused {
            xlib::XGrabButton(
                DPY,
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
        for button in &CONFIG.buttons {
            if button.click == Clk::ClientWin as u32 {
                for mod_ in modifiers {
                    xlib::XGrabButton(
                        DPY,
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

fn arrange(mut m: *mut Monitor) {
    log::trace!("arrange");
    unsafe {
        if !m.is_null() {
            showhide((*m).stack);
        } else {
            m = MONS;
            while !m.is_null() {
                showhide((*m).stack);
                m = (*m).next;
            }
        }

        if !m.is_null() {
            arrangemon(m);
            restack(m);
        } else {
            m = MONS;
            while !m.is_null() {
                arrangemon(m);
                m = (*m).next;
            }
        }
    }
}

fn arrangemon(m: *mut Monitor) {
    log::trace!("arrangemon");
    unsafe {
        libc::strncpy(
            (*m).ltsymbol.as_mut_ptr(),
            (*(*m).lt[(*m).sellt as usize]).symbol,
            size_of_val(&(*m).ltsymbol),
        );
        let arrange = (*(*m).lt[(*m).sellt as usize]).arrange;
        if let Some(arrange) = arrange {
            (arrange)(m);
        }
    }
}

fn restack(m: *mut Monitor) {
    log::trace!("restack");
    drawbar(m);
    unsafe {
        if (*m).sel.is_null() {
            return;
        }
        if (*(*m).sel).isfloating
            || (*(*m).lt[(*m).sellt as usize]).arrange.is_none()
        {
            xlib::XRaiseWindow(DPY, (*(*m).sel).win);
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
                        DPY,
                        (*c).win,
                        (CWSibling | CWStackMode) as c_uint,
                        &mut wc as *mut _,
                    );
                    wc.sibling = (*c).win;
                }
                c = (*c).snext;
            }
        }
        xlib::XSync(DPY, False);
        let mut ev = xlib::XEvent { type_: 0 };
        while xlib::XCheckMaskEvent(DPY, EnterWindowMask, &mut ev) != 0 {}
    }
}

fn showhide(c: *mut Client) {
    log::trace!("showhide");
    unsafe {
        if c.is_null() {
            return;
        }
        if is_visible(c) {
            // show clients top down
            xlib::XMoveWindow(DPY, (*c).win, (*c).x, (*c).y);
            if ((*(*(*c).mon).lt[(*(*c).mon).sellt as usize])
                .arrange
                .is_none()
                || (*c).isfloating)
                && !(*c).isfullscreen
            {
                resize(c, (*c).x, (*c).y, (*c).w, (*c).h, 0);
            }
            showhide((*c).snext);
        } else {
            // hide clients bottom up
            showhide((*c).snext);
            xlib::XMoveWindow(DPY, (*c).win, width(c) * -2, (*c).y);
        }
    }
}

fn resize(
    c: *mut Client,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    interact: c_int,
) {
    log::trace!("resize");
    if applysizehints(c, &mut x, &mut y, &mut w, &mut h, interact) != 0 {
        resizeclient(c, x, y, w, h);
    }
}

fn resizeclient(c: *mut Client, x: i32, y: i32, w: i32, h: i32) {
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
            DPY,
            (*c).win,
            (CWX | CWY | CWWidth | CWHeight | CWBorderWidth) as u32,
            &mut wc,
        );
        configure(c);
        xlib::XSync(DPY, False);
    }
}

fn resizebarwin(m: *mut Monitor) {
    unsafe {
        let mut w = (*m).ww;
        if CONFIG.showsystray && m == systraytomon(m) && !CONFIG.systrayonleft {
            w -= getsystraywidth() as i32;
        }
        XMoveResizeWindow(
            DPY,
            (*m).barwin,
            (*m).wx,
            (*m).by,
            w as u32,
            BH as u32,
        );
    }
}

fn configure(c: *mut Client) {
    log::trace!("configure");
    unsafe {
        let mut ce = xlib::XConfigureEvent {
            type_: x11::xlib::ConfigureNotify,
            serial: 0,
            send_event: 0,
            display: DPY,
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
            DPY,
            (*c).win,
            False,
            StructureNotifyMask,
            &mut ce as *mut xlib::XConfigureEvent as *mut xlib::XEvent,
        );
    }
}

fn applysizehints(
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
            if *x > SW {
                *x = SW - width(c);
            }
            if *y > SH {
                *y = SH - height(c);
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
        if *h < BH {
            *h = BH;
        }
        if *w < BH {
            *w = BH;
        }
        if CONFIG.resize_hints
            || (*c).isfloating
            || (*(*(*c).mon).lt[(*(*c).mon).sellt as usize])
                .arrange
                .is_none()
        {
            if (*c).hintsvalid == 0 {
                updatesizehints(c);
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

fn updatesizehints(c: *mut Client) {
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
        if xlib::XGetWMNormalHints(DPY, (*c).win, &mut size, &mut msize) == 0 {
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

fn pop(c: *mut Client) {
    log::trace!("pop");
    detach(c);
    attach(c);
    focus(c);
    unsafe {
        arrange((*c).mon);
    }
}

fn detach(c: *mut Client) {
    log::trace!("detach");
    unsafe {
        let mut tc: *mut *mut Client = &mut (*(*c).mon).clients;
        while !(*tc).is_null() && *tc != c {
            tc = &mut (*(*tc)).next;
        }
        *tc = (*c).next;
    }
}

fn nexttiled(mut c: *mut Client) -> *mut Client {
    log::trace!("nexttiled");
    unsafe {
        while !c.is_null() && ((*c).isfloating || !is_visible(c)) {
            c = (*c).next;
        }
        c
    }
}

fn grabkeys() {
    log::trace!("grabkeys");
    unsafe {
        updatenumlockmask();
        let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
        let (mut start, mut end, mut skip): (i32, i32, i32) = (0, 0, 0);
        xlib::XUngrabKey(DPY, AnyKey, AnyModifier, ROOT);
        xlib::XDisplayKeycodes(DPY, &mut start, &mut end);
        let syms = xlib::XGetKeyboardMapping(
            DPY,
            start as u8,
            end - start + 1,
            &mut skip,
        );
        if syms.is_null() {
            return;
        }
        for k in start..=end {
            for key in &CONFIG.keys {
                // skip modifier codes, we do that ourselves
                if key.keysym
                    == (*syms.offset(((k - start) * skip) as isize)) as u64
                {
                    for m in modifiers {
                        xlib::XGrabKey(
                            DPY,
                            k,
                            key.mod_ | m,
                            ROOT,
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

fn updatenumlockmask() {
    log::trace!("updatenumlockmask");
    unsafe {
        NUMLOCKMASK = 0;
        let modmap = xlib::XGetModifierMapping(DPY);
        for i in 0..8 {
            for j in 0..(*modmap).max_keypermod {
                if *(*modmap)
                    .modifiermap
                    .offset((i * (*modmap).max_keypermod + j) as isize)
                    == xlib::XKeysymToKeycode(DPY, XK_Num_Lock as u64)
                {
                    NUMLOCKMASK = 1 << i;
                }
            }
        }
        xlib::XFreeModifiermap(modmap);
    }
}

fn seturgent(c: *mut Client, urg: bool) {
    log::trace!("seturgent");
    unsafe {
        (*c).isurgent = urg as c_int;
        let wmh = xlib::XGetWMHints(DPY, (*c).win);
        if wmh.is_null() {
            return;
        }
        (*wmh).flags = if urg {
            (*wmh).flags | xlib::XUrgencyHint
        } else {
            (*wmh).flags & !{ xlib::XUrgencyHint }
        };
        xlib::XSetWMHints(DPY, (*c).win, wmh);
        XFree(wmh.cast());
    }
}

fn unfocus(c: *mut Client, setfocus: bool) {
    log::trace!("unfocus");
    if c.is_null() {
        return;
    }
    grabbuttons(c, false);
    unsafe {
        // scheme[SchemeNorm][ColBorder].pixel
        let color = (*(*SCHEME.offset(Scheme::Norm as isize))
            .offset(Col::Border as isize))
        .pixel;
        xlib::XSetWindowBorder(DPY, (*c).win, color);
        if setfocus {
            xlib::XSetInputFocus(DPY, ROOT, RevertToPointerRoot, CurrentTime);
            xlib::XDeleteProperty(
                DPY,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
            );
        }
    }
}

fn updatestatus() {
    log::trace!("updatestatus");
    unsafe {
        if gettextprop(
            ROOT,
            XA_WM_NAME,
            // cast pointer to the array itself as a pointer to the first
            // element, safe??
            addr_of_mut!(STEXT) as *mut _,
            // the lint leading to this instead of simply &stext is very scary,
            // but hopefully it's fine
            size_of_val(&*addr_of!(STEXT)) as u32,
        ) == 0
        {
            libc::strcpy(addr_of_mut!(STEXT) as *mut _, c"rwm-1.0".as_ptr());
        }
        drawbar(SELMON);
        updatesystray();
    }
}

fn updatesystrayicongeom(i: *mut Client, w: c_int, h: c_int) {
    if i.is_null() {
        return;
    }
    unsafe {
        let i = &mut *i;
        i.h = BH;
        if w == h {
            i.w = BH;
        } else if h == BH {
            i.w = w;
        } else {
            i.w = (BH as f32 * (w as f32 / h as f32)) as i32;
        }
        applysizehints(i, &mut i.x, &mut i.y, &mut i.w, &mut i.h, False);
        // force icons into the systray dimensions if they don't want to
        if i.h > BH {
            if i.w == i.h {
                i.w = BH;
            } else {
                i.w = (BH as f32 * (i.w as f32 / i.h as f32)) as i32;
            }
            i.h = BH;
        }
    }
}

fn updatesystrayiconstate(i: *mut Client, ev: *mut XPropertyEvent) {
    unsafe {
        let mut flags: Atom = 0;
        let code;
        if !CONFIG.showsystray
            || i.is_null()
            || (*ev).atom != XATOM[XEmbed::XEmbedInfo as usize]
        {
            flags = getatomprop(i, XATOM[XEmbed::XEmbedInfo as usize]);
            if flags == 0 {
                return;
            }
        }
        let i = &mut *i;
        if flags & XEMBED_MAPPED != 0 && i.tags == 0 {
            i.tags = 1;
            code = XEMBED_WINDOW_ACTIVATE;
            XMapRaised(DPY, i.win);
            setclientstate(i, NORMAL_STATE);
        } else if (flags & XEMBED_MAPPED) == 0 && i.tags != 0 {
            i.tags = 0;
            code = XEMBED_WINDOW_DEACTIVATE;
            XUnmapWindow(DPY, i.win);
            setclientstate(i, WITHDRAWN_STATE);
        } else {
            return;
        }
        sendevent(
            i.win,
            XATOM[XEmbed::XEmbed as usize],
            StructureNotifyMask as i32,
            CurrentTime as i64,
            code as i64,
            0,
            (*SYSTRAY).win as i64,
            XEMBED_EMBEDDED_VERSION as i64,
        );
    }
}

const fn default_window_attributes() -> XSetWindowAttributes {
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

fn updatesystray() {
    unsafe {
        let mut wa = default_window_attributes();
        let mut wc: XWindowChanges;
        let mut i: *mut Client;
        let m: *mut Monitor = systraytomon(null_mut());
        let mut x: c_int = (*m).mx + (*m).mw;
        let sw = textw(addr_of!(STEXT) as *const _) - LRPAD
            + CONFIG.systrayspacing as i32;
        let mut w = 1;

        if !CONFIG.showsystray {
            return;
        }
        if CONFIG.systrayonleft {
            x -= sw + LRPAD / 2;
        }
        if SYSTRAY.is_null() {
            // init systray
            SYSTRAY = ecalloc(1, size_of::<Systray>()).cast();
            (*SYSTRAY).win = XCreateSimpleWindow(
                DPY,
                ROOT,
                x,
                (*m).by,
                w,
                BH as u32,
                0,
                0,
                get_scheme_color(
                    SCHEME,
                    Scheme::Sel as usize,
                    Col::Bg as usize,
                )
                .pixel,
            );
            wa.event_mask = ButtonPressMask | ExposureMask;
            wa.override_redirect = True;
            wa.background_pixel = get_scheme_color(
                SCHEME,
                Scheme::Norm as usize,
                Col::Bg as usize,
            )
            .pixel;
            XSelectInput(DPY, (*SYSTRAY).win, SubstructureNotifyMask);
            XChangeProperty(
                DPY,
                (*SYSTRAY).win,
                NETATOM[Net::SystemTrayOrientation as usize],
                XA_CARDINAL,
                32,
                PropModeReplace,
                &NETATOM[Net::SystemTrayOrientationHorz as usize] as *const _
                    as *const _,
                1,
            );
            XChangeWindowAttributes(
                DPY,
                (*SYSTRAY).win,
                CWEventMask | CWOverrideRedirect | CWBackPixel,
                &mut wa,
            );
            XMapRaised(DPY, (*SYSTRAY).win);
            XSetSelectionOwner(
                DPY,
                NETATOM[Net::SystemTray as usize],
                (*SYSTRAY).win,
                CurrentTime,
            );
            if XGetSelectionOwner(DPY, NETATOM[Net::SystemTray as usize])
                == (*SYSTRAY).win
            {
                sendevent(
                    ROOT,
                    XATOM[XEmbed::Manager as usize],
                    StructureNotifyMask as i32,
                    CurrentTime as i64,
                    NETATOM[Net::SystemTray as usize] as i64,
                    (*SYSTRAY).win as i64,
                    0_i64,
                    0_i64,
                );
                XSync(DPY, False);
            } else {
                log::error!("unable to obtain system tray");
                libc::free(SYSTRAY.cast());
                SYSTRAY = null_mut();
                return;
            }
        } // end if !SYSTRAY
        cfor!(((w, i) = (0, (*SYSTRAY).icons);
        !i.is_null();
        i = (*i).next) {
            // make sure the background color stays the same
            wa.background_pixel = get_scheme_color(SCHEME, Scheme::Norm as usize, Col::Bg as usize).pixel;
            XChangeWindowAttributes(DPY, (*i).win, CWBackPixel, &mut wa);
            XMapRaised(DPY, (*i).win);
            w += CONFIG.systrayspacing;
            (*i).x = w as i32;
            XMoveResizeWindow(DPY, (*i).win, (*i).x, 0, (*i).w as u32, (*i).h as u32);
            w += (*i).w as u32;
            if (*i).mon != m {
                (*i).mon = m;
            }
        });
        w = if w != 0 { w + CONFIG.systrayspacing } else { 1 };
        x -= w as i32;
        XMoveResizeWindow(DPY, (*SYSTRAY).win, x, (*m).by, w, BH as u32);
        wc = XWindowChanges {
            x,
            y: (*m).by,
            width: w as i32,
            height: BH,
            border_width: 0,
            sibling: (*m).barwin,
            stack_mode: Above,
        };
        XConfigureWindow(
            DPY,
            (*SYSTRAY).win,
            (CWX | CWY | CWWidth | CWHeight | CWSibling | CWStackMode) as u32,
            &mut wc,
        );
        XMapWindow(DPY, (*SYSTRAY).win);
        XMapSubwindows(DPY, (*SYSTRAY).win);
        // redraw background
        XSetForeground(
            DPY,
            (*DRW).gc,
            get_scheme_color(SCHEME, Scheme::Norm as usize, Col::Bg as usize)
                .pixel,
        );
        XFillRectangle(DPY, (*SYSTRAY).win, (*DRW).gc, 0, 0, w, BH as u32);
        XSync(DPY, False);
    } // end unsafe
}

fn wintosystrayicon(w: Window) -> *mut Client {
    unsafe {
        let mut i = null_mut();
        if !CONFIG.showsystray || w == 0 {
            return i;
        }
        cfor!((i = (*SYSTRAY).icons; !i.is_null() && (*i).win != w;
            i = (*i).next) {});

        i
    }
}

fn systraytomon(m: *mut Monitor) -> *mut Monitor {
    unsafe {
        let mut t: *mut Monitor;
        let mut i;
        let mut n;
        if CONFIG.systraypinning == 0 {
            if m.is_null() {
                return SELMON;
            }
            if m == SELMON {
                return m;
            } else {
                return null_mut();
            }
        }
        cfor!(((n, t) = (1, MONS);
            !t.is_null() && !(*t).next.is_null();
            (n, t) = (n+1, (*t).next)) {});
        cfor!(((i, t) = (1, MONS);
            !t.is_null() && !(*t).next.is_null() && i < CONFIG.systraypinning;
            (i, t) = (i+1, (*t).next)) {});
        if CONFIG.systraypinningfailfirst && n < CONFIG.systraypinning {
            return MONS;
        }

        t
    }
}

fn textw(x: *const c_char) -> c_int {
    log::trace!("textw");
    unsafe { drw::fontset_getwidth(DRW, x) as c_int + LRPAD }
}

fn drawbar(m: *mut Monitor) {
    log::trace!("drawbar");
    unsafe {
        let mut tw = 0;
        let mut stw = 0;
        let boxs = (*(*DRW).fonts).h / 9;
        let boxw = (*(*DRW).fonts).h / 6 + 2;
        let (mut occ, mut urg) = (0, 0);

        if CONFIG.showsystray && m == systraytomon(m) && !CONFIG.systrayonleft {
            stw = getsystraywidth();
        }

        if !(*m).showbar {
            return;
        }

        // draw status first so it can be overdrawn by tags later
        if m == SELMON {
            // status is only drawn on selected monitor
            drw::setscheme(DRW, *SCHEME.add(Scheme::Norm as usize));
            tw = textw(addr_of!(STEXT) as *const _) - LRPAD / 2 + 2; // 2px right padding
            drw::text(
                DRW,
                (*m).ww - tw - stw as i32,
                0,
                tw as u32,
                BH as u32,
                (LRPAD / 2 - 2) as u32,
                addr_of!(STEXT) as *const _,
                0,
            );
        }

        resizebarwin(m);

        let mut c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent != 0 {
                urg |= (*c).tags;
            }
            c = (*c).next;
        }

        let mut x = 0;
        for (i, tag) in CONFIG.tags.iter().enumerate() {
            let text = tag.to_owned();
            let w = textw(text.as_ptr());
            drw::setscheme(
                DRW,
                *SCHEME.add(
                    if ((*m).tagset[(*m).seltags as usize] & 1 << i) != 0 {
                        Scheme::Sel as usize
                    } else {
                        Scheme::Norm as usize
                    },
                ),
            );
            drw::text(
                DRW,
                x,
                0,
                w as u32,
                BH as u32,
                LRPAD as u32 / 2,
                text.as_ptr(),
                (urg as i32) & 1 << i,
            );

            if (occ & 1 << i) != 0 {
                drw::rect(
                    DRW,
                    x + boxs as i32,
                    boxs as i32,
                    boxw,
                    boxw,
                    (m == SELMON
                        && !(*SELMON).sel.is_null()
                        && ((*(*SELMON).sel).tags & 1 << i) != 0)
                        as c_int,
                    (urg & 1 << i) as c_int,
                );
            }
            x += w as i32;
        }

        let w = textw((*m).ltsymbol.as_ptr());
        drw::setscheme(DRW, *SCHEME.add(Scheme::Norm as usize));
        x = drw::text(
            DRW,
            x,
            0,
            w as u32,
            BH as u32,
            LRPAD as u32 / 2,
            (*m).ltsymbol.as_ptr(),
            0,
        ) as i32;

        let w = (*m).ww - tw - stw as i32 - x;
        if w > BH {
            if !(*m).sel.is_null() {
                drw::setscheme(
                    DRW,
                    *SCHEME.offset(if m == SELMON {
                        Scheme::Sel as isize
                    } else {
                        Scheme::Norm as isize
                    }),
                );
                drw::text(
                    DRW,
                    x,
                    0,
                    w as u32,
                    BH as u32,
                    LRPAD as u32 / 2,
                    (*(*m).sel).name.as_ptr(),
                    0,
                );
                if (*(*m).sel).isfloating {
                    drw::rect(
                        DRW,
                        x + boxs as i32,
                        boxs as i32,
                        boxw,
                        boxw,
                        (*(*m).sel).isfixed,
                        0,
                    );
                }
            } else {
                drw::setscheme(DRW, *SCHEME.add(Scheme::Norm as usize));
                drw::rect(DRW, x, 0, w as u32, BH as u32, 1, 1);
            }
        }
        drw::map(DRW, (*m).barwin, 0, 0, (*m).ww as u32 - stw, BH as u32);
    }
}

fn gettextprop(w: Window, atom: Atom, text: *mut i8, size: u32) -> c_int {
    log::trace!("gettextprop");
    unsafe {
        if text.is_null() || size == 0 {
            return 0;
        }
        *text = '\0' as i8;
        let mut name = xlib::XTextProperty {
            value: std::ptr::null_mut(),
            encoding: 0,
            format: 0,
            nitems: 0,
        };
        let c = xlib::XGetTextProperty(DPY, w, &mut name, atom);
        if c == 0 || name.nitems == 0 {
            return 0;
        }

        let mut n = 0;
        let mut list: *mut *mut i8 = std::ptr::null_mut();
        if name.encoding == XA_STRING {
            libc::strncpy(text, name.value as *mut _, size as usize - 1);
        } else if xlib::XmbTextPropertyToTextList(
            DPY,
            &name,
            &mut list,
            &mut n as *mut _,
        ) >= Success as i32
            && n > 0
            && !(*list).is_null()
        {
            libc::strncpy(text, *list, size as usize - 1);
            xlib::XFreeStringList(list);
        }
        let p = text.offset(size as isize - 1);
        *p = '\0' as i8;
        xlib::XFree(name.value as *mut _);
    }
    1
}

fn updatebars() {
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
        let mut m = MONS;
        while !m.is_null() {
            if (*m).barwin != 0 {
                continue;
            }
            let mut w = (*m).ww;
            if CONFIG.showsystray && m == systraytomon(m) {
                w -= getsystraywidth() as i32;
            }
            (*m).barwin = xlib::XCreateWindow(
                DPY,
                ROOT,
                (*m).wx as c_int,
                (*m).by as c_int,
                w as c_uint,
                BH as c_uint,
                0,
                xlib::XDefaultDepth(DPY, SCREEN),
                CopyFromParent as c_uint,
                xlib::XDefaultVisual(DPY, SCREEN),
                CWOverrideRedirect | CWBackPixmap | CWEventMask,
                &mut wa,
            );
            xlib::XDefineCursor(
                DPY,
                (*m).barwin,
                (*CURSOR[Cur::Normal as usize]).cursor,
            );
            if CONFIG.showsystray && m == systraytomon(m) {
                xlib::XMapRaised(DPY, (*SYSTRAY).win);
            }
            xlib::XMapRaised(DPY, (*m).barwin);
            xlib::XSetClassHint(DPY, (*m).barwin, &mut ch);
            m = (*m).next;
        }
    }
}

fn updategeom() -> i32 {
    log::trace!("updategeom");
    unsafe {
        let mut dirty = 0;
        if x11::xinerama::XineramaIsActive(DPY) != 0 {
            log::trace!("updategeom: xinerama active");

            let mut nn = 0;
            let info =
                x11::xinerama::XineramaQueryScreens(DPY as *mut _, &mut nn);

            let mut n = 0;
            let mut m = MONS;
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
                let mut m = MONS;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }
                if !m.is_null() {
                    (*m).next = createmon();
                } else {
                    MONS = createmon();
                }
            }

            let mut i = 0;
            let mut m = MONS;
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

                    updatebarpos(m);
                }
                m = (*m).next;
                i += 1;
            }

            // removed monitors if n > nn
            if n > nn {
                log::trace!("updategeom: removing monitors");
            }
            for _ in nn..n {
                let mut m = MONS;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }
                let mut c = (*m).clients;
                while !c.is_null() {
                    dirty = 1;
                    (*m).clients = (*c).next;
                    detachstack(c);
                    (*c).mon = MONS;
                    attach(c);
                    attachstack(c);
                    c = (*m).clients;
                }
                if m == SELMON {
                    SELMON = MONS;
                }
                cleanupmon(m);
            }
            libc::free(unique.as_mut_ptr().cast());
        } else {
            log::trace!("updategeom: default monitor setup");

            // default monitor setup
            if MONS.is_null() {
                MONS = createmon();
            }
            if (*MONS).mw != SW || (*MONS).mh != SH {
                dirty = 1;
                (*MONS).mw = SW;
                (*MONS).ww = SW;
                (*MONS).mh = SH;
                (*MONS).wh = SH;
                updatebarpos(MONS);
            }
        }
        if dirty != 0 {
            SELMON = MONS;
            SELMON = wintomon(ROOT);
        }
        dirty
    }
}

fn wintomon(w: Window) -> *mut Monitor {
    log::trace!("wintomon");
    unsafe {
        let mut x = 0;
        let mut y = 0;
        if w == ROOT && getrootptr(&mut x, &mut y) != 0 {
            return recttomon(x, y, 1, 1);
        }
        let mut m = MONS;
        while !m.is_null() {
            if w == (*m).barwin {
                return m;
            }
            m = (*m).next;
        }
        let c = wintoclient(w);
        if !c.is_null() {
            return (*c).mon;
        }
        SELMON
    }
}

fn wintoclient(w: u64) -> *mut Client {
    log::trace!("wintoclient");
    unsafe {
        let mut m = MONS;
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

fn recttomon(x: c_int, y: c_int, w: c_int, h: c_int) -> *mut Monitor {
    log::trace!("recttomon");
    unsafe {
        let mut r = SELMON;
        let mut area = 0;
        let mut m = MONS;
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

fn removesystrayicon(i: *mut Client) {
    unsafe {
        if !CONFIG.showsystray || i.is_null() {
            return;
        }
        let mut ii: *mut *mut Client;
        cfor!((
            ii = &mut (*SYSTRAY).icons as *mut _;
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
fn intersect(x: c_int, y: c_int, w: c_int, h: c_int, m: *mut Monitor) -> c_int {
    use std::cmp::{max, min};
    unsafe {
        max(0, min((x) + (w), (*m).wx + (*m).ww) - max(x, (*m).wx))
            * max(0, min((y) + (h), (*m).wy + (*m).wh) - max(y, (*m).wy))
    }
}

#[inline]
fn width(x: *mut Client) -> i32 {
    unsafe { (*x).w + 2 * (*x).bw }
}

#[inline]
fn height(x: *mut Client) -> i32 {
    unsafe { (*x).h + 2 * (*x).bw }
}

#[inline]
fn cleanmask(mask: u32) -> u32 {
    unsafe {
        mask & !(NUMLOCKMASK | LockMask)
            & (ShiftMask
                | ControlMask
                | Mod1Mask
                | Mod2Mask
                | Mod3Mask
                | Mod4Mask
                | Mod5Mask)
    }
}

fn getrootptr(x: *mut c_int, y: *mut c_int) -> c_int {
    unsafe {
        let mut di = 0;
        let mut dui = 0;
        let mut dummy = 0;
        xlib::XQueryPointer(
            DPY, ROOT, &mut dummy, &mut dummy, x, y, &mut di, &mut di, &mut dui,
        )
    }
}

/// remove `mon` from the linked list of `Monitor`s in `MONS` and free it.
fn cleanupmon(mon: *mut Monitor) {
    unsafe {
        if mon == MONS {
            MONS = (*MONS).next;
        } else {
            let mut m = MONS;
            while !m.is_null() && (*m).next != mon {
                m = (*m).next;
            }
            (*m).next = (*mon).next;
        }
        xlib::XUnmapWindow(DPY, (*mon).barwin);
        xlib::XDestroyWindow(DPY, (*mon).barwin);
        libc::free(mon.cast());
    }
}

fn attachstack(c: *mut Client) {
    log::trace!("attachstack");
    unsafe {
        (*c).snext = (*(*c).mon).stack;
        (*(*c).mon).stack = c;
    }
}

fn attach(c: *mut Client) {
    log::trace!("attach");
    unsafe {
        (*c).next = (*(*c).mon).clients;
        (*(*c).mon).clients = c;
    }
}

fn detachstack(c: *mut Client) {
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
fn is_visible(c: *const Client) -> bool {
    unsafe {
        ((*c).tags & (*(*c).mon).tagset[(*(*c).mon).seltags as usize]) != 0
    }
}

fn updatebarpos(m: *mut Monitor) {
    log::trace!("updatebarpos");

    unsafe {
        (*m).wy = (*m).my;
        (*m).wh = (*m).mh;
        if (*m).showbar {
            (*m).wh -= BH;
            (*m).by = if (*m).topbar { (*m).wy } else { (*m).wy + (*m).wh };
            (*m).wy = if (*m).topbar { (*m).wy + BH } else { (*m).wy };
        } else {
            (*m).by = -BH;
        }
    }
}

fn isuniquegeom(
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

fn cleanup() {
    log::trace!("entering cleanup");

    unsafe {
        let a = Arg::Ui(!0);
        view(&a);
        (*SELMON).lt[(*SELMON).sellt as usize] =
            &Layout { symbol: c"".as_ptr(), arrange: None };

        let mut m = MONS;
        while !m.is_null() {
            while !(*m).stack.is_null() {
                unmanage((*m).stack, 0);
            }
            m = (*m).next;
        }

        xlib::XUngrabKey(DPY, AnyKey, AnyModifier, ROOT);

        while !MONS.is_null() {
            cleanupmon(MONS);
        }

        if CONFIG.showsystray {
            XUnmapWindow(DPY, (*SYSTRAY).win);
            XDestroyWindow(DPY, (*SYSTRAY).win);
            libc::free(SYSTRAY.cast());
        }

        for cur in CURSOR {
            drw::cur_free(DRW, cur);
        }

        // free each element in scheme (*mut *mut Clr), then free scheme itself
        for i in 0..CONFIG.colors.len() {
            let tmp: *mut Clr = *SCHEME.add(i);
            libc::free(tmp.cast());
        }
        libc::free(SCHEME.cast());

        xlib::XDestroyWindow(DPY, WMCHECKWIN);
        drw::free(DRW);
        xlib::XSync(DPY, False);
        xlib::XSetInputFocus(
            DPY,
            PointerRoot as u64,
            RevertToPointerRoot,
            CurrentTime,
        );
        xlib::XDeleteProperty(DPY, ROOT, NETATOM[Net::ActiveWindow as usize]);
    }

    log::trace!("finished cleanup");
}

fn unmanage(c: *mut Client, destroyed: c_int) {
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
            unswallow(c);
            return;
        }

        let s = swallowingclient((*c).win);
        if !s.is_null() {
            libc::free((*s).swallowing.cast());
            (*s).swallowing = null_mut();
            arrange(m);
            focus(null_mut());
            return;
        }

        detach(c);
        detachstack(c);
        if destroyed == 0 {
            wc.border_width = (*c).oldbw;
            xlib::XGrabServer(DPY); /* avoid race conditions */
            xlib::XSetErrorHandler(Some(xerrordummy));
            xlib::XSelectInput(DPY, (*c).win, NoEventMask);
            xlib::XConfigureWindow(
                DPY,
                (*c).win,
                CWBorderWidth as u32,
                &mut wc,
            ); /* restore border */
            xlib::XUngrabButton(DPY, AnyButton as u32, AnyModifier, (*c).win);
            setclientstate(c, WITHDRAWN_STATE);
            xlib::XSync(DPY, False);
            xlib::XSetErrorHandler(Some(xerror));
            xlib::XUngrabServer(DPY);
        }
        libc::free(c.cast());

        if s.is_null() {
            arrange(m);
            focus(null_mut());
            updateclientlist();
        }
    }
}

/// I'm just using the OpenBSD version of the code in the patch rather than the
/// Linux version that uses XCB
fn winpid(w: Window) -> pid_t {
    let mut result = 0;

    #[cfg(target_os = "linux")]
    unsafe {
        log::trace!("winpid linux");

        let spec = xcb::res::ClientIdSpec {
            client: w as u32,
            mask: xcb::res::ClientIdMask::LOCAL_CLIENT_PID,
        };
        assert!(!XCON.is_null(), "xcon is null");
        let xcon = &*XCON;
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
            DPY,
            w,
            XInternAtom(DPY, c"_NET_WM_PID".as_ptr(), 0),
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
        result = ret;
    }

    result
}

/// this looks insane... rust has std::os::unix::process::parent_id, but it
/// doesn't take any arguments. we need to get the parent of a specific process
/// here, so we read from /proc
fn getparentprocess(p: pid_t) -> pid_t {
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

fn isdescprocess(p: pid_t, mut c: pid_t) -> pid_t {
    while p != c && c != 0 {
        c = getparentprocess(c);
    }
    c
}

fn termforwin(w: *const Client) -> *mut Client {
    unsafe {
        let w = &*w;

        if w.pid == 0 || w.isterminal {
            return null_mut();
        }

        let mut c;
        let mut m;

        cfor!((m = MONS; !m.is_null(); m = (*m).next) {
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

fn swallowingclient(w: Window) -> *mut Client {
    unsafe {
        let mut c;
        let mut m;

        cfor!((m = MONS; !m.is_null(); m = (*m).next) {
            cfor!((c = (*m).clients; !c.is_null(); c = (*c).next) {
                if !(*c).swallowing.is_null() && (*(*c).swallowing).win == w {
                    return c;
                }
            });
        });

        null_mut()
    }
}

fn updateclientlist() {
    unsafe {
        xlib::XDeleteProperty(DPY, ROOT, NETATOM[Net::ClientList as usize]);
        let mut m = MONS;
        while !m.is_null() {
            let mut c = (*m).clients;
            while !c.is_null() {
                xlib::XChangeProperty(
                    DPY,
                    ROOT,
                    NETATOM[Net::ClientList as usize],
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

fn setclientstate(c: *mut Client, state: usize) {
    let mut data: [c_long; 2] = [state as c_long, XNONE as c_long];
    let ptr: *mut c_uchar = data.as_mut_ptr().cast();
    unsafe {
        xlib::XChangeProperty(
            DPY,
            (*c).win,
            WMATOM[WM::State as usize],
            WMATOM[WM::State as usize],
            32,
            PropModeReplace,
            ptr,
            2,
        );
    }
}

static HANDLER: LazyLock<
    [fn(*mut xlib::XEvent); x11::xlib::LASTEvent as usize],
> = LazyLock::new(|| {
    fn dh(_ev: *mut xlib::XEvent) {}
    let mut ret = [dh as fn(*mut xlib::XEvent); x11::xlib::LASTEvent as usize];
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
fn run() {
    unsafe {
        xlib::XSync(DPY, False);
        let mut ev: MaybeUninit<xlib::XEvent> = MaybeUninit::uninit();
        while RUNNING && xlib::XNextEvent(DPY, ev.as_mut_ptr()) == 0 {
            let mut ev: xlib::XEvent = ev.assume_init();
            if let Some(handler) = HANDLER.get(ev.type_ as usize) {
                handler(&mut ev);
            }
        }
    }
}

fn scan() {
    let mut num = 0;
    let mut d1 = 0;
    let mut d2 = 0;
    let mut wins: *mut Window = std::ptr::null_mut();
    let mut wa: MaybeUninit<xlib::XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        if xlib::XQueryTree(
            DPY,
            ROOT,
            &mut d1,
            &mut d2,
            &mut wins as *mut _,
            &mut num,
        ) != 0
        {
            for i in 0..num {
                if xlib::XGetWindowAttributes(
                    DPY,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                    || (*wa.as_mut_ptr()).override_redirect != 0
                    || xlib::XGetTransientForHint(
                        DPY,
                        *wins.offset(i as isize),
                        &mut d1,
                    ) != 0
                {
                    continue;
                }
                if (*wa.as_mut_ptr()).map_state == IsViewable
                    || getstate(*wins.offset(i as isize)) == ICONIC_STATE as i64
                {
                    manage(*wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            for i in 0..num {
                // now the transients
                if xlib::XGetWindowAttributes(
                    DPY,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                {
                    continue;
                }
                if xlib::XGetTransientForHint(
                    DPY,
                    *wins.offset(i as isize),
                    &mut d1,
                ) != 0
                    && ((*wa.as_mut_ptr()).map_state == IsViewable
                        || getstate(*wins.offset(i as isize))
                            == ICONIC_STATE as i64)
                {
                    manage(*wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            if !wins.is_null() {
                XFree(wins.cast());
            }
        }
    }
}

fn manage(w: Window, wa: *mut xlib::XWindowAttributes) {
    log::trace!("manage");
    let mut trans = 0;
    unsafe {
        let wa = *wa;
        let c: *mut Client = util::ecalloc(1, size_of::<Client>()) as *mut _;
        (*c).win = w;
        (*c).pid = winpid(w);
        (*c).x = wa.x;
        (*c).oldx = wa.x;
        (*c).y = wa.y;
        (*c).oldy = wa.y;
        (*c).w = wa.width;
        (*c).oldw = wa.width;
        (*c).h = wa.height;
        (*c).oldh = wa.height;
        (*c).oldbw = wa.border_width;

        let mut term: *mut Client = null_mut();

        updatetitle(c);
        log::trace!("manage: XGetTransientForHint");
        if xlib::XGetTransientForHint(DPY, w, &mut trans) != 0 {
            let t = wintoclient(trans);
            if !t.is_null() {
                (*c).mon = (*t).mon;
                (*c).tags = (*t).tags;
            } else {
                // NOTE must keep in sync with else below
                (*c).mon = SELMON;
                applyrules(c);
                term = termforwin(c);
            }
        } else {
            // copied else case from above because the condition is supposed
            // to be xgettransientforhint && (t = wintoclient)
            (*c).mon = SELMON;
            applyrules(c);
            term = termforwin(c);
        }
        if (*c).x + width(c) > ((*(*c).mon).wx + (*(*c).mon).ww) as i32 {
            (*c).x = ((*(*c).mon).wx + (*(*c).mon).ww) as i32 - width(c);
        }
        if (*c).y + height(c) > ((*(*c).mon).wy + (*(*c).mon).wh) as i32 {
            (*c).y = ((*(*c).mon).wy + (*(*c).mon).wh) as i32 - height(c);
        }
        (*c).x = max((*c).x, (*(*c).mon).wx as i32);
        (*c).y = max((*c).y, (*(*c).mon).wy as i32);
        (*c).bw = CONFIG.borderpx as i32;

        // TODO pretty sure this doesn't work with pertags, which explains some
        // behavior I saw before in dwm. probably need to operate on
        // selmon.pertag.tags[selmon.pertag.curtag].
        //
        // TODO I'm also pretty sure this is _not_ the right way to be handling
        // this. checking the name of the window and applying these rules seems
        // like something meant to be handled by RULES
        (*SELMON).tagset[(*SELMON).seltags as usize] &= !*SCRATCHTAG;
        if libc::strcmp((*c).name.as_ptr(), CONFIG.scratchpadname.as_ptr()) == 0
        {
            (*c).tags = *SCRATCHTAG;
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
        xlib::XConfigureWindow(DPY, w, CWBorderWidth as u32, &mut wc);
        log::trace!(
            "manage: XSetWindowBorder with DPY = {:?} and w = {w:?}",
            &raw const DPY
        );
        log::trace!("scheme: {:?}", &raw const SCHEME);
        let scheme_norm: *mut Clr = *SCHEME.offset(Scheme::Norm as isize);
        log::trace!("scheme[SchemeNorm]: {scheme_norm:?}");
        let border: Clr = *scheme_norm.offset(Col::Border as isize);
        log::trace!("scheme[SchemeNorm][ColBorder]: {border:?}");
        let pixel = border.pixel;
        log::trace!("pixel = {pixel:?}");
        xlib::XSetWindowBorder(DPY, w, pixel);
        configure(c); // propagates border width, if size doesn't change
        updatewindowtype(c);
        updatesizehints(c);
        updatewmhints(c);
        xlib::XSelectInput(
            DPY,
            w,
            EnterWindowMask
                | FocusChangeMask
                | PropertyChangeMask
                | StructureNotifyMask,
        );
        grabbuttons(c, false);
        if !(*c).isfloating {
            (*c).oldstate = trans != 0 || (*c).isfixed != 0;
            (*c).isfloating = (*c).oldstate;
        }
        if (*c).isfloating {
            xlib::XRaiseWindow(DPY, (*c).win);
        }
        attach(c);
        attachstack(c);
        xlib::XChangeProperty(
            DPY,
            ROOT,
            NETATOM[Net::ClientList as usize],
            XA_WINDOW,
            32,
            PropModeAppend,
            &((*c).win as c_uchar),
            1,
        );
        // some windows require this
        xlib::XMoveResizeWindow(
            DPY,
            (*c).win,
            (*c).x + 2 * SW,
            (*c).y,
            (*c).w as u32,
            (*c).h as u32,
        );
        setclientstate(c, NORMAL_STATE);
        if (*c).mon == SELMON {
            unfocus((*SELMON).sel, false);
        }
        (*(*c).mon).sel = c;
        arrange((*c).mon);
        xlib::XMapWindow(DPY, (*c).win);
        if !term.is_null() {
            swallow(term, c);
        }
        focus(std::ptr::null_mut());
    }
}

fn updatewmhints(c: *mut Client) {
    log::trace!("updatewmhints");
    const URGENT: i64 = xlib::XUrgencyHint;
    unsafe {
        let wmh = xlib::XGetWMHints(DPY, (*c).win);
        if !wmh.is_null() {
            if c == (*SELMON).sel && (*wmh).flags & URGENT != 0 {
                (*wmh).flags &= !URGENT;
                xlib::XSetWMHints(DPY, (*c).win, wmh);
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

fn updatewindowtype(c: *mut Client) {
    log::trace!("updatewindowtype");
    unsafe {
        let state = getatomprop(c, NETATOM[Net::WMState as usize]);
        let wtype = getatomprop(c, NETATOM[Net::WMWindowType as usize]);
        if state == NETATOM[Net::WMFullscreen as usize] {
            setfullscreen(c, true);
        }
        if wtype == NETATOM[Net::WMWindowTypeDialog as usize] {
            (*c).isfloating = true;
        }
    }
}

fn setfullscreen(c: *mut Client, fullscreen: bool) {
    unsafe {
        if fullscreen && !(*c).isfullscreen {
            xlib::XChangeProperty(
                DPY,
                (*c).win,
                NETATOM[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                // trying to emulate (unsigned char*)&netatom[NetWMFullscreen],
                // so take a reference and then cast
                &mut NETATOM[Net::WMFullscreen as usize] as *mut u64
                    as *mut c_uchar,
                1,
            );
            (*c).isfullscreen = true;
            (*c).oldstate = (*c).isfloating;
            (*c).oldbw = (*c).bw;
            (*c).bw = 0;
            (*c).isfloating = true;
            resizeclient(
                c,
                (*(*c).mon).mx,
                (*(*c).mon).my,
                (*(*c).mon).mw,
                (*(*c).mon).mh,
            );
            xlib::XRaiseWindow(DPY, (*c).win);
        } else if !fullscreen && (*c).isfullscreen {
            xlib::XChangeProperty(
                DPY,
                (*c).win,
                NETATOM[Net::WMState as usize],
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
            resizeclient(c, (*c).x, (*c).y, (*c).w, (*c).h);
            arrange((*c).mon);
        }
    }
}

fn getatomprop(c: *mut Client, prop: Atom) -> Atom {
    let mut di = 0;
    let mut dl = 0;
    let mut p = std::ptr::null_mut();
    let mut da = 0;
    let mut atom: Atom = 0;
    unsafe {
        // FIXME (systray author) getatomprop should return the number of items
        // and a pointer to the stored data instead of this workaround
        let mut req = XA_ATOM;
        if prop == XATOM[XEmbed::XEmbedInfo as usize] {
            req = XATOM[XEmbed::XEmbedInfo as usize];
        }
        if xlib::XGetWindowProperty(
            DPY,
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
            if da == XATOM[XEmbed::XEmbedInfo as usize] && dl == 2 {
                atom = *(p as *mut Atom).add(1);
            }
            XFree(p.cast());
        }
    }
    atom
}

fn getsystraywidth() -> c_uint {
    unsafe {
        let mut w = 0;
        let mut i;
        if CONFIG.showsystray {
            cfor!((
            i = (*SYSTRAY).icons;
            !i.is_null();
            (w, i) = (w + (*i).w + config::CONFIG.systrayspacing as i32, (*i).next))
            {});
        }
        if w != 0 {
            w as c_uint + CONFIG.systrayspacing
        } else {
            1
        }
    }
}

fn applyrules(c: *mut Client) {
    log::trace!("applyrules");
    unsafe {
        let mut ch = xlib::XClassHint {
            res_name: std::ptr::null_mut(),
            res_class: std::ptr::null_mut(),
        };
        // rule matching
        (*c).isfloating = false;
        (*c).tags = 0;
        xlib::XGetClassHint(DPY, (*c).win, &mut ch);
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

        for r in &CONFIG.rules {
            if (r.title.is_null()
                || !libc::strstr((*c).name.as_ptr(), r.title).is_null())
                && (r.class.is_null()
                    || !libc::strstr(class.as_ptr(), r.class).is_null())
                && (r.instance.is_null()
                    || !libc::strstr(instance.as_ptr(), r.instance).is_null())
            {
                (*c).isterminal = r.isterminal;
                (*c).noswallow = r.noswallow;
                (*c).isfloating = r.isfloating;
                (*c).tags |= r.tags;
                let mut m = MONS;
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
        (*c).tags = if (*c).tags & *TAGMASK != 0 {
            (*c).tags & *TAGMASK
        } else {
            (*(*c).mon).tagset[(*(*c).mon).seltags as usize]
        };
    }
}

fn swallow(p: *mut Client, c: *mut Client) {
    unsafe {
        let c = &mut *c;
        if c.noswallow || c.isterminal {
            return;
        }
        if c.noswallow && !CONFIG.swallowfloating && c.isfloating {
            return;
        }
        detach(c);
        detachstack(c);

        setclientstate(c, WITHDRAWN_STATE);
        let p = &mut *p;
        XUnmapWindow(DPY, p.win);
        p.swallowing = c;
        c.mon = p.mon;

        std::mem::swap(&mut p.win, &mut c.win);
        updatetitle(p);
        XMoveResizeWindow(DPY, p.win, p.x, p.y, p.w as u32, p.h as u32);
        arrange(p.mon);
        configure(p);
        updateclientlist();
    }
}

fn unswallow(c: *mut Client) {
    unsafe {
        let c = &mut *c;

        c.win = (*c.swallowing).win;

        libc::free(c.swallowing.cast());
        c.swallowing = null_mut();

        // unfullscreen the client
        setfullscreen(c, false);
        updatetitle(c);
        arrange(c.mon);
        XMapWindow(DPY, c.win);
        XMoveResizeWindow(DPY, c.win, c.x, c.y, c.w as u32, c.h as u32);
        setclientstate(c, NORMAL_STATE);
        focus(null_mut());
        arrange(c.mon);
    }
}

// #define TAGMASK                 ((1 << LENGTH(tags)) - 1)
static TAGMASK: LazyLock<u32> = LazyLock::new(|| (1 << CONFIG.tags.len()) - 1);
const BUTTONMASK: i64 = ButtonPressMask | ButtonReleaseMask;
const MOUSEMASK: i64 = BUTTONMASK | PointerMotionMask;

static SCRATCHTAG: LazyLock<u32> = LazyLock::new(|| 1 << CONFIG.tags.len());

fn updatetitle(c: *mut Client) {
    log::trace!("updatetitle");
    unsafe {
        if gettextprop(
            (*c).win,
            NETATOM[Net::WMName as usize],
            &mut (*c).name as *mut _,
            size_of_val(&(*c).name) as u32,
        ) == 0
        {
            gettextprop(
                (*c).win,
                XA_WM_NAME,
                &mut (*c).name as *mut _,
                size_of_val(&(*c).name) as u32,
            );
        }
        if (*c).name[0] == '\0' as i8 {
            /* hack to mark broken clients */
            libc::strcpy(
                &mut (*c).name as *mut _,
                BROKEN.as_ptr() as *const c_char,
            );
        }
    }
}

fn getstate(w: Window) -> c_long {
    let mut format = 0;
    let mut result: c_long = -1;
    let mut p: *mut c_uchar = std::ptr::null_mut();
    let mut n = 0;
    let mut extra = 0;
    let mut real = 0;
    unsafe {
        let cond = xlib::XGetWindowProperty(
            DPY,
            w,
            WMATOM[WM::State as usize],
            0,
            2,
            False,
            WMATOM[WM::State as usize],
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

mod config;
mod drw;
pub use rwm::enums;
use xembed::{
    XEMBED_EMBEDDED_VERSION, XEMBED_MAPPED, XEMBED_WINDOW_ACTIVATE,
    XEMBED_WINDOW_DEACTIVATE,
};
mod handlers;
mod key_handlers;
mod layouts;
mod util;
mod xembed;

fn main() {
    env_logger::init();
    // a bit weird but demand config load at startup
    let _ = CONFIG;
    unsafe {
        DPY = xlib::XOpenDisplay(std::ptr::null_mut());
        if DPY.is_null() {
            die("rwm: cannot open display");
        }
        #[cfg(target_os = "linux")]
        {
            let Ok((xcon, _)) = Connection::connect(None) else {
                die("rwm: cannot get xcb connection");
            };
            XCON = Box::into_raw(Box::new(xcon));
        }
    }
    checkotherwm();
    setup();
    scan();
    run();
    cleanup();
    unsafe {
        xlib::XCloseDisplay(DPY);

        #[cfg(target_os = "linux")]
        drop(Box::from_raw(XCON));
    }
}
