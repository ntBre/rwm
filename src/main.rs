//! tiling window manager based on dwm

#![allow(unused)]
#![feature(vec_into_raw_parts, lazy_cell)]
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

mod bindgen {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(improper_ctypes)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::cmp::{max, min};
use std::ffi::{c_int, c_ulong, CString};
use std::fs::File;
use std::mem::{size_of, MaybeUninit};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::Command;
use std::ptr::{addr_of, addr_of_mut, null_mut};

use config::{
    COLORS, DMENUMON, FONTS, KEYS, LAYOUTS, MFACT, NMASTER, SHOWBAR, TOPBAR,
};
use drw::Drw;
use libc::{
    abs, c_uchar, c_uint, calloc, memcpy, sigaction, sigemptyset, waitpid,
    SA_NOCLDSTOP, SA_NOCLDWAIT, SA_RESTART, SIGCHLD, SIG_IGN, WNOHANG,
};
use x11::keysym::XK_Num_Lock;
use x11::xft::XftColor;
use x11::xinerama::{
    XineramaIsActive, XineramaQueryScreens, XineramaScreenInfo,
};
use x11::xlib::{
    AnyButton, AnyKey, AnyModifier, BadAccess, BadAtom, BadDrawable, BadMatch,
    BadWindow, Below, ButtonPress, ButtonPressMask, ButtonReleaseMask,
    CWBackPixmap, CWBorderWidth, CWCursor, CWEventMask, CWHeight,
    CWOverrideRedirect, CWSibling, CWStackMode, CWWidth, ClientMessage,
    ConfigureNotify, ConfigureRequest, ControlMask, CopyFromParent,
    CurrentTime, DestroyAll, DestroyNotify, EnterNotify, EnterWindowMask,
    Expose, ExposureMask, False, FocusChangeMask, FocusIn, GrabModeAsync,
    GrabModeSync, GrabSuccess, InputHint, IsViewable, KeyPress, KeySym,
    LeaveNotify, LeaveWindowMask, LockMask, MapNotify, MapRequest,
    MappingKeyboard, MappingNotify, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask,
    Mod5Mask, MotionNotify, NoEventMask, NoExpose, NotifyInferior,
    NotifyNormal, PAspect, PBaseSize, PMaxSize, PMinSize, PResizeInc, PSize,
    ParentRelative, PointerMotionMask, PointerRoot, PropModeAppend,
    PropModeReplace, PropertyChangeMask, PropertyDelete, PropertyNotify,
    ReplayPointer, RevertToPointerRoot, ShiftMask, StructureNotifyMask,
    SubstructureNotifyMask, SubstructureRedirectMask, Success, True,
    UnmapNotify, XAllowEvents, XChangeProperty, XChangeWindowAttributes,
    XCheckMaskEvent, XClassHint, XCloseDisplay, XConfigureEvent,
    XConfigureWindow, XCreateSimpleWindow, XCreateWindow, XDefaultDepth,
    XDefaultRootWindow, XDefaultScreen, XDefaultVisual, XDefineCursor,
    XDeleteProperty, XDestroyWindow, XDisplayHeight, XDisplayKeycodes,
    XDisplayWidth, XEvent, XFree, XFreeModifiermap, XFreeStringList,
    XGetClassHint, XGetKeyboardMapping, XGetModifierMapping, XGetTextProperty,
    XGetTransientForHint, XGetWMHints, XGetWMNormalHints, XGetWMProtocols,
    XGetWindowAttributes, XGetWindowProperty, XGrabButton, XGrabKey,
    XGrabPointer, XGrabServer, XInternAtom, XKeycodeToKeysym, XKeysymToKeycode,
    XKillClient, XMapRaised, XMapWindow, XMaskEvent, XMoveResizeWindow,
    XMoveWindow, XNextEvent, XQueryPointer, XQueryTree, XRaiseWindow,
    XRefreshKeyboardMapping, XRootWindow, XSelectInput, XSendEvent,
    XSetClassHint, XSetCloseDownMode, XSetInputFocus, XSetWMHints,
    XSetWindowAttributes, XSetWindowBorder, XSizeHints, XSync, XUngrabButton,
    XUngrabKey, XUngrabPointer, XUngrabServer, XUnmapWindow, XUrgencyHint,
    XWarpPointer, XWindowAttributes, XWindowChanges, XmbTextPropertyToTextList,
    CWX, CWY, XA_ATOM, XA_STRING, XA_WINDOW, XA_WM_HINTS, XA_WM_NAME,
    XA_WM_NORMAL_HINTS, XA_WM_TRANSIENT_FOR,
};
use x11::xlib::{BadAlloc, BadValue, Display as XDisplay};
use x11::xlib::{XErrorEvent, XOpenDisplay, XSetErrorHandler};

use crate::bindgen::dpy;
use crate::config::{
    BORDERPX, BUTTONS, DMENUCMD, LOCKFULLSCREEN, RESIZEHINTS, RULES, SNAP, TAGS,
};

pub struct Display {
    inner: *mut XDisplay,
}

impl Display {
    fn open() -> Self {
        let inner = unsafe { XOpenDisplay(std::ptr::null()) };
        if inner.is_null() {
            panic!("cannot open display");
        }
        Display { inner }
    }
}

/// function to be called on a startup error
extern "C" fn xerrorstart(_: *mut XDisplay, _: *mut XErrorEvent) -> c_int {
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
const BUTTON_RELEASE: i32 = 5;

// from Xutil.h
/// for windows that are not mapped
const WITHDRAWN_STATE: usize = 0;
/// most applications want to start this way
const NORMAL_STATE: usize = 1;
/// application wants to start as an icon
const ICONIC_STATE: usize = 3;

extern "C" fn xerror(mdpy: *mut XDisplay, ee: *mut XErrorEvent) -> c_int {
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

extern "C" fn xerrordummy(_dpy: *mut XDisplay, _ee: *mut XErrorEvent) -> c_int {
    0
}

/// I hate to start using globals already, but I'm not sure how else to do it.
/// maybe we can pack this stuff into a struct eventually
static mut XERRORXLIB: Option<
    unsafe extern "C" fn(*mut XDisplay, *mut XErrorEvent) -> i32,
> = None;

static mut SELMON: *mut Monitor = std::ptr::null_mut();

static mut MONS: *mut Monitor = std::ptr::null_mut();

static mut DRW: *mut Drw = std::ptr::null_mut();

static mut SCREEN: i32 = 0;

const BROKEN: &str = "broken";
static mut STEXT: String = String::new();

/// bar height
static mut BH: i16 = 0;
static mut SW: c_int = 0;
static mut SH: c_int = 0;

static mut ROOT: Window = 0;
static mut WMCHECKWIN: Window = 0;

static mut WMATOM: [Atom; WM::Last as usize] = [0; WM::Last as usize];
static mut NETATOM: [Atom; Net::Last as usize] = [0; Net::Last as usize];

static mut RUNNING: bool = true;

static mut CURSOR: [Cursor; Cur::Last as usize] = [0; Cur::Last as usize];

/// color scheme
static mut SCHEME: Vec<Vec<Clr>> = Vec::new();

/// sum of left and right padding for text
static mut LRPAD: usize = 0;

static mut NUMLOCKMASK: u32 = 0;
const BUTTONMASK: i64 = ButtonPressMask | ButtonReleaseMask;

const TAGMASK: usize = (1 << TAGS.len()) - 1;
const MOUSEMASK: i64 = BUTTONMASK | PointerMotionMask;

#[derive(Clone)]
pub enum Arg {
    Uint(usize),
    Int(isize),
    Float(f64),
    Str(&'static [&'static str]),
    Layout(&'static Layout),
    None,
}

pub struct Button {
    pub click: Clk,
    pub mask: u32,
    pub button: u32,
    pub func: fn(mdpy: &Display, arg: Arg),
    pub arg: Arg,
}

impl Button {
    pub const fn new(
        click: Clk,
        mask: u32,
        button: u32,
        func: fn(mdpy: &Display, arg: Arg),
        arg: Arg,
    ) -> Self {
        Self {
            click,
            mask,
            button,
            func,
            arg,
        }
    }
}

struct Client {
    name: String,
    mina: f64,
    maxa: f64,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    oldx: i32,
    oldy: i32,
    oldw: i32,
    oldh: i32,
    basew: i32,
    baseh: i32,
    incw: i32,
    inch: i32,
    maxw: i32,
    maxh: i32,
    minw: i32,
    minh: i32,
    hintsvalid: bool,
    bw: i32,
    oldbw: i32,
    tags: usize,
    isfixed: bool,
    isfloating: bool,
    isurgent: bool,
    neverfocus: bool,
    oldstate: bool,
    isfullscreen: bool,
    next: *mut Client,
    snext: *mut Client,
    mon: *mut Monitor,
    win: Window,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            name: Default::default(),
            mina: Default::default(),
            maxa: Default::default(),
            x: Default::default(),
            y: Default::default(),
            w: Default::default(),
            h: Default::default(),
            oldx: Default::default(),
            oldy: Default::default(),
            oldw: Default::default(),
            oldh: Default::default(),
            basew: Default::default(),
            baseh: Default::default(),
            incw: Default::default(),
            inch: Default::default(),
            maxw: Default::default(),
            maxh: Default::default(),
            minw: Default::default(),
            minh: Default::default(),
            hintsvalid: Default::default(),
            bw: Default::default(),
            oldbw: Default::default(),
            tags: Default::default(),
            isfixed: Default::default(),
            isfloating: Default::default(),
            isurgent: Default::default(),
            neverfocus: Default::default(),
            oldstate: Default::default(),
            isfullscreen: Default::default(),
            next: std::ptr::null_mut(),
            snext: std::ptr::null_mut(),
            mon: std::ptr::null_mut(),
            win: Default::default(),
        }
    }
}

type Window = u64;
type Atom = u64;
type Cursor = u64;
type Clr = XftColor;

pub struct Key {
    pub modkey: u32,
    pub keysym: u32,
    pub func: fn(mdpy: &Display, arg: Arg),
    pub arg: Arg,
}

impl Key {
    pub const fn new(
        modkey: u32,
        keysym: u32,
        func: fn(mdpy: &Display, arg: Arg),
        arg: Arg,
    ) -> Self {
        Self {
            modkey,
            keysym,
            func,
            arg,
        }
    }
}

#[derive(PartialEq)]
pub struct Layout {
    symbol: &'static str,
    arrange: Option<fn(mdpy: &Display, mon: *mut Monitor)>,
}

pub struct Monitor {
    ltsymbol: String,
    mfact: f64,
    nmaster: i32,
    num: i32,
    /// bar geometry
    by: i16,
    /// screen size
    mx: i16,
    my: i16,
    mw: i16,
    mh: i16,
    /// window area
    wx: i16,
    wy: i16,
    ww: i16,
    wh: i16,
    seltags: usize,
    sellt: usize,
    tagset: [usize; 2],
    showbar: bool,
    topbar: bool,
    clients: *mut Client,
    /// index into clients vec, pointer in C
    sel: *mut Client,
    stack: *mut Client,
    next: *mut Monitor,
    barwin: Window,
    lt: [*const Layout; 2],
}

impl Monitor {
    fn new() -> Self {
        Self {
            ltsymbol: LAYOUTS[0].symbol.to_owned(),
            mfact: MFACT,
            nmaster: NMASTER,
            num: 0,
            by: 0,
            mx: 0,
            my: 0,
            mw: 0,
            mh: 0,
            wx: 0,
            wy: 0,
            ww: 0,
            wh: 0,
            seltags: 0,
            sellt: 0,
            tagset: [1, 1],
            showbar: SHOWBAR,
            topbar: TOPBAR,
            clients: std::ptr::null_mut(),
            sel: std::ptr::null_mut(),
            stack: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            barwin: 0,
            lt: [&LAYOUTS[0], &LAYOUTS[1 % LAYOUTS.len()]],
        }
    }
}

pub struct Rule {
    pub class: Option<&'static str>,
    pub instance: Option<&'static str>,
    pub title: Option<&'static str>,
    pub tags: usize,
    pub isfloating: bool,
    pub monitor: isize,
}

impl Rule {
    pub const fn new(
        class: Option<&'static str>,
        instance: Option<&'static str>,
        title: Option<&'static str>,
        tags: usize,
        isfloating: bool,
        monitor: isize,
    ) -> Self {
        Self {
            class,
            instance,
            title,
            tags,
            isfloating,
            monitor,
        }
    }
}

fn createmon() -> *mut Monitor {
    let mon = Monitor::new();
    Box::into_raw(Box::new(mon))
}

fn checkotherwm() {
    unsafe {
        XERRORXLIB = XSetErrorHandler(Some(xerrorstart));
        bindgen::XSelectInput(
            dpy,
            bindgen::XDefaultRootWindow(dpy),
            SubstructureRedirectMask,
        );
        XSetErrorHandler(Some(xerror));
        bindgen::XSync(dpy, False);
    }
}

#[derive(Debug)]
#[repr(C)]
enum WM {
    Protocols,
    Delete,
    State,
    TakeFocus,
    Last,
}

#[repr(C)]
enum Net {
    Supported,
    WMName,
    WMState,
    WMCheck,
    WMFullscreen,
    ActiveWindow,
    WMWindowType,
    WMWindowTypeDialog,
    ClientList,
    Last,
}

#[repr(C)]
enum Cur {
    Normal,
    Resize,
    Move,
    Last,
}

#[repr(C)]
enum Scheme {
    Norm,
    Sel,
}

/// Color scheme index
#[repr(C)]
enum Col {
    Fg,
    Bg,
    Border,
}

#[derive(Debug, PartialEq)]
#[repr(C)]
pub enum Clk {
    TagBar,
    LtSymbol,
    StatusText,
    WinTitle,
    ClientWin,
    RootWin,
    Last,
}

fn setup() {
    unsafe { bindgen::setup() }
    // let mut sa: MaybeUninit<sigaction> = MaybeUninit::uninit();
    // let mut wa: MaybeUninit<XSetWindowAttributes> = MaybeUninit::uninit();

    // unsafe {
    //     // do not transform children into zombies when they terminate
    //     {
    //         let sa = sa.as_mut_ptr();
    //         sigemptyset(&mut (*sa).sa_mask);
    //         (*sa).sa_flags = SA_NOCLDSTOP | SA_NOCLDWAIT | SA_RESTART;
    //         (*sa).sa_sigaction = SIG_IGN;
    //     }
    //     let sa = sa.assume_init();
    //     sigaction(SIGCHLD, &sa, null_mut());

    //     // clean up any zombies (inherited from .xinitrc etc) immediately
    //     while waitpid(-1, null_mut(), WNOHANG) > 0 {}

    //     // init screen
    //     SCREEN = XDefaultScreen(mdpy.inner);
    //     SW = XDisplayWidth(mdpy.inner, SCREEN) as c_int;
    //     SH = XDisplayHeight(mdpy.inner, SCREEN) as c_int;
    //     ROOT = XRootWindow(mdpy.inner, SCREEN);
    //     DRW = Box::into_raw(Box::new(Drw::new(
    //         mdpy,
    //         SCREEN,
    //         ROOT,
    //         SW as usize,
    //         SH as usize,
    //     )));

    //     let drw = DRW.as_mut().unwrap();

    //     drw.fontset_create(FONTS).expect("no fonts could be loaded");
    //     LRPAD = (*(*DRW).fonts).h;
    //     BH = ((*(*DRW).fonts).h + 2) as i16;
    //     updategeom(mdpy);

    //     // init atoms - I really hope these CStrings live long enough.
    //     let utf8_string: CString = CString::new("UTF8_STRING").unwrap();
    //     let utf8string = XInternAtom(mdpy.inner, utf8_string.as_ptr(), False);

    //     for (k, s) in [
    //         (WM::Protocols, "WM_PROTOCOLS"),
    //         (WM::Delete, "WM_DELETE_WINDOW"),
    //         (WM::State, "WM_STATE"),
    //         (WM::TakeFocus, "WM_TAKE_FOCUS"),
    //     ] {
    //         let s = CString::new(s).unwrap();
    //         let v = XInternAtom(mdpy.inner, s.as_ptr(), False);
    //         if v == BadAlloc as u64 || v == BadValue as u64 {
    //             panic!("XInternAtom failed with {v}");
    //         }
    //         WMATOM[k as usize] = v;
    //     }

    //     for (k, s) in [
    //         (Net::ActiveWindow, "_NET_ACTIVE_WINDOW"),
    //         (Net::Supported, "_NET_SUPPORTED"),
    //         (Net::WMName, "_NET_WM_NAME"),
    //         (Net::WMState, "_NET_WM_STATE"),
    //         (Net::WMCheck, "_NET_SUPPORTING_WM_CHECK"),
    //         (Net::WMFullscreen, "_NET_WM_STATE_FULLSCREEN"),
    //         (Net::WMWindowType, "_NET_WM_WINDOW_TYPE"),
    //         (Net::WMWindowTypeDialog, "_NET_WM_WINDOW_TYPE_DIALOG"),
    //         (Net::ClientList, "_NET_CLIENT_LIST"),
    //     ] {
    //         let s = CString::new(s).unwrap();
    //         let v = XInternAtom(mdpy.inner, s.as_ptr(), False);
    //         if v == BadAlloc as u64 || v == BadValue as u64 {
    //             panic!("XInternAtom failed with {v}");
    //         }
    //         NETATOM[k as usize] = v;
    //     }

    //     // init cursors
    //     CURSOR[Cur::Normal as usize] = drw.cur_create(XC_LEFT_PTR);
    //     CURSOR[Cur::Resize as usize] = drw.cur_create(XC_SIZING);
    //     CURSOR[Cur::Move as usize] = drw.cur_create(XC_FLEUR);

    //     // init appearance
    //     SCHEME = Vec::with_capacity(COLORS.len());
    //     for i in 0..COLORS.len() {
    //         SCHEME.push(drw.scm_create(COLORS[i], 3));
    //     }

    //     // init bars
    //     updatebars(mdpy);

    //     updatestatus(mdpy);

    //     // supporting window for NetWMCheck
    //     WMCHECKWIN = XCreateSimpleWindow(mdpy.inner, ROOT, 0, 0, 1, 1, 0, 0, 0);
    //     xchangeproperty(
    //         mdpy,
    //         WMCHECKWIN,
    //         NETATOM[Net::WMCheck as usize],
    //         XA_WINDOW,
    //         32,
    //         PropModeReplace,
    //         &mut (WMCHECKWIN as u8),
    //         1,
    //     );
    //     let rwm = CString::new("rwm").unwrap();
    //     xchangeproperty(
    //         mdpy,
    //         WMCHECKWIN,
    //         NETATOM[Net::WMName as usize],
    //         utf8string,
    //         8,
    //         PropModeReplace,
    //         rwm.as_ptr().cast_mut().cast(),
    //         3,
    //     );
    //     xchangeproperty(
    //         mdpy,
    //         ROOT,
    //         NETATOM[Net::WMCheck as usize],
    //         XA_WINDOW,
    //         32,
    //         PropModeReplace,
    //         &mut (WMCHECKWIN as u8),
    //         1,
    //     );

    //     // EWMH support per view
    //     xchangeproperty(
    //         mdpy,
    //         ROOT,
    //         NETATOM[Net::Supported as usize],
    //         XA_ATOM,
    //         32,
    //         PropModeReplace,
    //         NETATOM.as_ptr() as *mut _,
    //         Net::Last as i32,
    //     );
    //     XDeleteProperty(mdpy.inner, ROOT, NETATOM[Net::ClientList as usize]);

    //     // select events
    //     {
    //         let wa = wa.as_mut_ptr();
    //         (*wa).cursor = CURSOR[Cur::Normal as usize];
    //         (*wa).event_mask = SubstructureRedirectMask
    //             | SubstructureNotifyMask
    //             | ButtonPressMask
    //             | PointerMotionMask
    //             | EnterWindowMask
    //             | LeaveWindowMask
    //             | StructureNotifyMask
    //             | PropertyChangeMask;
    //     }
    //     let mut wa = wa.assume_init();
    //     xchangewindowattributes(mdpy, ROOT, CWEventMask | CWCursor, &mut wa);
    //     if XSelectInput(mdpy.inner, ROOT, wa.event_mask) == BadWindow as i32 {
    //         panic!("selecting bad window");
    //     }
    //     grabkeys(mdpy);
    //     focus(mdpy, std::ptr::null_mut());
    // }
}

fn xchangewindowattributes(
    mdpy: &Display,
    w: Window,
    value_mask: c_ulong,
    wa: *mut XSetWindowAttributes,
) {
    unsafe {
        let ret = XChangeWindowAttributes(mdpy.inner, w, value_mask, wa);
        if matches!(
            ret as u8,
            x11::xlib::BadAccess
                | x11::xlib::BadColor
                | x11::xlib::BadCursor
                | x11::xlib::BadMatch
                | x11::xlib::BadPixmap
                | x11::xlib::BadValue
                | x11::xlib::BadWindow
        ) {
            panic!("failed");
        }
    }
}

fn focus(mdpy: &Display, c: *mut Client) {
    unsafe {
        if c.is_null() || !is_visible(c) {
            let mut c = (*SELMON).stack;
            while !c.is_null() && !is_visible(c) {
                c = (*c).snext;
            }
        }
        if !(*SELMON).sel.is_null() && (*SELMON).sel != c {
            unfocus(mdpy, (*SELMON).sel, false);
        }
        if !c.is_null() {
            if (*c).mon != SELMON {
                SELMON = (*c).mon;
            }
            if (*c).isurgent {
                seturgent(mdpy, c, false);
            }
            detachstack(c);
            attachstack(c);
            grabbuttons(mdpy, c, true);
            XSetWindowBorder(
                mdpy.inner,
                (*c).win,
                SCHEME[Scheme::Sel as usize][Col::Border as usize].pixel,
            );
            setfocus(mdpy, c);
        } else {
            XSetInputFocus(mdpy.inner, ROOT, RevertToPointerRoot, CurrentTime);
            XDeleteProperty(
                mdpy.inner,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
            );
        }
        (*SELMON).sel = c;
        drawbars();
    }
}

fn drawbars() {
    unsafe {
        let mut m = MONS;
        while !m.is_null() {
            drawbar(m);
            m = (*m).next;
        }
    }
}

#[allow(non_upper_case_globals)]
fn xchangeproperty(
    mdpy: &Display,
    w: Window,
    prop: Atom,
    typ: Atom,
    fmt: c_int,
    mode: c_int,
    data: *mut c_uchar,
    nelements: c_int,
) {
    unsafe {
        let ret = XChangeProperty(
            mdpy.inner, w, prop, typ, fmt, mode, data, nelements,
        );
        if matches!(
            ret as u8,
            BadAlloc | BadAtom | BadMatch | BadValue | BadWindow
        ) {
            panic!("failed");
        }
    }
}

fn setfocus(mdpy: &Display, c: *mut Client) {
    unsafe {
        if !(*c).neverfocus {
            XSetInputFocus(
                mdpy.inner,
                (*c).win,
                RevertToPointerRoot,
                CurrentTime,
            );
            xchangeproperty(
                mdpy,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
                XA_WINDOW,
                32,
                PropModeReplace,
                &mut ((*c).win as u8),
                1,
            );
        }
        sendevent(mdpy, c, WMATOM[WM::TakeFocus as usize]);
    }
}

fn sendevent(mdpy: &Display, c: *mut Client, proto: Atom) -> bool {
    let mut n = 0;
    let mut protocols = std::ptr::null_mut();
    let mut exists = false;
    let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
    unsafe {
        if XGetWMProtocols(mdpy.inner, (*c).win, &mut protocols, &mut n) != 0 {
            while !exists && n > 0 {
                exists = *protocols.offset(n as isize) == proto;
                n -= 1;
            }
            XFree(protocols.cast());
        }
        if exists {
            {
                let ev = ev.as_mut_ptr();
                (*ev).type_ = ClientMessage;
                (*ev).client_message.window = (*c).win;
                (*ev).client_message.message_type =
                    WMATOM[WM::Protocols as usize];
                (*ev).client_message.format = 32;
                (*ev).client_message.data.set_long(0, proto as i64);
                (*ev).client_message.data.set_long(1, CurrentTime as i64);
            }
            let mut ev: XEvent = ev.assume_init();
            XSendEvent(mdpy.inner, (*c).win, False, NoEventMask, &mut ev);
        }
        exists
    }
}

fn grabbuttons(mdpy: &Display, c: *mut Client, focused: bool) {
    updatenumlockmask(mdpy);
    unsafe {
        let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
        XUngrabButton(mdpy.inner, AnyButton as u32, AnyModifier, (*c).win);
        if !focused {
            XGrabButton(
                mdpy.inner,
                AnyButton as u32,
                AnyModifier,
                (*c).win,
                False,
                BUTTONMASK as u32,
                GrabModeSync,
                GrabModeSync,
                0,
                0,
            );
        }
        for i in 0..BUTTONS.len() {
            if BUTTONS[i].click == Clk::ClientWin {
                for j in 0..modifiers.len() {
                    XGrabButton(
                        mdpy.inner,
                        BUTTONS[i].button,
                        BUTTONS[i].mask | modifiers[j],
                        (*c).win,
                        False,
                        BUTTONMASK as u32,
                        GrabModeAsync,
                        GrabModeSync,
                        0,
                        0,
                    );
                }
            }
        }
    }
}

pub fn setlayout(mdpy: &Display, arg: Arg) {
    unsafe {
        if let Arg::Layout(lt) = arg {
            if lt as *const _ != (*SELMON).lt[(*SELMON).sellt] {
                (*SELMON).sellt ^= 1;
            }
            (*SELMON).lt[(*SELMON).sellt] = lt;
        } else {
            // same as inner if above but not sure how to chain them otherwise
            (*SELMON).sellt ^= 1;
        }
        (*SELMON).ltsymbol = (*(*SELMON).lt[(*SELMON).sellt]).symbol.to_owned();
        if !(*SELMON).sel.is_null() {
            arrange(mdpy, SELMON);
        } else {
            drawbar(SELMON);
        }
    }
}

fn arrange(mdpy: &Display, mut m: *mut Monitor) {
    unsafe {
        if !m.is_null() {
            showhide(mdpy, (*m).stack);
        } else {
            m = MONS;
            while !m.is_null() {
                showhide(mdpy, (*m).stack);
                m = (*m).next;
            }
        }

        if !m.is_null() {
            arrangemon(mdpy, m);
            restack(mdpy, m);
        } else {
            m = MONS;
            while !m.is_null() {
                arrangemon(mdpy, m);
            }
        }
    }
}

fn arrangemon(mdpy: &Display, m: *mut Monitor) {
    unsafe {
        (*m).ltsymbol = (*(*m).lt[(*m).sellt]).symbol.to_owned();
        let layout = &(*(*m).lt[(*m).sellt]);
        if let Some(arrange) = layout.arrange {
            (arrange)(mdpy, m)
        }
    }
}

fn restack(mdpy: &Display, m: *mut Monitor) {
    drawbar(m);
    unsafe {
        if (*m).sel.is_null() {
            return;
        }
        if (*(*m).sel).isfloating {
            // supposed to be or arrange is null, but we only have empty arrange
            // instead
            XRaiseWindow(mdpy.inner, (*(*m).sel).win);
        }
        let mut wc: MaybeUninit<XWindowChanges> = MaybeUninit::uninit();
        {
            let wc = wc.as_mut_ptr();
            (*wc).stack_mode = Below;
            (*wc).sibling = (*m).barwin;
        }
        let mut wc = wc.assume_init();
        let mut c = (*m).stack;
        while !c.is_null() {
            if !(*c).isfloating && is_visible(c) {
                XConfigureWindow(
                    mdpy.inner,
                    (*c).win,
                    (CWSibling | CWStackMode) as u32,
                    &mut wc as *mut _,
                );
                wc.sibling = (*c).win;
            }
            c = (*c).snext;
        }
        XSync(mdpy.inner, False);
        let mut ev: XEvent = MaybeUninit::uninit().assume_init();
        while XCheckMaskEvent(mdpy.inner, EnterWindowMask, &mut ev as *mut _)
            != 0
        {}
    }
}

fn showhide(mdpy: &Display, c: *mut Client) {
    if c.is_null() {
        return;
    }
    if is_visible(c) {
        // show clients top down
        unsafe {
            XMoveWindow(mdpy.inner, (*c).win, (*c).x, (*c).y);
            if (*c).isfloating && !(*c).isfullscreen {
                resize(mdpy, c, (*c).x, (*c).y, (*c).w, (*c).h, false);
            }
            showhide(mdpy, (*c).snext);
        }
    } else {
        // hide clients bottom up
        unsafe {
            showhide(mdpy, (*c).snext);
            XMoveWindow(mdpy.inner, (*c).win, width(c) * -2, (*c).y);
        }
    }
}

fn resize(
    mdpy: &Display,
    c: *mut Client,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    interact: bool,
) {
    if applysizehints(mdpy, c, &mut x, &mut y, &mut w, &mut h, interact) {
        resizeclient(mdpy, c, x, y, w, h);
    }
}

fn resizeclient(
    mdpy: &Display,
    c: *mut Client,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    unsafe {
        let mut wc: MaybeUninit<XWindowChanges> = MaybeUninit::uninit();
        (*c).oldx = (*c).x;
        (*c).oldy = (*c).y;
        (*c).oldw = (*c).w;
        (*c).oldh = (*c).h;
        (*c).x = x;
        (*c).y = y;
        (*c).w = w;
        (*c).h = h;
        {
            let wc = wc.as_mut_ptr();
            (*wc).x = x;
            (*wc).y = y;
            (*wc).width = w;
            (*wc).height = h;
            (*wc).border_width = (*c).bw;
        }
        let mut wc = wc.assume_init();
        XConfigureWindow(
            mdpy.inner,
            (*c).win,
            (CWX | CWY | CWWidth | CWHeight | CWBorderWidth) as u32,
            &mut wc,
        );
        configure(mdpy, c);
        XSync(mdpy.inner, False);
    }
}

fn configure(mdpy: &Display, c: *mut Client) {
    // TODO this looks like a nice Into impl
    unsafe {
        let mut ce: MaybeUninit<XConfigureEvent> = MaybeUninit::uninit();
        {
            let ce = ce.as_mut_ptr();
            (*ce).type_ = ConfigureNotify;
            (*ce).display = mdpy.inner;
            (*ce).event = (*c).win;
            (*ce).window = (*c).win;
            (*ce).x = (*c).x;
            (*ce).y = (*c).y;
            (*ce).width = (*c).w;
            (*ce).height = (*c).h;
            (*ce).border_width = (*c).bw;
            (*ce).above = 0;
            (*ce).override_redirect = False;
        }
        let mut ce = ce.assume_init();
        XSendEvent(
            mdpy.inner,
            (*c).win,
            False,
            StructureNotifyMask,
            &mut ce as *mut _ as *mut _,
        );
    }
}

fn applysizehints(
    mdpy: &Display,
    c: *mut Client,
    x: &mut i32,
    y: &mut i32,
    w: &mut i32,
    h: &mut i32,
    interact: bool,
) -> bool {
    unsafe {
        let m = (*c).mon;
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
            if *x >= ((*m).wx + (*m).ww) as i32 {
                *x = ((*m).wx + (*m).ww - width(c) as i16) as i32;
            }
            if *y >= ((*m).wy + (*m).wh) as i32 {
                *y = ((*m).wy + (*m).wh - height(c) as i16) as i32;
            }
            if *x + *w + 2 * (*c).bw <= (*m).wx as i32 {
                *x = (*m).wx as i32;
            }
            if *y + *h + 2 * (*c).bw <= (*m).wy as i32 {
                *y = (*m).wy as i32;
            }
        }
        if *h < BH as i32 {
            *h = BH as i32;
        }
        if *w < BH as i32 {
            *w = BH as i32;
        }
        if RESIZEHINTS || (*c).isfloating {
            if !(*c).hintsvalid {
                updatesizehints(mdpy, c);
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
                if (*c).maxa < *w as f64 / *h as f64 {
                    *w = (*h as f64 * (*c).maxa + 0.5) as i32;
                } else if (*c).mina < *h as f64 / *w as f64 {
                    *h = (*w as f64 * (*c).mina + 0.5) as i32;
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
                *w = min(*w, (*c).maxw);
            }
            if (*c).maxh != 0 {
                *h = min(*h, (*c).maxh);
            }
        }
        *x != (*c).x || *y != (*c).y || *w != (*c).w || *h != (*c).h
    }
}

fn updatesizehints(mdpy: &Display, c: *mut Client) {
    let mut msize: i64 = 0;
    unsafe {
        let mut size: MaybeUninit<XSizeHints> = MaybeUninit::uninit();
        if XGetWMNormalHints(
            mdpy.inner,
            (*c).win,
            size.as_mut_ptr(),
            &mut msize as *mut _,
        ) != 0
        {
            (*size.as_mut_ptr()).flags = PSize;
        }
        let size = size.assume_init();

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
            (*c).mina = size.min_aspect.y as f64 / size.min_aspect.x as f64;
            (*c).maxa = size.max_aspect.y as f64 / size.max_aspect.x as f64;
        } else {
            (*c).mina = 0.0;
            (*c).maxa = 0.0;
        }

        (*c).isfixed = (*c).maxw != 0
            && (*c).maxh != 0
            && (*c).maxw == (*c).minw
            && (*c).maxh == (*c).minh;
        (*c).hintsvalid = true;
    }
}

