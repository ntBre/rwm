//! tiling window manager based on dwm

use std::ffi::c_int;
use std::mem::MaybeUninit;

use drw::Drw;
use libc::{
    sigaction, sigemptyset, waitpid, SA_NOCLDSTOP, SA_NOCLDWAIT, SA_RESTART,
    SIGCHLD, SIG_IGN, WNOHANG,
};
use x11::xlib::{
    BadAccess, BadDrawable, BadMatch, BadWindow, Display as XDisplay, False,
    SubstructureRedirectMask, XDefaultRootWindow, XDefaultScreen,
    XDisplayHeight, XDisplayWidth, XRootWindow, XSelectInput, XSync,
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

        // TODO read from config
        let fonts = ["monospace:size=10"];
        drw.fontset_create(fonts).expect("no fonts could be loaded");
    }
}

mod drw;

fn main() {
    let dpy = Display::open();
    checkotherwm(&dpy);
    setup(&dpy);
}
