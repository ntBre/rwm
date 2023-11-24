//! tiling window manager based on dwm

use std::ffi::c_int;
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
    XDisplayHeight, XDisplayWidth, XFree, XRootWindow, XSelectInput, XSync,
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

static mut SELMON: *const Monitor = std::ptr::null();

/// again using a vec instead of a linked list
static mut MONS: Vec<Monitor> = Vec::new();

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
    next: *const Client,
    snext: *const Client,
    mon: *const Monitor,
    win: Window,
}

struct Window {}

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
    by: i32,
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
    clients: Vec<Client>,
    /// index into clients vec, pointer in C
    sel: usize,
    /// this is probably part of a linked list in C, so maybe a vec of indices?
    stack: Vec<usize>,
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
            clients: Vec::new(),
            sel: 0,
            stack: Vec::new(),
            barwin: Window {},
            lt: [&LAYOUTS[0], &LAYOUTS[1 % LAYOUTS.len()]],
        }
    }
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
        let root = XRootWindow(dpy.inner, screen);
        let mut drw = Drw::new(dpy, screen, root, sw as usize, sh as usize);

        drw.fontset_create(FONTS).expect("no fonts could be loaded");
        let lrpa = drw.fonts[0].h;
        let bh = drw.fonts[0].h + 2;
        update_geom(dpy);
    }
}

fn update_geom(dpy: &Display) -> i32 {
    let mut dirty = 0;
    unsafe {
        if XineramaIsActive(dpy.inner) != 0 {
            // I think this is the number of monitors
            let mut nn: i32 = 0;
            let info = XineramaQueryScreens(dpy.inner, &mut nn as *mut _);
            let mut unique = vec![MaybeUninit::uninit(); nn as usize];
            let n = MONS.len();
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
                // this is the C code:
                // for (m = mons; m && m->next; m = m->next);
                //
                // so I think it's walking the linked list of monitors, leaving
                // m as the last monitor. then it checks if (m) and sets m->next
                // to createmon if m else mons = createmon, which I guess
                // translates to always MONITORS.push(createmon()) in my case
                MONS.push(Monitor::new());
            }

            for (i, m) in MONS.iter_mut().enumerate() {
                if i >= n
                    || unique[i].x_org != m.mx
                    || unique[i].y_org != m.my
                    || unique[i].width != m.mw
                    || unique[i].height != m.mh
                {
                    dirty = 1;
                    m.num = i as i32;
                    m.mx = unique[i].x_org;
                    m.wx = unique[i].x_org;
                    m.my = unique[i].y_org;
                    m.wy = unique[i].y_org;
                    m.mw = unique[i].width;
                    m.ww = unique[i].width;
                    m.mh = unique[i].height;
                    m.wh = unique[i].height;
                    m.update_bar_pos();
                }
            }

            // removed monitors if n > nn
            for i in nn..n {
                let m = MONS.last().unwrap();
                for c in m.clients {
                    dirty = 1;
                    c.detach_stack();
                    c.mon = &MONS[0] as *const _;
                    c.attach();
                    c.attach_stack();
                }
                if m == SELMON {
                    SELMON = &MONS[0] as *const _;
                }
                m.cleanup();
            }
        }
    }
    todo!()
}

fn isuniquegeom(
    unique: &Vec<MaybeUninit<XineramaScreenInfo>>,
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
