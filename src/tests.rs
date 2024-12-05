use rwm::events::Event;
use xlib::Button1;

use super::*;

use std::process::Command;

#[test]
fn main() {
    // setup xephyr
    #[cfg(target_os = "linux")]
    let mut cmd = Command::new("Xvfb").arg(":1").spawn().unwrap();

    #[cfg(not(target_os = "linux"))]
    let mut cmd = Command::new("xvfb").arg(":1").spawn().unwrap();

    // wait for xephyr to start
    let mut dpy = unsafe { xlib::XOpenDisplay(c":1.0".as_ptr()) };
    while dpy.is_null() {
        dpy = unsafe { xlib::XOpenDisplay(c":1.0".as_ptr()) };
    }

    // goto for killing xephyr no matter what
    let ok = 'defer: {
        #[cfg(target_os = "linux")]
        unsafe {
            let xcon = match Connection::connect(Some(":1.0")) {
                Ok((xcon, _)) => xcon,
                Err(e) => {
                    eprintln!("rwm: cannot get xcb connection: {e:?}");
                    break 'defer false;
                }
            };
            XCON = Box::into_raw(Box::new(xcon));
        }
        checkotherwm(dpy);
        let state = setup(dpy);
        scan(&state);

        // instead of calling `run`, manually send some XEvents

        // test that a mouse click on the initial (tiling) layout icon
        // switches to floating mode
        handlers::buttonpress(
            &state,
            &mut Event::button(
                (unsafe { *SELMON }).barwin,
                Button1,
                CONFIG
                    .tags
                    .iter()
                    .map(|tag| textw(tag.as_ptr()))
                    .sum::<i32>()
                    + 5,
                state.dpy,
                0,
            )
            .into_button(),
        );
        unsafe {
            assert!((*(*SELMON).lt[(*SELMON).sellt as usize])
                .arrange
                .is_none());
        }

        cleanup(&state);

        unsafe {
            let State { dpy, cursors, .. } = state;

            // this needs to be dropped before DRW
            drop(cursors);

            drw::free(DRW);

            xlib::XCloseDisplay(dpy);

            #[cfg(target_os = "linux")]
            drop(Box::from_raw(XCON));
        }

        break 'defer true;
    };

    // kill xephyr when finished
    cmd.kill().unwrap();
    cmd.try_wait().unwrap();

    assert!(ok);
}