pub fn zoom(mdpy: &Display, _arg: Arg) {
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() || (*c).isfloating {
            return;
        }
        if c == nexttiled((*SELMON).clients) {
            let c = nexttiled((*c).next);
            if c.is_null() {
                return;
            }
        }
        pop(mdpy, c);
    }
}

fn pop(mdpy: &Display, c: *mut Client) {
    detach(c);
    attach(c);
    focus(mdpy, c);
    unsafe {
        arrange(mdpy, (*c).mon);
    }
}

fn detach(c: *mut Client) {
    unsafe {
        let mut tc = &mut (*(*c).mon).clients;
        while !(*tc).is_null() && *tc != c {
            tc = &mut (*(*tc)).next;
        }
        *tc = (*c).next;
    }
}

fn nexttiled(mut c: *mut Client) -> *mut Client {
    unsafe {
        while !c.is_null() && ((*c).isfloating || !is_visible(c)) {
            c = (*c).next;
        }
    }
    c
}

pub fn spawn(_dpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Str(s) = arg else {
            return;
        };

        if s == DMENUCMD {
            // this looks like a memory leak, not sure how to fix it. at least
            // we're only leaking a single-character &str at a time
            let r: &'static str = format!("{}", (*SELMON).num).leak();
            let r: Box<&'static str> = Box::new(r);
            let mut r: &'static &'static str = Box::leak(r);
            std::ptr::swap(addr_of_mut!(DMENUMON), &mut r);
        }
        Command::new(s[0])
            .args(&s[1..])
            .spawn()
            .expect("spawn failed");
    }
}

