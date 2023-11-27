//! tiling window manager based on dwm

#![feature(vec_into_raw_parts, lazy_cell)]

use std::cmp::{max, min};
use std::ffi::{c_int, CString};
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::null_mut;

use config::{
    COLORS, DMENUMON, FONTS, KEYS, LAYOUTS, MFACT, NMASTER, SHOWBAR, TOPBAR,
};
use drw::Drw;
use env_logger::Env;
use libc::{
    abs, c_uint, close, execvp, fork, setsid, sigaction, sigemptyset, waitpid,
    SA_NOCLDSTOP, SA_NOCLDWAIT, SA_RESTART, SIGCHLD, SIG_DFL, SIG_IGN, WNOHANG,
};
use log::info;
use x11::keysym::XK_Num_Lock;
use x11::xft::XftColor;
use x11::xinerama::{
    XineramaIsActive, XineramaQueryScreens, XineramaScreenInfo,
};
use x11::xlib::Display as XDisplay;
use x11::xlib::{
    AnyButton, AnyKey, AnyModifier, BadAccess, BadDrawable, BadMatch,
    BadWindow, Below, ButtonPress, ButtonPressMask, ButtonReleaseMask,
    CWBackPixmap, CWBorderWidth, CWCursor, CWEventMask, CWHeight,
    CWOverrideRedirect, CWSibling, CWStackMode, CWWidth, ClientMessage,
    ConfigureNotify, ConfigureRequest, ControlMask, CopyFromParent,
    CurrentTime, DestroyAll, DestroyNotify, EnterNotify, EnterWindowMask,
    Expose, ExposureMask, False, FocusChangeMask, FocusIn, GrabModeAsync,
    GrabModeSync, GrabSuccess, InputHint, IsViewable, KeyPress,
    LeaveWindowMask, LockMask, MapRequest, MappingKeyboard, MappingNotify,
    Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask, MotionNotify,
    NoEventMask, NotifyInferior, NotifyNormal, PAspect, PBaseSize, PMaxSize,
    PMinSize, PResizeInc, PSize, ParentRelative, PointerMotionMask,
    PointerRoot, PropModeAppend, PropModeReplace, PropertyChangeMask,
    PropertyDelete, PropertyNotify, ReplayPointer, RevertToPointerRoot,
    ShiftMask, StructureNotifyMask, SubstructureNotifyMask,
    SubstructureRedirectMask, Success, True, UnmapNotify, XAllowEvents,
    XChangeProperty, XChangeWindowAttributes, XCheckMaskEvent, XClassHint,
    XCloseDisplay, XConfigureEvent, XConfigureWindow, XConnectionNumber,
    XCreateSimpleWindow, XCreateWindow, XDefaultDepth, XDefaultRootWindow,
    XDefaultScreen, XDefaultVisual, XDefineCursor, XDeleteProperty,
    XDestroyWindow, XDisplayHeight, XDisplayKeycodes, XDisplayWidth, XEvent,
    XFree, XFreeModifiermap, XFreeStringList, XGetClassHint,
    XGetKeyboardMapping, XGetModifierMapping, XGetTextProperty,
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
use x11::xlib::{XErrorEvent, XOpenDisplay, XSetErrorHandler};

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

extern "C" fn xerror(dpy: *mut XDisplay, ee: *mut XErrorEvent) -> c_int {
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
        (XERRORXLIB.unwrap())(dpy, ee)
    }
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
static mut SW: i16 = 0;
static mut SH: i16 = 0;

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

const NUMLOCKMASK: u32 = 0;
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
    pub func: fn(dpy: &Display, arg: Arg),
    pub arg: Arg,
}

