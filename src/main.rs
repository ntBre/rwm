//! tiling window manager based on dwm

use rwm::{cleanup, run, scan, setup};
#[cfg(target_os = "linux")]
use xcb::Connection;

use rwm::checkotherwm;
use rwm::util::die;

pub use rwm::enums;

#[cfg(test)]
mod tests;

fn main() {
    env_logger::init();
    let dpy = unsafe { x11::xlib::XOpenDisplay(std::ptr::null_mut()) };
    if dpy.is_null() {
        die("rwm: cannot open display");
    }

    checkotherwm(dpy);
    let mut state = setup(dpy);

    #[cfg(target_os = "linux")]
    {
        let Ok((xcon, _)) = Connection::connect(None) else {
            die("rwm: cannot get xcb connection");
        };
        state.xcon = Box::into_raw(Box::new(xcon));
    }

    scan(&mut state);
    run(&mut state);
    cleanup(state);
}