pub fn movemouse(mdpy: &Display, _arg: Arg) {
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        // no support moving fullscreen windows by mouse
        if (*c).isfullscreen {
            return;
        }
        restack(mdpy, SELMON);
        let ocx = (*c).x;
        let ocy = (*c).y;
        let mut lasttime = 0;
        let mut x = 0;
        let mut y = 0;
        if XGrabPointer(
            mdpy.inner,
            ROOT,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            0,
            CURSOR[Cur::Move as usize],
            CurrentTime,
        ) != GrabSuccess
        {
            return;
        }
        if !getrootptr(mdpy, &mut x, &mut y) {
            return;
        }
        let mut first = true;
        let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
        // emulating do while
        while first || (*ev.as_mut_ptr()).type_ != BUTTON_RELEASE {
            XMaskEvent(
                mdpy.inner,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                ev.as_mut_ptr(),
            );
            #[allow(non_upper_case_globals)]
            match (*ev.as_mut_ptr()).type_ {
                ConfigureRequest | Expose | MapRequest => {
                    handler(mdpy, ev.as_mut_ptr())
                }
                MotionNotify => {
                    let ev = ev.as_mut_ptr();
                    if ((*ev).motion.time - lasttime) <= (1000 / 60) {
                        continue;
                    }
                    lasttime = (*ev).motion.time;

                    let mut nx = ocx + (*ev).motion.x - x;
                    let mut ny = ocy + (*ev).motion.y - y;
                    let snap = SNAP as i16;
                    if ((*SELMON).wx - nx as i16).abs() < snap {
                        nx = (*SELMON).wx as i32;
                    } else if (((*SELMON).wx + (*SELMON).ww)
                        - (nx + width(c)) as i16)
                        .abs()
                        < snap
                    {
                        nx = ((*SELMON).wx + (*SELMON).ww) as i32 - width(c);
                    }

                    if ((*SELMON).wy - ny as i16).abs() < snap {
                        ny = (*SELMON).wy as i32;
                    } else if (((*SELMON).wy + (*SELMON).wh)
                        - (ny + height(c)) as i16)
                        .abs()
                        < snap
                    {
                        ny = ((*SELMON).wy + (*SELMON).wh) as i32 - height(c);
                    }

                    if !(*c).isfloating
                        && (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_some()
                        && ((nx - (*c).x).abs() > SNAP
                            || (ny - (*c).y).abs() > SNAP)
                    {
                        togglefloating(mdpy, Arg::None);
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
                        || (*c).isfloating
                    {
                        resize(mdpy, c, nx, ny, (*c).w, (*c).h, true);
                    }
                }
                _ => {}
            }
            first = false;
        }
        XUngrabPointer(mdpy.inner, CurrentTime);
        let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
        if m != SELMON {
            sendmon(mdpy, c, m);
            SELMON = m;
            focus(mdpy, null_mut());
        }
    }
}

pub fn togglefloating(mdpy: &Display, _arg: Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        if (*(*SELMON).sel).isfullscreen {
            // no support for fullscreen windows
            return;
        }
        // either toggle or use fixed value
        (*(*SELMON).sel).isfloating =
            !(*(*SELMON).sel).isfloating || (*(*SELMON).sel).isfixed;

        if (*(*SELMON).sel).isfloating {
            resize(
                mdpy,
                (*SELMON).sel,
                (*(*SELMON).sel).x,
                (*(*SELMON).sel).y,
                (*(*SELMON).sel).w,
                (*(*SELMON).sel).h,
                false,
            );
        }
        arrange(mdpy, SELMON);
    }
}

