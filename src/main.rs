//! tiling window manager based on dwm

#![allow(unused)]

use std::ffi::{c_int, CString};
use std::mem::MaybeUninit;

use config::{COLORS, FONTS, LAYOUTS, MFACT, NMASTER, SHOWBAR, TOPBAR};
use drw::Drw;
use libc::{
    c_uint, sigaction, sigemptyset, waitpid, SA_NOCLDSTOP, SA_NOCLDWAIT,
    SA_RESTART, SIGCHLD, SIG_IGN, WNOHANG,
};
use x11::keysym::XK_Num_Lock;
use x11::xft::XftColor;
use x11::xinerama::{
    XineramaIsActive, XineramaQueryScreens, XineramaScreenInfo,
};
use x11::xlib::{
    AnyButton, AnyModifier, BadAccess, BadDrawable, BadMatch, BadWindow,
    ButtonPressMask, ButtonReleaseMask, CWBackPixmap, CWCursor, CWEventMask,
    CWOverrideRedirect, ClientMessage, CopyFromParent, CurrentTime,
    Display as XDisplay, EnterWindowMask, ExposureMask, False, GrabModeAsync,
    GrabModeSync, LeaveWindowMask, LockMask, NoEventMask, ParentRelative,
    PointerMotionMask, PropModeReplace, PropertyChangeMask,
    RevertToPointerRoot, StructureNotifyMask, SubstructureNotifyMask,
    SubstructureRedirectMask, Success, True, XChangeProperty,
    XChangeWindowAttributes, XClassHint, XCreateSimpleWindow, XCreateWindow,
    XDefaultDepth, XDefaultRootWindow, XDefaultScreen, XDefaultVisual,
    XDefineCursor, XDeleteProperty, XDestroyWindow, XDisplayHeight,
    XDisplayWidth, XEvent, XFree, XFreeModifiermap, XFreeStringList,
    XGetModifierMapping, XGetTextProperty, XGetWMHints, XGetWMProtocols,
    XGrabButton, XInternAtom, XKeysymToKeycode, XMapRaised, XQueryPointer,
    XRootWindow, XSelectInput, XSendEvent, XSetClassHint, XSetInputFocus,
    XSetWMHints, XSetWindowAttributes, XSetWindowBorder, XSync, XUngrabButton,
    XUnmapWindow, XUrgencyHint, XmbTextPropertyToTextList, XA_ATOM, XA_STRING,
    XA_WINDOW, XA_WM_NAME,
};
use x11::xlib::{XErrorEvent, XOpenDisplay, XSetErrorHandler};

use crate::config::{BUTTONS, TAGS};

