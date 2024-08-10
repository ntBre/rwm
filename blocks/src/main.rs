use std::{
    ffi::{c_int, CString},
    process::Command,
    ptr::null,
    thread::sleep,
    time::Duration,
};

use config::BLOCKS;
use libc::{c_uint, c_void, sighandler_t, signal, SIGINT, SIGRTMIN, SIGTERM};
use x11::xlib::{
    XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow, XStoreName,
};

use crate::config::DELIM;

mod config;

struct Globals<const N: usize> {
    statusbar: [String; N],
    statusstr: [String; 2],
}

impl<const N: usize> Globals<N> {
    const fn new() -> Self {
        const S: String = String::new();
        Self { statusbar: [S; N], statusstr: [S; 2] }
    }

    fn getcmds(&mut self, time: c_int) {
        for (i, current) in BLOCKS.iter().enumerate() {
            if current.interval != 0 && time % current.interval as i32 == 0
                || time == -1
            {
                self.statusbar[i] = current.getcmd();
            }
        }
    }

    fn getsigcmds(&mut self, signal: c_int) {
        for (i, current) in BLOCKS.iter().enumerate() {
            if current.signal == signal {
                self.statusbar[i] = current.getcmd();
            }
        }
    }

    fn getstatus(&mut self) -> bool {
        self.statusstr[1] = std::mem::take(&mut self.statusstr[0]);
        self.statusstr[0] = self.statusbar.join("").replace('\n', "");
        self.statusstr[0] != self.statusstr[1]
    }

    /// NOTE: inlined from setroot, can also be pstdout with -p flag, presumably
    /// for debugging
    fn writestatus(&mut self) {
        // only set root if text has changed
        if !self.getstatus() {
            return;
        }
        unsafe {
            let dpy = XOpenDisplay(null());
            let screen = XDefaultScreen(dpy);
            let root = XRootWindow(dpy, screen);
            let s = CString::new(self.statusstr[0].clone()).unwrap();
            XStoreName(dpy, root, s.as_ptr());
            XCloseDisplay(dpy);
        }
    }

    fn statusloop(&mut self) {
        setupsignals();
        self.getcmds(-1);
        for i in 0.. {
            self.getcmds(i);
            self.writestatus();
            sleep(Duration::from_secs(1));
        }
    }
}

/// adapted from
/// https://users.rust-lang.org/t/how-to-use-libcs-signal-function/3067
fn get_handler(handler: extern "C" fn(c_int)) -> sighandler_t {
    handler as *mut c_void as sighandler_t
}

extern "C" fn termhandler(_: c_int) {
    std::process::exit(0);
}

extern "C" fn sighandler(signum: c_int) {
    unsafe {
        GLOB.getsigcmds(signum - SIGRTMIN());
        GLOB.writestatus();
    }
}

struct Block {
    icon: &'static str,
    command: &'static str,
    interval: c_uint,
    signal: c_int,
}

impl Block {
    fn getcmd(&self) -> String {
        let mut output = String::new();
        use std::fmt::Write;
        write!(output, "{}", self.icon).unwrap();
        let out = match Command::new(self.command).output() {
            Ok(output) => output,
            Err(e) => {
                eprintln!("command `{}` failed with `{e}`", self.command);
                return String::new();
            }
        };
        write!(output, "{}{DELIM}", String::from_utf8(out.stdout).unwrap())
            .unwrap();
        output
    }
}

fn setupsignals() {
    for block in &BLOCKS {
        if block.signal > 0 {
            unsafe {
                signal(SIGRTMIN() + block.signal, get_handler(sighandler));
            }
        }
    }
}

const N: usize = BLOCKS.len();
static mut GLOB: Globals<N> = Globals::new();

fn main() {
    // TODO argument parsing
    unsafe {
        signal(SIGTERM, get_handler(termhandler));
        signal(SIGINT, get_handler(termhandler));
        GLOB.statusloop();
    }
}