pub fn resizemouse(mdpy: &Display, _arg: Arg) {
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        // no support for resizing fullscreen windows by mouse
        if (*c).isfullscreen {
            return;
        }
        restack(mdpy, SELMON);
        let ocx = (*c).x;
        let ocy = (*c).y;
        let mut lasttime = 0;
        if XGrabPointer(
            mdpy.inner,
            ROOT,
            False,
            MOUSEMASK as u32,
            GrabModeAsync,
            GrabModeAsync,
            0,
            CURSOR[Cur::Resize as usize],
            CurrentTime,
        ) != GrabSuccess
        {
            return;
        }
        XWarpPointer(
            mdpy.inner,
            0,
            (*c).win,
            0,
            0,
            0,
            0,
            (*c).w + (*c).bw - 1,
            (*c).h + (*c).bw - 1,
        );
        let mut first = true;
        // is this allowed? no warning from the compiler. probably I should use
        // an Option since this gets initialized in the first iteration of the
        // loop
        let ev: *mut XEvent = MaybeUninit::uninit().as_mut_ptr();
        while first || (*ev).type_ != BUTTON_RELEASE {
            XMaskEvent(
                mdpy.inner,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                ev,
            );
            #[allow(non_upper_case_globals)]
            match (*ev).type_ {
                ConfigureRequest | Expose | MapRequest => handler(mdpy, ev),
                MotionNotify => {
                    if ((*ev).motion.time - lasttime) <= (1000 / 60) {
                        continue;
                    }
                    lasttime = (*ev).motion.time;

                    let nw = max((*ev).motion.x - ocx - 2 * (*c).bw + 1, 1);
                    let nh = max((*ev).motion.y - ocy - 2 * (*c).bw + 1, 1);
                    if ((*(*c).mon).wx + nw as i16 >= (*SELMON).wx
                        && (*(*c).mon).wx + nw as i16
                            <= (*SELMON).wx + (*SELMON).ww
                        && (*(*c).mon).wy + nh as i16 >= (*SELMON).wy
                        && (*(*c).mon).wy + nh as i16
                            <= (*SELMON).wy + (*SELMON).wh)
                        && (!(*c).isfloating
                            && (*(*SELMON).lt[(*SELMON).sellt])
                                .arrange
                                .is_some()
                            && (abs(nw - (*c).w) > SNAP
                                || abs(nh - (*c).h) > SNAP))
                    {
                        togglefloating(mdpy, Arg::None);
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
                        || (*c).isfloating
                    {
                        resize(mdpy, c, (*c).x, (*c).y, nw, nh, true);
                    }
                }
                _ => {}
            }
            first = false;
        }
        XWarpPointer(
            mdpy.inner,
            0,
            (*c).win,
            0,
            0,
            0,
            0,
            (*c).w + (*c).bw - 1,
            (*c).h + (*c).bw - 1,
        );
        XUngrabPointer(mdpy.inner, CurrentTime);
        while XCheckMaskEvent(mdpy.inner, EnterWindowMask, ev) != 0 {}
        let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
        if m != SELMON {
            sendmon(mdpy, c, m);
            SELMON = m;
            focus(mdpy, null_mut());
        }
    }
}

pub fn view(mdpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Uint(ui) = arg else { return };
        if (ui & TAGMASK) == (*SELMON).tagset[(*SELMON).seltags] {
            return;
        }
        (*SELMON).seltags ^= 1; /* toggle sel tagset */
        if (ui & TAGMASK) != 0 {
            (*SELMON).tagset[(*SELMON).seltags] = ui & TAGMASK;
        }
        focus(mdpy, null_mut());
        arrange(mdpy, SELMON);
    }
}

pub fn toggleview(mdpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Uint(ui) = arg else { return };
        let newtagset = (*SELMON).tagset[(*SELMON).seltags] ^ (ui & TAGMASK);
        if newtagset != 0 {
            (*SELMON).tagset[(*SELMON).seltags] = newtagset;
            focus(mdpy, null_mut());
            arrange(mdpy, SELMON);
        }
    }
}

pub fn tag(mdpy: &Display, arg: Arg) {
    let Arg::Uint(ui) = arg else { return };
    unsafe {
        if !(*SELMON).sel.is_null() && ui & TAGMASK != 0 {
            (*(*SELMON).sel).tags = ui & TAGMASK;
            focus(mdpy, null_mut());
            arrange(mdpy, SELMON);
        }
    }
}

pub fn toggletag(mdpy: &Display, arg: Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        let Arg::Uint(ui) = arg else { return };
        let newtags = (*(*SELMON).sel).tags ^ (ui & TAGMASK);
        if newtags != 0 {
            (*(*SELMON).sel).tags = newtags;
            focus(mdpy, null_mut());
            arrange(mdpy, SELMON);
        }
    }
}

pub fn togglebar(mdpy: &Display, _arg: Arg) {
    unsafe {
        (*SELMON).showbar = !(*SELMON).showbar;
        updatebarpos(SELMON);
        XMoveResizeWindow(
            mdpy.inner,
            (*SELMON).barwin,
            (*SELMON).wx as i32,
            (*SELMON).by as i32,
            (*SELMON).ww as u32,
            BH as u32,
        );
        arrange(mdpy, SELMON);
    }
}

pub fn focusstack(mdpy: &Display, arg: Arg) {
    let Arg::Int(ai) = arg else { return };
    let mut c = null_mut();
    unsafe {
        if (*SELMON).sel.is_null()
            || ((*(*SELMON).sel).isfullscreen && LOCKFULLSCREEN)
        {
            return;
        }

        if ai > 0 {
            c = (*(*SELMON).sel).next;
            while !c.is_null() && !is_visible(c) {
                c = (*c).next;
            }
            if c.is_null() {
                c = (*SELMON).clients;
                while !c.is_null() && !is_visible(c) {
                    c = (*c).next;
                }
            }
        } else {
            let mut i = (*SELMON).clients;
            while i != (*SELMON).sel {
                if is_visible(i) {
                    c = i;
                }
                i = (*i).next
            }
            if c.is_null() {
                while !i.is_null() {
                    if is_visible(i) {
                        c = i;
                    }
                    i = (*i).next;
                }
            }
        }
        if !c.is_null() {
            focus(mdpy, c);
            restack(mdpy, SELMON);
        }
    }
}

