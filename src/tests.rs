use rwm::events::Event;
use xlib::Button1;

use super::*;

use std::process::Command;

#[test]
fn main() {
    // setup xvfb
    let mut cmd = Command::new("Xvfb").arg(":1").spawn().unwrap();

    // wait for xephyr to start
    let mut dpy = unsafe { xlib::XOpenDisplay(c":1.0".as_ptr()) };
    while dpy.is_null() {
        dpy = unsafe { xlib::XOpenDisplay(c":1.0".as_ptr()) };
    }

    // goto for killing xephyr no matter what
    let ok = 'defer: {
        checkotherwm(dpy);
        let mut state = setup(dpy);

        #[cfg(target_os = "linux")]
        {
            let xcon = match Connection::connect(Some(":1.0")) {
                Ok((xcon, _)) => xcon,
                Err(e) => {
                    eprintln!("rwm: cannot get xcb connection: {e:?}");
                    break 'defer false;
                }
            };
            state.xcon = Box::into_raw(Box::new(xcon));
        }

        scan(&mut state);

        // instead of calling `run`, manually send some XEvents

        // test that a mouse click on the initial (tiling) layout icon
        // switches to floating mode
        let mut button = Event::button(
            unsafe { (*state.selmon).barwin },
            Button1,
            CONFIG
                .tags
                .iter()
                .map(|tag| textw(&mut state.drw, tag, state.lrpad))
                .sum::<i32>()
                + 5,
            state.dpy,
            0,
        )
        .into_button();
        handlers::buttonpress(&mut state, &mut button);
        unsafe {
            assert!((*(*state.selmon).lt[(*state.selmon).sellt as usize])
                .arrange
                .0
                .is_none());
        }

        cleanup(state);

        break 'defer true;
    };

    // kill xephyr when finished
    cmd.kill().unwrap();
    cmd.try_wait().unwrap();

    assert!(ok);
}
