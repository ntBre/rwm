use std::{
    ffi::c_uint,
    ptr::{addr_of, null_mut},
};

use crate::{
    arrange,
    bindgen::{self, dpy, selmon, tags, Arg, XEvent},
    cleanmask, configure, drw, focus, height, is_visible, resizeclient,
    restack, setfullscreen, seturgent, textw, unmanage, updatebars, updategeom,
    width, wintoclient, wintomon,
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
        let m = wintomon(ev.window);
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
                if ev.x < x {
                    break;
                }
                i += 1;
                if i >= tags.len() {
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

pub(crate) fn clientmessage(e: *mut XEvent) {
    unsafe {
        let cme = &(*e).xclient;
        let c = wintoclient(cme.window);

        if c.is_null() {
            return;
        }
        use bindgen::{netatom, NetActiveWindow, NetWMFullscreen, NetWMState};
        if cme.message_type == netatom[NetWMState as usize] {
            if cme.data.l[1] == netatom[NetWMFullscreen as usize] as i64
                || cme.data.l[2] == netatom[NetWMFullscreen as usize] as i64
            {
                setfullscreen(
                    c,
                    cme.data.l[0] == 1 // _NET_WM_STATE_ADD
                        || (cme.data.l[0] == 2 // _NET_WM_STATE_TOGGLE
                            && (*c).isfullscreen == 0),
                );
            }
        } else if cme.message_type == netatom[NetActiveWindow as usize]
            && c != (*selmon).sel
            && (*c).isurgent == 0
        {
            seturgent(c, true);
        }
    }
}

pub(crate) fn configurerequest(e: *mut XEvent) {
    use bindgen::{CWBorderWidth, CWHeight, CWWidth, CWX, CWY};
    unsafe {
        let ev = &(*e).xconfigurerequest;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if (ev.value_mask & CWBorderWidth as u64) != 0 {
                (*c).bw = ev.border_width;
            } else if (*c).isfloating != 0
                || (*(*selmon).lt[(*selmon).sellt as usize]).arrange.is_none()
            {
                let m = (*c).mon;
                if ev.value_mask & CWX as u64 != 0 {
                    (*c).oldx = (*c).x;
                    (*c).x = (*m).mx + ev.x;
                }
                if ev.value_mask & CWY as u64 != 0 {
                    (*c).oldy = (*c).y;
                    (*c).y = (*m).my + ev.y;
                }
                if ev.value_mask & CWWidth as u64 != 0 {
                    (*c).oldw = (*c).w;
                    (*c).w = ev.width;
                }
                if ev.value_mask & CWHeight as u64 != 0 {
                    (*c).oldh = (*c).h;
                    (*c).h = ev.height;
                }
                assert!(!c.is_null());
                assert!(!m.is_null());
                let c = &mut *c;
                let m = &mut *m;
                if (c.x + c.w) > m.mx + m.mw && c.isfloating != 0 {
                    c.x = m.mx + (m.mw / 2 - width(c) / 2); // center x
                }
                if (c.y + c.h) > m.my + m.mh && c.isfloating != 0 {
                    c.y = m.my + (m.mh / 2 - height(c) / 2); // center y
                }
                if (ev.value_mask & (CWX | CWY) as u64) != 0
                    && (ev.value_mask & (CWWidth | CWHeight) as u64) == 0
                {
                    configure(c);
                }
                if is_visible(c) {
                    bindgen::XMoveResizeWindow(
                        dpy, c.win, c.x, c.y, c.w as u32, c.h as u32,
                    );
                }
            } else {
                configure(c);
            }
        } else {
            let x = ev.x;
            let y = ev.y;
            let width = ev.width;
            let height = ev.height;
            let border_width = ev.border_width;
            let sibling = ev.above;
            let stack_mode = ev.detail;
            let mut wc = bindgen::XWindowChanges {
                x,
                y,
                width,
                height,
                border_width,
                sibling,
                stack_mode,
            };
            bindgen::XConfigureWindow(
                dpy,
                ev.window,
                ev.value_mask as u32,
                &mut wc,
            );
        }
        bindgen::XSync(dpy, bindgen::False as i32);
    }
}

pub(crate) fn configurenotify(e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).xconfigure;
        /* TODO: updategeom handling sucks, needs to be simplified */
        if ev.window == bindgen::root {
            let dirty = bindgen::sw != ev.width || bindgen::sh != ev.height;
            bindgen::sw = ev.width;
            bindgen::sh = ev.height;
            if updategeom() != 0 || dirty {
                drw::resize(drw, bindgen::sw as c_uint, bindgen::bh as c_uint);
                updatebars();
                let mut m = bindgen::mons;
                while !m.is_null() {
                    let mut c = (*m).clients;
                    while !c.is_null() {
                        if (*c).isfullscreen != 0 {
                            resizeclient(c, (*m).mx, (*m).my, (*m).mw, (*m).mh);
                        }
                        c = (*c).next;
                    }
                    bindgen::XMoveResizeWindow(
                        dpy,
                        (*m).barwin,
                        (*m).wx,
                        (*m).by,
                        (*m).ww as u32,
                        bindgen::bh as u32,
                    );
                    m = (*m).next;
                }
                focus(null_mut());
                arrange(null_mut());
            }
        }
    }
}

pub(crate) fn destroynotify(e: *mut XEvent) {
    unsafe {
        let ev = &(*e).xdestroywindow;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            unmanage(c, 1);
        }
    }
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