pub fn incnmaster(mdpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Int(ai) = arg else { return };
        (*SELMON).nmaster = max((*SELMON).nmaster + ai as i32, 0);
        arrange(mdpy, SELMON);
    }
}

pub fn setmfact(mdpy: &Display, arg: Arg) {
    let Arg::Float(mut f) = arg else { return };
    unsafe {
        if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none() {
            return;
        }
        f = if f < 1.0 {
            f + (*SELMON).mfact
        } else {
            f - 1.0
        };
        if !(0.05..=0.95).contains(&f) {
            return;
        }
        (*SELMON).mfact = f;
        arrange(mdpy, SELMON);
    }
}

pub fn killclient(mdpy: &Display, _arg: Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        if !sendevent(mdpy, (*SELMON).sel, WMATOM[WM::Delete as usize]) {
            XGrabServer(mdpy.inner);
            XSetErrorHandler(Some(xerrordummy));
            XSetCloseDownMode(mdpy.inner, DestroyAll);
            XKillClient(mdpy.inner, (*(*SELMON).sel).win);
            XSync(mdpy.inner, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(mdpy.inner);
        }
    }
}

pub fn focusmon(mdpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Int(ai) = arg else { return };
        if (*MONS).next.is_null() {
            return;
        }
        let m = dirtomon(ai);
        if m == SELMON {
            return;
        }
        unfocus(mdpy, (*SELMON).sel, false);
        SELMON = m;
        focus(mdpy, null_mut());
    }
}

fn dirtomon(dir: isize) -> *mut Monitor {
    let mut m = null_mut();
    unsafe {
        if dir > 0 {
            m = (*SELMON).next;
            if m.is_null() {
                m = MONS;
            }
        } else if SELMON == MONS {
            m = MONS;
            while !(*m).next.is_null() {
                m = (*m).next;
            }
        } else {
            while (*m).next != SELMON {
                m = (*m).next;
            }
        }
    }
    m
}

pub fn tagmon(mdpy: &Display, arg: Arg) {
    let Arg::Int(ai) = arg else { return };
    unsafe {
        if (*SELMON).sel.is_null() || (*MONS).next.is_null() {
            return;
        }
        sendmon(mdpy, (*SELMON).sel, dirtomon(ai));
    }
}

fn sendmon(mdpy: &Display, c: *mut Client, m: *mut Monitor) {
    unsafe {
        if (*c).mon == m {
            return;
        }
        unfocus(mdpy, c, true);
        detach(c);
        detachstack(c);
        (*c).mon = m;
        (*c).tags = (*m).tagset[(*m).seltags]; // assign tags of target monitor
        attach(c);
        attachstack(c);
        focus(mdpy, null_mut());
        arrange(mdpy, null_mut());
    }
}

pub fn quit(_dpy: &Display, _arg: Arg) {
    unsafe { RUNNING = false }
}

fn grabkeys(mdpy: &Display) {
    updatenumlockmask(mdpy);
    unsafe {
        let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
        let (mut start, mut end, mut skip): (i32, i32, i32) = (0, 0, 0);
        XUngrabKey(mdpy.inner, AnyKey, AnyModifier, ROOT);
        XDisplayKeycodes(mdpy.inner, &mut start, &mut end);
        let syms = XGetKeyboardMapping(
            mdpy.inner,
            start as u8,
            end - start + 1,
            &mut skip,
        );
        if syms.is_null() {
            return;
        }
        for k in start..=end {
            for i in 0..KEYS.len() {
                // skip modifier codes, we do that ourselves
                if KEYS[i].keysym
                    == (*syms.offset(((k - start) * skip) as isize)) as u32
                {
                    for j in 0..modifiers.len() {
                        let ret = XGrabKey(
                            mdpy.inner,
                            k,
                            KEYS[i].modkey | modifiers[j],
                            ROOT,
                            True,
                            GrabModeAsync,
                            GrabModeAsync,
                        );
                        if [BadAccess, BadValue, BadWindow]
                            .contains(&(ret as u8))
                        {
                            panic!("XGrabKey error on {k}: {ret}");
                        }
                    }
                }
            }
        }
        XFree(syms.cast());
    }
}

fn updatenumlockmask(mdpy: &Display) {
    unsafe {
        NUMLOCKMASK = 0;
        let modmap = XGetModifierMapping(mdpy.inner);
        for i in 0..8 {
            for j in 0..(*modmap).max_keypermod {
                if *(*modmap)
                    .modifiermap
                    .offset((i * (*modmap).max_keypermod + j) as isize)
                    == XKeysymToKeycode(mdpy.inner, XK_Num_Lock as u64)
                {
                    NUMLOCKMASK = 1 << i;
                }
            }
        }
        XFreeModifiermap(modmap);
    }
}

fn seturgent(mdpy: &Display, c: *mut Client, urg: bool) {
    unsafe {
        (*c).isurgent = urg;
        let wmh = XGetWMHints(mdpy.inner, (*c).win);
        if wmh.is_null() {
            return;
        }
        (*wmh).flags = if urg {
            (*wmh).flags | XUrgencyHint
        } else {
            (*wmh).flags & !XUrgencyHint
        };
        XSetWMHints(mdpy.inner, (*c).win, wmh);
        XFree(wmh.cast());
    }
}

fn unfocus(mdpy: &Display, c: *mut Client, setfocus: bool) {
    if c.is_null() {
        return;
    }
    grabbuttons(mdpy, c, false);
    unsafe {
        XSetWindowBorder(
            mdpy.inner,
            (*c).win,
            SCHEME[Scheme::Norm as usize][Col::Border as usize].pixel,
        );
        if setfocus {
            XSetInputFocus(mdpy.inner, ROOT, RevertToPointerRoot, CurrentTime);
            XDeleteProperty(
                mdpy.inner,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
            );
        }
    }
}

fn updatestatus(mdpy: &Display) {
    unsafe {
        let c = gettextprop(mdpy, ROOT, XA_WM_NAME, addr_of_mut!(STEXT));
        if !c {
            STEXT = "rwm-0.0.1".to_owned();
        }
        drawbar(SELMON);
    }
}

fn drawbar(m: *mut Monitor) {
    unsafe {
        let boxs = (*(*DRW).fonts).h / 9;
        let boxw = (*(*DRW).fonts).h / 6 + 2;
        let mut occ = 0;
        let mut urg = 0;
        let mut x = 0;
        let mut w;
        let mut tw = 0;

        if !(*m).showbar {
            return;
        }

        let drw = DRW.as_mut().unwrap();

        // draw status first so it can be overdrawn by tags later
        if m == SELMON {
            // status is only drawn on selected monitor
            drw.setscheme(&mut SCHEME[Scheme::Norm as usize]);
            tw = drw.textw(addr_of!(STEXT)) - LRPAD + 2; // 2px right padding
            drw.text(
                ((*m).ww - tw as i16) as i32,
                0,
                tw,
                BH as usize,
                0,
                addr_of!(STEXT),
                0,
            );
        }

        let mut c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent {
                urg |= (*c).tags;
            }
            c = (*c).next;
        }

        for i in 0..TAGS.len() {
            let text = TAGS[i].to_owned();
            w = drw.textw(&text);
            drw.setscheme(
                &mut SCHEME[if ((*m).tagset[(*m).seltags] & 1 << i) != 0 {
                    Scheme::Sel as usize
                } else {
                    Scheme::Norm as usize
                }],
            );
            drw.text(
                x,
                0,
                w,
                BH as usize,
                LRPAD / 2,
                &text,
                (urg as i32) & 1 << i,
            );

            if (occ & 1 << i) != 0 {
                drw.rect(
                    x + boxs as i32,
                    boxs,
                    boxw,
                    boxw,
                    m == SELMON
                        && !(*SELMON).sel.is_null()
                        && ((*(*SELMON).sel).tags & 1 << i) != 0,
                    (urg & 1 << i) != 0,
                );
            }
            x += w as i32;
        }

        w = drw.textw(&(*m).ltsymbol);
        drw.setscheme(&mut SCHEME[Scheme::Norm as usize]);
        x = drw.text(x, 0, w, BH as usize, LRPAD / 2, &(*m).ltsymbol, 0) as i32;

        w = ((*m).ww - tw as i16 - x as i16) as usize;
        if w > BH as usize {
            if !(*m).sel.is_null() {
                drw.setscheme(
                    &mut SCHEME[if m == SELMON {
                        Scheme::Sel
                    } else {
                        Scheme::Norm
                    } as usize],
                );
                drw.text(x, 0, w, BH as usize, LRPAD / 2, &(*(*m).sel).name, 0);
                if (*(*m).sel).isfloating {
                    drw.rect(
                        x + boxs as i32,
                        boxs,
                        boxw,
                        boxw,
                        (*(*m).sel).isfixed,
                        false,
                    );
                }
            } else {
                drw.setscheme(&mut SCHEME[Scheme::Norm as usize]);
                drw.rect(x, 0, w, BH as usize, true, true);
            }
        }
        drw.map((*m).barwin, 0, 0, (*m).ww, BH);
    }
}

fn gettextprop(
    mdpy: &Display,
    w: Window,
    atom: Atom,
    text: *mut String,
) -> bool {
    unsafe {
        if (*text).is_empty() {
            return false;
        }
        let mut name = MaybeUninit::uninit();
        let c = XGetTextProperty(mdpy.inner, w, name.as_mut_ptr(), atom);
        let name = name.assume_init();
        if c != 0 || name.nitems == 0 {
            return false;
        }

        let mut n = 0;
        let list = std::ptr::null_mut();
        if name.encoding == XA_STRING {
            let t = CString::from_raw(name.value as *mut _);
            *text = t.to_str().unwrap().to_owned();
        } else if XmbTextPropertyToTextList(
            mdpy.inner,
            &name,
            list,
            &mut n as *mut _,
        ) >= Success as i32
            && n > 0
            && !list.is_null()
        {
            let t = CString::from_raw(list as *mut _);
            *text = t.to_str().unwrap().to_owned();
            XFreeStringList(*list);
        }
        XFree(name.value as *mut _);
    }
    true
}

