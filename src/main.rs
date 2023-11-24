//! tiling window manager based on dwm

#![allow(unused)]

use std::ffi::{c_int, CString};
use std::mem::MaybeUninit;

use config::{FONTS, LAYOUTS, MFACT, NMASTER, SHOWBAR, TOPBAR};
use drw::Drw;
use libc::{
    sigaction, sigemptyset, waitpid, SA_NOCLDSTOP, SA_NOCLDWAIT, SA_RESTART,
    SIGCHLD, SIG_IGN, WNOHANG,
};
use x11::xinerama::{
    XineramaIsActive, XineramaQueryScreens, XineramaScreenInfo,
};
use x11::xlib::{
    BadAccess, BadDrawable, BadMatch, BadWindow, Display as XDisplay, False,
    SubstructureRedirectMask, XDefaultRootWindow, XDefaultScreen,
    XDestroyWindow, XDisplayHeight, XDisplayWidth, XFree, XInternAtom,
    XQueryPointer, XRootWindow, XSelectInput, XSync, XUnmapWindow,
};
use x11::xlib::{XErrorEvent, XOpenDisplay, XSetErrorHandler};

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

/// again using a vec instead of a linked list
static mut MONS: *mut Monitor = std::ptr::null_mut();

/// bar height
static mut BH: i16 = 0;
static mut SW: i16 = 0;
static mut SH: i16 = 0;

static mut ROOT: Window = 0;

static mut WMATOM: [Atom; WM::Last as usize] = [0; WM::Last as usize];
static mut NETATOM: [Atom; Net::Last as usize] = [0; Net::Last as usize];

static mut CURSOR: [Cursor; Cur::Last as usize] = [0; Cur::Last as usize];

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

struct Layout {
    symbol: &'static str,
    arrange: fn(mon: &Monitor),
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

fn setup(dpy: &Display) {
    let mut sa: MaybeUninit<sigaction> = MaybeUninit::uninit();

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
        let screen = XDefaultScreen(dpy.inner);
        let sw = XDisplayWidth(dpy.inner, screen);
        let sh = XDisplayHeight(dpy.inner, screen);
        ROOT = XRootWindow(dpy.inner, screen);
        let mut drw = Drw::new(dpy, screen, ROOT, sw as usize, sh as usize);

        drw.fontset_create(FONTS).expect("no fonts could be loaded");
        let lrpa = drw.fonts[0].h;
        BH = (drw.fonts[0].h + 2) as i16;
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
        CURSOR[Cur::Normal as usize] = drw.cur_create(XC_LEFT_PTR);
        CURSOR[Cur::Resize as usize] = drw.cur_create(XC_SIZING);
        CURSOR[Cur::Move as usize] = drw.cur_create(XC_FLEUR);
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
    let dpy = Display::open();
    checkotherwm(&dpy);
    setup(&dpy);
}