impl Button {
    pub const fn new(
        click: Clk,
        mask: u32,
        button: u32,
        func: fn(dpy: &Display, arg: Arg),
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
    pub func: fn(dpy: &Display, arg: Arg),
    pub arg: Arg,
}

impl Key {
    pub const fn new(
        modkey: u32,
        keysym: u32,
        func: fn(dpy: &Display, arg: Arg),
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
    arrange: Option<fn(dpy: &Display, mon: *mut Monitor)>,
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

fn checkotherwm(dpy: &Display) {
    unsafe {
        XERRORXLIB = XSetErrorHandler(Some(xerrorstart));
        XSelectInput(
            dpy.inner,
            XDefaultRootWindow(dpy.inner),
            SubstructureRedirectMask,
        );
        XSetErrorHandler(Some(xerror));
        XSync(dpy.inner, False);
    }
}

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

#[derive(PartialEq)]
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

// crash is in here
fn setup(dpy: &mut Display) {
    let mut sa: MaybeUninit<sigaction> = MaybeUninit::uninit();
    let mut wa: MaybeUninit<XSetWindowAttributes> = MaybeUninit::uninit();

    unsafe {
        // do not transform children into zombies when they terminate
        sigemptyset(&mut (*(sa.as_mut_ptr())).sa_mask as *mut _);
        (*(sa.as_mut_ptr())).sa_flags =
            SA_NOCLDSTOP | SA_NOCLDWAIT | SA_RESTART;
        (*(sa.as_mut_ptr())).sa_sigaction = SIG_IGN;
        let sa = sa.assume_init();
        sigaction(SIGCHLD, &sa, std::ptr::null::<sigaction>() as *mut _);

        // clean up any zombies (inherited from .xinitrc etc) immediately
        while waitpid(-1, std::ptr::null::<c_int>() as *mut _, WNOHANG) > 0 {}

        // init screen
        SCREEN = XDefaultScreen(dpy.inner);
        let sw = XDisplayWidth(dpy.inner, SCREEN);
        let sh = XDisplayHeight(dpy.inner, SCREEN);
        ROOT = XRootWindow(dpy.inner, SCREEN);
        DRW = Box::into_raw(Box::new(Drw::new(
            dpy,
            SCREEN,
            ROOT,
            sw as usize,
            sh as usize,
        )));

        DRW.as_mut()
            .unwrap()
            .fontset_create(FONTS)
            .expect("no fonts could be loaded");
        let _lrpa = (*(*DRW).fonts).h;
        BH = ((*(*DRW).fonts).h + 2) as i16;
        updategeom(dpy);

        // init atoms - I really hope these CStrings live long enough.
        let s = CString::new("UTF8_STRING").unwrap();
        let utf8string = XInternAtom(dpy.inner, s.as_ptr(), False);

        for (k, s) in [
            (WM::Protocols, "WM_PROTOCOLS"),
            (WM::Delete, "WM_DELETE_WINDOW"),
            (WM::State, "WM_STATE"),
            (WM::TakeFocus, "WM_TAKE_FOCUS"),
        ] {
            let s = CString::new(s).unwrap();
            WMATOM[k as usize] = XInternAtom(dpy.inner, s.as_ptr(), False);
        }

        for (k, s) in [
            (Net::ActiveWindow, "_NET_ACTIVE_WINDOW"),
            (Net::Supported, "_NET_SUPPORTED"),
            (Net::WMName, "_NET_WM_NAME"),
            (Net::WMState, "_NET_WM_STATE"),
            (Net::WMCheck, "_NET_SUPPORTING_WM_CHECK"),
            (Net::WMFullscreen, "_NET_WM_STATE_FULLSCREEN"),
            (Net::WMWindowType, "_NET_WM_WINDOW_TYPE"),
            (Net::WMWindowTypeDialog, "_NET_WM_WINDOW_TYPE_DIALOG"),
            (Net::ClientList, "_NET_CLIENT_LIST"),
        ] {
            let s = CString::new(s).unwrap();
            NETATOM[k as usize] = XInternAtom(dpy.inner, s.as_ptr(), False);
        }

        info!("halfway through");

        // init cursors
        CURSOR[Cur::Normal as usize] =
            DRW.as_ref().unwrap().cur_create(XC_LEFT_PTR);
        CURSOR[Cur::Resize as usize] =
            DRW.as_ref().unwrap().cur_create(XC_SIZING);
        CURSOR[Cur::Move as usize] = DRW.as_ref().unwrap().cur_create(XC_FLEUR);

        info!("init cursors");

        // init appearance
        SCHEME = Vec::with_capacity(COLORS.len());
        for i in 0..COLORS.len() {
            SCHEME.push(DRW.as_ref().unwrap().scm_create(COLORS[i], 3));
        }

        info!("init appearance");

        // init bars
        updatebars(dpy);
        info!("updatebars");

        updatestatus(dpy);
        info!("updatestatus");

        // supporting window for NetWMCheck
        WMCHECKWIN = XCreateSimpleWindow(dpy.inner, ROOT, 0, 0, 1, 1, 0, 0, 0);
        XChangeProperty(
            dpy.inner,
            WMCHECKWIN,
            NETATOM[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            &mut (WMCHECKWIN as u8) as *mut _,
            1,
        );
        let rwm = CString::new("rwm").unwrap();
        XChangeProperty(
            dpy.inner,
            WMCHECKWIN,
            NETATOM[Net::WMName as usize],
            utf8string,
            8,
            PropModeReplace,
            rwm.as_ptr().cast(),
            3,
        );

        info!("3/4");

        XChangeProperty(
            dpy.inner,
            ROOT,
            NETATOM[Net::WMCheck as usize],
            XA_WINDOW,
            32,
            PropModeReplace,
            &mut (WMCHECKWIN as u8) as *mut _,
            1,
        );

        // EWMH support per view
        XChangeProperty(
            dpy.inner,
            ROOT,
            NETATOM[Net::Supported as usize],
            XA_ATOM,
            32,
            PropModeReplace,
            NETATOM.as_ptr().cast(),
            Net::Last as i32,
        );
        XDeleteProperty(dpy.inner, ROOT, NETATOM[Net::ClientList as usize]);

        info!("almost done");

        // select events
        {
            let wa = wa.as_mut_ptr();
            (*wa).cursor = CURSOR[Cur::Normal as usize];
            (*wa).event_mask = SubstructureRedirectMask
                | SubstructureNotifyMask
                | ButtonPressMask
                | PointerMotionMask
                | EnterWindowMask
                | LeaveWindowMask
                | StructureNotifyMask
                | PropertyChangeMask;
        }
        let mut wa = wa.assume_init();
        XChangeWindowAttributes(
            dpy.inner,
            ROOT,
            CWEventMask | CWCursor,
            &mut wa as *mut _,
        );
        XSelectInput(dpy.inner, ROOT, wa.event_mask);
        grabkeys(dpy);
        focus(dpy, std::ptr::null_mut());
    }
}

fn focus(dpy: &Display, c: *mut Client) {
    unsafe {
        if c.is_null() || !is_visible(c) {
            let mut c = (*SELMON).stack;
            while !c.is_null() && !is_visible(c) {
                c = (*c).snext;
            }
        }
        if !(*SELMON).sel.is_null() && (*SELMON).sel != c {
            unfocus(dpy, (*SELMON).sel, false);
        }
        if !c.is_null() {
            if (*c).mon != SELMON {
                SELMON = (*c).mon;
            }
            if (*c).isurgent {
                seturgent(dpy, c, false);
            }
            detachstack(c);
            attachstack(c);
            grabbuttons(dpy, c, true);
            XSetWindowBorder(
                dpy.inner,
                (*c).win,
                SCHEME[Scheme::Sel as usize][Col::Border as usize].pixel,
            );
            setfocus(dpy, c);
        } else {
            XSetInputFocus(dpy.inner, ROOT, RevertToPointerRoot, CurrentTime);
            XDeleteProperty(
                dpy.inner,
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

fn setfocus(dpy: &Display, c: *mut Client) {
    unsafe {
        if !(*c).neverfocus {
            XSetInputFocus(
                dpy.inner,
                (*c).win,
                RevertToPointerRoot,
                CurrentTime,
            );
            XChangeProperty(
                dpy.inner,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
                XA_WINDOW,
                32,
                PropModeReplace,
                &mut ((*c).win as u8) as *mut _,
                1,
            );
        }
        sendevent(dpy, c, WMATOM[WM::TakeFocus as usize]);
    }
}

fn sendevent(dpy: &Display, c: *mut Client, proto: Atom) -> bool {
    let mut n = 0;
    let mut protocols = std::ptr::null_mut();
    let mut exists = false;
    let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
    unsafe {
        if XGetWMProtocols(dpy.inner, (*c).win, &mut protocols, &mut n) != 0 {
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
            XSendEvent(
                dpy.inner,
                (*c).win,
                False,
                NoEventMask,
                &mut ev as *mut _,
            );
        }
        exists
    }
}

fn grabbuttons(dpy: &Display, c: *mut Client, focused: bool) {
    updatenumlockmask(dpy);
    let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
    unsafe {
        XUngrabButton(dpy.inner, AnyButton as u32, AnyModifier, (*c).win);
        if !focused {
            XGrabButton(
                dpy.inner,
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
                        dpy.inner,
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

pub fn setlayout(dpy: &Display, arg: Arg) {
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
            arrange(dpy, SELMON);
        } else {
            drawbar(SELMON);
        }
    }
}

fn arrange(dpy: &Display, mut m: *mut Monitor) {
    unsafe {
        if !m.is_null() {
            showhide(dpy, (*m).stack);
        } else {
            m = MONS;
            while !m.is_null() {
                showhide(dpy, (*m).stack);
                m = (*m).next;
            }
        }

        if !m.is_null() {
            arrangemon(dpy, m);
            restack(dpy, m);
        } else {
            m = MONS;
            while !m.is_null() {
                arrangemon(dpy, m);
            }
        }
    }
}

fn arrangemon(dpy: &Display, m: *mut Monitor) {
    unsafe {
        (*m).ltsymbol = (*(*m).lt[(*m).sellt]).symbol.to_owned();
        if let Some(arrange) = (*(*m).lt[(*m).sellt]).arrange {
            (arrange)(dpy, m)
        }
    }
}

fn restack(dpy: &Display, m: *mut Monitor) {
    drawbar(m);
    unsafe {
        if (*m).sel.is_null() {
            return;
        }
        if (*(*m).sel).isfloating {
            // supposed to be or arrange is null, but we only have empty arrange
            // instead
            XRaiseWindow(dpy.inner, (*(*m).sel).win);
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
                    dpy.inner,
                    (*c).win,
                    (CWSibling | CWStackMode) as u32,
                    &mut wc as *mut _,
                );
                wc.sibling = (*c).win;
            }
            c = (*c).snext;
        }
        XSync(dpy.inner, False);
        let mut ev: XEvent = MaybeUninit::uninit().assume_init();
        while XCheckMaskEvent(dpy.inner, EnterWindowMask, &mut ev as *mut _)
            != 0
        {}
    }
}

fn showhide(dpy: &Display, c: *mut Client) {
    if c.is_null() {
        return;
    }
    if is_visible(c) {
        // show clients top down
        unsafe {
            XMoveWindow(dpy.inner, (*c).win, (*c).x, (*c).y);
            if (*c).isfloating && !(*c).isfullscreen {
                resize(dpy, c, (*c).x, (*c).y, (*c).w, (*c).h, false);
            }
            showhide(dpy, (*c).snext);
        }
    } else {
        // hide clients bottom up
        unsafe {
            showhide(dpy, (*c).snext);
            XMoveWindow(dpy.inner, (*c).win, width(c) * -2, (*c).y);
        }
    }
}

fn resize(
    dpy: &Display,
    c: *mut Client,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    interact: bool,
) {
    if applysizehints(dpy, c, &mut x, &mut y, &mut w, &mut h, interact) {
        resizeclient(dpy, c, x, y, w, h);
    }
}

fn resizeclient(dpy: &Display, c: *mut Client, x: i32, y: i32, w: i32, h: i32) {
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
            dpy.inner,
            (*c).win,
            (CWX | CWY | CWWidth | CWHeight | CWBorderWidth) as u32,
            &mut wc,
        );
        configure(dpy, c);
        XSync(dpy.inner, False);
    }
}

fn configure(dpy: &Display, c: *mut Client) {
    // TODO this looks like a nice Into impl
    unsafe {
        let mut ce: MaybeUninit<XConfigureEvent> = MaybeUninit::uninit();
        {
            let ce = ce.as_mut_ptr();
            (*ce).type_ = ConfigureNotify;
            (*ce).display = dpy.inner;
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
            dpy.inner,
            (*c).win,
            False,
            StructureNotifyMask,
            &mut ce as *mut _ as *mut _,
        );
    }
}

fn applysizehints(
    dpy: &Display,
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
            if *x > SW as i32 {
                *x = SW as i32 - width(c);
            }
            if *y > SH as i32 {
                *y = SH as i32 - height(c);
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
                updatesizehints(dpy, c);
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

fn updatesizehints(dpy: &Display, c: *mut Client) {
    let mut msize: i64 = 0;
    unsafe {
        let mut size: MaybeUninit<XSizeHints> = MaybeUninit::uninit();
        if XGetWMNormalHints(
            dpy.inner,
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

pub fn zoom(dpy: &Display, _arg: Arg) {
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
        pop(dpy, c);
    }
}

fn pop(dpy: &Display, c: *mut Client) {
    detach(c);
    attach(c);
    focus(dpy, c);
    unsafe {
        arrange(dpy, (*c).mon);
    }
}

fn detach(c: *mut Client) {
    unsafe {
        let mut tc = (*(*c).mon).clients;
        while !tc.is_null() && tc != c {
            tc = (*tc).next;
        }
        tc = (*c).next;
    }
}

fn nexttiled(mut c: *mut Client) -> *mut Client {
    while !c.is_null() || !is_visible(c) {
        unsafe {
            c = (*c).next;
        }
    }
    c
}

pub fn spawn(dpy: &Display, arg: Arg) {
    unsafe {
        let mut sa: MaybeUninit<sigaction> = MaybeUninit::uninit();
        let Arg::Str(s) = arg else {
            return;
        };

        if s == DMENUCMD {
            // this looks like a memory leak, not sure how to fix it. at least
            // we're only leaking a single-character &str at a time
            let r: &'static str = format!("{}", (*SELMON).num).leak();
            let r: Box<&'static str> = Box::new(r);
            let mut r: &'static &'static str = Box::leak(r);
            std::mem::swap(&mut DMENUMON, &mut r);
        }

        if fork() == 0 {
            // might need to be if dpy but our dpy always is
            close(XConnectionNumber(dpy.inner));
            setsid();
            sigemptyset(&mut (*sa.as_mut_ptr()).sa_mask as *mut _);
            {
                let sa = sa.as_mut_ptr();
                (*sa).sa_flags = 0;
                (*sa).sa_sigaction = SIG_DFL;
            }
            let mut sa = sa.assume_init();
            sigaction(SIGCHLD, &mut sa as *mut _, std::ptr::null_mut());

            let (s, _, _) = s.to_vec().into_raw_parts();
            execvp(s.offset(0).cast(), s.cast());
            panic!("execvp has failed");
        }
    }
}

pub fn movemouse(dpy: &Display, _arg: Arg) {
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        // no support moving fullscreen windows by mouse
        if (*c).isfullscreen {
            return;
        }
        restack(dpy, SELMON);
        let ocx = (*c).x;
        let ocy = (*c).y;
        let mut lasttime = 0;
        let mut x = 0;
        let mut y = 0;
        if XGrabPointer(
            dpy.inner,
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
        if !getrootptr(dpy, &mut x, &mut y) {
            return;
        }
        let mut first = true;
        let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
        // emulating do while
        while first || (*ev.as_mut_ptr()).type_ != BUTTON_RELEASE {
            XMaskEvent(
                dpy.inner,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                ev.as_mut_ptr(),
            );
            #[allow(non_upper_case_globals)]
            match (*ev.as_mut_ptr()).type_ {
                ConfigureRequest | Expose | MapRequest => {
                    handler(dpy, ev.as_mut_ptr())
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
                        togglefloating(dpy, Arg::None);
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
                        || (*c).isfloating
                    {
                        resize(dpy, c, nx, ny, (*c).w, (*c).h, true);
                    }
                }
                _ => {}
            }
            first = false;
        }
        XUngrabPointer(dpy.inner, CurrentTime);
        let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
        if m != SELMON {
            sendmon(dpy, c, m);
            SELMON = m;
            focus(dpy, null_mut());
        }
    }
}

pub fn togglefloating(dpy: &Display, _arg: Arg) {
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
                dpy,
                (*SELMON).sel,
                (*(*SELMON).sel).x,
                (*(*SELMON).sel).y,
                (*(*SELMON).sel).w,
                (*(*SELMON).sel).h,
                false,
            );
        }
        arrange(dpy, SELMON);
    }
}

pub fn resizemouse(dpy: &Display, _arg: Arg) {
    unsafe {
        let c = (*SELMON).sel;
        if c.is_null() {
            return;
        }
        // no support for resizing fullscreen windows by mouse
        if (*c).isfullscreen {
            return;
        }
        restack(dpy, SELMON);
        let ocx = (*c).x;
        let ocy = (*c).y;
        let mut lasttime = 0;
        if XGrabPointer(
            dpy.inner,
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
            dpy.inner,
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
                dpy.inner,
                MOUSEMASK | ExposureMask | SubstructureRedirectMask,
                ev,
            );
            #[allow(non_upper_case_globals)]
            match (*ev).type_ {
                ConfigureRequest | Expose | MapRequest => handler(dpy, ev),
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
                        togglefloating(dpy, Arg::None);
                    }
                    if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
                        || (*c).isfloating
                    {
                        resize(dpy, c, (*c).x, (*c).y, nw, nh, true);
                    }
                }
                _ => {}
            }
            first = false;
        }
        XWarpPointer(
            dpy.inner,
            0,
            (*c).win,
            0,
            0,
            0,
            0,
            (*c).w + (*c).bw - 1,
            (*c).h + (*c).bw - 1,
        );
        XUngrabPointer(dpy.inner, CurrentTime);
        while XCheckMaskEvent(dpy.inner, EnterWindowMask, ev) != 0 {}
        let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
        if m != SELMON {
            sendmon(dpy, c, m);
            SELMON = m;
            focus(dpy, null_mut());
        }
    }
}

pub fn view(dpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Uint(ui) = arg else { return };
        if (ui & TAGMASK) == (*SELMON).tagset[(*SELMON).seltags] {
            return;
        }
        (*SELMON).seltags ^= 1; /* toggle sel tagset */
        if (ui & TAGMASK) != 0 {
            (*SELMON).tagset[(*SELMON).seltags] = ui & TAGMASK;
        }
        focus(dpy, null_mut());
        arrange(dpy, SELMON);
    }
}

pub fn toggleview(dpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Uint(ui) = arg else { return };
        let newtagset = (*SELMON).tagset[(*SELMON).seltags] ^ (ui & TAGMASK);
        if newtagset != 0 {
            (*SELMON).tagset[(*SELMON).seltags] = newtagset;
            focus(dpy, null_mut());
            arrange(dpy, SELMON);
        }
    }
}

pub fn tag(dpy: &Display, arg: Arg) {
    let Arg::Uint(ui) = arg else { return };
    unsafe {
        if !(*SELMON).sel.is_null() && ui & TAGMASK != 0 {
            (*(*SELMON).sel).tags = ui & TAGMASK;
            focus(dpy, null_mut());
            arrange(dpy, SELMON);
        }
    }
}

pub fn toggletag(dpy: &Display, arg: Arg) {
    let mut newtags = 0;
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        let Arg::Uint(ui) = arg else { return };
        newtags = (*(*SELMON).sel).tags ^ (ui & TAGMASK);
        if newtags != 0 {
            (*(*SELMON).sel).tags = newtags;
            focus(dpy, null_mut());
            arrange(dpy, SELMON);
        }
    }
}

pub fn togglebar(dpy: &Display, _arg: Arg) {
    unsafe {
        (*SELMON).showbar = !(*SELMON).showbar;
        updatebarpos(SELMON);
        XMoveResizeWindow(
            dpy.inner,
            (*SELMON).barwin,
            (*SELMON).wx as i32,
            (*SELMON).by as i32,
            (*SELMON).ww as u32,
            BH as u32,
        );
        arrange(dpy, SELMON);
    }
}

pub fn focusstack(dpy: &Display, arg: Arg) {
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
            focus(dpy, c);
            restack(dpy, SELMON);
        }
    }
}