fn updatebars(mdpy: &Display) {
    let mut wa = XSetWindowAttributes {
        background_pixmap: ParentRelative as u64,
        event_mask: ButtonPressMask | ExposureMask,
        override_redirect: True,
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
    let rwm = CString::new("rwm").unwrap();
    let mut ch = XClassHint {
        res_name: rwm.as_ptr().cast_mut(),
        res_class: rwm.as_ptr().cast_mut(),
    };

    unsafe {
        let mut m = MONS;
        while !m.is_null() {
            if (*m).barwin != 0 {
                continue;
            }
            (*m).barwin = XCreateWindow(
                mdpy.inner,
                ROOT,
                (*m).wx as c_int,
                (*m).by as c_int,
                (*m).ww as c_uint,
                BH as c_uint,
                0,
                XDefaultDepth(mdpy.inner, SCREEN),
                CopyFromParent as c_uint,
                XDefaultVisual(mdpy.inner, SCREEN),
                CWOverrideRedirect | CWBackPixmap | CWEventMask,
                &mut wa,
            );
            XDefineCursor(
                mdpy.inner,
                (*m).barwin,
                CURSOR[Cur::Normal as usize],
            );
            XMapRaised(mdpy.inner, (*m).barwin);
            XSetClassHint(mdpy.inner, (*m).barwin, &mut ch);
            m = (*m).next;
        }
    }
}

fn updategeom(mdpy: &Display) -> bool {
    let mut dirty = false;
    unsafe {
        if XineramaIsActive(mdpy.inner) != 0 {
            // I think this is the number of monitors
            let mut nn: i32 = 0;
            let info = XineramaQueryScreens(mdpy.inner, &mut nn);

            let mut n = 0;
            let mut m = MONS;
            while !m.is_null() {
                m = (*m).next;
                n += 1;
            }

            let unique: *mut XineramaScreenInfo =
                calloc(nn as usize, size_of::<XineramaScreenInfo>()).cast();

            if unique.is_null() {
                panic!("calloc failed");
            }

            let mut j = 0;
            for i in 0..nn {
                if isuniquegeom(unique, j, info.offset(i as isize)) {
                    memcpy(
                        unique.offset(j).cast(),
                        info.offset(i as isize).cast(),
                        size_of::<XineramaScreenInfo>(),
                    );
                    j += 1;
                }
            }
            XFree(info.cast());
            nn = j as i32;

            // new monitors if nn > n
            for _ in n..nn as usize {
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
            while i < nn as usize && !m.is_null() {
                let u = unique.add(i);
                if i >= n
                    || (*u).x_org != (*m).mx
                    || (*u).y_org != (*m).my
                    || (*u).width != (*m).mw
                    || (*u).height != (*m).mh
                {
                    dirty = true;
                    (*m).num = i as i32;
                    (*m).mx = (*u).x_org;
                    (*m).wx = (*u).x_org;
                    (*m).my = (*u).y_org;
                    (*m).wy = (*u).y_org;
                    (*m).mw = (*u).width;
                    (*m).ww = (*u).width;
                    (*m).mh = (*u).height;
                    (*m).wh = (*u).height;
                    updatebarpos(m);
                }

                m = (*m).next;
                i += 1;
            }

            // removed monitors if n > nn
            for _ in nn..n as i32 {
                let mut m = MONS;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }

                let c = (*m).clients;
                while !c.is_null() {
                    dirty = true;
                    (*m).clients = (*c).next;
                    detachstack(c);
                    (*c).mon = MONS;
                    attach(c);
                    attachstack(c);
                }
                if m == SELMON {
                    SELMON = MONS;
                }
                cleanupmon(m, mdpy);
            }
            libc::free(unique.cast());
        } else {
            // default monitor setup
            if MONS.is_null() {
                MONS = createmon();
            }

            if (*MONS).mw as i32 != SW || (*MONS).mh as i32 != SH {
                dirty = true;
                (*MONS).mw = SW as i16;
                (*MONS).ww = SW as i16;
                (*MONS).mh = SH as i16;
                (*MONS).wh = SH as i16;
                updatebarpos(MONS);
            }
        }
        if dirty {
            SELMON = MONS;
            SELMON = wintomon(mdpy, ROOT);
        }
    }
    dirty
}

fn wintomon(mdpy: &Display, w: Window) -> *mut Monitor {
    unsafe {
        let mut x = 0;
        let mut y = 0;
        if w == ROOT && getrootptr(mdpy, &mut x, &mut y) {
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

fn recttomon(x: i32, y: i32, w: i32, h: i32) -> *mut Monitor {
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

// "macros"

#[inline]
fn intersect(x: i32, y: i32, w: i32, h: i32, m: *mut Monitor) -> i32 {
    unsafe {
        i32::max(
            0,
            i32::min((x) + (w), (*m).wx as i32 + (*m).ww as i32)
                - i32::max(x, (*m).wx as i32),
        ) * i32::max(
            0,
            i32::min((y) + (h), (*m).wy as i32 + (*m).wh as i32)
                - i32::max(y, (*m).wy as i32),
        )
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

fn getrootptr(mdpy: &Display, x: &mut i32, y: &mut i32) -> bool {
    let mut di = 0;
    let mut dui = 0;
    let mut dummy = 0;
    unsafe {
        let ret = XQueryPointer(
            mdpy.inner, ROOT, &mut dummy, &mut dummy, x, y, &mut di, &mut di,
            &mut dui,
        );
        ret != 0
    }
}

fn cleanupmon(mon: *mut Monitor, mdpy: &Display) {
    unsafe {
        if mon == MONS {
            MONS = (*MONS).next;
        } else {
            let mut m = MONS;
            while !m.is_null() && (*m).next != mon {
                m = (*m).next;
            }
        }
        XUnmapWindow(mdpy.inner, (*mon).barwin);
        XDestroyWindow(mdpy.inner, (*mon).barwin);
        drop(Box::from_raw(mon)); // free mon
    }
}

fn attachstack(c: *mut Client) {
    unsafe {
        (*c).snext = (*(*c).mon).stack;
        (*(*c).mon).stack = c;
    }
}

fn attach(c: *mut Client) {
    unsafe {
        (*c).next = (*(*c).mon).clients;
        (*(*c).mon).clients = c;
    }
}

fn detachstack(c: *mut Client) {
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
    unsafe { ((*c).tags & (*(*c).mon).tagset[(*(*c).mon).seltags]) != 0 }
}

fn updatebarpos(m: *mut Monitor) {
    unsafe {
        (*m).wy = (*m).my;
        (*m).wh = (*m).mh;
        if (*m).showbar {
            (*m).wh -= BH;
            (*m).by = if (*m).topbar {
                (*m).wy
            } else {
                (*m).wy + (*m).wh
            };
            (*m).wy = if (*m).topbar { (*m).wy + BH } else { (*m).wy };
        } else {
            (*m).by = -BH;
        }
    }
}

fn isuniquegeom(
    unique: *mut XineramaScreenInfo,
    mut n: isize,
    info: *const XineramaScreenInfo,
) -> bool {
    while n > 0 {
        unsafe {
            let u = unique.offset(n);
            if (*u).x_org == (*info).x_org
                && (*u).y_org == (*info).y_org
                && (*u).width == (*info).width
                && (*u).height == (*info).height
            {
                return false;
            }
        }
        n -= 1;
    }
    true
}

fn cleanup() {
    unsafe {
        bindgen::cleanup();
    }
    // let a = Arg::Uint(!0);
    // let l = Box::new(Layout {
    //     symbol: "",
    //     arrange: None,
    // });
    // let _i = 0;

    // view(mdpy, a);
    // unsafe {
    //     (*SELMON).lt[(*SELMON).sellt] = Box::into_raw(l);
    //     let mut m = MONS;
    //     while !m.is_null() {
    //         while !(*m).stack.is_null() {
    //             unmanage(mdpy, (*m).stack, false);
    //         }
    //         m = (*m).next;
    //     }
    //     XUngrabKey(mdpy.inner, AnyKey, AnyModifier, ROOT);
    //     while !MONS.is_null() {
    //         cleanupmon(MONS, mdpy);
    //     }
    //     for i in 0..Cur::Last as usize {
    //         DRW.as_ref().unwrap().cur_free(CURSOR[i]);
    //     }
    //     // shouldn't have to free SCHEME because it's actually a vec
    //     XDestroyWindow(mdpy.inner, WMCHECKWIN);
    //     drop(Box::from_raw(DRW));
    //     XSync(mdpy.inner, False);
    //     XSetInputFocus(
    //         mdpy.inner,
    //         PointerRoot as u64,
    //         RevertToPointerRoot,
    //         CurrentTime,
    //     );
    //     XDeleteProperty(mdpy.inner, ROOT, NETATOM[Net::ActiveWindow as usize]);
    // }
}

fn unmanage(mdpy: &Display, c: *mut Client, destroyed: bool) {
    unsafe {
        let m = (*c).mon;
        let mut wc: MaybeUninit<XWindowChanges> = MaybeUninit::uninit();
        detach(c);
        detachstack(c);
        if !destroyed {
            (*wc.as_mut_ptr()).border_width = (*c).oldbw;
            XGrabServer(mdpy.inner); // avoid race conditions
            XSetErrorHandler(Some(xerrordummy));
            XSelectInput(mdpy.inner, (*c).win, NoEventMask);
            // restore border
            XConfigureWindow(
                mdpy.inner,
                (*c).win,
                CWBorderWidth as u32,
                wc.as_mut_ptr(),
            );
            XUngrabButton(mdpy.inner, AnyButton as u32, AnyModifier, (*c).win);
            setclientstate(mdpy, c, WITHDRAWN_STATE);
            XSync(mdpy.inner, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(mdpy.inner);
        }
        drop(Box::from_raw(c));
        focus(mdpy, std::ptr::null_mut());
        updateclientlist(mdpy);
        arrange(mdpy, m);
    }
}

fn updateclientlist(mdpy: &Display) {
    unsafe {
        XDeleteProperty(mdpy.inner, ROOT, NETATOM[Net::ClientList as usize]);
        let mut m = MONS;
        while !m.is_null() {
            let mut c = (*m).clients;
            while !c.is_null() {
                xchangeproperty(
                    mdpy,
                    ROOT,
                    NETATOM[Net::ClientList as usize],
                    XA_WINDOW,
                    32,
                    PropModeAppend,
                    &mut ((*c).win as u8) as *mut _,
                    1,
                );
                c = (*c).next;
            }
            m = (*m).next;
        }
    }
}

fn setclientstate(mdpy: &Display, c: *mut Client, state: usize) {
    let mut data: [c_uchar; 2] = [state as c_uchar, 0]; // this zero is None
    unsafe {
        xchangeproperty(
            mdpy,
            (*c).win,
            WMATOM[WM::State as usize],
            WMATOM[WM::State as usize],
            32,
            PropModeReplace,
            data.as_mut_ptr(),
            2,
        );
    }
}

fn run() {
    unsafe { bindgen::run() }
    // // main event loop
    // let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
    // unsafe {
    //     XSync(mdpy.inner, False);
    //     while RUNNING && XNextEvent(mdpy.inner, ev.as_mut_ptr()) == 0 {
    //         handler(mdpy, ev.as_mut_ptr());
    //     }
    // }
}

// not sure how this is my problem...
#[allow(non_upper_case_globals, non_snake_case)]
fn handler(mdpy: &Display, ev: *mut XEvent) {
    unsafe {
        match (*ev).type_ {
            ButtonPress => buttonpress(mdpy, ev),
            ClientMessage => clientmessage(mdpy, ev),
            ConfigureRequest => configurerequest(mdpy, ev),
            ConfigureNotify => configurenotify(mdpy, ev),
            DestroyNotify => destroynotify(mdpy, ev),
            EnterNotify => enternotify(mdpy, ev),
            Expose => expose(mdpy, ev),
            FocusIn => focusin(mdpy, ev),
            KeyPress => keypress(mdpy, ev),
            MappingNotify => mappingnotify(mdpy, ev),
            MapRequest => maprequest(mdpy, ev),
            MotionNotify => motionnotify(mdpy, ev),
            PropertyNotify => propertynotify(mdpy, ev),
            UnmapNotify => unmapnotify(mdpy, ev),
            NoExpose | LeaveNotify | MapNotify => (),
            _ => (),
        }
    }
}

fn unmapnotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).unmap;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if ev.send_event != 0 {
                setclientstate(mdpy, c, WITHDRAWN_STATE);
            } else {
                unmanage(mdpy, c, false);
            }
        }
    }
}

fn propertynotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let mut trans: Window = 0;
        let ev = (*e).property;
        if ev.window == ROOT && ev.atom == XA_WM_NAME {
            updatestatus(mdpy);
        } else if ev.state == PropertyDelete {
            return;
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                match ev.atom {
                    XA_WM_TRANSIENT_FOR => {
                        if !(*c).isfloating
                            && xgettransientforhint(mdpy, (*c).win, &mut trans)
                        {
                            (*c).isfloating = !wintoclient(trans).is_null();
                            if (*c).isfloating {
                                arrange(mdpy, (*c).mon);
                            }
                        }
                    }
                    XA_WM_NORMAL_HINTS => {
                        (*c).hintsvalid = false;
                    }
                    XA_WM_HINTS => {
                        updatewmhints(mdpy, c);
                        drawbars();
                    }
                    _ => (),
                }
                if ev.atom == XA_WM_NAME
                    || ev.atom == NETATOM[Net::WMName as usize]
                {
                    updatetitle(mdpy, c);
                    if c == (*(*c).mon).sel {
                        drawbar((*c).mon);
                    }
                }
                if ev.atom == NETATOM[Net::WMWindowType as usize] {
                    updatewindowtype(mdpy, c);
                }
            }
        }
    }
}

/// declared static inside motionnotify, which apparently means it persists
/// between function calls
static mut MOTIONNOTIFY_MON: *mut Monitor = null_mut();
fn motionnotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).motion;
        if ev.window != ROOT {
            return;
        }
        let m = recttomon(ev.x_root, ev.y_root, 1, 1);
        if m != MOTIONNOTIFY_MON && !MOTIONNOTIFY_MON.is_null() {
            unfocus(mdpy, (*SELMON).sel, true);
            SELMON = m;
            focus(mdpy, null_mut());
        }
        MOTIONNOTIFY_MON = m;
    }
}

