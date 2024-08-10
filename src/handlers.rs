use std::{
    ffi::{c_int, c_uint},
    ptr::{addr_of, addr_of_mut, null_mut},
};

use x11::xlib::{
    CurrentTime, PropertyDelete, XA_WM_HINTS, XA_WM_NAME, XA_WM_NORMAL_HINTS,
    XA_WM_TRANSIENT_FOR,
};

use crate::{
    arrange,
    bindgen::{self, dpy, selmon, tags, Arg, XEvent},
    cleanmask, configure, drawbar, drawbars, drw,
    enums::Net,
    focus, grabkeys, height, is_visible, manage, recttomon, resizeclient,
    restack, setclientstate, setfocus, setfullscreen, seturgent, textw,
    unfocus, unmanage, updatebars, updategeom, updatestatus, updatetitle,
    updatewindowtype, updatewmhints, width, wintoclient, wintomon,
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
                    CurrentTime,
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

pub(crate) fn enternotify(e: *mut XEvent) {
    log::trace!("enternotify");
    unsafe {
        let ev = &mut (*e).xcrossing;
        if (ev.mode != bindgen::NotifyNormal as i32
            || ev.detail == bindgen::NotifyInferior as i32)
            && ev.window != bindgen::root
        {
            return;
        }
        let c = wintoclient(ev.window);
        let m = if !c.is_null() { (*c).mon } else { wintomon(ev.window) };
        if m != selmon {
            unfocus((*selmon).sel, true);
            selmon = m;
        } else if c.is_null() || c == (*selmon).sel {
            return;
        }
        focus(c)
    }
}

pub(crate) fn expose(e: *mut XEvent) {
    unsafe {
        let ev = &(*e).xexpose;
        if ev.count == 0 {
            let m = wintomon(ev.window);
            if !m.is_null() {
                drawbar(m);
            }
        }
    }
}

/* there are some broken focus acquiring clients needing extra handling */
pub(crate) fn focusin(e: *mut XEvent) {
    unsafe {
        let ev = &(*e).xfocus;
        if !(*selmon).sel.is_null() && ev.window != (*(*selmon).sel).win {
            setfocus((*selmon).sel);
        }
    }
}

pub(crate) fn keypress(e: *mut XEvent) {
    use bindgen::keys;

    unsafe {
        let ev = &mut (*e).xkey;
        let keysym =
            bindgen::XKeycodeToKeysym(dpy, ev.keycode as bindgen::KeyCode, 0);
        for i in 0..keys.len() {
            if keysym == keys[i].keysym
                && cleanmask(keys[i].mod_) == cleanmask(ev.state)
                && keys[i].func.is_some()
            {
                keys[i].func.unwrap()(&(keys[i].arg));
            }
        }
    }
}

pub(crate) fn mappingnotify(e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).xmapping;
        bindgen::XRefreshKeyboardMapping(ev);
        if ev.request == bindgen::MappingKeyboard as i32 {
            grabkeys();
        }
    }
}

pub(crate) fn maprequest(e: *mut XEvent) {
    use bindgen::XWindowAttributes;

    static mut WA: XWindowAttributes = XWindowAttributes {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        border_width: 0,
        depth: 0,
        visual: null_mut(),
        root: 0,
        class: 0,
        bit_gravity: 0,
        win_gravity: 0,
        backing_store: 0,
        backing_planes: 0,
        backing_pixel: 0,
        save_under: 0,
        colormap: 0,
        map_installed: 0,
        map_state: 0,
        all_event_masks: 0,
        your_event_mask: 0,
        do_not_propagate_mask: 0,
        override_redirect: 0,
        screen: null_mut(),
    };

    // I don't really see a pratical reason why WA is static except to prevent
    // re-allocating it on the stack on each call since it is pretty big. The
    // first call here is always rebuilding it, so I don't think we're using it
    // in its previous state.
    unsafe {
        let ev = &(*e).xmaprequest;
        log::trace!("maprequest: XGetWindowAttributes");
        let res =
            bindgen::XGetWindowAttributes(dpy, ev.window, addr_of_mut!(WA));
        // XGetWindowAttributes returns a zero if the function fails
        if res == 0 || WA.override_redirect != 0 {
            return;
        }
        if wintoclient(ev.window).is_null() {
            manage(ev.window, addr_of_mut!(WA));
        }
    }
}

pub(crate) fn motionnotify(e: *mut XEvent) {
    log::trace!("motionnotify");
    static mut MON: *mut bindgen::Monitor = null_mut();
    unsafe {
        let ev = &(*e).xmotion;
        if ev.window != bindgen::root {
            return;
        }
        let m = recttomon(ev.x_root, ev.y_root, 1, 1);
        if m != MON && !MON.is_null() {
            unfocus((*selmon).sel, true);
            selmon = m;
            focus(null_mut());
        }
        MON = m;
    }
}

pub(crate) fn propertynotify(e: *mut XEvent) {
    log::trace!("propertynotify");
    unsafe {
        let mut trans: bindgen::Window = 0;
        let ev = &mut (*e).xproperty;
        if ev.window == bindgen::root && ev.atom == XA_WM_NAME {
            updatestatus();
        } else if ev.state == PropertyDelete {
            return; // ignore
        } else {
            let c = wintoclient(ev.window);
            if c.is_null() {
                return;
            }
            let c = &mut *c;
            match ev.atom {
                XA_WM_TRANSIENT_FOR => {
                    if c.isfloating == 0
                        && (bindgen::XGetTransientForHint(
                            dpy, c.win, &mut trans,
                        ) != 0)
                    {
                        c.isfloating = !wintoclient(trans).is_null() as c_int;
                        if c.isfloating != 0 {
                            arrange(c.mon);
                        }
                    }
                }
                XA_WM_NORMAL_HINTS => {
                    c.hintsvalid = 0;
                }
                XA_WM_HINTS => {
                    updatewmhints(c);
                    drawbars();
                }
                _ => {}
            }
            if ev.atom == XA_WM_NAME
                || ev.atom == bindgen::netatom[Net::WMName as usize]
            {
                updatetitle(c);
                if c as *mut _ == (*c.mon).sel {
                    drawbar(c.mon);
                }
            }
            if ev.atom == bindgen::netatom[Net::WMWindowType as usize] {
                updatewindowtype(c);
            }
        }
    }
}

pub(crate) fn unmapnotify(e: *mut XEvent) {
    log::trace!("unmapnotify");
    unsafe {
        let ev = &(*e).xunmap;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if ev.send_event != 0 {
                setclientstate(c, bindgen::WithdrawnState as usize);
            } else {
                unmanage(c, 0);
            }
        }
    }
}