pub fn incnmaster(dpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Int(ai) = arg else { return };
        (*SELMON).nmaster = max((*SELMON).nmaster + ai as i32, 0);
        arrange(dpy, SELMON);
    }
}

pub fn setmfact(dpy: &Display, arg: Arg) {
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
        arrange(dpy, SELMON);
    }
}

pub fn killclient(dpy: &Display, _arg: Arg) {
    unsafe {
        if (*SELMON).sel.is_null() {
            return;
        }
        if !sendevent(dpy, (*SELMON).sel, WMATOM[WM::Delete as usize]) {
            XGrabServer(dpy.inner);
            XSetErrorHandler(None);
            XSetCloseDownMode(dpy.inner, DestroyAll);
            XKillClient(dpy.inner, (*(*SELMON).sel).win);
            XSync(dpy.inner, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(dpy.inner);
        }
    }
}

pub fn focusmon(dpy: &Display, arg: Arg) {
    unsafe {
        let Arg::Int(ai) = arg else { return };
        if (*MONS).next.is_null() {
            return;
        }
        let m = dirtomon(ai);
        if m == SELMON {
            return;
        }
        unfocus(dpy, (*SELMON).sel, false);
        SELMON = m;
        focus(dpy, null_mut());
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

pub fn tagmon(dpy: &Display, arg: Arg) {
    let Arg::Int(ai) = arg else { return };
    unsafe {
        if (*SELMON).sel.is_null() || (*MONS).next.is_null() {
            return;
        }
        sendmon(dpy, (*SELMON).sel, dirtomon(ai));
    }
}

fn sendmon(dpy: &Display, c: *mut Client, m: *mut Monitor) {
    unsafe {
        if (*c).mon == m {
            return;
        }
        unfocus(dpy, c, true);
        detach(c);
        detachstack(c);
        (*c).mon = m;
        (*c).tags = (*m).tagset[(*m).seltags]; // assign tags of target monitor
        attach(c);
        attachstack(c);
        focus(dpy, null_mut());
        arrange(dpy, null_mut());
    }
}

pub fn quit(_dpy: &Display, _arg: Arg) {
    unsafe { RUNNING = false }
}

fn grabkeys(dpy: &Display) {
    updatenumlockmask(dpy);
    unsafe {
        let modifiers = [0, LockMask, NUMLOCKMASK, NUMLOCKMASK | LockMask];
        let (mut start, mut end, mut skip): (i32, i32, i32) = (0, 0, 0);
        let mut syms = std::ptr::null_mut();
        XUngrabKey(dpy.inner, AnyKey, AnyModifier, ROOT);
        XDisplayKeycodes(dpy.inner, &mut start as *mut _, &mut end as *mut _);
        syms = XGetKeyboardMapping(
            dpy.inner,
            start as u8,
            end - start + 1,
            &mut skip as *mut _,
        );
        if syms.is_null() {
            return;
        }
        for k in start..end {
            for i in 0..KEYS.len() {
                // skip modifier codes, we do that ourselves
                if KEYS[i].keysym
                    == (*syms.offset((k - start * skip) as isize)) as u32
                {
                    for j in 0..modifiers.len() {
                        XGrabKey(
                            dpy.inner,
                            k,
                            KEYS[i].modkey | modifiers[j],
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

fn updatenumlockmask(dpy: &Display) {
    let mut numlockmask = 0;
    unsafe {
        let modmap = XGetModifierMapping(dpy.inner);
        for i in 0..8 {
            for j in 0..(*modmap).max_keypermod {
                if *(*modmap)
                    .modifiermap
                    .offset((i * (*modmap).max_keypermod + j) as isize)
                    == XKeysymToKeycode(dpy.inner, XK_Num_Lock as u64)
                {
                    numlockmask = 1 << i;
                }
            }
        }
        XFreeModifiermap(modmap);
    }
}

fn seturgent(dpy: &Display, c: *mut Client, urg: bool) {
    unsafe {
        (*c).isurgent = urg;
        let wmh = XGetWMHints(dpy.inner, (*c).win);
        if wmh.is_null() {
            return;
        }
        (*wmh).flags = if urg {
            (*wmh).flags | XUrgencyHint
        } else {
            (*wmh).flags & !XUrgencyHint
        };
        XSetWMHints(dpy.inner, (*c).win, wmh);
        XFree(wmh.cast());
    }
}

fn unfocus(dpy: &Display, c: *mut Client, setfocus: bool) {
    if c.is_null() {
        return;
    }
    grabbuttons(dpy, c, false);
    unsafe {
        XSetWindowBorder(
            dpy.inner,
            (*c).win,
            SCHEME[Scheme::Norm as usize][Col::Border as usize].pixel,
        );
        if setfocus {
            XSetInputFocus(dpy.inner, ROOT, RevertToPointerRoot, CurrentTime);
            XDeleteProperty(
                dpy.inner,
                ROOT,
                NETATOM[Net::ActiveWindow as usize],
            );
        }
    }
}

fn updatestatus(dpy: &Display) {
    info!("entering updatestatus");
    unsafe {
        let c = gettextprop(dpy, ROOT, XA_WM_NAME, &mut STEXT);
        info!("gettextprop");
        if !c {
            info!("setting STEXT");
            STEXT = "rwm-0.0.1".to_owned();
        }
        info!("calling drawbar");
        drawbar(SELMON);
        info!("returning from updatestatus");
    }
}

fn drawbar(m: *mut Monitor) {
    info!("entering drawbar");
    unsafe {
        let boxs = (*(*DRW).fonts).h / 9;
        let boxw = (*(*DRW).fonts).h / 6 + 2;
        let mut occ = 0;
        let mut urg = 0;
        let mut tw = 0;

        info!("checking showbar with {m:?}");
        if !(*m).showbar {
            return;
        }
        info!("checked showbar");

        // draw status first so it can be overdrawn by tags later
        if m == SELMON {
            // status is only drawn on selected monitor
            DRW.as_mut()
                .unwrap()
                .setscheme(&mut SCHEME[Scheme::Norm as usize]);
            tw = DRW.as_ref().unwrap().textw(&STEXT, LRPAD) - LRPAD + 2; // 2px right padding
            DRW.as_ref().unwrap().text(
                ((*m).ww - tw as i16) as i32,
                0,
                tw,
                BH as usize,
                0,
                &STEXT,
                false,
            );
        }

        let c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent {
                urg |= (*c).tags;
            }
        }

        let mut x = 0;
        for i in 0..TAGS.len() {
            let w = DRW.as_ref().unwrap().textw(TAGS[i], LRPAD);
            DRW.as_mut().unwrap().setscheme(
                &mut SCHEME[if ((*m).tagset[(*m).seltags] & 1 << i) != 0 {
                    Scheme::Sel as usize
                } else {
                    Scheme::Norm as usize
                }],
            );
            DRW.as_ref().unwrap().text(
                x,
                0,
                w,
                BH as usize,
                LRPAD / 2,
                TAGS[i],
                (urg & 1 << i) != 0,
            );

            if (occ & 1 << i) != 0 {
                DRW.as_ref().unwrap().rect(
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
        let w = DRW.as_ref().unwrap().textw(&(*m).ltsymbol, LRPAD);
        DRW.as_mut()
            .unwrap()
            .setscheme(&mut SCHEME[Scheme::Norm as usize]);
        let x = DRW.as_ref().unwrap().text(
            x,
            0,
            w,
            BH as usize,
            LRPAD / 2,
            &(*m).ltsymbol,
            false,
        );

        let w = (*m).ww - tw as i16 - x as i16;
        if w > BH {
            if !(*m).sel.is_null() {
                DRW.as_mut().unwrap().setscheme(
                    &mut SCHEME[if m == SELMON {
                        Scheme::Sel
                    } else {
                        Scheme::Norm
                    } as usize],
                );
                DRW.as_ref().unwrap().text(
                    x as i32,
                    0,
                    w as usize,
                    BH as usize,
                    LRPAD / 2,
                    &(*(*m).sel).name,
                    false,
                );
                if (*(*m).sel).isfloating {
                    DRW.as_ref().unwrap().rect(
                        (x + boxs) as i32,
                        boxs,
                        boxw,
                        boxw,
                        (*(*m).sel).isfixed,
                        false,
                    );
                }
            } else {
                DRW.as_mut()
                    .unwrap()
                    .setscheme(&mut SCHEME[Scheme::Norm as usize]);
                DRW.as_ref().unwrap().rect(
                    x as i32,
                    0,
                    w as usize,
                    BH as usize,
                    true,
                    true,
                );
            }
        }
        DRW.as_ref().unwrap().map((*m).barwin, 0, 0, (*m).ww, BH);
    }
}

fn gettextprop(
    dpy: &Display,
    w: Window,
    atom: Atom,
    text: &mut String,
) -> bool {
    let _size = text.len();
    if text.is_empty() {
        return false;
    }
    unsafe {
        let mut name = MaybeUninit::uninit();
        let c = XGetTextProperty(dpy.inner, w, name.as_mut_ptr(), atom);
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
            dpy.inner,
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

fn updatebars(dpy: &Display) {
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
    let s = CString::new("rwm").unwrap();
    // I think this is technically a memory leak, but who cares about "leaking"
    // two 3 character strings
    let mut ch = XClassHint {
        res_name: s.clone().into_raw(),
        res_class: s.into_raw(),
    };

    unsafe {
        let mut m = MONS;
        while !m.is_null() {
            if (*m).barwin != 0 {
                continue;
            }
            (*m).barwin = XCreateWindow(
                dpy.inner,
                ROOT,
                (*m).wx as c_int,
                (*m).by as c_int,
                (*m).ww as c_uint,
                BH as c_uint,
                0,
                XDefaultDepth(dpy.inner, SCREEN),
                CopyFromParent as c_uint,
                XDefaultVisual(dpy.inner, SCREEN),
                CWOverrideRedirect | CWBackPixmap | CWEventMask,
                &mut wa as *mut _,
            );
            XDefineCursor(dpy.inner, (*m).barwin, CURSOR[Cur::Normal as usize]);
            XMapRaised(dpy.inner, (*m).barwin);
            XSetClassHint(dpy.inner, (*m).barwin, &mut ch as *mut _);
            m = (*m).next;
        }
    }
}

fn updategeom(dpy: &Display) -> bool {
    let mut dirty = false;
    unsafe {
        if XineramaIsActive(dpy.inner) != 0 {
            // I think this is the number of monitors
            let mut nn: i32 = 0;
            let info = XineramaQueryScreens(dpy.inner, &mut nn as *mut _);
            let mut unique = vec![MaybeUninit::uninit(); nn as usize];

            let mut n = 0;
            let mut m = MONS;
            while !m.is_null() {
                m = (*m).next;
                n += 1;
            }

            let mut j = 0;
            for i in 0..nn {
                if isuniquegeom(&unique, j, info.offset(i as isize)) {
                    // this is a memcpy in C, is that what read does?
                    unique[j as usize] =
                        MaybeUninit::new(info.offset(i as isize).read());
                    j += 1;
                }
            }
            XFree(info as *mut _);
            nn = j as i32;

            let unique: Vec<_> =
                unique.into_iter().map(|u| u.assume_init()).collect();

            // new monitors if nn > n
            for _ in n..nn as usize {
                let mut m = MONS;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }

                if !m.is_null() {
                    (*m).next = createmon();
                } else {
                    m = createmon();
                }
            }

            let mut i: usize = 0;
            let mut m = MONS;
            while i < nn as usize && !m.is_null() {
                if i >= n
                    || unique[i].x_org != (*m).mx
                    || unique[i].y_org != (*m).my
                    || unique[i].width != (*m).mw
                    || unique[i].height != (*m).mh
                {
                    dirty = true;
                    (*m).num = i as i32;
                    (*m).mx = unique[i].x_org;
                    (*m).wx = unique[i].x_org;
                    (*m).my = unique[i].y_org;
                    (*m).wy = unique[i].y_org;
                    (*m).mw = unique[i].width;
                    (*m).ww = unique[i].width;
                    (*m).mh = unique[i].height;
                    (*m).wh = unique[i].height;
                    updatebarpos(m);
                }

                m = (*m).next;
                i += 1;
            }

            // removed monitors if n > nn
            for _i in nn..n as i32 {
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
                cleanupmon(m, dpy);
            }
        } else {
            // default monitor setup
            if MONS.is_null() {
                MONS = createmon();
            }

            if (*MONS).mw != SW || (*MONS).mh != SH {
                dirty = true;
                (*MONS).mw = SW;
                (*MONS).ww = SW;
                (*MONS).mh = SH;
                (*MONS).wh = SH;
                updatebarpos(MONS);
            }
        }
        if dirty {
            SELMON = MONS;
            SELMON = wintomon(dpy, ROOT);
        }
    }
    dirty
}

fn wintomon(dpy: &Display, w: Window) -> *mut Monitor {
    unsafe {
        let mut x = 0;
        let mut y = 0;
        if w == ROOT && getrootptr(dpy, &mut x, &mut y) {
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
const fn cleanmask(mask: u32) -> u32 {
    mask & !(NUMLOCKMASK | LockMask)
        & (ShiftMask
            | ControlMask
            | Mod1Mask
            | Mod2Mask
            | Mod3Mask
            | Mod4Mask
            | Mod5Mask)
}

fn getrootptr(dpy: &Display, x: &mut i32, y: &mut i32) -> bool {
    let mut di = 0;
    let mut dui = 0;
    let mut dummy = 0;
    unsafe {
        let ret = XQueryPointer(
            dpy.inner, ROOT, &mut dummy, &mut dummy, x, y, &mut di, &mut di,
            &mut dui,
        );
        ret != 0
    }
}

fn cleanupmon(mon: *mut Monitor, dpy: &Display) {
    unsafe {
        if mon == MONS {
            MONS = (*MONS).next;
        } else {
            let mut m = MONS;
            while !m.is_null() && (*m).next != mon {
                m = (*m).next;
            }
        }
        XUnmapWindow(dpy.inner, (*mon).barwin);
        XDestroyWindow(dpy.inner, (*mon).barwin);
        Box::from_raw(mon); // free mon
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
        let mut tc = (*(*c).mon).stack;
        while !tc.is_null() && tc != c {
            tc = (*tc).snext;
        }
        tc = (*c).snext;

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
    unique: &[MaybeUninit<XineramaScreenInfo>],
    n: isize,
    info: *const XineramaScreenInfo,
) -> bool {
    for n in (0..n).rev() {
        unsafe {
            let u = unique[n as usize].assume_init();
            if u.x_org == (*info).x_org
                && u.y_org == (*info).y_org
                && u.width == (*info).width
                && u.height == (*info).height
            {
                return false;
            }
        }
    }
    true
}

fn cleanup(dpy: &Display) {
    let a = Arg::Uint(!0);
    let l = Box::new(Layout {
        symbol: "",
        arrange: None,
    });
    let mut m: *mut Monitor = std::ptr::null_mut();
    let _i = 0;

    view(dpy, a);
    unsafe {
        (*SELMON).lt[(*SELMON).sellt] = Box::into_raw(l);
        m = MONS;
        while !m.is_null() {
            while !(*m).stack.is_null() {
                unmanage(dpy, (*m).stack, false);
            }
            m = (*m).next;
        }
        XUngrabKey(dpy.inner, AnyKey, AnyModifier, ROOT);
        while !MONS.is_null() {
            cleanupmon(MONS, dpy);
        }
        for i in 0..Cur::Last as usize {
            DRW.as_ref().unwrap().cur_free(CURSOR[i]);
        }
        // shouldn't have to free SCHEME because it's actually a vec
        XDestroyWindow(dpy.inner, WMCHECKWIN);
        Box::from_raw(DRW);
        XSync(dpy.inner, False);
        XSetInputFocus(
            dpy.inner,
            PointerRoot as u64,
            RevertToPointerRoot,
            CurrentTime,
        );
        XDeleteProperty(dpy.inner, ROOT, NETATOM[Net::ActiveWindow as usize]);
    }
}

fn unmanage(dpy: &Display, c: *mut Client, destroyed: bool) {
    unsafe {
        let m = (*c).mon;
        let mut wc: MaybeUninit<XWindowChanges> = MaybeUninit::uninit();
        detach(c);
        detachstack(c);
        if !destroyed {
            (*wc.as_mut_ptr()).border_width = (*c).oldbw;
            XGrabServer(dpy.inner); // avoid race conditions
            XSetErrorHandler(None);
            XSelectInput(dpy.inner, (*c).win, NoEventMask);
            // restore border
            XConfigureWindow(
                dpy.inner,
                (*c).win,
                CWBorderWidth as u32,
                wc.as_mut_ptr(),
            );
            XUngrabButton(dpy.inner, AnyButton as u32, AnyModifier, (*c).win);
            setclientstate(dpy, c, WITHDRAWN_STATE);
            XSync(dpy.inner, False);
            XSetErrorHandler(Some(xerror));
            XUngrabServer(dpy.inner);
        }
        Box::from_raw(c);
        focus(dpy, std::ptr::null_mut());
        updateclientlist(dpy);
        arrange(dpy, m);
    }
}

fn updateclientlist(dpy: &Display) {
    unsafe {
        XDeleteProperty(dpy.inner, ROOT, NETATOM[Net::ClientList as usize]);
        let mut m = MONS;
        while !m.is_null() {
            let mut c = (*m).clients;
            while !c.is_null() {
                XChangeProperty(
                    dpy.inner,
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

fn setclientstate(dpy: &Display, c: *mut Client, state: usize) {
    let data = [state, 0]; // this zero is None
    unsafe {
        XChangeProperty(
            dpy.inner,
            (*c).win,
            WMATOM[WM::State as usize],
            WMATOM[WM::State as usize],
            32,
            PropModeReplace,
            data.as_ptr() as *const _,
            2,
        );
    }
}

fn run(dpy: &Display) {
    // main event loop
    let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
    unsafe {
        XSync(dpy.inner, False);
        while RUNNING && XNextEvent(dpy.inner, ev.as_mut_ptr()) == 0 {
            handler(dpy, ev.as_mut_ptr());
        }
    }
}

// not sure how this is my problem...
#[allow(non_upper_case_globals, non_snake_case)]
fn handler(dpy: &Display, ev: *mut XEvent) {
    unsafe {
        match (*ev).type_ {
            ButtonPress => buttonpress(dpy, ev),
            ClientMessage => clientmessage(dpy, ev),
            ConfigureRequest => configurerequest(dpy, ev),
            ConfigureNotify => configurenotify(dpy, ev),
            DestroyNotify => destroynotify(dpy, ev),
            EnterNotify => enternotify(dpy, ev),
            Expose => expose(dpy, ev),
            FocusIn => focusin(dpy, ev),
            KeyPress => keypress(dpy, ev),
            MappingNotify => mappingnotify(dpy, ev),
            MapRequest => maprequest(dpy, ev),
            MotionNotify => motionnotify(dpy, ev),
            PropertyNotify => propertynotify(dpy, ev),
            UnmapNotify => unmapnotify(dpy, ev),
            _ => (),
        }
    }
}

fn unmapnotify(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).unmap;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if ev.send_event != 0 {
                setclientstate(dpy, c, WITHDRAWN_STATE);
            } else {
                unmanage(dpy, c, false);
            }
        }
    }
}

fn propertynotify(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let mut trans: Window = 0;
        let ev = (*e).property;
        if ev.window == ROOT && ev.atom == XA_WM_NAME {
            updatestatus(dpy);
        } else if ev.state == PropertyDelete {
            return;
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                match ev.atom {
                    XA_WM_TRANSIENT_FOR => {
                        if !(*c).isfloating
                            && xgettransientforhint(dpy, (*c).win, &mut trans)
                        {
                            (*c).isfloating = !wintoclient(trans).is_null();
                            if (*c).isfloating {
                                arrange(dpy, (*c).mon);
                            }
                        }
                    }
                    XA_WM_NORMAL_HINTS => {
                        (*c).hintsvalid = false;
                    }
                    XA_WM_HINTS => {
                        updatewmhints(dpy, c);
                        drawbars();
                    }
                    _ => (),
                }
                if ev.atom == XA_WM_NAME
                    || ev.atom == NETATOM[Net::WMName as usize]
                {
                    updatetitle(dpy, c);
                    if c == (*(*c).mon).sel {
                        drawbar((*c).mon);
                    }
                }
                if ev.atom == NETATOM[Net::WMWindowType as usize] {
                    updatewindowtype(dpy, c);
                }
            }
        }
    }
}

fn motionnotify(dpy: &Display, e: *mut XEvent) {
    let mut mon = null_mut();
    unsafe {
        let ev = &(*e).motion;
        if ev.window != ROOT {
            return;
        }
        let m = recttomon(ev.x_root, ev.y_root, 1, 1);
        if m != mon && !mon.is_null() {
            unfocus(dpy, (*SELMON).sel, true);
            SELMON = m;
            focus(dpy, null_mut());
        }
        mon = m;
    }
}

fn maprequest(dpy: &Display, e: *mut XEvent) {
    let mut wa: MaybeUninit<XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        let ev = &(*e).map_request;
        if XGetWindowAttributes(dpy.inner, ev.window, wa.as_mut_ptr()) == 0
            || (*wa.as_mut_ptr()).override_redirect != 0
        {
            return;
        }
        if wintoclient(ev.window).is_null() {
            manage(dpy, ev.window, wa.as_mut_ptr());
        }
    }
}

fn mappingnotify(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let mut ev = (*e).mapping;
        XRefreshKeyboardMapping(&mut ev);
        if ev.request == MappingKeyboard {
            grabkeys(dpy);
        }
    }
}

fn keypress(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).key;
        let keysym = XKeycodeToKeysym(dpy.inner, ev.keycode as u8, 0);
        for i in 0..KEYS.len() {
            if keysym == KEYS[i].keysym as u64
                && cleanmask(KEYS[i].modkey) == cleanmask(ev.state)
            {
                (KEYS[i].func)(dpy, KEYS[i].arg.clone());
            }
        }
    }
}

fn focusin(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).focus_change;
        if !(*SELMON).sel.is_null() && ev.window != (*(*SELMON).sel).win {
            setfocus(dpy, (*SELMON).sel);
        }
    }
}

fn expose(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).expose;
        if ev.count == 0 {
            let m = wintomon(dpy, ev.window);
            if !m.is_null() {
                drawbar(m);
            }
        }
    }
}

fn enternotify(dpy: &Display, e: *mut XEvent) {
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
            wintomon(dpy, ev.window)
        };
        if m != SELMON {
            unfocus(dpy, (*SELMON).sel, true);
            SELMON = m;
        } else if c.is_null() || c == (*SELMON).sel {
            return;
        }
        focus(dpy, c);
    }
}

fn destroynotify(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).destroy_window;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            unmanage(dpy, c, true);
        }
    }
}

fn configurenotify(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).configure;
        // dwm TODO updategeom handling sucks, needs to be simplified
        if ev.window == ROOT {
            let dirty = (SW != ev.width as i16) || (SH != ev.height as i16);
            SW = ev.width as i16;
            SH = ev.height as i16;
            if updategeom(dpy) || dirty {
                DRW.as_mut().unwrap().resize(SW, BH);
                updatebars(dpy);
                let mut m = MONS;
                while !m.is_null() {
                    let mut c = (*m).clients;
                    while !c.is_null() {
                        if (*c).isfullscreen {
                            resizeclient(
                                dpy,
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
                        dpy.inner,
                        (*m).barwin,
                        (*m).wx as i32,
                        (*m).by as i32,
                        (*m).ww as u32,
                        BH as u32,
                    );
                    m = (*m).next;
                }
                focus(dpy, null_mut());
                arrange(dpy, null_mut());
            }
        }
    }
}

fn configurerequest(dpy: &Display, e: *mut XEvent) {
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
                    configure(dpy, c);
                }
                if is_visible(c) {
                    XMoveResizeWindow(
                        dpy.inner,
                        (*c).win,
                        (*c).x,
                        (*c).y,
                        (*c).w as u32,
                        (*c).h as u32,
                    );
                }
            } else {
                configure(dpy, c);
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
                dpy.inner,
                ev.window,
                ev.value_mask as u32,
                &mut wc,
            );
        }
        XSync(dpy.inner, False);
    }
}

