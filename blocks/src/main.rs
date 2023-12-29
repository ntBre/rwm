use std::{
    ffi::c_int,
    process::Command,
    ptr::{null, null_mut},
    thread::sleep,
    time::Duration,
};

use config::BLOCKS;
use libc::{c_uint, c_void, sighandler_t, signal, SIGINT, SIGRTMIN, SIGTERM};
use x11::xlib::{
    BadAlloc, BadWindow, Display, Window, XCloseDisplay, XDefaultScreen,
    XOpenDisplay, XRootWindow, XStoreName,
};

use crate::config::DELIM;

mod config;

static mut STATUS_CONTINUE: bool = true;
static mut STATUSBAR: [String; BLOCKS.len()] =
    [String::new(), String::new(), String::new()];
static mut STATUSSTR: [String; 2] = [String::new(), String::new()];
static mut DPY: *mut Display = null_mut();
static mut SCREEN: c_int = 0;
static mut ROOT: Window = 0;

/// adapted from
/// https://users.rust-lang.org/t/how-to-use-libcs-signal-function/3067
fn get_handler(handler: extern "C" fn(c_int)) -> sighandler_t {
    handler as *mut c_void as sighandler_t
}

extern "C" fn termhandler(_: c_int) {
    unsafe {
        STATUS_CONTINUE = false;
    }
    std::process::exit(0);
}

extern "C" fn sighandler(signum: c_int) {
    getsigcmds(signum - SIGRTMIN());
    writestatus();
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
        write!(output, "{}", String::from_utf8(out.stdout).unwrap()).unwrap();
        if !DELIM.is_empty() {
            write!(output, "{}", DELIM).unwrap();
        }
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

fn getcmds(time: c_int) {
    for (i, current) in BLOCKS.iter().enumerate() {
        if current.interval != 0 && time % current.interval as i32 == 0
            || time == -1
        {
            unsafe {
                STATUSBAR[i] = current.getcmd();
            }
        }
    }
}

fn getsigcmds(signal: c_int) {
    for (i, current) in BLOCKS.iter().enumerate() {
        if current.signal == signal {
            unsafe {
                STATUSBAR[i] = current.getcmd();
            }
        }
    }
}

fn getstatus(s: &mut String, last: &mut String) -> bool {
    *last = std::mem::take(s);
    unsafe {
        *s = STATUSBAR.join("").replace('\n', "");
    }
    s != last
}

/// NOTE: inlined from setroot, can also be pstdout with -p flag, presumably for
/// debugging
fn writestatus() {
    unsafe {
        // only set root if text has changed
        if !getstatus(&mut STATUSSTR[0], &mut STATUSSTR[1]) {
            return;
        }
        let d = XOpenDisplay(null());
        if !d.is_null() {
            DPY = d;
        }
        SCREEN = XDefaultScreen(DPY);
        ROOT = XRootWindow(DPY, SCREEN);
        let s = &STATUSSTR[0];
        eprintln!("updating status to `{s}`");
        let ret = XStoreName(DPY, ROOT, s.as_ptr().cast());
        #[allow(non_upper_case_globals)]
        if matches!(ret as u8, BadAlloc | BadWindow) {
            eprintln!("storing name failed");
        }
        XCloseDisplay(DPY);
    }
}

fn statusloop() {
    setupsignals();
    let mut i = 0;
    getcmds(-1);
    while unsafe { STATUS_CONTINUE } {
        getcmds(i);
        writestatus();
        sleep(Duration::from_secs(1));
        i += 1;
    }
}

fn main() {
    // TODO argument parsing
    unsafe {
        signal(SIGTERM, get_handler(termhandler));
        signal(SIGINT, get_handler(termhandler));
    }
    statusloop();
}
