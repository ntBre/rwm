use std::ptr::{addr_of, null_mut};

use crate::{
    bindgen::{self, selmon, tags, Arg, XEvent},
    cleanmask, restack, textw, wintoclient,
};

pub(crate) fn buttonpress(e: *mut XEvent) {
    use bindgen::{
        ClkClientWin, ClkLtSymbol, ClkRootWin, ClkStatusText, ClkTagBar,
        ClkWinTitle,
    };
    unsafe {
        let mut arg = Arg { i: 0 };
        let ev = &(*e).xbutton;
        let mut click = ClkRootWin;
        // focus monitor if necessary
        let m = bindgen::wintomon(ev.window);
        if !m.is_null() && m != selmon {
            crate::unfocus((*selmon).sel, true);
            selmon = m;
            crate::focus(null_mut());
        }
        if ev.window == (*selmon).barwin {
            let mut i = 0;
            let mut x = 0;
            // emulating do-while
            loop {
                x += textw(tags[i]);
                // condition
                i += 1;
                if !(ev.x >= x && i < tags.len()) {
                    break;
                }
            }
            if i < tags.len() {
                click = ClkTagBar;
                arg = Arg { ui: 1 << i };
            } else if ev.x < x + textw(addr_of!((*selmon).ltsymbol) as *const _)
            {
                click = ClkLtSymbol;
            } else if ev.x
                > (*selmon).ww - textw(addr_of!(bindgen::stext) as *const _)
            {
                click = ClkStatusText;
            } else {
                click = ClkWinTitle;
            }
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                crate::focus(c);
                restack(selmon);
                bindgen::XAllowEvents(
                    bindgen::dpy,
                    bindgen::ReplayPointer as i32,
                    bindgen::CurrentTime as u64,
                );
                click = ClkClientWin;
            }
        }
        use bindgen::buttons;
        for i in 0..buttons.len() {
            if click == buttons[i].click
                && buttons[i].func.is_some()
                && buttons[i].button == ev.button
                && cleanmask(buttons[i].mask) == cleanmask(ev.state)
            {
                let f = buttons[i].func.unwrap();
                let a = if click == ClkTagBar && buttons[i].arg.i == 0 {
                    &arg
                } else {
                    &buttons[i].arg
                };
                f(a)
            }
        }
    }
}

// DUMMY
pub(crate) fn clientmessage(e: *mut XEvent) {
    unsafe { bindgen::clientmessage(e) }
}

// DUMMY
pub(crate) fn configurerequest(e: *mut XEvent) {
    unsafe { bindgen::configurerequest(e) }
}

// DUMMY
pub(crate) fn configurenotify(e: *mut XEvent) {
    unsafe { bindgen::configurenotify(e) }
}

// DUMMY
pub(crate) fn destroynotify(e: *mut XEvent) {
    unsafe { bindgen::destroynotify(e) }
}

// DUMMY
pub(crate) fn enternotify(e: *mut XEvent) {
    unsafe { bindgen::enternotify(e) }
}

// DUMMY
pub(crate) fn expose(e: *mut XEvent) {
    unsafe { bindgen::expose(e) }
}

// DUMMY
pub(crate) fn focusin(e: *mut XEvent) {
    unsafe { bindgen::focusin(e) }
}

// DUMMY
pub(crate) fn keypress(e: *mut XEvent) {
    unsafe { bindgen::keypress(e) }
}

// DUMMY
pub(crate) fn mappingnotify(e: *mut XEvent) {
    unsafe { bindgen::mappingnotify(e) }
}

// DUMMY
pub(crate) fn maprequest(e: *mut XEvent) {
    unsafe { bindgen::maprequest(e) }
}

// DUMMY
pub(crate) fn motionnotify(e: *mut XEvent) {
    unsafe { bindgen::motionnotify(e) }
}

// DUMMY
pub(crate) fn propertynotify(e: *mut XEvent) {
    unsafe { bindgen::propertynotify(e) }
}

// DUMMY
pub(crate) fn unmapnotify(e: *mut XEvent) {
    unsafe { bindgen::unmapnotify(e) }
}