fn clientmessage(dpy: &Display, e: *mut XEvent) {
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
                    dpy,
                    c,
                    cme.data.get_long(0) == 1
                        || (cme.data.get_long(0) == 2 && !(*c).isfullscreen),
                );
            }
        } else if cme.message_type == NETATOM[Net::ActiveWindow as usize]
            && c != (*SELMON).sel
            && !(*c).isurgent
        {
            seturgent(dpy, c, true);
        }
    }
}

fn buttonpress(dpy: &Display, e: *mut XEvent) {
    unsafe {
        let ev = (*e).button;
        let mut click = Clk::RootWin;
        let mut arg = Arg::Uint(0);
        // focus monitor if necessary
        let m = wintomon(dpy, ev.window);
        if m != SELMON {
            unfocus(dpy, (*SELMON).sel, true);
            SELMON = m;
            focus(dpy, null_mut());
        }
        if ev.window == (*SELMON).barwin {
            let mut x = 0;
            let mut i = 0;
            // do while with ++i in condition
            x += DRW.as_ref().unwrap().textw(TAGS[i], LRPAD);
            i += 1;
            let drw = &DRW.as_ref().unwrap();
            while ev.x >= x as i32 && i < TAGS.len() {
                x += drw.textw(TAGS[i], LRPAD);
                i += 1;
            }
            if i < TAGS.len() {
                click = Clk::TagBar;
                arg = Arg::Uint(1 << i);
            } else if ev.x < (x + drw.textw(&(*SELMON).ltsymbol, LRPAD)) as i32
            {
                click = Clk::LtSymbol;
            } else if ev.x
                > ((*SELMON).ww as usize - drw.textw(&STEXT, LRPAD)) as i32
            {
                click = Clk::StatusText;
            } else {
                click = Clk::WinTitle;
            }
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                focus(dpy, c);
                restack(dpy, SELMON);
                XAllowEvents(dpy.inner, ReplayPointer, CurrentTime);
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
                (b.func)(dpy, arg);
            }
        }
    }
}