fn maprequest(mdpy: &Display, e: *mut XEvent) {
    let mut wa: MaybeUninit<XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        let ev = &(*e).map_request;
        if XGetWindowAttributes(mdpy.inner, ev.window, wa.as_mut_ptr()) == 0
            || (*wa.as_mut_ptr()).override_redirect != 0
        {
            return;
        }
        if wintoclient(ev.window).is_null() {
            manage(mdpy, ev.window, wa.as_mut_ptr());
        }
    }
}

fn mappingnotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let mut ev = (*e).mapping;
        XRefreshKeyboardMapping(&mut ev);
        if ev.request == MappingKeyboard {
            grabkeys(mdpy);
        }
    }
}

fn keypress(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).key;
        let keysym: KeySym = XKeycodeToKeysym(mdpy.inner, ev.keycode as u8, 0);
        for i in 0..KEYS.len() {
            if keysym == KEYS[i].keysym as u64
                && cleanmask(KEYS[i].modkey) == cleanmask(ev.state)
            {
                (KEYS[i].func)(mdpy, KEYS[i].arg.clone());
            }
        }
    }
}

fn focusin(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).focus_change;
        if !(*SELMON).sel.is_null() && ev.window != (*(*SELMON).sel).win {
            setfocus(mdpy, (*SELMON).sel);
        }
    }
}

fn expose(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).expose;
        if ev.count == 0 {
            let m = wintomon(mdpy, ev.window);
            if !m.is_null() {
                drawbar(m);
            }
        }
    }
}

fn enternotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).crossing;
        if (ev.mode != NotifyNormal || ev.detail == NotifyInferior)
            && ev.window != ROOT
        {
            return;
        }
        let c = wintoclient(ev.window);
        let m = if !c.is_null() {
            (*c).mon
        } else {
            wintomon(mdpy, ev.window)
        };
        if m != SELMON {
            unfocus(mdpy, (*SELMON).sel, true);
            SELMON = m;
        } else if c.is_null() || c == (*SELMON).sel {
            return;
        }
        focus(mdpy, c);
    }
}

fn destroynotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).destroy_window;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            unmanage(mdpy, c, true);
        }
    }
}

fn configurenotify(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).configure;
        // dwm TODO updategeom handling sucks, needs to be simplified
        if ev.window == ROOT {
            let dirty = (SW != ev.width) || (SH != ev.height);
            SW = ev.width;
            SH = ev.height;
            if updategeom(mdpy) || dirty {
                DRW.as_mut().unwrap().resize(SW as i16, BH);
                updatebars(mdpy);
                let mut m = MONS;
                while !m.is_null() {
                    let mut c = (*m).clients;
                    while !c.is_null() {
                        if (*c).isfullscreen {
                            resizeclient(
                                mdpy,
                                c,
                                (*m).mx as i32,
                                (*m).my as i32,
                                (*m).mw as i32,
                                (*m).mh as i32,
                            );
                        }
                        c = (*c).next;
                    }
                    XMoveResizeWindow(
                        mdpy.inner,
                        (*m).barwin,
                        (*m).wx as i32,
                        (*m).by as i32,
                        (*m).ww as u32,
                        BH as u32,
                    );
                    m = (*m).next;
                }
                focus(mdpy, null_mut());
                arrange(mdpy, null_mut());
            }
        }
    }
}

fn configurerequest(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).configure_request;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if ev.value_mask & CWBorderWidth as u64 != 0 {
                (*c).bw = ev.border_width;
            } else if (*c).isfloating
                || (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
            {
                let m = (*c).mon;
                let vm = ev.value_mask as u16;
                if vm & CWX != 0 {
                    (*c).oldx = (*c).x;
                    (*c).x = (*m).mx as i32 + ev.x;
                }
                if vm & CWY != 0 {
                    (*c).oldy = (*c).y;
                    (*c).y = (*m).my as i32 + ev.y;
                }
                if vm & CWWidth != 0 {
                    (*c).oldw = (*c).w;
                    (*c).w = (*m).mw as i32 + ev.width;
                }
                if vm & CWHeight != 0 {
                    (*c).oldh = (*c).h;
                    (*c).h = (*m).mh as i32 + ev.height;
                }
                if ((*c).x + (*c).w) as i16 > (*m).mx + (*m).mw
                    && (*c).isfloating
                {
                    // center in x direction
                    (*c).x =
                        ((*m).mx + ((*m).mw / 2 - width(c) as i16 / 2)) as i32;
                }
                if ((*c).y + (*c).h) > ((*m).my + (*m).mh) as i32
                    && (*c).isfloating
                {
                    // center in y direction
                    (*c).y =
                        ((*m).my + ((*m).mh / 2 - height(c) as i16 / 2)) as i32;
                }
                if (vm & (CWX | CWY) != 0) && (vm & (CWWidth | CWHeight)) == 0 {
                    configure(mdpy, c);
                }
                if is_visible(c) {
                    XMoveResizeWindow(
                        mdpy.inner,
                        (*c).win,
                        (*c).x,
                        (*c).y,
                        (*c).w as u32,
                        (*c).h as u32,
                    );
                }
            } else {
                configure(mdpy, c);
            }
        } else {
            let mut wc = XWindowChanges {
                x: ev.x,
                y: ev.y,
                width: ev.width,
                height: ev.height,
                border_width: ev.border_width,
                sibling: ev.above,
                stack_mode: ev.detail,
            };
            XConfigureWindow(
                mdpy.inner,
                ev.window,
                ev.value_mask as u32,
                &mut wc,
            );
        }
        XSync(mdpy.inner, False);
    }
}

fn clientmessage(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let cme = (*e).client_message;
        let c = wintoclient(cme.window);
        if c.is_null() {
            return;
        }
        if cme.message_type == NETATOM[Net::WMState as usize] {
            if cme.data.get_long(1)
                == NETATOM[Net::WMFullscreen as usize] as i64
                || cme.data.get_long(2)
                    == NETATOM[Net::WMFullscreen as usize] as i64
            {
                setfullscreen(
                    mdpy,
                    c,
                    cme.data.get_long(0) == 1
                        || (cme.data.get_long(0) == 2 && !(*c).isfullscreen),
                );
            }
        } else if cme.message_type == NETATOM[Net::ActiveWindow as usize]
            && c != (*SELMON).sel
            && !(*c).isurgent
        {
            seturgent(mdpy, c, true);
        }
    }
}

fn buttonpress(mdpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).button;
        let mut click = Clk::RootWin;
        let mut arg = Arg::Uint(0);
        // focus monitor if necessary
        let m = wintomon(mdpy, ev.window);
        if m != SELMON {
            unfocus(mdpy, (*SELMON).sel, true);
            SELMON = m;
            focus(mdpy, null_mut());
        }
        if ev.window == (*SELMON).barwin {
            let mut x = 0;
            let mut i = 0;
            // do while with ++i in condition
            let drw = &DRW.as_ref().unwrap();
            let text = TAGS[i].to_owned();
            x += drw.textw(&text);
            i += 1;
            while ev.x >= x as i32 && i < TAGS.len() {
                let text = TAGS[i].to_owned();
                x += drw.textw(&text);
                i += 1;
            }
            if i < TAGS.len() {
                click = Clk::TagBar;
                arg = Arg::Uint(1 << i);
            } else if ev.x < (x + drw.textw(&(*SELMON).ltsymbol)) as i32 {
                click = Clk::LtSymbol;
            } else if ev.x
                > ((*SELMON).ww as usize - drw.textw(addr_of!(STEXT))) as i32
            {
                click = Clk::StatusText;
            } else {
                click = Clk::WinTitle;
            }
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                focus(mdpy, c);
                restack(mdpy, SELMON);
                XAllowEvents(mdpy.inner, ReplayPointer, CurrentTime);
                click = Clk::ClientWin;
            }
        }
        for i in 0..BUTTONS.len() {
            let b = &BUTTONS[i];
            if click == b.click
                && b.button == ev.button
                && cleanmask(b.mask) == cleanmask(ev.state)
            {
                let arg = if click == Clk::TagBar
                    && matches!(b.arg, Arg::Int(i) if i == 0)
                {
                    arg.clone()
                } else {
                    b.arg.clone()
                };
                (b.func)(mdpy, arg);
            }
        }
    }
}

