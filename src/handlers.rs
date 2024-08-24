use std::{
    ffi::{c_int, c_uint},
    ptr::{addr_of, addr_of_mut, null_mut},
};

use x11::xlib::{
    self, CWBorderWidth, CWHeight, CWWidth, CurrentTime, False, KeyCode,
    MappingKeyboard, NotifyInferior, NotifyNormal, PropertyDelete,
    ReplayPointer, XEvent, XWindowAttributes, CWX, CWY, XA_WM_HINTS,
    XA_WM_NAME, XA_WM_NORMAL_HINTS, XA_WM_TRANSIENT_FOR,
};

use rwm::{Arg, Monitor, Window};

use crate::{
    arrange, cleanmask,
    config::{BUTTONS, CONFIG, KEYS},
    configure, drawbar, drawbars, drw,
    enums::{Clk, Net},
    focus, grabkeys, height, is_visible, manage, recttomon, resizeclient,
    restack, setclientstate, setfocus, setfullscreen, seturgent, textw,
    unfocus, unmanage, updatebars, updategeom, updatestatus, updatetitle,
    updatewindowtype, updatewmhints, width, wintoclient, wintomon, BH, DPY,
    DRW, MONS, NETATOM, ROOT, SELMON, SH, STEXT, SW, WITHDRAWN_STATE,
};

pub(crate) fn buttonpress(e: *mut XEvent) {
    unsafe {
        let mut arg = Arg { i: 0 };
        let ev = &(*e).button;
        let mut click = Clk::RootWin;
        // focus monitor if necessary
        let m = wintomon(ev.window);
        if !m.is_null() && m != SELMON {
            crate::unfocus((*SELMON).sel, true);
            SELMON = m;
            crate::focus(null_mut());
        }
        if ev.window == (*SELMON).barwin {
            let mut i = 0;
            let mut x = 0;
            // emulating do-while
            loop {
                x += textw(CONFIG.tags[i].as_ptr());
                // condition
                if ev.x < x {
                    break;
                }
                i += 1;
                if i >= CONFIG.tags.len() {
                    break;
                }
            }
            if i < CONFIG.tags.len() {
                click = Clk::TagBar;
                arg = Arg { ui: 1 << i };
            } else if ev.x < x + textw(addr_of!((*SELMON).ltsymbol) as *const _)
            {
                click = Clk::LtSymbol;
            } else if ev.x > (*SELMON).ww - textw(addr_of!(STEXT) as *const _) {
                click = Clk::StatusText;
            } else {
                click = Clk::WinTitle;
            }
        } else {
            let c = wintoclient(ev.window);
            if !c.is_null() {
                crate::focus(c);
                restack(SELMON);
                xlib::XAllowEvents(DPY, ReplayPointer, CurrentTime);
                click = Clk::ClientWin;
            }
        }
        for button in BUTTONS {
            if click as u32 == button.click
                && button.func.is_some()
                && button.button == ev.button
                && cleanmask(button.mask) == cleanmask(ev.state)
            {
                let f = button.func.unwrap();
                let a = if click == Clk::TagBar && button.arg.i == 0 {
                    &arg
                } else {
                    &button.arg
                };
                f(a)
            }
        }
    }
}

pub(crate) fn clientmessage(e: *mut XEvent) {
    unsafe {
        let cme = &(*e).client_message;
        let c = wintoclient(cme.window);

        if c.is_null() {
            return;
        }
        if cme.message_type == NETATOM[Net::WMState as usize] {
            if cme.data.get_long(1)
                == NETATOM[Net::WMFullscreen as usize] as i64
                || cme.data.get_long(2)
                    == NETATOM[Net::WMFullscreen as usize] as i64
            {
                setfullscreen(
                    c,
                    cme.data.get_long(0) == 1 // _NET_WM_STATE_ADD
                        || (cme.data.get_long(0) == 2 // _NET_WM_STATE_TOGGLE
                            && (*c).isfullscreen == 0),
                );
            }
        } else if cme.message_type == NETATOM[Net::ActiveWindow as usize]
            && c != (*SELMON).sel
            && (*c).isurgent == 0
        {
            seturgent(c, true);
        }
    }
}

pub(crate) fn configurerequest(e: *mut XEvent) {
    unsafe {
        let ev = &(*e).configure_request;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if (ev.value_mask & CWBorderWidth as u64) != 0 {
                (*c).bw = ev.border_width;
            } else if (*c).isfloating != 0
                || (*(*SELMON).lt[(*SELMON).sellt as usize]).arrange.is_none()
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
                    xlib::XMoveResizeWindow(
                        DPY, c.win, c.x, c.y, c.w as u32, c.h as u32,
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
            let mut wc = xlib::XWindowChanges {
                x,
                y,
                width,
                height,
                border_width,
                sibling,
                stack_mode,
            };
            xlib::XConfigureWindow(
                DPY,
                ev.window,
                ev.value_mask as u32,
                &mut wc,
            );
        }
        xlib::XSync(DPY, False);
    }
}