fn scan(dpy: &Display) {
    let mut num = 0;
    let mut d1 = 0;
    let mut d2 = 0;
    let mut wins: *mut Window = std::ptr::null_mut();
    let mut wa: MaybeUninit<XWindowAttributes> = MaybeUninit::uninit();
    unsafe {
        if XQueryTree(
            dpy.inner,
            ROOT,
            &mut d1,
            &mut d2,
            &mut wins as *mut _,
            &mut num,
        ) != 0
        {
            for i in 0..num {
                if not_xgetwindowattributes(dpy, wins, i, &mut wa)
                    || (*wa.as_mut_ptr()).override_redirect != 0
                    || xgettransientforhint(
                        dpy,
                        *wins.offset(i as isize),
                        &mut d1,
                    )
                {
                    continue;
                }
                if (*wa.as_mut_ptr()).map_state == IsViewable
                    || getstate(dpy, *wins.offset(i as isize))
                        .is_ok_and(|v| v == ICONIC_STATE)
                {
                    manage(dpy, *wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            for i in 0..num {
                // now the transients
                if not_xgetwindowattributes(dpy, wins, i, &mut wa) {
                    continue;
                }
                if xgettransientforhint(dpy, *wins.offset(i as isize), &mut d1)
                    && ((*wa.as_mut_ptr()).map_state == IsViewable
                        || getstate(dpy, *wins.offset(i as isize))
                            .is_ok_and(|v| v == ICONIC_STATE))
                {
                    manage(dpy, *wins.offset(i as isize), wa.as_mut_ptr());
                }
            }
            if !wins.is_null() {
                XFree(wins.cast());
            }
        }
    }
}

fn manage(dpy: &Display, w: Window, wa: *mut XWindowAttributes) {
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
        updatetitle(dpy, c);
        if xgettransientforhint(dpy, w, &mut trans) {
            let t = wintoclient(trans);
            if !t.is_null() {
                (*c).mon = (*t).mon;
                (*c).tags = (*t).tags;
            } else {
                (*c).mon = SELMON;
                applyrules(dpy, c);
            }
        } else {
            // copied else case from above because the condition is supposed
            // to be xgettransientforhint && (t = wintoclient)
            (*c).mon = SELMON;
            applyrules(dpy, c);
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
        XConfigureWindow(dpy.inner, w, CWBorderWidth as u32, &mut wc);
        XSetWindowBorder(
            dpy.inner,
            w,
            SCHEME[Scheme::Norm as usize][Col::Border as usize].pixel,
        );
        configure(dpy, c); // propagates border width, if size doesn't change
        updatewindowtype(dpy, c);
        updatesizehints(dpy, c);
        updatewmhints(dpy, c);
        XSelectInput(
            dpy.inner,
            w,
            EnterWindowMask
                | FocusChangeMask
                | PropertyChangeMask
                | StructureNotifyMask,
        );
        grabbuttons(dpy, c, false);
        if !(*c).isfloating {
            (*c).oldstate = trans != 0 || (*c).isfixed;
            (*c).isfloating = (*c).oldstate;
        }
        if (*c).isfloating {
            XRaiseWindow(dpy.inner, (*c).win);
        }
        attach(c);
        attachstack(c);
        XChangeProperty(
            dpy.inner,
            ROOT,
            NETATOM[Net::ClientList as usize],
            XA_WINDOW,
            32,
            PropModeAppend,
            ((*c).win as i8) as *const _,
            1,
        );
        // some windows require this
        XMoveResizeWindow(
            dpy.inner,
            (*c).win,
            (*c).x + (2 * SW) as i32,
            (*c).y,
            (*c).w as u32,
            (*c).h as u32,
        );
        setclientstate(dpy, c, NORMAL_STATE);
        if (*c).mon == SELMON {
            unfocus(dpy, (*SELMON).sel, false);
        }
        (*(*c).mon).sel = c;
        arrange(dpy, (*c).mon);
        XMapWindow(dpy.inner, (*c).win);
        focus(dpy, std::ptr::null_mut());
    }
}

fn updatewmhints(dpy: &Display, c: *mut Client) {
    unsafe {
        let wmh = XGetWMHints(dpy.inner, (*c).win);
        if !wmh.is_null() {
            if c == (*SELMON).sel && (*wmh).flags & XUrgencyHint != 0 {
                (*wmh).flags &= !XUrgencyHint;
                XSetWMHints(dpy.inner, (*c).win, wmh);
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

fn updatewindowtype(dpy: &Display, c: *mut Client) {
    unsafe {
        let state = getatomprop(dpy, c, NETATOM[Net::WMState as usize]);
        let wtype = getatomprop(dpy, c, NETATOM[Net::WMWindowType as usize]);
        if state == NETATOM[Net::WMFullscreen as usize] {
            setfullscreen(dpy, c, true);
        }
        if wtype == NETATOM[Net::WMWindowTypeDialog as usize] {
            (*c).isfloating = true;
        }
    }
}

fn setfullscreen(dpy: &Display, c: *mut Client, fullscreen: bool) {
    unsafe {
        if fullscreen && !(*c).isfullscreen {
            XChangeProperty(
                dpy.inner,
                (*c).win,
                NETATOM[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                &(NETATOM[Net::WMFullscreen as usize] as u8) as *const _,
                1,
            );
            (*c).isfullscreen = true;
            (*c).oldstate = (*c).isfloating;
            (*c).oldbw = (*c).bw;
            (*c).bw = 0;
            (*c).isfloating = true;
            resizeclient(
                dpy,
                c,
                (*(*c).mon).mx as i32,
                (*(*c).mon).my as i32,
                (*(*c).mon).mw as i32,
                (*(*c).mon).mh as i32,
            );
            XRaiseWindow(dpy.inner, (*c).win);
        } else if !fullscreen && (*c).isfullscreen {
            XChangeProperty(
                dpy.inner,
                (*c).win,
                NETATOM[Net::WMState as usize],
                XA_ATOM,
                32,
                PropModeReplace,
                &0_u8 as *const _,
                0,
            );
            (*c).isfullscreen = false;
            (*c).isfloating = (*c).oldstate;
            (*c).bw = (*c).oldbw;
            (*c).x = (*c).oldx;
            (*c).y = (*c).oldy;
            (*c).w = (*c).oldw;
            (*c).h = (*c).oldh;
            resizeclient(dpy, c, (*c).x, (*c).y, (*c).w, (*c).h);
            arrange(dpy, (*c).mon);
        }
    }
}

fn getatomprop(dpy: &Display, c: *mut Client, prop: Atom) -> Atom {
    let mut di = 0;
    let mut dl = 0;
    let mut p = std::ptr::null_mut();
    let mut da = 0;
    let mut atom: Atom = 0;
    unsafe {
        if XGetWindowProperty(
            dpy.inner,
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

fn applyrules(dpy: &Display, c: *mut Client) {
    unsafe {
        let mut ch = XClassHint {
            res_name: std::ptr::null_mut(),
            res_class: std::ptr::null_mut(),
        };
        // rule matching
        (*c).isfloating = false;
        (*c).tags = 0;
        XGetClassHint(dpy.inner, (*c).win, &mut ch);
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
        if !ch.res_class.is_null() {
            XFree(ch.res_class.cast());
        }
        if !ch.res_name.is_null() {
            XFree(ch.res_name.cast());
        }
        (*c).tags = if (*c).tags & TAGMASK != 0 {
            (*c).tags & TAGMASK
        } else {
            (*(*c).mon).tagset[(*(*c).mon).seltags]
        };
    }
}

fn updatetitle(dpy: &Display, c: *mut Client) {
    unsafe {
        if !gettextprop(
            dpy,
            (*c).win,
            NETATOM[Net::WMName as usize],
            &mut (*c).name,
        ) {
            gettextprop(dpy, (*c).win, XA_WM_NAME, &mut (*c).name);
        }
        if (*c).name.is_empty() {
            /* hack to mark broken clients */
            (*c).name = BROKEN.to_owned();
        }
    }
}

fn getstate(dpy: &Display, w: Window) -> Result<usize, ()> {
    let mut fmt = 0;
    let p = std::ptr::null_mut();
    let mut n = 0;
    let mut extra = 0;
    let mut real = 0;
    let mut result = Err(());
    unsafe {
        let cond = XGetWindowProperty(
            dpy.inner,
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
            p,
        ) != Success as i32;
        if cond {
            return Err(());
        }
        if n != 0 {
            // they do this cast in the call to XGetWindowProperty, not sure if
            // it matters
            result = Ok(**p as usize);
        }
        XFree(p.cast());
        result
    }
}

fn xgettransientforhint(dpy: &Display, w: u64, d1: &mut u64) -> bool {
    unsafe { XGetTransientForHint(dpy.inner, w, d1) != 0 }
}

fn not_xgetwindowattributes(
    dpy: &Display,
    wins: *mut u64,
    i: u32,
    wa: &mut MaybeUninit<XWindowAttributes>,
) -> bool {
    unsafe {
        XGetWindowAttributes(
            dpy.inner,
            *wins.offset(i as isize),
            wa.as_mut_ptr(),
        ) == 0
    }
}

mod config;
mod drw;
mod layouts;

fn main() {
    let env = Env::default().default_filter_or("info");
    env_logger::init_from_env(env);
    let home = &std::env::var("HOME").unwrap();
    let home = Path::new(home);
    let outfile =
        File::create(home.join("rwm.out")).expect("failed to create outfile");
    let logfile =
        File::create(home.join("rwm.err")).expect("failed to create log file");
    let out_fd = outfile.as_raw_fd();
    let log_fd = logfile.as_raw_fd();
    unsafe {
        libc::dup2(out_fd, 1);
        libc::dup2(log_fd, 2);
    }

    info!("dup2 finished");
    let mut dpy = Display::open();
    info!("display opened");
    checkotherwm(&dpy);
    info!("checked other wm");
    setup(&mut dpy);
    info!("setup finished");
    scan(&dpy);
    info!("scan finished");
    run(&dpy);
    info!("run finished");
    cleanup(&dpy);
    info!("cleanup finished");
    unsafe {
        XCloseDisplay(dpy.inner);
    }
    info!("exiting");
}
