//! tiling window manager based on dwm

#![allow(clippy::needless_range_loop)]

mod bindgen {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(improper_ctypes)]
    #![allow(clippy::upper_case_acronyms)]
    #![allow(unused)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::cmp::max;
use std::ffi::{c_char, c_int, c_uint, CString};
use std::mem::size_of_val;
use std::mem::{size_of, MaybeUninit};
use std::ptr::{addr_of, addr_of_mut, null_mut};
use std::sync::LazyLock;

use bindgen::{drw, strncpy, Atom, Client};
use libc::{c_long, c_uchar, c_ulong, sigaction};
use x11::xlib::{
    BadAccess, BadDrawable, BadMatch, BadWindow, CWBorderWidth,
    EnterWindowMask, False, FocusChangeMask, IsViewable, PropModeAppend,
    PropertyChangeMask, RevertToPointerRoot, StructureNotifyMask,
    SubstructureRedirectMask, Success, XFree, XA_ATOM, XA_STRING, XA_WINDOW,
};
use x11::xlib::{Display as XDisplay, XA_WM_NAME};
use x11::xlib::{XErrorEvent, XSetErrorHandler};

use crate::bindgen::{dpy, CurrentTime};
// use crate::config::{
//     BORDERPX, BUTTONS, DMENUCMD, LOCKFULLSCREEN, RESIZEHINTS, RULES, SNAP, TAGS,
// };

// pub struct Display {
//     inner: *mut XDisplay,
// }

// impl Display {
// fn open() -> Self {
//     let inner = unsafe { XOpenDisplay(std::ptr::null()) };
//     if inner.is_null() {
//         panic!("cannot open display");
//     }
//     Display { inner }
// }
// }

/// function to be called on a startup error
extern "C" fn xerrorstart(_: *mut XDisplay, _: *mut XErrorEvent) -> c_int {
    panic!("another window manager is already running")
}

extern "C" {
    static mut numlockmask: c_uint;
    static mut running: c_int;
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
// const XC_LEFT_PTR: u8 = 68;
// const XC_SIZING: u8 = 120;
// const XC_FLEUR: u8 = 52;

// from X.h
// const BUTTON_RELEASE: i32 = 5;

// from Xutil.h
/// for windows that are not mapped
// const WITHDRAWN_STATE: usize = 0;
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

// extern "C" fn xerrordummy(_dpy: *mut XDisplay, _ee: *mut XErrorEvent) -> c_int {
//     0
// }

/// I hate to start using globals already, but I'm not sure how else to do it.
/// maybe we can pack this stuff into a struct eventually
static mut XERRORXLIB: Option<
    unsafe extern "C" fn(*mut XDisplay, *mut XErrorEvent) -> i32,
> = None;

// static mut SELMON: *mut Monitor = std::ptr::null_mut();

// static mut MONS: *mut Monitor = std::ptr::null_mut();

// static mut DRW: *mut Drw = std::ptr::null_mut();

// static mut SCREEN: i32 = 0;

// const BROKEN: &str = "broken";
// static mut STEXT: String = String::new();

/// bar height
// static mut BH: i16 = 0;
// static mut SW: c_int = 0;
// static mut SH: c_int = 0;

// static mut ROOT: Window = 0;
// static mut WMCHECKWIN: Window = 0;

// static mut WMATOM: [Atom; WM::Last as usize] = [0; WM::Last as usize];
// static mut NETATOM: [Atom; Net::Last as usize] = [0; Net::Last as usize];

// static mut RUNNING: bool = true;

// static mut CURSOR: [Cursor; Cur::Last as usize] = [0; Cur::Last as usize];

/// color scheme
// static mut SCHEME: Vec<Vec<Clr>> = Vec::new();

/// sum of left and right padding for text
// static mut LRPAD: usize = 0;

// #[allow(non_upper_case_globals)]
// static mut numlockmask: u32 = 0;
// const BUTTONMASK: i64 = ButtonPressMask | ButtonReleaseMask;

// const TAGMASK: usize = (1 << TAGS.len()) - 1;
// const MOUSEMASK: i64 = BUTTONMASK | PointerMotionMask;

// #[derive(Clone)]
// pub enum Arg {
//     Uint(usize),
//     Int(isize),
//     Float(f64),
//     Str(&'static [&'static str]),
//     Layout(&'static Layout),
//     None,
// }

// pub struct Button {
//     pub click: Clk,
//     pub mask: u32,
//     pub button: u32,
//     pub func: fn(mdpy: &Display, arg: Arg),
//     pub arg: Arg,
// }

// impl Button {
//     pub const fn new(
//         click: Clk,
//         mask: u32,
//         button: u32,
//         func: fn(mdpy: &Display, arg: Arg),
//         arg: Arg,
//     ) -> Self {
//         Self {
//             click,
//             mask,
//             button,
//             func,
//             arg,
//         }
//     }
// }

// struct Client {
//     name: String,
//     mina: f64,
//     maxa: f64,
//     x: i32,
//     y: i32,
//     w: i32,
//     h: i32,
//     oldx: i32,
//     oldy: i32,
//     oldw: i32,
//     oldh: i32,
//     basew: i32,
//     baseh: i32,
//     incw: i32,
//     inch: i32,
//     maxw: i32,
//     maxh: i32,
//     minw: i32,
//     minh: i32,
//     hintsvalid: bool,
//     bw: i32,
//     oldbw: i32,
//     tags: usize,
//     isfixed: bool,
//     isfloating: bool,
//     isurgent: bool,
//     neverfocus: bool,
//     oldstate: bool,
//     isfullscreen: bool,
//     next: *mut Client,
//     snext: *mut Client,
//     mon: *mut Monitor,
//     win: Window,
// }

// impl Default for Client {
//     fn default() -> Self {
//         Self {
//             name: Default::default(),
//             mina: Default::default(),
//             maxa: Default::default(),
//             x: Default::default(),
//             y: Default::default(),
//             w: Default::default(),
//             h: Default::default(),
//             oldx: Default::default(),
//             oldy: Default::default(),
//             oldw: Default::default(),
//             oldh: Default::default(),
//             basew: Default::default(),
//             baseh: Default::default(),
//             incw: Default::default(),
//             inch: Default::default(),
//             maxw: Default::default(),
//             maxh: Default::default(),
//             minw: Default::default(),
//             minh: Default::default(),
//             hintsvalid: Default::default(),
//             bw: Default::default(),
//             oldbw: Default::default(),
//             tags: Default::default(),
//             isfixed: Default::default(),
//             isfloating: Default::default(),
//             isurgent: Default::default(),
//             neverfocus: Default::default(),
//             oldstate: Default::default(),
//             isfullscreen: Default::default(),
//             next: std::ptr::null_mut(),
//             snext: std::ptr::null_mut(),
//             mon: std::ptr::null_mut(),
//             win: Default::default(),
//         }
//     }
// }

type Window = u64;
// type Atom = u64;
// type Cursor = u64;
// type Clr = XftColor;

// pub struct Key {
//     pub modkey: u32,
//     pub keysym: u32,
//     pub func: fn(mdpy: &Display, arg: Arg),
//     pub arg: Arg,
// }

// impl Key {
//     pub const fn new(
//         modkey: u32,
//         keysym: u32,
//         func: fn(mdpy: &Display, arg: Arg),
//         arg: Arg,
//     ) -> Self {
//         Self {
//             modkey,
//             keysym,
//             func,
//             arg,
//         }
//     }
// }

#[derive(PartialEq)]
// pub struct Layout {
//     symbol: &'static str,
//     arrange: Option<fn(mdpy: &Display, mon: *mut Monitor)>,
// }

// pub struct Monitor {
//     ltsymbol: String,
//     mfact: f64,
//     nmaster: i32,
//     num: i32,
//     /// bar geometry
//     by: i16,
//     /// screen size
//     mx: i16,
//     my: i16,
//     mw: i16,
//     mh: i16,
//     /// window area
//     wx: i16,
//     wy: i16,
//     ww: i16,
//     wh: i16,
//     seltags: usize,
//     sellt: usize,
//     tagset: [usize; 2],
//     showbar: bool,
//     topbar: bool,
//     clients: *mut Client,
//     /// index into clients vec, pointer in C
//     sel: *mut Client,
//     stack: *mut Client,
//     next: *mut Monitor,
//     barwin: Window,
//     lt: [*const Layout; 2],
// }

// impl Monitor {
// fn new() -> Self {
//     Self {
//         ltsymbol: LAYOUTS[0].symbol.to_owned(),
//         mfact: MFACT,
//         nmaster: NMASTER,
//         num: 0,
//         by: 0,
//         mx: 0,
//         my: 0,
//         mw: 0,
//         mh: 0,
//         wx: 0,
//         wy: 0,
//         ww: 0,
//         wh: 0,
//         seltags: 0,
//         sellt: 0,
//         tagset: [1, 1],
//         showbar: SHOWBAR,
//         topbar: TOPBAR,
//         clients: std::ptr::null_mut(),
//         sel: std::ptr::null_mut(),
//         stack: std::ptr::null_mut(),
//         next: std::ptr::null_mut(),
//         barwin: 0,
//         lt: [&LAYOUTS[0], &LAYOUTS[1 % LAYOUTS.len()]],
//     }
// }
// }

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

// fn createmon() -> *mut Monitor {
//     let mon = Monitor::new();
//     Box::into_raw(Box::new(mon))
// }

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

// #[derive(Debug)]
// #[repr(C)]
// enum WM {
//     Protocols,
//     Delete,
//     State,
//     TakeFocus,
//     Last,
// }

// #[repr(C)]
// enum Net {
//     Supported,
//     WMName,
//     WMState,
//     WMCheck,
//     WMFullscreen,
//     ActiveWindow,
//     WMWindowType,
//     WMWindowTypeDialog,
//     ClientList,
//     Last,
// }

// #[repr(C)]
// enum Cur {
//     Normal,
//     Resize,
//     Move,
//     Last,
// }

// #[repr(C)]
// enum Scheme {
//     Norm,
//     Sel,
// }

/// Color scheme index
// #[repr(C)]
// enum Col {
//     Fg,
//     Bg,
//     Border,
// }

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
    unsafe {
        let mut wa = bindgen::XSetWindowAttributes {
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
            sa_restorer: None,
        };
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGCHLD, &sa, null_mut());

        while libc::waitpid(-1, null_mut(), libc::WNOHANG) > 0 {}

        use bindgen::{bh, drw, fonts, lrpad, root, screen, sh, sw};

        screen = bindgen::XDefaultScreen(dpy);
        sw = bindgen::XDisplayWidth(dpy, screen);
        sh = bindgen::XDisplayHeight(dpy, screen);
        root = bindgen::XRootWindow(dpy, screen);
        drw = drw::create(dpy, screen, root, sw as u32, sh as u32);
        if drw::fontset_create(drw, &mut *addr_of_mut!(fonts), fonts.len())
            .is_null()
        {
            panic!("no fonts could be loaded");
        }
        lrpad = (*(*drw).fonts).h as i32;
        bh = (*(*drw).fonts).h as i32 + 2;
        updategeom();

        use bindgen::{netatom, wmatom, XInternAtom};

        /* init atoms */
        let utf8string =
            bindgen::XInternAtom(dpy, c"UTF8_STRING".as_ptr(), False);
        wmatom[bindgen::WMProtocols as usize] =
            XInternAtom(dpy, c"WM_PROTOCOLS".as_ptr(), False);
        wmatom[bindgen::WMDelete as usize] =
            XInternAtom(dpy, c"WM_DELETE_WINDOW".as_ptr(), False);
        wmatom[bindgen::WMState as usize] =
            XInternAtom(dpy, c"WM_STATE".as_ptr(), False);
        wmatom[bindgen::WMTakeFocus as usize] =
            XInternAtom(dpy, c"WM_TAKE_FOCUS".as_ptr(), False);

        netatom[bindgen::NetActiveWindow as usize] =
            XInternAtom(dpy, c"_NET_ACTIVE_WINDOW".as_ptr(), False);
        netatom[bindgen::NetSupported as usize] =
            XInternAtom(dpy, c"_NET_SUPPORTED".as_ptr(), False);
        netatom[bindgen::NetWMName as usize] =
            XInternAtom(dpy, c"_NET_WM_NAME".as_ptr(), False);
        netatom[bindgen::NetWMState as usize] =
            XInternAtom(dpy, c"_NET_WM_STATE".as_ptr(), False);
        netatom[bindgen::NetWMCheck as usize] =
            XInternAtom(dpy, c"_NET_SUPPORTING_WM_CHECK".as_ptr(), False);
        netatom[bindgen::NetWMFullscreen as usize] =
            XInternAtom(dpy, c"_NET_WM_STATE_FULLSCREEN".as_ptr(), False);
        netatom[bindgen::NetWMWindowType as usize] =
            XInternAtom(dpy, c"_NET_WM_WINDOW_TYPE".as_ptr(), False);
        netatom[bindgen::NetWMWindowTypeDialog as usize] =
            XInternAtom(dpy, c"_NET_WM_WINDOW_TYPE_DIALOG".as_ptr(), False);
        netatom[bindgen::NetClientList as usize] =
            XInternAtom(dpy, c"_NET_CLIENT_LIST".as_ptr(), False);

        use bindgen::cursor;
        /* init cursors */
        cursor[bindgen::CurNormal as usize] =
            drw::cur_create(drw, bindgen::XC_left_ptr as i32);
        cursor[bindgen::CurResize as usize] =
            drw::cur_create(drw, bindgen::XC_sizing as i32);
        cursor[bindgen::CurMove as usize] =
            drw::cur_create(drw, bindgen::XC_fleur as i32);

        use bindgen::{colors, scheme, Clr};

        /* init appearance */
        scheme = util::ecalloc(colors.len(), size_of::<*mut Clr>()).cast();
        for i in 0..colors.len() {
            *scheme.add(i) =
                drw::scm_create(drw, &mut *addr_of_mut!(colors[i]), 3);
        }

        /* init bars */
        updatebars();
        updatestatus();

        use bindgen::wmcheckwin;

        /* supporting window for NetWMCheck */
        wmcheckwin =
            bindgen::XCreateSimpleWindow(dpy, root, 0, 0, 1, 1, 0, 0, 0);
        bindgen::XChangeProperty(
            dpy,
            wmcheckwin,
            netatom[bindgen::NetWMCheck as usize],
            XA_WINDOW,
            32,
            bindgen::PropModeReplace as i32,
            addr_of_mut!(wmcheckwin) as *mut c_uchar,
            1,
        );
        bindgen::XChangeProperty(
            dpy,
            wmcheckwin,
            netatom[bindgen::NetWMName as usize],
            utf8string,
            8,
            bindgen::PropModeReplace as i32,
            c"dwm".as_ptr() as *mut c_uchar,
            3,
        );
        bindgen::XChangeProperty(
            dpy,
            root,
            netatom[bindgen::NetWMCheck as usize],
            XA_WINDOW,
            32,
            bindgen::PropModeReplace as i32,
            addr_of_mut!(wmcheckwin) as *mut c_uchar,
            1,
        );
        /* EWMH support per view */
        bindgen::XChangeProperty(
            dpy,
            root,
            netatom[bindgen::NetSupported as usize],
            XA_ATOM,
            32,
            bindgen::PropModeReplace as i32,
            netatom.as_ptr() as *mut c_uchar,
            bindgen::NetLast as i32,
        );
        bindgen::XDeleteProperty(
            dpy,
            root,
            netatom[bindgen::NetClientList as usize],
        );

        // /* select events */
        wa.cursor = (*cursor[bindgen::CurNormal as usize]).cursor;
        wa.event_mask = SubstructureRedirectMask
            | bindgen::SubstructureNotifyMask as i64
            | bindgen::ButtonPressMask as i64
            | bindgen::PointerMotionMask as i64
            | EnterWindowMask
            | bindgen::LeaveWindowMask as i64
            | StructureNotifyMask
            | PropertyChangeMask;
        bindgen::XChangeWindowAttributes(
            dpy,
            root,
            bindgen::CWEventMask as u64 | bindgen::CWCursor as u64,
            &mut wa,
        );
        bindgen::XSelectInput(dpy, root, wa.event_mask);
        grabkeys();
        focus(null_mut());
    }
}