fn scan() {
    let mut num = 0;
    let mut d1 = 0;
    let mut d2 = 0;
    let mut wins: *mut Window = std::ptr::null_mut();
    let mut wa: MaybeUninit<bindgen::XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        if bindgen::XQueryTree(
            dpy,
            bindgen::root,
            &mut d1,
            &mut d2,
            &mut wins as *mut _,
            &mut num,
        ) != 0
        {
            for i in 0..num {
                if bindgen::XGetWindowAttributes(
                    dpy,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                    || (*wa.as_mut_ptr()).override_redirect != 0
                    || bindgen::XGetTransientForHint(
                        dpy,
                        *wins.offset(i as isize),
                        &mut d1,
                    ) != 0
                {
                    continue;
                }
                if (*wa.as_mut_ptr()).map_state == IsViewable
                    || bindgen::getstate(*wins.offset(i as isize))
                        == ICONIC_STATE as i64
                {
                    bindgen::manage(*wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            for i in 0..num {
                // now the transients
                if bindgen::XGetWindowAttributes(
                    dpy,
                    *wins.offset(i as isize),
                    wa.as_mut_ptr(),
                ) == 0
                {
                    continue;
                }
                if bindgen::XGetTransientForHint(
                    dpy,
                    *wins.offset(i as isize),
                    &mut d1,
                ) != 0
                    && ((*wa.as_mut_ptr()).map_state == IsViewable
                        || bindgen::getstate(*wins.offset(i as isize))
                            == ICONIC_STATE as i64)
                {
                    bindgen::manage(*wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            if !wins.is_null() {
                XFree(wins.cast());
            }
        }
    }
}

fn manage(mdpy: &Display, w: Window, wa: *mut XWindowAttributes) {
    let mut trans = 0;
    unsafe {
        let wa = *wa;
        let c = Box::into_raw(Box::new(Client {
            x: wa.x,
            y: wa.y,
            w: wa.width,
            h: wa.height,
            oldx: wa.x,
            oldy: wa.y,
            oldw: wa.width,
            oldh: wa.height,
            oldbw: wa.border_width,
            win: w,
            ..Default::default()
        }));
        updatetitle(mdpy, c);
        if xgettransientforhint(mdpy, w, &mut trans) {
            let t = wintoclient(trans);
            if !t.is_null() {
                (*c).mon = (*t).mon;
                (*c).tags = (*t).tags;
            } else {
                (*c).mon = SELMON;
                applyrules(mdpy, c);
            }
        } else {
            // copied else case from above because the condition is supposed
            // to be xgettransientforhint && (t = wintoclient)
            (*c).mon = SELMON;
            applyrules(mdpy, c);
        }
        if (*c).x + width(c) > ((*(*c).mon).wx + (*(*c).mon).ww) as i32 {
            (*c).x = ((*(*c).mon).wx + (*(*c).mon).ww) as i32 - width(c);
        }
        if (*c).y + height(c) > ((*(*c).mon).wy + (*(*c).mon).wh) as i32 {
            (*c).y = ((*(*c).mon).wy + (*(*c).mon).wh) as i32 - height(c);
        }
        (*c).x = max((*c).x, (*(*c).mon).wx as i32);
        (*c).y = max((*c).y, (*(*c).mon).wy as i32);
        (*c).bw = BORDERPX;
        let mut wc = XWindowChanges {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            border_width: (*c).bw,
            sibling: 0,
            stack_mode: 0,
        };
        XConfigureWindow(mdpy.inner, w, CWBorderWidth as u32, &mut wc);
        XSetWindowBorder(
            mdpy.inner,
            w,
            SCHEME[Scheme::Norm as usize][Col::Border as usize].pixel,
        );
        configure(mdpy, c); // propagates border width, if size doesn't change
        updatewindowtype(mdpy, c);
        updatesizehints(mdpy, c);
        updatewmhints(mdpy, c);
        XSelectInput(
            mdpy.inner,
            w,
            EnterWindowMask
                | FocusChangeMask
                | PropertyChangeMask
                | StructureNotifyMask,
        );
        grabbuttons(mdpy, c, false);
        if !(*c).isfloating {
            (*c).oldstate = trans != 0 || (*c).isfixed;
            (*c).isfloating = (*c).oldstate;
        }
        if (*c).isfloating {
            XRaiseWindow(mdpy.inner, (*c).win);
        }
        attach(c);
        attachstack(c);
        xchangeproperty(
            mdpy,
            ROOT,
            NETATOM[Net::ClientList as usize],
            XA_WINDOW,
            32,
            PropModeAppend,
            &mut ((*c).win as c_uchar),
            1,
        );
        // some windows require this
        XMoveResizeWindow(
            mdpy.inner,
            (*c).win,
            (*c).x + 2 * SW,
            (*c).y,
            (*c).w as u32,
            (*c).h as u32,
        );
        setclientstate(mdpy, c, NORMAL_STATE);
        if (*c).mon == SELMON {
            unfocus(mdpy, (*SELMON).sel, false);
        }
        (*(*c).mon).sel = c;
        arrange(mdpy, (*c).mon);
        XMapWindow(mdpy.inner, (*c).win);
        focus(mdpy, std::ptr::null_mut());
    }
}

fn updatewmhints(mdpy: &Display, c: *mut Client) {
    unsafe {
        let wmh = XGetWMHints(mdpy.inner, (*c).win);
        if !wmh.is_null() {
            if c == (*SELMON).sel && (*wmh).flags & XUrgencyHint != 0 {
                (*wmh).flags &= !XUrgencyHint;
                XSetWMHints(mdpy.inner, (*c).win, wmh);
            } else {
                (*c).isurgent = (*wmh).flags & XUrgencyHint != 0;
            }
            if (*wmh).flags & InputHint != 0 {
                (*c).neverfocus = (*wmh).input == 0;
            } else {
                (*c).neverfocus = false;
            }
            XFree(wmh.cast());
        }
    }
}

fn updatewindowtype(mdpy: &Display, c: *mut Client) {
    unsafe {
        let state = getatomprop(mdpy, c, NETATOM[Net::WMState as usize]);
        let wtype = getatomprop(mdpy, c, NETATOM[Net::WMWindowType as usize]);
        if state == NETATOM[Net::WMFullscreen as usize] {
            setfullscreen(mdpy, c, true);
        }
        if wtype == NETATOM[Net::WMWindowTypeDialog as usize] {
            (*c).isfloating = true;
        }
    }
}

fn setfullscreen(mdpy: &Display, c: *mut Client, fullscreen: bool) {
    unsafe {
        if fullscreen && !(*c).isfullscreen {
            xchangeproperty(
                mdpy,
                (*c).win,
                NETATOM[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                &mut (NETATOM[Net::WMFullscreen as usize] as u8) as *mut _,
                1,
            );
            (*c).isfullscreen = true;
            (*c).oldstate = (*c).isfloating;
            (*c).oldbw = (*c).bw;
            (*c).bw = 0;
            (*c).isfloating = true;
            resizeclient(
                mdpy,
                c,
                (*(*c).mon).mx as i32,
                (*(*c).mon).my as i32,
                (*(*c).mon).mw as i32,
                (*(*c).mon).mh as i32,
            );
            XRaiseWindow(mdpy.inner, (*c).win);
        } else if !fullscreen && (*c).isfullscreen {
            xchangeproperty(
                mdpy,
                (*c).win,
                NETATOM[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                &mut 0_u8 as *mut _,
                0,
            );
            (*c).isfullscreen = false;
            (*c).isfloating = (*c).oldstate;
            (*c).bw = (*c).oldbw;
            (*c).x = (*c).oldx;
            (*c).y = (*c).oldy;
            (*c).w = (*c).oldw;
            (*c).h = (*c).oldh;
            resizeclient(mdpy, c, (*c).x, (*c).y, (*c).w, (*c).h);
            arrange(mdpy, (*c).mon);
        }
    }
}

fn getatomprop(mdpy: &Display, c: *mut Client, prop: Atom) -> Atom {
    let mut di = 0;
    let mut dl = 0;
    let mut p = std::ptr::null_mut();
    let mut da = 0;
    let mut atom: Atom = 0;
    unsafe {
        if XGetWindowProperty(
            mdpy.inner,
            (*c).win,
            prop,
            0,
            std::mem::size_of::<Atom>() as i64,
            False,
            XA_ATOM,
            &mut da,
            &mut di,
            &mut dl,
            &mut dl,
            &mut p,
        ) == Success as i32
            && !p.is_null()
        {
            atom = *p as u64;
            XFree(p.cast());
        }
    }
    atom
}

fn applyrules(mdpy: &Display, c: *mut Client) {
    unsafe {
        let mut ch = XClassHint {
            res_name: std::ptr::null_mut(),
            res_class: std::ptr::null_mut(),
        };
        // rule matching
        (*c).isfloating = false;
        (*c).tags = 0;
        XGetClassHint(mdpy.inner, (*c).win, &mut ch);
        let class = if !ch.res_class.is_null() {
            CString::from_raw(ch.res_class).into_string().unwrap()
        } else {
            BROKEN.to_owned()
        };
        let instance = if !ch.res_name.is_null() {
            CString::from_raw(ch.res_name).into_string().unwrap()
        } else {
            BROKEN.to_owned()
        };

        for i in 0..RULES.len() {
            let r = &RULES[i];
            if (r.title.is_none()
                || r.title.is_some_and(|t| (*c).name.contains(t)))
                && (r.class.is_none()
                    || r.class.is_some_and(|t| class.contains(t)))
                && (r.instance.is_none()
                    || r.instance.is_some_and(|t| instance.contains(t)))
            {
                (*c).isfloating = r.isfloating;
                (*c).tags |= r.tags;
                let mut m = MONS;
                while !m.is_null() && (*m).num != r.monitor as i32 {
                    m = (*m).next;
                }
                if !m.is_null() {
                    (*c).mon = m;
                }
            }
        }
        (*c).tags = if (*c).tags & TAGMASK != 0 {
            (*c).tags & TAGMASK
        } else {
            (*(*c).mon).tagset[(*(*c).mon).seltags]
        };
    }
}

fn updatetitle(mdpy: &Display, c: *mut Client) {
    unsafe {
        if !gettextprop(
            mdpy,
            (*c).win,
            NETATOM[Net::WMName as usize],
            &mut (*c).name,
        ) {
            gettextprop(mdpy, (*c).win, XA_WM_NAME, &mut (*c).name);
        }
        if (*c).name.is_empty() {
            /* hack to mark broken clients */
            (*c).name = BROKEN.to_owned();
        }
    }
}

fn getstate(mdpy: &Display, w: Window) -> Result<usize, ()> {
    let mut fmt = 0;
    let mut p: *mut c_uchar = std::ptr::null_mut();
    let mut n = 0;
    let mut extra = 0;
    let mut real = 0;
    let mut result = Err(());
    unsafe {
        let cond = XGetWindowProperty(
            mdpy.inner,
            w,
            WMATOM[WM::State as usize],
            0,
            2,
            False,
            WMATOM[WM::State as usize],
            &mut real,
            &mut fmt,
            &mut n,
            &mut extra,
            &mut p,
        );
        if cond != Success as i32 {
            return Err(());
        }
        if n != 0 {
            // they do this cast in the call to XGetWindowProperty, not sure if
            // it matters
            result = Ok(*p as usize);
        }
        XFree(p.cast());
        result
    }
}

fn xgettransientforhint(mdpy: &Display, w: u64, d1: &mut u64) -> bool {
    unsafe { XGetTransientForHint(mdpy.inner, w, d1) != 0 }
}

fn not_xgetwindowattributes(
    mdpy: &Display,
    wins: *mut u64,
    i: u32,
    wa: &mut MaybeUninit<XWindowAttributes>,
) -> bool {
    unsafe {
        XGetWindowAttributes(
            mdpy.inner,
            *wins.offset(i as isize),
            wa.as_mut_ptr(),
        ) == 0
    }
}

mod config;
mod drw;
mod layouts;

fn die(msg: &str) {
    eprintln!("{}", msg);
    std::process::exit(1);
}

fn main() {
    unsafe {
        dpy = bindgen::XOpenDisplay(std::ptr::null_mut());
        if dpy.is_null() {
            die("rwm: cannot open display");
        }
    }
    checkotherwm(); // DONE
    setup(); // Scary - drawing code
    scan();
    run();
    cleanup();
    unsafe {
        bindgen::XCloseDisplay(dpy);
    }
}