struct Display {
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

static mut STEXT: String = String::new();

/// bar height
static mut BH: i16 = 0;
static mut SW: i16 = 0;
static mut SH: i16 = 0;

static mut ROOT: Window = 0;
static mut WMCHECKWIN: Window = 0;

static mut WMATOM: [Atom; WM::Last as usize] = [0; WM::Last as usize];
static mut NETATOM: [Atom; Net::Last as usize] = [0; Net::Last as usize];

static mut CURSOR: [Cursor; Cur::Last as usize] = [0; Cur::Last as usize];

/// color scheme
static mut SCHEME: Vec<Vec<Clr>> = Vec::new();

/// sum of left and right padding for text
static mut LRPAD: usize = 0;

const NUMLOCKMASK: u32 = 0;
const BUTTONMASK: i64 = ButtonPressMask | ButtonReleaseMask;

pub enum Arg {
    Uint(usize),
    Str(&'static str),
    Layout(&'static Layout),
}

pub struct Button {
    pub click: Clk,
    pub mask: u32,
    pub button: u32,
    pub func: fn(arg: Arg),
    pub arg: Arg,
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
    hintsvalid: i32,
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

type Window = u64;
type Atom = u64;
type Cursor = u64;
type Clr = XftColor;

#[derive(PartialEq)]
pub struct Layout {
    symbol: &'static str,
    arrange: fn(mon: *mut Monitor),
}

struct Monitor {
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

fn setup(dpy: &mut Display) {
    let mut sa: MaybeUninit<sigaction> = MaybeUninit::uninit();
    let mut wa: MaybeUninit<XSetWindowAttributes> = MaybeUninit::uninit();

    unsafe {
        // Safety: None except that dwm does it
        let mut wa = wa.assume_init();
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
        let lrpa = (*(*DRW).fonts).h;
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

        // init cursors
        CURSOR[Cur::Normal as usize] =
            DRW.as_ref().unwrap().cur_create(XC_LEFT_PTR);
        CURSOR[Cur::Resize as usize] =
            DRW.as_ref().unwrap().cur_create(XC_SIZING);
        CURSOR[Cur::Move as usize] = DRW.as_ref().unwrap().cur_create(XC_FLEUR);

        // init appearance
        SCHEME = Vec::with_capacity(COLORS.len());
        for i in 0..COLORS.len() {
            SCHEME.push(DRW.as_ref().unwrap().scm_create(COLORS[i], 3));
        }

        // init bars
        updatebars(dpy);
        updatestatus(dpy);

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

        // select events
        wa.cursor = CURSOR[Cur::Normal as usize];
        wa.event_mask = SubstructureRedirectMask
            | SubstructureNotifyMask
            | ButtonPressMask
            | PointerMotionMask
            | EnterWindowMask
            | LeaveWindowMask
            | StructureNotifyMask
            | PropertyChangeMask;
        XChangeWindowAttributes(
            dpy.inner,
            ROOT,
            CWEventMask | CWCursor,
            &mut wa as *mut _,
        );
        XSelectInput(dpy.inner, ROOT, wa.event_mask);
        grabkeys();
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
            detach_stack(c);
            attach_stack(c);
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
    let mut ev = MaybeUninit::uninit();
    unsafe {
        let mut ev: XEvent = ev.assume_init();
        if XGetWMProtocols(dpy.inner, (*c).win, &mut protocols, &mut n) != 0 {
            while !exists && n > 0 {
                exists = *protocols.offset(n as isize) == proto;
                n -= 1;
            }
            XFree(protocols.cast());
        }
        if exists {
            ev.type_ = ClientMessage;
            ev.client_message.window = (*c).win;
            ev.client_message.message_type = WMATOM[WM::Protocols as usize];
            ev.client_message.format = 32;
            ev.client_message.data.set_long(0, proto as i64);
            ev.client_message.data.set_long(1, CurrentTime as i64);
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
                        BUTTONS[i].button as u32,
                        (BUTTONS[i].mask | modifiers[j]) as u32,
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

pub fn setlayout(arg: Arg) {
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
            arrange(SELMON);
        } else {
            drawbar(SELMON);
        }
    }
}

fn arrange(mut m: *mut Monitor) {
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
            }
        }
    }
}

fn arrangemon(m: *mut Monitor) {
    unsafe {
        (*m).ltsymbol = (*(*m).lt[(*m).sellt]).symbol.to_owned();
        ((*(*m).lt[(*m).sellt]).arrange)(m)
    }
}

fn restack(m: *mut Monitor) {
    todo!()
}

fn showhide(stack: *mut Client) {
    todo!()
}

pub fn zoom(arg: Arg) {}
pub fn spawn(arg: Arg) {}
pub fn movemouse(arg: Arg) {}
pub fn togglefloating(arg: Arg) {}
pub fn resizemouse(arg: Arg) {}
pub fn view(arg: Arg) {}
pub fn toggleview(arg: Arg) {}
pub fn tag(arg: Arg) {}
pub fn toggletag(arg: Arg) {}

fn grabkeys() {
    todo!()
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
                    numlockmask = (1 << i);
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
    unsafe {
        let c = gettextprop(dpy, ROOT, XA_WM_NAME, &mut STEXT);
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
        let mut tw = 0;

        if !(*m).showbar {
            return;
        }

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

        let mut c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent {
                urg |= (*c).tags;
            }
        }

        let mut x = 0;
        for i in 0..TAGS.len() {
            let w = DRW.as_ref().unwrap().textw(&TAGS[i], LRPAD);
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
                &TAGS[i],
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
    let size = text.len();
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
        let mut list = std::ptr::null_mut();
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

fn updategeom(dpy: &Display) -> i32 {
    let mut dirty = 0;
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
                    dirty = 1;
                    (*m).num = i as i32;
                    (*m).mx = unique[i].x_org;
                    (*m).wx = unique[i].x_org;
                    (*m).my = unique[i].y_org;
                    (*m).wy = unique[i].y_org;
                    (*m).mw = unique[i].width;
                    (*m).ww = unique[i].width;
                    (*m).mh = unique[i].height;
                    (*m).wh = unique[i].height;
                    update_bar_pos(m);
                }

                m = (*m).next;
                i += 1;
            }

            // removed monitors if n > nn
            for i in nn..n as i32 {
                let mut m = MONS;
                while !m.is_null() && !(*m).next.is_null() {
                    m = (*m).next;
                }

                let mut c = (*m).clients;
                while !c.is_null() {
                    dirty = 1;
                    (*m).clients = (*c).next;
                    detach_stack(c);
                    (*c).mon = MONS;
                    attach(c);
                    attach_stack(c);
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
                dirty = 1;
                (*MONS).mw = SW;
                (*MONS).ww = SW;
                (*MONS).mh = SH;
                (*MONS).wh = SH;
                update_bar_pos(MONS);
            }
        }
        if dirty != 0 {
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

#[inline]
fn intersect(x: i32, y: i32, w: i32, h: i32, m: *mut Monitor) -> i32 {
    unsafe {
        i32::max(
            0,
            i32::min((x) + (w), (*m).wx as i32 + (*m).ww as i32)
                - i32::max((x), (*m).wx as i32),
        ) * i32::max(
            0,
            i32::min((y) + (h), (*m).wy as i32 + (*m).wh as i32)
                - i32::max((y), (*m).wy as i32),
        )
    }
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

fn attach_stack(c: *mut Client) {
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

fn detach_stack(c: *mut Client) {
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

fn update_bar_pos(m: *mut Monitor) {
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

mod config;
mod drw;

fn main() {
    let mut dpy = Display::open();
    checkotherwm(&dpy);
    setup(&mut dpy);
}