fn focus(c: *mut bindgen::Client) {
    use bindgen::{scheme, selmon, ColBorder, SchemeSel};
    unsafe {
        if c.is_null() || !is_visible(c) {
            let mut c = (*selmon).stack;
            while !c.is_null() && !is_visible(c) {
                c = (*c).snext;
            }
        }
        if !(*selmon).sel.is_null() && (*selmon).sel != c {
            unfocus((*selmon).sel, false);
        }
        if !c.is_null() {
            if (*c).mon != selmon {
                selmon = (*c).mon;
            }
            if (*c).isurgent != 0 {
                seturgent(c, false);
            }
            detachstack(c);
            attachstack(c);
            grabbuttons(c, true);
            let color = (*(*scheme.offset(SchemeSel as isize))
                .offset(ColBorder as isize))
            .pixel;
            bindgen::XSetWindowBorder(dpy, (*c).win, color);
            setfocus(c);
        } else {
            bindgen::XSetInputFocus(
                dpy,
                bindgen::root,
                RevertToPointerRoot,
                CurrentTime as u64,
            );
            bindgen::XDeleteProperty(
                dpy,
                bindgen::root,
                bindgen::netatom[bindgen::NetActiveWindow as usize],
            );
        }
        (*bindgen::selmon).sel = c;
        drawbars();
    }
}

fn drawbars() {
    unsafe {
        let mut m = bindgen::mons;
        while !m.is_null() {
            drawbar(m);
            m = (*m).next;
        }
    }
}

fn setfocus(c: *mut Client) {
    unsafe {
        if (*c).neverfocus == 0 {
            bindgen::XSetInputFocus(
                dpy,
                (*c).win,
                RevertToPointerRoot,
                bindgen::CurrentTime as u64,
            );
            bindgen::XChangeProperty(
                dpy,
                bindgen::root,
                bindgen::netatom[bindgen::NetActiveWindow as usize],
                XA_WINDOW,
                32,
                bindgen::PropModeReplace as i32,
                (&mut (*c).win) as *mut u64 as *mut c_uchar,
                1,
            );
        }
        sendevent(c, bindgen::wmatom[bindgen::WMTakeFocus as usize]);
    }
}

fn sendevent(c: *mut Client, proto: Atom) -> c_int {
    let mut n = 0;
    let mut protocols = std::ptr::null_mut();
    let mut exists = 0;
    unsafe {
        if bindgen::XGetWMProtocols(dpy, (*c).win, &mut protocols, &mut n) != 0
        {
            while exists == 0 && n > 0 {
                exists = (*protocols.offset(n as isize) == proto) as c_int;
                n -= 1;
            }
            XFree(protocols.cast());
        }
        use bindgen::{wmatom, WMProtocols};
        if exists != 0 {
            let mut ev = bindgen::XEvent {
                type_: bindgen::ClientMessage as i32,
            };
            ev.xclient.window = (*c).win;
            ev.xclient.message_type = wmatom[WMProtocols as usize];
            ev.xclient.format = 32;
            ev.xclient.data.l[0] = proto as c_long;
            ev.xclient.data.l[1] = CurrentTime as c_long;
            bindgen::XSendEvent(
                dpy,
                (*c).win,
                False,
                bindgen::NoEventMask as i64,
                &mut ev,
            );
        }
        exists
    }
}