pub(crate) fn configurenotify(e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).configure;
        /* TODO: updategeom handling sucks, needs to be simplified */
        if ev.window == ROOT {
            let dirty = SW != ev.width || SH != ev.height;
            SW = ev.width;
            SH = ev.height;
            if updategeom() != 0 || dirty {
                drw::resize(DRW, SW as c_uint, BH as c_uint);
                updatebars();
                let mut m = MONS;
                while !m.is_null() {
                    let mut c = (*m).clients;
                    while !c.is_null() {
                        if (*c).isfullscreen != 0 {
                            resizeclient(c, (*m).mx, (*m).my, (*m).mw, (*m).mh);
                        }
                        c = (*c).next;
                    }
                    xlib::XMoveResizeWindow(
                        DPY,
                        (*m).barwin,
                        (*m).wx,
                        (*m).by,
                        (*m).ww as u32,
                        BH as u32,
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
        let ev = &(*e).destroy_window;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            unmanage(c, 1);
        }
    }
}

pub(crate) fn enternotify(e: *mut XEvent) {
    log::trace!("enternotify");
    unsafe {
        let ev = &mut (*e).crossing;
        if (ev.mode != NotifyNormal || ev.detail == NotifyInferior)
            && ev.window != ROOT
        {
            return;
        }
        let c = wintoclient(ev.window);
        let m = if !c.is_null() { (*c).mon } else { wintomon(ev.window) };
        if m != SELMON {
            unfocus((*SELMON).sel, true);
            SELMON = m;
        } else if c.is_null() || c == (*SELMON).sel {
            return;
        }
        focus(c)
    }
}

pub(crate) fn expose(e: *mut XEvent) {
    unsafe {
        let ev = &(*e).expose;
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
        let ev = &(*e).focus_change;
        if !(*SELMON).sel.is_null() && ev.window != (*(*SELMON).sel).win {
            setfocus((*SELMON).sel);
        }
    }
}

pub(crate) fn keypress(e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).key;
        let keysym = xlib::XKeycodeToKeysym(DPY, ev.keycode as KeyCode, 0);
        for key in KEYS {
            if keysym == key.keysym
                && cleanmask(key.mod_) == cleanmask(ev.state)
                && key.func.is_some()
            {
                key.func.unwrap()(&(key.arg));
            }
        }
    }
}

pub(crate) fn mappingnotify(e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).mapping;
        xlib::XRefreshKeyboardMapping(ev);
        if ev.request == MappingKeyboard {
            grabkeys();
        }
    }
}

pub(crate) fn maprequest(e: *mut XEvent) {
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
        let ev = &(*e).map_request;
        log::trace!("maprequest: XGetWindowAttributes");
        let res = xlib::XGetWindowAttributes(DPY, ev.window, addr_of_mut!(WA));
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
    static mut MON: *mut Monitor = null_mut();
    unsafe {
        let ev = &(*e).motion;
        if ev.window != ROOT {
            return;
        }
        let m = recttomon(ev.x_root, ev.y_root, 1, 1);
        if m != MON && !MON.is_null() {
            unfocus((*SELMON).sel, true);
            SELMON = m;
            focus(null_mut());
        }
        MON = m;
    }
}

pub(crate) fn propertynotify(e: *mut XEvent) {
    log::trace!("propertynotify");
    unsafe {
        let mut trans: Window = 0;
        let ev = &mut (*e).property;
        if ev.window == ROOT && ev.atom == XA_WM_NAME {
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
                        && (xlib::XGetTransientForHint(DPY, c.win, &mut trans)
                            != 0)
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
            if ev.atom == XA_WM_NAME || ev.atom == NETATOM[Net::WMName as usize]
            {
                updatetitle(c);
                if c as *mut _ == (*c.mon).sel {
                    drawbar(c.mon);
                }
            }
            if ev.atom == NETATOM[Net::WMWindowType as usize] {
                updatewindowtype(c);
            }
        }
    }
}

pub(crate) fn unmapnotify(e: *mut XEvent) {
    log::trace!("unmapnotify");
    unsafe {
        let ev = &(*e).unmap;
        let c = wintoclient(ev.window);
        if !c.is_null() {
            if ev.send_event != 0 {
                setclientstate(c, WITHDRAWN_STATE);
            } else {
                unmanage(c, 0);
            }
        }
    }
}