fn grabbuttons(c: *mut bindgen::Client, focused: bool) {
    unsafe {
        updatenumlockmask();
        let modifiers = [
            0,
            bindgen::LockMask,
            numlockmask,
            numlockmask | bindgen::LockMask,
        ];
        bindgen::XUngrabButton(
            dpy,
            bindgen::AnyButton,
            bindgen::AnyModifier,
            (*c).win,
        );
        const BUTTONMASK: u32 =
            bindgen::ButtonPressMask | bindgen::ButtonReleaseMask;
        if !focused {
            bindgen::XGrabButton(
                dpy,
                bindgen::AnyButton,
                bindgen::AnyModifier,
                (*c).win,
                False,
                BUTTONMASK,
                bindgen::GrabModeSync as i32,
                bindgen::GrabModeSync as i32,
                bindgen::None as u64,
                bindgen::None as u64,
            );
        }
        for i in 0..bindgen::buttons.len() {
            if bindgen::buttons[i].click == bindgen::ClkClientWin {
                for j in 0..modifiers.len() {
                    bindgen::XGrabButton(
                        dpy,
                        bindgen::buttons[i].button,
                        bindgen::buttons[i].mask | modifiers[j],
                        (*c).win,
                        False,
                        BUTTONMASK,
                        bindgen::GrabModeAsync as i32,
                        bindgen::GrabModeSync as i32,
                        bindgen::None as u64,
                        bindgen::None as u64,
                    );
                }
            }
        }
    }
}

// pub fn setlayout(mdpy: &Display, arg: Arg) {
//     unsafe {
//         if let Arg::Layout(lt) = arg {
//             if lt as *const _ != (*SELMON).lt[(*SELMON).sellt] {
//                 (*SELMON).sellt ^= 1;
//             }
//             (*SELMON).lt[(*SELMON).sellt] = lt;
//         } else {
//             // same as inner if above but not sure how to chain them otherwise
//             (*SELMON).sellt ^= 1;
//         }
//         (*SELMON).ltsymbol = (*(*SELMON).lt[(*SELMON).sellt]).symbol.to_owned();
//         if !(*SELMON).sel.is_null() {
//             arrange(mdpy, SELMON);
//         } else {
//             drawbar(SELMON);
//         }
//     }
// }

fn arrange(mut m: *mut bindgen::Monitor) {
    unsafe {
        if !m.is_null() {
            showhide((*m).stack);
        } else {
            m = bindgen::mons;
            while !m.is_null() {
                showhide((*m).stack);
                m = (*m).next;
            }
        }

        if !m.is_null() {
            arrangemon(m);
            restack(m);
        } else {
            m = bindgen::mons;
            while !m.is_null() {
                arrangemon(m);
            }
        }
    }
}

fn arrangemon(m: *mut bindgen::Monitor) {
    unsafe {
        strncpy(
            (*m).ltsymbol.as_mut_ptr(),
            (*(*m).lt[(*m).sellt as usize]).symbol,
            size_of_val(&(*m).ltsymbol) as c_ulong,
        );
        // how did bindgen make this an Option??
        let arrange = (*(*m).lt[(*m).sellt as usize]).arrange;
        if let Some(arrange) = arrange {
            (arrange)(m);
        }
    }
}

fn restack(m: *mut bindgen::Monitor) {
    drawbar(m);
    unsafe {
        if (*m).sel.is_null() {
            return;
        }
        if (*(*m).sel).isfloating != 0
            || (*(*m).lt[(*m).sellt as usize]).arrange.is_none()
        {
            bindgen::XRaiseWindow(dpy, (*(*m).sel).win);
        }
        if (*(*m).lt[(*m).sellt as usize]).arrange.is_some() {
            let mut wc = bindgen::XWindowChanges {
                stack_mode: bindgen::Below as i32,
                sibling: (*m).barwin,
                x: Default::default(),
                y: Default::default(),
                width: Default::default(),
                height: Default::default(),
                border_width: Default::default(),
            };
            let mut c = (*m).stack;
            while !c.is_null() {
                if (*c).isfloating == 0 && is_visible(c) {
                    bindgen::XConfigureWindow(
                        dpy,
                        (*c).win,
                        bindgen::CWSibling | bindgen::CWStackMode,
                        &mut wc as *mut _,
                    );
                    wc.sibling = (*c).win;
                }
                c = (*c).snext;
            }
        }
        bindgen::XSync(dpy, False);
        let mut ev = bindgen::XEvent { type_: 0 };
        while bindgen::XCheckMaskEvent(dpy, EnterWindowMask, &mut ev) != 0 {}
    }
}

fn showhide(c: *mut Client) {
    unsafe {
        if c.is_null() {
            return;
        }
        if is_visible(c) {
            // show clients top down
            bindgen::XMoveWindow(dpy, (*c).win, (*c).x, (*c).y);
            if ((*(*(*c).mon).lt[(*(*c).mon).sellt as usize])
                .arrange
                .is_none()
                || (*c).isfloating != 0)
                && (*c).isfullscreen == 0
            {
                resize(c, (*c).x, (*c).y, (*c).w, (*c).h, 0);
            }
            showhide((*c).snext);
        } else {
            // hide clients bottom up
            showhide((*c).snext);
            bindgen::XMoveWindow(dpy, (*c).win, width(c) * -2, (*c).y);
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
    if applysizehints(c, &mut x, &mut y, &mut w, &mut h, interact) != 0 {
        resizeclient(c, x, y, w, h);
    }
}

fn resizeclient(c: *mut Client, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        (*c).oldx = (*c).x;
        (*c).oldy = (*c).y;
        (*c).oldw = (*c).w;
        (*c).oldh = (*c).h;
        (*c).x = x;
        (*c).y = y;
        (*c).w = w;
        (*c).h = h;
        let mut wc = bindgen::XWindowChanges {
            x,
            y,
            width: w,
            height: h,
            border_width: (*c).bw,
            sibling: 0,
            stack_mode: 0,
        };
        bindgen::XConfigureWindow(
            dpy,
            (*c).win,
            bindgen::CWX
                | bindgen::CWY
                | bindgen::CWWidth
                | bindgen::CWHeight
                | bindgen::CWBorderWidth,
            &mut wc,
        );
        configure(c);
        bindgen::XSync(dpy, False);
    }
}

fn configure(c: *mut bindgen::Client) {
    // TODO this looks like a nice Into impl
    unsafe {
        let mut ce = bindgen::XConfigureEvent {
            type_: bindgen::ConfigureNotify as i32,
            serial: 0,
            send_event: 0,
            display: bindgen::dpy,
            event: (*c).win,
            window: (*c).win,
            x: (*c).x,
            y: (*c).y,
            width: (*c).w,
            height: (*c).h,
            border_width: (*c).bw,
            above: bindgen::None as u64,
            override_redirect: bindgen::False as i32,
        };
        bindgen::XSendEvent(
            bindgen::dpy,
            (*c).win,
            False,
            StructureNotifyMask,
            &mut ce as *mut bindgen::XConfigureEvent as *mut bindgen::XEvent,
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
    unsafe {
        let m = (*c).mon;
        let interact = interact != 0;
        // set minimum possible
        *w = 1.max(*w);
        *h = 1.max(*h);
        if interact {
            if *x > bindgen::sw {
                *x = bindgen::sw - width(c);
            }
            if *y > bindgen::sh {
                *y = bindgen::sh - height(c);
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
        if *h < bindgen::bh {
            *h = bindgen::bh;
        }
        if *w < bindgen::bh {
            *w = bindgen::bh;
        }
        if bindgen::resizehints != 0
            || (*c).isfloating != 0
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

fn updatesizehints(c: *mut bindgen::Client) {
    let mut msize: i64 = 0;
    let mut size = bindgen::XSizeHints {
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
        min_aspect: bindgen::XSizeHints__bindgen_ty_1 { x: 0, y: 0 },
        max_aspect: bindgen::XSizeHints__bindgen_ty_1 { x: 0, y: 0 },
        base_width: Default::default(),
        base_height: Default::default(),
        win_gravity: Default::default(),
    };
    unsafe {
        if bindgen::XGetWMNormalHints(dpy, (*c).win, &mut size, &mut msize) == 0
        {
            /* size is uninitialized, ensure that size.flags aren't used */
            size.flags = bindgen::PSize as i64;
        }
        if size.flags & bindgen::PBaseSize as i64 != 0 {
            (*c).basew = size.base_width;
            (*c).baseh = size.base_height;
        } else if size.flags & bindgen::PMinSize as i64 != 0 {
            (*c).basew = size.min_width;
            (*c).baseh = size.min_height;
        } else {
            (*c).basew = 0;
            (*c).baseh = 0;
        }

        if size.flags & bindgen::PResizeInc as i64 != 0 {
            (*c).incw = size.width_inc;
            (*c).inch = size.height_inc;
        } else {
            (*c).incw = 0;
            (*c).inch = 0;
        }

        if size.flags & bindgen::PMaxSize as i64 != 0 {
            (*c).maxw = size.max_width;
            (*c).maxh = size.max_height;
        } else {
            (*c).maxw = 0;
            (*c).maxh = 0;
        }

        if size.flags & bindgen::PMinSize as i64 != 0 {
            (*c).minw = size.min_width;
            (*c).minh = size.min_height;
        } else if size.flags & bindgen::PBaseSize as i64 != 0 {
            (*c).minw = size.base_width;
            (*c).minh = size.base_height;
        } else {
            (*c).minw = 0;
            (*c).minh = 0;
        }

        if size.flags & bindgen::PAspect as i64 != 0 {
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

// pub fn zoom(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         let c = (*SELMON).sel;
//         if c.is_null() || (*c).isfloating {
//             return;
//         }
//         if c == nexttiled((*SELMON).clients) {
//             let c = nexttiled((*c).next);
//             if c.is_null() {
//                 return;
//             }
//         }
//         pop(mdpy, c);
//     }
// }

// fn pop(mdpy: &Display, c: *mut Client) {
//     detach(c);
//     attach(c);
//     focus(mdpy, c);
//     unsafe {
//         arrange(mdpy, (*c).mon);
//     }
// }

// fn detach(c: *mut Client) {
//     unsafe {
//         let mut tc = &mut (*(*c).mon).clients;
//         while !(*tc).is_null() && *tc != c {
//             tc = &mut (*(*tc)).next;
//         }
//         *tc = (*c).next;
//     }
// }

// fn nexttiled(mut c: *mut Client) -> *mut Client {
//     unsafe {
//         while !c.is_null() && ((*c).isfloating || !is_visible(c)) {
//             c = (*c).next;
//         }
//     }
//     c
// }

// pub fn spawn(_dpy: &Display, arg: Arg) {
//     unsafe {
//         let Arg::Str(s) = arg else {
//             return;
//         };

//         if s == DMENUCMD {
//             // this looks like a memory leak, not sure how to fix it. at least
//             // we're only leaking a single-character &str at a time
//             let r: &'static str = format!("{}", (*SELMON).num).leak();
//             let r: Box<&'static str> = Box::new(r);
//             let mut r: &'static &'static str = Box::leak(r);
//             std::ptr::swap(addr_of_mut!(DMENUMON), &mut r);
//         }
//         Command::new(s[0])
//             .args(&s[1..])
//             .spawn()
//             .expect("spawn failed");
//     }
// }

// pub fn movemouse(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         let c = (*SELMON).sel;
//         if c.is_null() {
//             return;
//         }
//         // no support moving fullscreen windows by mouse
//         if (*c).isfullscreen {
//             return;
//         }
//         restack(mdpy, SELMON);
//         let ocx = (*c).x;
//         let ocy = (*c).y;
//         let mut lasttime = 0;
//         let mut x = 0;
//         let mut y = 0;
//         if XGrabPointer(
//             mdpy.inner,
//             ROOT,
//             False,
//             MOUSEMASK as u32,
//             GrabModeAsync,
//             GrabModeAsync,
//             0,
//             CURSOR[Cur::Move as usize],
//             CurrentTime,
//         ) != GrabSuccess
//         {
//             return;
//         }
//         if !getrootptr(mdpy, &mut x, &mut y) {
//             return;
//         }
//         let mut first = true;
//         let mut ev: MaybeUninit<XEvent> = MaybeUninit::uninit();
//         // emulating do while
//         while first || (*ev.as_mut_ptr()).type_ != BUTTON_RELEASE {
//             XMaskEvent(
//                 mdpy.inner,
//                 MOUSEMASK | ExposureMask | SubstructureRedirectMask,
//                 ev.as_mut_ptr(),
//             );
//             #[allow(non_upper_case_globals)]
//             match (*ev.as_mut_ptr()).type_ {
//                 ConfigureRequest | Expose | MapRequest => {
//                     handler(mdpy, ev.as_mut_ptr())
//                 }
//                 MotionNotify => {
//                     let ev = ev.as_mut_ptr();
//                     if ((*ev).motion.time - lasttime) <= (1000 / 60) {
//                         continue;
//                     }
//                     lasttime = (*ev).motion.time;

//                     let mut nx = ocx + (*ev).motion.x - x;
//                     let mut ny = ocy + (*ev).motion.y - y;
//                     let snap = SNAP as i16;
//                     if ((*SELMON).wx - nx as i16).abs() < snap {
//                         nx = (*SELMON).wx as i32;
//                     } else if (((*SELMON).wx + (*SELMON).ww)
//                         - (nx + width(c)) as i16)
//                         .abs()
//                         < snap
//                     {
//                         nx = ((*SELMON).wx + (*SELMON).ww) as i32 - width(c);
//                     }

//                     if ((*SELMON).wy - ny as i16).abs() < snap {
//                         ny = (*SELMON).wy as i32;
//                     } else if (((*SELMON).wy + (*SELMON).wh)
//                         - (ny + height(c)) as i16)
//                         .abs()
//                         < snap
//                     {
//                         ny = ((*SELMON).wy + (*SELMON).wh) as i32 - height(c);
//                     }

//                     if !(*c).isfloating
//                         && (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_some()
//                         && ((nx - (*c).x).abs() > SNAP
//                             || (ny - (*c).y).abs() > SNAP)
//                     {
//                         togglefloating(mdpy, Arg::None);
//                     }
//                     if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
//                         || (*c).isfloating
//                     {
//                         resize(mdpy, c, nx, ny, (*c).w, (*c).h, true);
//                     }
//                 }
//                 _ => {}
//             }
//             first = false;
//         }
//         XUngrabPointer(mdpy.inner, CurrentTime);
//         let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
//         if m != SELMON {
//             sendmon(mdpy, c, m);
//             SELMON = m;
//             focus(mdpy, null_mut());
//         }
//     }
// }

// pub fn togglefloating(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         if (*SELMON).sel.is_null() {
//             return;
//         }
//         if (*(*SELMON).sel).isfullscreen {
//             // no support for fullscreen windows
//             return;
//         }
//         // either toggle or use fixed value
//         (*(*SELMON).sel).isfloating =
//             !(*(*SELMON).sel).isfloating || (*(*SELMON).sel).isfixed;

//         if (*(*SELMON).sel).isfloating {
//             resize(
//                 mdpy,
//                 (*SELMON).sel,
//                 (*(*SELMON).sel).x,
//                 (*(*SELMON).sel).y,
//                 (*(*SELMON).sel).w,
//                 (*(*SELMON).sel).h,
//                 false,
//             );
//         }
//         arrange(mdpy, SELMON);
//     }
// }

// pub fn resizemouse(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         let c = (*SELMON).sel;
//         if c.is_null() {
//             return;
//         }
//         // no support for resizing fullscreen windows by mouse
//         if (*c).isfullscreen {
//             return;
//         }
//         restack(mdpy, SELMON);
//         let ocx = (*c).x;
//         let ocy = (*c).y;
//         let mut lasttime = 0;
//         if XGrabPointer(
//             mdpy.inner,
//             ROOT,
//             False,
//             MOUSEMASK as u32,
//             GrabModeAsync,
//             GrabModeAsync,
//             0,
//             CURSOR[Cur::Resize as usize],
//             CurrentTime,
//         ) != GrabSuccess
//         {
//             return;
//         }
//         XWarpPointer(
//             mdpy.inner,
//             0,
//             (*c).win,
//             0,
//             0,
//             0,
//             0,
//             (*c).w + (*c).bw - 1,
//             (*c).h + (*c).bw - 1,
//         );
//         let mut first = true;
//         // is this allowed? no warning from the compiler. probably I should use
//         // an Option since this gets initialized in the first iteration of the
//         // loop
//         let ev: *mut XEvent = MaybeUninit::uninit().as_mut_ptr();
//         while first || (*ev).type_ != BUTTON_RELEASE {
//             XMaskEvent(
//                 mdpy.inner,
//                 MOUSEMASK | ExposureMask | SubstructureRedirectMask,
//                 ev,
//             );
//             #[allow(non_upper_case_globals)]
//             match (*ev).type_ {
//                 ConfigureRequest | Expose | MapRequest => handler(mdpy, ev),
//                 MotionNotify => {
//                     if ((*ev).motion.time - lasttime) <= (1000 / 60) {
//                         continue;
//                     }
//                     lasttime = (*ev).motion.time;

//                     let nw = max((*ev).motion.x - ocx - 2 * (*c).bw + 1, 1);
//                     let nh = max((*ev).motion.y - ocy - 2 * (*c).bw + 1, 1);
//                     if ((*(*c).mon).wx + nw as i16 >= (*SELMON).wx
//                         && (*(*c).mon).wx + nw as i16
//                             <= (*SELMON).wx + (*SELMON).ww
//                         && (*(*c).mon).wy + nh as i16 >= (*SELMON).wy
//                         && (*(*c).mon).wy + nh as i16
//                             <= (*SELMON).wy + (*SELMON).wh)
//                         && (!(*c).isfloating
//                             && (*(*SELMON).lt[(*SELMON).sellt])
//                                 .arrange
//                                 .is_some()
//                             && (abs(nw - (*c).w) > SNAP
//                                 || abs(nh - (*c).h) > SNAP))
//                     {
//                         togglefloating(mdpy, Arg::None);
//                     }
//                     if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none()
//                         || (*c).isfloating
//                     {
//                         resize(mdpy, c, (*c).x, (*c).y, nw, nh, true);
//                     }
//                 }
//                 _ => {}
//             }
//             first = false;
//         }
//         XWarpPointer(
//             mdpy.inner,
//             0,
//             (*c).win,
//             0,
//             0,
//             0,
//             0,
//             (*c).w + (*c).bw - 1,
//             (*c).h + (*c).bw - 1,
//         );
//         XUngrabPointer(mdpy.inner, CurrentTime);
//         while XCheckMaskEvent(mdpy.inner, EnterWindowMask, ev) != 0 {}
//         let m = recttomon((*c).x, (*c).y, (*c).w, (*c).h);
//         if m != SELMON {
//             sendmon(mdpy, c, m);
//             SELMON = m;
//             focus(mdpy, null_mut());
//         }
//     }
// }

// pub fn view(mdpy: &Display, arg: Arg) {
//     unsafe {
//         let Arg::Uint(ui) = arg else { return };
//         if (ui & TAGMASK) == (*SELMON).tagset[(*SELMON).seltags] {
//             return;
//         }
//         (*SELMON).seltags ^= 1; /* toggle sel tagset */
//         if (ui & TAGMASK) != 0 {
//             (*SELMON).tagset[(*SELMON).seltags] = ui & TAGMASK;
//         }
//         focus(mdpy, null_mut());
//         arrange(mdpy, SELMON);
//     }
// }

// pub fn toggleview(mdpy: &Display, arg: Arg) {
//     unsafe {
//         let Arg::Uint(ui) = arg else { return };
//         let newtagset = (*SELMON).tagset[(*SELMON).seltags] ^ (ui & TAGMASK);
//         if newtagset != 0 {
//             (*SELMON).tagset[(*SELMON).seltags] = newtagset;
//             focus(mdpy, null_mut());
//             arrange(mdpy, SELMON);
//         }
//     }
// }

// pub fn tag(mdpy: &Display, arg: Arg) {
//     let Arg::Uint(ui) = arg else { return };
//     unsafe {
//         if !(*SELMON).sel.is_null() && ui & TAGMASK != 0 {
//             (*(*SELMON).sel).tags = ui & TAGMASK;
//             focus(mdpy, null_mut());
//             arrange(mdpy, SELMON);
//         }
//     }
// }

// pub fn toggletag(mdpy: &Display, arg: Arg) {
//     unsafe {
//         if (*SELMON).sel.is_null() {
//             return;
//         }
//         let Arg::Uint(ui) = arg else { return };
//         let newtags = (*(*SELMON).sel).tags ^ (ui & TAGMASK);
//         if newtags != 0 {
//             (*(*SELMON).sel).tags = newtags;
//             focus(mdpy, null_mut());
//             arrange(mdpy, SELMON);
//         }
//     }
// }

// pub fn togglebar(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         (*SELMON).showbar = !(*SELMON).showbar;
//         updatebarpos(SELMON);
//         XMoveResizeWindow(
//             mdpy.inner,
//             (*SELMON).barwin,
//             (*SELMON).wx as i32,
//             (*SELMON).by as i32,
//             (*SELMON).ww as u32,
//             BH as u32,
//         );
//         arrange(mdpy, SELMON);
//     }
// }

// pub fn focusstack(mdpy: &Display, arg: Arg) {
//     let Arg::Int(ai) = arg else { return };
//     let mut c = null_mut();
//     unsafe {
//         if (*SELMON).sel.is_null()
//             || ((*(*SELMON).sel).isfullscreen && LOCKFULLSCREEN)
//         {
//             return;
//         }

//         if ai > 0 {
//             c = (*(*SELMON).sel).next;
//             while !c.is_null() && !is_visible(c) {
//                 c = (*c).next;
//             }
//             if c.is_null() {
//                 c = (*SELMON).clients;
//                 while !c.is_null() && !is_visible(c) {
//                     c = (*c).next;
//                 }
//             }
//         } else {
//             let mut i = (*SELMON).clients;
//             while i != (*SELMON).sel {
//                 if is_visible(i) {
//                     c = i;
//                 }
//                 i = (*i).next
//             }
//             if c.is_null() {
//                 while !i.is_null() {
//                     if is_visible(i) {
//                         c = i;
//                     }
//                     i = (*i).next;
//                 }
//             }
//         }
//         if !c.is_null() {
//             focus(mdpy, c);
//             restack(mdpy, SELMON);
//         }
//     }
// }

// pub fn incnmaster(mdpy: &Display, arg: Arg) {
//     unsafe {
//         let Arg::Int(ai) = arg else { return };
//         (*SELMON).nmaster = max((*SELMON).nmaster + ai as i32, 0);
//         arrange(mdpy, SELMON);
//     }
// }

// pub fn setmfact(mdpy: &Display, arg: Arg) {
//     let Arg::Float(mut f) = arg else { return };
//     unsafe {
//         if (*(*SELMON).lt[(*SELMON).sellt]).arrange.is_none() {
//             return;
//         }
//         f = if f < 1.0 {
//             f + (*SELMON).mfact
//         } else {
//             f - 1.0
//         };
//         if !(0.05..=0.95).contains(&f) {
//             return;
//         }
//         (*SELMON).mfact = f;
//         arrange(mdpy, SELMON);
//     }
// }

// pub fn killclient(mdpy: &Display, _arg: Arg) {
//     unsafe {
//         if (*SELMON).sel.is_null() {
//             return;
//         }
//         if !sendevent(mdpy, (*SELMON).sel, WMATOM[WM::Delete as usize]) {
//             XGrabServer(mdpy.inner);
//             XSetErrorHandler(Some(xerrordummy));
//             XSetCloseDownMode(mdpy.inner, DestroyAll);
//             XKillClient(mdpy.inner, (*(*SELMON).sel).win);
//             XSync(mdpy.inner, False);
//             XSetErrorHandler(Some(xerror));
//             XUngrabServer(mdpy.inner);
//         }
//     }
// }

// pub fn focusmon(mdpy: &Display, arg: Arg) {
//     unsafe {
//         let Arg::Int(ai) = arg else { return };
//         if (*MONS).next.is_null() {
//             return;
//         }
//         let m = dirtomon(ai);
//         if m == SELMON {
//             return;
//         }
//         unfocus(mdpy, (*SELMON).sel, false);
//         SELMON = m;
//         focus(mdpy, null_mut());
//     }
// }

// fn dirtomon(dir: isize) -> *mut Monitor {
//     let mut m = null_mut();
//     unsafe {
//         if dir > 0 {
//             m = (*SELMON).next;
//             if m.is_null() {
//                 m = MONS;
//             }
//         } else if SELMON == MONS {
//             m = MONS;
//             while !(*m).next.is_null() {
//                 m = (*m).next;
//             }
//         } else {
//             while (*m).next != SELMON {
//                 m = (*m).next;
//             }
//         }
//     }
//     m
// }

// pub fn tagmon(mdpy: &Display, arg: Arg) {
//     let Arg::Int(ai) = arg else { return };
//     unsafe {
//         if (*SELMON).sel.is_null() || (*MONS).next.is_null() {
//             return;
//         }
//         sendmon(mdpy, (*SELMON).sel, dirtomon(ai));
//     }
// }

// fn sendmon(mdpy: &Display, c: *mut Client, m: *mut Monitor) {
//     unsafe {
//         if (*c).mon == m {
//             return;
//         }
//         unfocus(mdpy, c, true);
//         detach(c);
//         detachstack(c);
//         (*c).mon = m;
//         (*c).tags = (*m).tagset[(*m).seltags]; // assign tags of target monitor
//         attach(c);
//         attachstack(c);
//         focus(mdpy, null_mut());
//         arrange(mdpy, null_mut());
//     }
// }

// pub fn quit(_dpy: &Display, _arg: Arg) {
//     unsafe { RUNNING = false }
// }

fn grabkeys() {
    unsafe {
        updatenumlockmask();
        let modifiers = [
            0,
            bindgen::LockMask,
            numlockmask,
            numlockmask | bindgen::LockMask,
        ];
        let (mut start, mut end, mut skip): (i32, i32, i32) = (0, 0, 0);
        bindgen::XUngrabKey(
            dpy,
            bindgen::AnyKey as i32,
            bindgen::AnyModifier,
            bindgen::root,
        );
        bindgen::XDisplayKeycodes(dpy, &mut start, &mut end);
        let syms = bindgen::XGetKeyboardMapping(
            dpy,
            start as u8,
            end - start + 1,
            &mut skip,
        );
        if syms.is_null() {
            return;
        }
        for k in start..=end {
            for i in 0..bindgen::keys.len() {
                // skip modifier codes, we do that ourselves
                if bindgen::keys[i].keysym
                    == (*syms.offset(((k - start) * skip) as isize)) as u64
                {
                    for j in 0..modifiers.len() {
                        bindgen::XGrabKey(
                            dpy,
                            k,
                            bindgen::keys[i].mod_ | modifiers[j],
                            bindgen::root,
                            bindgen::True as i32,
                            bindgen::GrabModeAsync as i32,
                            bindgen::GrabModeAsync as i32,
                        );
                    }
                }
            }
        }
        XFree(syms.cast());
    }
}

fn updatenumlockmask() {
    unsafe {
        numlockmask = 0;
        let modmap = bindgen::XGetModifierMapping(dpy);
        for i in 0..8 {
            for j in 0..(*modmap).max_keypermod {
                if *(*modmap)
                    .modifiermap
                    .offset((i * (*modmap).max_keypermod + j) as isize)
                    == bindgen::XKeysymToKeycode(
                        dpy,
                        bindgen::XK_Num_Lock as u64,
                    )
                {
                    numlockmask = 1 << i;
                }
            }
        }
        bindgen::XFreeModifiermap(modmap);
    }
}

fn seturgent(c: *mut Client, urg: bool) {
    unsafe {
        (*c).isurgent = urg as c_int;
        let wmh = bindgen::XGetWMHints(dpy, (*c).win);
        if wmh.is_null() {
            return;
        }
        (*wmh).flags = if urg {
            (*wmh).flags | bindgen::XUrgencyHint as i64
        } else {
            (*wmh).flags & !(bindgen::XUrgencyHint as i64)
        };
        bindgen::XSetWMHints(dpy, (*c).win, wmh);
        XFree(wmh.cast());
    }
}

fn unfocus(c: *mut bindgen::Client, setfocus: bool) {
    use bindgen::{
        netatom, root, scheme, ColBorder, NetActiveWindow, SchemeNorm,
    };
    if c.is_null() {
        return;
    }
    grabbuttons(c, false);
    unsafe {
        // scheme[SchemeNorm][ColBorder].pixel
        let color = (*(*scheme.offset(SchemeNorm as isize))
            .offset(ColBorder as isize))
        .pixel;
        bindgen::XSetWindowBorder(dpy, (*c).win, color);
        if setfocus {
            bindgen::XSetInputFocus(
                dpy,
                root,
                RevertToPointerRoot,
                CurrentTime as u64,
            );
            bindgen::XDeleteProperty(
                dpy,
                root,
                netatom[NetActiveWindow as usize],
            );
        }
    }
}

fn updatestatus() {
    unsafe {
        if gettextprop(
            bindgen::root,
            XA_WM_NAME,
            // cast pointer to the array itself as a pointer to the first
            // element, safe??
            addr_of_mut!(bindgen::stext) as *mut _,
            // the lint leading to this instead of simply &bindgen::stext is
            // very scary, but hopefully it's fine
            size_of_val(&*addr_of!(bindgen::stext)) as u32,
        ) == 0
        {
            libc::strcpy(
                addr_of_mut!(bindgen::stext) as *mut _,
                c"rwm-1.0".as_ptr(),
            );
        }
        drawbar(bindgen::selmon);
    }
}

fn textw(x: *const c_char) -> c_int {
    unsafe { drw::fontset_getwidth(drw, x) as c_int + bindgen::lrpad }
}

fn drawbar(m: *mut bindgen::Monitor) {
    unsafe {
        let mut tw = 0;
        let boxs = (*(*drw).fonts).h / 9;
        let boxw = (*(*drw).fonts).h / 6 + 2;
        let (mut occ, mut urg) = (0, 0);

        if (*m).showbar == 0 {
            return;
        }

        use bindgen::bh;
        use bindgen::scheme;
        use bindgen::selmon;
        use bindgen::stext;
        use bindgen::tags;
        use bindgen::{SchemeNorm, SchemeSel};

        // draw status first so it can be overdrawn by tags later
        if m == selmon {
            // status is only drawn on selected monitor
            drw::setscheme(drw, *scheme.add(SchemeNorm as usize));
            tw = textw(addr_of!(stext) as *const _) - bindgen::lrpad + 2; // 2px right padding
            drw::text(
                drw,
                (*m).ww - tw,
                0,
                tw as u32,
                bindgen::bh as u32,
                0,
                addr_of!(stext) as *const _,
                0,
            );
        }

        let mut c = (*m).clients;
        while !c.is_null() {
            occ |= (*c).tags;
            if (*c).isurgent != 0 {
                urg |= (*c).tags;
            }
            c = (*c).next;
        }

        let mut x = 0;
        for i in 0..tags.len() {
            let text = tags[i].to_owned();
            let w = textw(text);
            drw::setscheme(
                drw,
                *scheme.add(
                    if ((*m).tagset[(*m).seltags as usize] & 1 << i) != 0 {
                        SchemeSel as usize
                    } else {
                        SchemeNorm as usize
                    },
                ),
            );
            drw::text(
                drw,
                x,
                0,
                w as u32,
                bindgen::bh as u32,
                bindgen::lrpad as u32 / 2,
                text,
                (urg as i32) & 1 << i,
            );

            if (occ & 1 << i) != 0 {
                drw::rect(
                    drw,
                    x + boxs as i32,
                    boxs as i32,
                    boxw,
                    boxw,
                    (m == selmon
                        && !(*selmon).sel.is_null()
                        && ((*(*selmon).sel).tags & 1 << i) != 0)
                        as c_int,
                    (urg & 1 << i) as c_int,
                );
            }
            x += w as i32;
        }

        use bindgen::lrpad;

        let w = textw((*m).ltsymbol.as_ptr());
        drw::setscheme(drw, *scheme.add(SchemeNorm as usize));
        x = drw::text(
            drw,
            x,
            0,
            w as u32,
            bh as u32,
            lrpad as u32 / 2,
            (*m).ltsymbol.as_ptr(),
            0,
        ) as i32;

        let w = (*m).ww - tw - x;
        if w > bh {
            if !(*m).sel.is_null() {
                drw::setscheme(
                    drw,
                    *scheme.offset(if m == selmon {
                        SchemeSel as isize
                    } else {
                        SchemeNorm as isize
                    }),
                );
                drw::text(
                    drw,
                    x,
                    0,
                    w as u32,
                    bh as u32,
                    lrpad as u32 / 2,
                    (*(*m).sel).name.as_ptr(),
                    0,
                );
                if (*(*m).sel).isfloating != 0 {
                    drw::rect(
                        drw,
                        x + boxs as i32,
                        boxs as i32,
                        boxw,
                        boxw,
                        (*(*m).sel).isfixed,
                        0,
                    );
                }
            } else {
                drw::setscheme(drw, *scheme.add(SchemeNorm as usize));
                drw::rect(drw, x, 0, w as u32, bh as u32, 1, 1);
            }
        }
        drw::map(drw, (*m).barwin, 0, 0, (*m).ww as u32, bh as u32);
    }
}

fn gettextprop(w: Window, atom: Atom, text: *mut i8, size: u32) -> c_int {
    unsafe {
        if text.is_null() || size == 0 {
            return 0;
        }
        *text = '\0' as i8;
        let mut name = bindgen::XTextProperty {
            value: std::ptr::null_mut(),
            encoding: 0,
            format: 0,
            nitems: 0,
        };
        let c = bindgen::XGetTextProperty(dpy, w, &mut name, atom);
        if c == 0 || name.nitems == 0 {
            return 0;
        }

        let mut n = 0;
        let mut list: *mut *mut i8 = std::ptr::null_mut();
        if name.encoding == XA_STRING {
            libc::strncpy(text, name.value as *mut _, size as usize - 1);
        } else if bindgen::XmbTextPropertyToTextList(
            dpy,
            &name,
            &mut list,
            &mut n as *mut _,
        ) >= Success as i32
            && n > 0
            && !(*list).is_null()
        {
            libc::strncpy(text, *list, size as usize - 1);
            bindgen::XFreeStringList(list);
        }
        let p = text.offset(size as isize - 1);
        *p = '\0' as i8;
        bindgen::XFree(name.value as *mut _);
    }
    1
}

fn updatebars() {
    let mut wa = bindgen::XSetWindowAttributes {
        override_redirect: bindgen::True as i32,
        background_pixmap: bindgen::ParentRelative as u64,
        event_mask: bindgen::ButtonPressMask as i64
            | bindgen::ExposureMask as i64,
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
    let mut ch = bindgen::XClassHint {
        res_name: c"rwm".as_ptr().cast_mut(),
        res_class: c"rwm".as_ptr().cast_mut(),
    };

    unsafe {
        let mut m = bindgen::mons;
        while !m.is_null() {
            if (*m).barwin != 0 {
                continue;
            }
            (*m).barwin = bindgen::XCreateWindow(
                dpy,
                bindgen::root,
                (*m).wx as c_int,
                (*m).by as c_int,
                (*m).ww as c_uint,
                bindgen::bh as c_uint,
                0,
                bindgen::XDefaultDepth(dpy, bindgen::screen),
                bindgen::CopyFromParent as c_uint,
                bindgen::XDefaultVisual(dpy, bindgen::screen),
                (bindgen::CWOverrideRedirect
                    | bindgen::CWBackPixmap
                    | bindgen::CWEventMask) as u64,
                &mut wa,
            );
            bindgen::XDefineCursor(
                dpy,
                (*m).barwin,
                (*bindgen::cursor[bindgen::CurNormal as usize]).cursor,
            );
            bindgen::XMapRaised(dpy, (*m).barwin);
            bindgen::XSetClassHint(dpy, (*m).barwin, &mut ch);
            m = (*m).next;
        }
    }
}

// DUMMY
fn updategeom() -> i32 {
    unsafe { bindgen::updategeom() }
    //     let mut dirty = false;
    //     unsafe {
    //         if XineramaIsActive(mdpy.inner) != 0 {
    //             // I think this is the number of monitors
    //             let mut nn: i32 = 0;
    //             let info = XineramaQueryScreens(mdpy.inner, &mut nn);

    //             let mut n = 0;
    //             let mut m = MONS;
    //             while !m.is_null() {
    //                 m = (*m).next;
    //                 n += 1;
    //             }

    //             let unique: *mut XineramaScreenInfo =
    //                 calloc(nn as usize, size_of::<XineramaScreenInfo>()).cast();

    //             if unique.is_null() {
    //                 panic!("calloc failed");
    //             }

    //             let mut j = 0;
    //             for i in 0..nn {
    //                 if isuniquegeom(unique, j, info.offset(i as isize)) {
    //                     memcpy(
    //                         unique.offset(j).cast(),
    //                         info.offset(i as isize).cast(),
    //                         size_of::<XineramaScreenInfo>(),
    //                     );
    //                     j += 1;
    //                 }
    //             }
    //             XFree(info.cast());
    //             nn = j as i32;

    //             // new monitors if nn > n
    //             for _ in n..nn as usize {
    //                 let mut m = MONS;
    //                 while !m.is_null() && !(*m).next.is_null() {
    //                     m = (*m).next;
    //                 }

    //                 if !m.is_null() {
    //                     (*m).next = createmon();
    //                 } else {
    //                     MONS = createmon();
    //                 }
    //             }

    //             let mut i = 0;
    //             let mut m = MONS;
    //             while i < nn as usize && !m.is_null() {
    //                 let u = unique.add(i);
    //                 if i >= n
    //                     || (*u).x_org != (*m).mx
    //                     || (*u).y_org != (*m).my
    //                     || (*u).width != (*m).mw
    //                     || (*u).height != (*m).mh
    //                 {
    //                     dirty = true;
    //                     (*m).num = i as i32;
    //                     (*m).mx = (*u).x_org;
    //                     (*m).wx = (*u).x_org;
    //                     (*m).my = (*u).y_org;
    //                     (*m).wy = (*u).y_org;
    //                     (*m).mw = (*u).width;
    //                     (*m).ww = (*u).width;
    //                     (*m).mh = (*u).height;
    //                     (*m).wh = (*u).height;
    //                     updatebarpos(m);
    //                 }

    //                 m = (*m).next;
    //                 i += 1;
    //             }

    //             // removed monitors if n > nn
    //             for _ in nn..n as i32 {
    //                 let mut m = MONS;
    //                 while !m.is_null() && !(*m).next.is_null() {
    //                     m = (*m).next;
    //                 }

    //                 let c = (*m).clients;
    //                 while !c.is_null() {
    //                     dirty = true;
    //                     (*m).clients = (*c).next;
    //                     detachstack(c);
    //                     (*c).mon = MONS;
    //                     attach(c);
    //                     attachstack(c);
    //                 }
    //                 if m == SELMON {
    //                     SELMON = MONS;
    //                 }
    //                 cleanupmon(m, mdpy);
    //             }
    //             libc::free(unique.cast());
    //         } else {
    //             // default monitor setup
    //             if MONS.is_null() {
    //                 MONS = createmon();
    //             }

    //             if (*MONS).mw as i32 != SW || (*MONS).mh as i32 != SH {
    //                 dirty = true;
    //                 (*MONS).mw = SW as i16;
    //                 (*MONS).ww = SW as i16;
    //                 (*MONS).mh = SH as i16;
    //                 (*MONS).wh = SH as i16;
    //                 updatebarpos(MONS);
    //             }
    //         }
    //         if dirty {
    //             SELMON = MONS;
    //             SELMON = wintomon(mdpy, ROOT);
    //         }
    //     }
    //     dirty
}

// DUMMY
fn wintomon(w: Window) -> *mut bindgen::Monitor {
    unsafe { bindgen::wintomon(w) }
}

fn wintoclient(w: u64) -> *mut bindgen::Client {
    unsafe {
        let mut m = bindgen::mons;
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

// fn recttomon(x: i32, y: i32, w: i32, h: i32) -> *mut Monitor {
//     unsafe {
//         let mut r = SELMON;
//         let mut area = 0;

//         let mut m = MONS;
//         while !m.is_null() {
//             let a = intersect(x, y, w, h, m);
//             if a > area {
//                 area = a;
//                 r = m;
//             }
//             m = (*m).next;
//         }
//         r
//     }
// }

// "macros"

// fn intersect(x: i32, y: i32, w: i32, h: i32, m: *mut Monitor) -> i32 {
//     unsafe {
//         i32::max(
//             0,
//             i32::min((x) + (w), (*m).wx as i32 + (*m).ww as i32)
//                 - i32::max(x, (*m).wx as i32),
//         ) * i32::max(
//             0,
//             i32::min((y) + (h), (*m).wy as i32 + (*m).wh as i32)
//                 - i32::max(y, (*m).wy as i32),
//         )
//     }
// }

#[inline]
fn width(x: *mut bindgen::Client) -> i32 {
    unsafe { (*x).w + 2 * (*x).bw }
}

#[inline]
fn height(x: *mut bindgen::Client) -> i32 {
    unsafe { (*x).h + 2 * (*x).bw }
}

#[inline]
fn cleanmask(mask: u32) -> u32 {
    unsafe {
        mask & !(numlockmask | bindgen::LockMask)
            & (bindgen::ShiftMask
                | bindgen::ControlMask
                | bindgen::Mod1Mask
                | bindgen::Mod2Mask
                | bindgen::Mod3Mask
                | bindgen::Mod4Mask
                | bindgen::Mod5Mask)
    }
}

// fn getrootptr(mdpy: &Display, x: &mut i32, y: &mut i32) -> bool {
//     let mut di = 0;
//     let mut dui = 0;
//     let mut dummy = 0;
//     unsafe {
//         let ret = XQueryPointer(
//             mdpy.inner, ROOT, &mut dummy, &mut dummy, x, y, &mut di, &mut di,
//             &mut dui,
//         );
//         ret != 0
//     }
// }

// fn cleanupmon(mon: *mut Monitor, mdpy: &Display) {
//     unsafe {
//         if mon == MONS {
//             MONS = (*MONS).next;
//         } else {
//             let mut m = MONS;
//             while !m.is_null() && (*m).next != mon {
//                 m = (*m).next;
//             }
//         }
//         XUnmapWindow(mdpy.inner, (*mon).barwin);
//         XDestroyWindow(mdpy.inner, (*mon).barwin);
//         drop(Box::from_raw(mon)); // free mon
//     }
// }

fn attachstack(c: *mut bindgen::Client) {
    unsafe {
        (*c).snext = (*(*c).mon).stack;
        (*(*c).mon).stack = c;
    }
}

fn attach(c: *mut bindgen::Client) {
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
    unsafe {
        ((*c).tags & (*(*c).mon).tagset[(*(*c).mon).seltags as usize]) != 0
    }
}

// fn updatebarpos(m: *mut Monitor) {
//     unsafe {
//         (*m).wy = (*m).my;
//         (*m).wh = (*m).mh;
//         if (*m).showbar {
//             (*m).wh -= BH;
//             (*m).by = if (*m).topbar {
//                 (*m).wy
//             } else {
//                 (*m).wy + (*m).wh
//             };
//             (*m).wy = if (*m).topbar { (*m).wy + BH } else { (*m).wy };
//         } else {
//             (*m).by = -BH;
//         }
//     }
// }

// fn isuniquegeom(
//     unique: *mut XineramaScreenInfo,
//     mut n: isize,
//     info: *const XineramaScreenInfo,
// ) -> bool {
//     while n > 0 {
//         unsafe {
//             let u = unique.offset(n);
//             if (*u).x_org == (*info).x_org
//                 && (*u).y_org == (*info).y_org
//                 && (*u).width == (*info).width
//                 && (*u).height == (*info).height
//             {
//                 return false;
//             }
//         }
//         n -= 1;
//     }
//     true
// }

// DUMMY
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

// DUMMY
fn unmanage(c: *mut Client, destroyed: c_int) {
    unsafe { bindgen::unmanage(c, destroyed) }
}

// fn updateclientlist(mdpy: &Display) {
//     unsafe {
//         XDeleteProperty(mdpy.inner, ROOT, NETATOM[Net::ClientList as usize]);
//         let mut m = MONS;
//         while !m.is_null() {
//             let mut c = (*m).clients;
//             while !c.is_null() {
//                 xchangeproperty(
//                     mdpy,
//                     ROOT,
//                     NETATOM[Net::ClientList as usize],
//                     XA_WINDOW,
//                     32,
//                     PropModeAppend,
//                     &mut ((*c).win as u8) as *mut _,
//                     1,
//                 );
//                 c = (*c).next;
//             }
//             m = (*m).next;
//         }
//     }
// }

fn setclientstate(c: *mut bindgen::Client, state: usize) {
    let mut data: [c_long; 2] = [state as c_long, bindgen::None as c_long];
    let ptr: *mut c_uchar = data.as_mut_ptr().cast();
    unsafe {
        bindgen::XChangeProperty(
            dpy,
            (*c).win,
            bindgen::wmatom[bindgen::WMState as usize],
            bindgen::wmatom[bindgen::WMState as usize],
            32,
            bindgen::PropModeReplace as i32,
            ptr,
            2,
        );
    }
}

fn default_handler(_ev: *mut bindgen::XEvent) {}

static HANDLER: LazyLock<
    [fn(*mut bindgen::XEvent); bindgen::LASTEvent as usize],
> = LazyLock::new(|| {
    let mut handler = [default_handler as fn(*mut bindgen::XEvent);
        bindgen::LASTEvent as usize];
    handler[bindgen::ButtonPress as usize] = handlers::buttonpress;
    handler[bindgen::ClientMessage as usize] = handlers::clientmessage;
    handler[bindgen::ConfigureRequest as usize] = handlers::configurerequest;
    handler[bindgen::ConfigureNotify as usize] = handlers::configurenotify;
    handler[bindgen::DestroyNotify as usize] = handlers::destroynotify;
    handler[bindgen::EnterNotify as usize] = handlers::enternotify;
    handler[bindgen::Expose as usize] = handlers::expose;
    handler[bindgen::FocusIn as usize] = handlers::focusin;
    handler[bindgen::KeyPress as usize] = handlers::keypress;
    handler[bindgen::MappingNotify as usize] = handlers::mappingnotify;
    handler[bindgen::MapRequest as usize] = handlers::maprequest;
    handler[bindgen::MotionNotify as usize] = handlers::motionnotify;
    handler[bindgen::PropertyNotify as usize] = handlers::propertynotify;
    handler[bindgen::UnmapNotify as usize] = handlers::unmapnotify;
    handler
});

/// main event loop
fn run() {
    unsafe {
        bindgen::XSync(dpy, bindgen::False as i32);
        let mut ev: MaybeUninit<bindgen::XEvent> = MaybeUninit::uninit();
        while running != 0 && bindgen::XNextEvent(dpy, ev.as_mut_ptr()) == 0 {
            let mut ev: bindgen::XEvent = ev.assume_init();
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
                    || getstate(*wins.offset(i as isize)) == ICONIC_STATE as i64
                {
                    manage(*wins.offset(i as isize), wa.as_mut_ptr());
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

fn manage(w: Window, wa: *mut bindgen::XWindowAttributes) {
    let mut trans = 0;
    unsafe {
        let wa = *wa;
        let c: *mut bindgen::Client =
            util::ecalloc(1, size_of::<bindgen::Client>()) as *mut _;
        (*c).win = w;
        (*c).x = wa.x;
        (*c).oldx = wa.x;
        (*c).y = wa.y;
        (*c).oldy = wa.y;
        (*c).w = wa.width;
        (*c).oldw = wa.width;
        (*c).h = wa.height;
        (*c).oldh = wa.height;
        (*c).oldbw = wa.border_width;

        updatetitle(c);
        if bindgen::XGetTransientForHint(dpy, w, &mut trans) != 0 {
            let t = wintoclient(trans);
            if !t.is_null() {
                (*c).mon = (*t).mon;
                (*c).tags = (*t).tags;
            } else {
                (*c).mon = bindgen::selmon;
                applyrules(c);
            }
        } else {
            // copied else case from above because the condition is supposed
            // to be xgettransientforhint && (t = wintoclient)
            (*c).mon = bindgen::selmon;
            applyrules(c);
        }
        if (*c).x + width(c) > ((*(*c).mon).wx + (*(*c).mon).ww) as i32 {
            (*c).x = ((*(*c).mon).wx + (*(*c).mon).ww) as i32 - width(c);
        }
        if (*c).y + height(c) > ((*(*c).mon).wy + (*(*c).mon).wh) as i32 {
            (*c).y = ((*(*c).mon).wy + (*(*c).mon).wh) as i32 - height(c);
        }
        (*c).x = max((*c).x, (*(*c).mon).wx as i32);
        (*c).y = max((*c).y, (*(*c).mon).wy as i32);
        (*c).bw = bindgen::borderpx as i32;
        let mut wc = bindgen::XWindowChanges {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            border_width: (*c).bw,
            sibling: 0,
            stack_mode: 0,
        };
        bindgen::XConfigureWindow(dpy, w, CWBorderWidth as u32, &mut wc);
        bindgen::XSetWindowBorder(
            dpy,
            w,
            (*(*bindgen::scheme
                .offset(bindgen::SchemeNorm as isize)
                .offset(bindgen::ColBorder as isize)))
            .pixel,
        );
        configure(c); // propagates border width, if size doesn't change
        updatewindowtype(c);
        updatesizehints(c);
        updatewmhints(c);
        bindgen::XSelectInput(
            dpy,
            w,
            EnterWindowMask
                | FocusChangeMask
                | PropertyChangeMask
                | StructureNotifyMask,
        );
        grabbuttons(c, false);
        if (*c).isfloating == 0 {
            (*c).oldstate = (trans != 0 || (*c).isfixed != 0) as c_int;
            (*c).isfloating = (*c).oldstate;
        }
        if (*c).isfloating != 0 {
            bindgen::XRaiseWindow(dpy, (*c).win);
        }
        attach(c);
        attachstack(c);
        bindgen::XChangeProperty(
            dpy,
            bindgen::root,
            bindgen::netatom[bindgen::NetClientList as usize],
            XA_WINDOW,
            32,
            PropModeAppend,
            &((*c).win as c_uchar),
            1,
        );
        // some windows require this
        bindgen::XMoveResizeWindow(
            dpy,
            (*c).win,
            (*c).x + 2 * bindgen::sw,
            (*c).y,
            (*c).w as u32,
            (*c).h as u32,
        );
        setclientstate(c, NORMAL_STATE);
        if (*c).mon == bindgen::selmon {
            unfocus((*bindgen::selmon).sel, false);
        }
        (*(*c).mon).sel = c;
        arrange((*c).mon);
        bindgen::XMapWindow(dpy, (*c).win);
        focus(std::ptr::null_mut());
    }
}

fn updatewmhints(c: *mut bindgen::Client) {
    const URGENT: i64 = bindgen::XUrgencyHint as i64;
    unsafe {
        let wmh = bindgen::XGetWMHints(dpy, (*c).win);
        if !wmh.is_null() {
            if c == (*bindgen::selmon).sel && (*wmh).flags & URGENT != 0 {
                (*wmh).flags &= !URGENT;
                bindgen::XSetWMHints(dpy, (*c).win, wmh);
            } else {
                (*c).isurgent = ((*wmh).flags & URGENT != 0) as bool as c_int;
            }
            if (*wmh).flags & bindgen::InputHint as i64 != 0 {
                (*c).neverfocus = ((*wmh).input == 0) as c_int;
            } else {
                (*c).neverfocus = 0;
            }
            bindgen::XFree(wmh.cast());
        }
    }
}

fn updatewindowtype(c: *mut bindgen::Client) {
    use bindgen::{
        netatom, NetWMFullscreen, NetWMState, NetWMWindowType,
        NetWMWindowTypeDialog,
    };
    unsafe {
        let state = getatomprop(c, netatom[NetWMState as usize]);
        let wtype = getatomprop(c, netatom[NetWMWindowType as usize]);
        if state == netatom[NetWMFullscreen as usize] {
            setfullscreen(c, true);
        }
        if wtype == netatom[NetWMWindowTypeDialog as usize] {
            (*c).isfloating = 1;
        }
    }
}

fn setfullscreen(c: *mut Client, fullscreen: bool) {
    use bindgen::{netatom, NetWMFullscreen, NetWMState};
    unsafe {
        if fullscreen && (*c).isfullscreen == 0 {
            bindgen::XChangeProperty(
                dpy,
                (*c).win,
                netatom[NetWMState as usize],
                XA_ATOM,
                32,
                bindgen::PropModeReplace as i32,
                // trying to emulate (unsigned char*)&netatom[NetWMFullscreen],
                // so take a reference and then cast
                &mut netatom[NetWMFullscreen as usize] as *mut u64
                    as *mut c_uchar,
                1,
            );
            (*c).isfullscreen = 1;
            (*c).oldstate = (*c).isfloating;
            (*c).oldbw = (*c).bw;
            (*c).bw = 0;
            (*c).isfloating = 1;
            resizeclient(
                c,
                (*(*c).mon).mx,
                (*(*c).mon).my,
                (*(*c).mon).mw,
                (*(*c).mon).mh,
            );
            bindgen::XRaiseWindow(dpy, (*c).win);
        } else if !fullscreen && (*c).isfullscreen != 0 {
            bindgen::XChangeProperty(
                dpy,
                (*c).win,
                netatom[NetWMState as usize],
                XA_ATOM,
                32,
                bindgen::PropModeReplace as i32,
                std::ptr::null_mut::<c_uchar>(),
                0,
            );
            (*c).isfullscreen = 0;
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
        if bindgen::XGetWindowProperty(
            dpy,
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
            // the C code is *(Atom *)p. is that different from (Atom) *p?
            // that's closer to what I had before
            atom = *(p as *mut Atom);
            XFree(p.cast());
        }
    }
    atom
}

fn applyrules(c: *mut bindgen::Client) {
    unsafe {
        let mut ch = bindgen::XClassHint {
            res_name: std::ptr::null_mut(),
            res_class: std::ptr::null_mut(),
        };
        // rule matching
        (*c).isfloating = 0;
        (*c).tags = 0;
        bindgen::XGetClassHint(dpy, (*c).win, &mut ch);
        let class = if !ch.res_class.is_null() {
            CString::from_raw(ch.res_class)
        } else {
            CString::from_raw(bindgen::broken.as_ptr() as *mut _)
        };
        let instance = if !ch.res_name.is_null() {
            CString::from_raw(ch.res_name)
        } else {
            CString::from_raw(bindgen::broken.as_ptr() as *mut _)
        };

        for i in 0..bindgen::rules.len() {
            let r = &bindgen::rules[i];
            if (r.title.is_null()
                || !libc::strstr((*c).name.as_ptr(), r.title).is_null())
                && (r.class.is_null()
                    || !libc::strstr(class.as_ptr(), r.class).is_null())
                && (r.instance.is_null()
                    || !libc::strstr(instance.as_ptr(), r.instance).is_null())
            {
                (*c).isfloating = r.isfloating;
                (*c).tags |= r.tags;
                let mut m = bindgen::mons;
                while !m.is_null() && (*m).num != r.monitor {
                    m = (*m).next;
                }
                if !m.is_null() {
                    (*c).mon = m;
                }
            }
        }
        if !ch.res_class.is_null() {
            bindgen::XFree(ch.res_class.cast());
        }
        if !ch.res_name.is_null() {
            bindgen::XFree(ch.res_name.cast());
        }
        (*c).tags = if (*c).tags & TAGMASK != 0 {
            (*c).tags & TAGMASK
        } else {
            (*(*c).mon).tagset[(*(*c).mon).seltags as usize]
        };
    }
}

// #define TAGMASK                 ((1 << LENGTH(tags)) - 1)
const TAGMASK: u32 = (1 << 9) - 1;

fn updatetitle(c: *mut bindgen::Client) {
    unsafe {
        if gettextprop(
            (*c).win,
            bindgen::netatom[bindgen::NetWMName as usize],
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
                bindgen::broken.as_ptr() as *const c_char,
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
        let cond = bindgen::XGetWindowProperty(
            dpy,
            w,
            bindgen::wmatom[bindgen::WMState as usize],
            0,
            2,
            False,
            bindgen::wmatom[bindgen::WMState as usize],
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

// mod config;
// mod layouts;
mod drw;
mod handlers;
mod util {
    use libc::{c_void, size_t};

    pub(crate) fn ecalloc(nmemb: size_t, size: size_t) -> *mut c_void {
        let ret = unsafe { libc::calloc(nmemb, size) };
        if ret.is_null() {
            crate::die("calloc:");
        }
        ret
    }
}

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
    scan(); // DONE except trivial impls from manage
    run();
    cleanup();
    unsafe {
        bindgen::XCloseDisplay(dpy);
    }
}
