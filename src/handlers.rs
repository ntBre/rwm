use std::{
    ffi::{c_long, c_uint},
    mem::MaybeUninit,
    ptr::null_mut,
};

use x11::xlib::{
    self, CWBackPixel, CWBorderWidth, CWHeight, CWWidth, CurrentTime, False,
    KeyCode, MappingKeyboard, NotifyInferior, NotifyNormal, PropertyChangeMask,
    PropertyDelete, ReplayPointer, ResizeRedirectMask, StructureNotifyMask,
    XAddToSaveSet, XChangeWindowAttributes, XEvent, XGetWindowAttributes,
    XMapRaised, XReparentWindow, XSelectInput, XSetWindowAttributes, XSync,
    XWindowAttributes, CWX, CWY, XA_WM_HINTS, XA_WM_NAME, XA_WM_NORMAL_HINTS,
    XA_WM_TRANSIENT_FOR,
};

use crate::{
    drw,
    enums::{Col, Scheme, XEmbed},
    util::ecalloc,
    Arg, Client, Monitor, State, Window,
};

use crate::{
    arrange, cleanmask, configure, drawbar, drawbars,
    enums::{Clk, Net},
    focus, getsystraywidth, grabkeys, height, is_visible, manage, recttomon,
    removesystrayicon, resizebarwin, resizeclient, restack, sendevent,
    setclientstate, setfocus, setfullscreen, seturgent, swallowingclient,
    textw, unfocus, unmanage, updatebars, updategeom, updatesizehints,
    updatestatus, updatesystray, updatesystrayicongeom, updatesystrayiconstate,
    updatetitle, updatewindowtype, updatewmhints, width, wintoclient, wintomon,
    wintosystrayicon,
    xembed::{
        SYSTEM_TRAY_REQUEST_DOCK, XEMBED_EMBEDDED_NOTIFY,
        XEMBED_EMBEDDED_VERSION, XEMBED_FOCUS_IN, XEMBED_MODALITY_ON,
        XEMBED_WINDOW_ACTIVATE,
    },
    NORMAL_STATE, WITHDRAWN_STATE,
};

pub fn buttonpress(state: &mut State, e: *mut XEvent) {
    unsafe {
        let mut arg = Arg::I(0);
        let ev = &(*e).button;
        let mut click = Clk::RootWin;
        // focus monitor if necessary
        let m = wintomon(state, ev.window);
        if !m.is_null() && m != state.selmon {
            crate::unfocus(state, (*state.selmon).sel, true);
            state.selmon = m;
            crate::focus(state, null_mut());
        }
        if ev.window == (*state.selmon).barwin {
            let mut i = 0;
            let mut x = 0;
            // emulating do-while
            loop {
                x += textw(&mut state.drw, &state.config.tags[i], state.lrpad);
                // condition
                if ev.x < x {
                    break;
                }
                i += 1;
                if i >= state.config.tags.len() {
                    break;
                }
            }
            if i < state.config.tags.len() {
                click = Clk::TagBar;
                arg = Arg::Ui(1 << i);
            } else if ev.x
                < x + textw(
                    &mut state.drw,
                    &(*state.selmon).ltsymbol,
                    state.lrpad,
                )
            {
                click = Clk::LtSymbol;
            } else if ev.x
                > (*state.selmon).ww
                    - textw(&mut state.drw, &state.stext, state.lrpad)
                    - getsystraywidth(state) as i32
            {
                click = Clk::StatusText;
            } else {
                click = Clk::WinTitle;
            }
        } else {
            let c = wintoclient(state, ev.window);
            if !c.is_null() {
                crate::focus(state, c);
                restack(state, state.selmon);
                xlib::XAllowEvents(state.dpy, ReplayPointer, CurrentTime);
                click = Clk::ClientWin;
            }
        }
        for i in 0..state.config.buttons.len() {
            if click as u32 == state.config.buttons[i].click
                && state.config.buttons[i].func.0.is_some()
                && state.config.buttons[i].button == ev.button
                && cleanmask(state, state.config.buttons[i].mask)
                    == cleanmask(state, ev.state)
            {
                let f = state.config.buttons[i].func.0.unwrap();
                let a = if click == Clk::TagBar
                    && state.config.buttons[i].arg.i() == 0
                {
                    &arg
                } else {
                    &state.config.buttons[i].arg
                };
                f(state, a)
            }
        }
    }
}

pub(crate) fn clientmessage(state: &mut State, e: *mut XEvent) {
    unsafe {
        let cme = &(*e).client_message;
        let mut c = wintoclient(state, cme.window);

        if state.config.showsystray
            && cme.window == state.systray().win
            && cme.message_type == state.netatom[Net::SystemTrayOP as usize]
        {
            // add systray icons
            if cme.data.get_long(1) == SYSTEM_TRAY_REQUEST_DOCK as c_long {
                c = ecalloc(1, size_of::<Client>()).cast();
            }
            (*c).win = cme.data.get_long(2) as u64;
            if (*c).win == 0 {
                libc::free(c.cast());
                return;
            }
            (*c).mon = state.selmon;
            (*c).next = state.systray().icons;
            let place = &mut state.systray_mut().icons;
            *place = c;
            let mut wa = MaybeUninit::uninit();
            if XGetWindowAttributes(state.dpy, (*c).win, wa.as_mut_ptr()) == 0 {
                // use sane defaults
                (*wa.as_mut_ptr()).width = state.bh;
                (*wa.as_mut_ptr()).height = state.bh;
                (*wa.as_mut_ptr()).border_width = 0;
            }
            let wa = wa.assume_init();
            // Safety: we already returned if c was null in ecalloc. could have
            // done this earlier too
            let c = &mut *c;

            c.x = 0;
            c.oldx = 0;
            c.y = 0;
            c.oldy = 0;

            c.w = wa.width;
            c.oldw = wa.width;

            c.h = wa.height;
            c.oldh = wa.height;

            c.oldbw = wa.border_width;
            c.bw = 0;
            c.isfloating = true;

            // reuse tags field as mapped status
            c.tags = 1;
            updatesizehints(state, c);
            updatesystrayicongeom(state, c, wa.width, wa.height);
            XAddToSaveSet(state.dpy, c.win);
            XSelectInput(
                state.dpy,
                c.win,
                StructureNotifyMask | PropertyChangeMask | ResizeRedirectMask,
            );
            XReparentWindow(state.dpy, c.win, state.systray().win, 0, 0);
            // use parent's background color
            let mut swa = XSetWindowAttributes {
                background_pixmap: 0,
                background_pixel: state.scheme[Scheme::Norm][Col::Bg as usize]
                    .pixel,
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
            XChangeWindowAttributes(state.dpy, c.win, CWBackPixel, &mut swa);
            // TODO this looks like the wrong index. xembed should be used to
            // index xatom. this could be coincidentally correct since they're
            // all integers though. the net atom at the same index would be
            // Net::WMName
            sendevent(
                state,
                c.win,
                state.netatom[XEmbed::XEmbed as usize],
                StructureNotifyMask as i32,
                CurrentTime as i64,
                XEMBED_EMBEDDED_NOTIFY as i64,
                0,
                state.systray().win as i64,
                XEMBED_EMBEDDED_VERSION as i64,
            );

            // FIXME (original author) not sure if I have to send these events
            // too
            sendevent(
                state,
                c.win,
                state.netatom[XEmbed::XEmbed as usize],
                StructureNotifyMask as i32,
                CurrentTime as i64,
                XEMBED_FOCUS_IN as i64,
                0,
                state.systray().win as i64,
                XEMBED_EMBEDDED_VERSION as i64,
            );
            sendevent(
                state,
                c.win,
                state.netatom[XEmbed::XEmbed as usize],
                StructureNotifyMask as i32,
                CurrentTime as i64,
                XEMBED_WINDOW_ACTIVATE as i64,
                0,
                state.systray().win as i64,
                XEMBED_EMBEDDED_VERSION as i64,
            );
            sendevent(
                state,
                c.win,
                state.netatom[XEmbed::XEmbed as usize],
                StructureNotifyMask as i32,
                CurrentTime as i64,
                XEMBED_MODALITY_ON as i64,
                0,
                state.systray().win as i64,
                XEMBED_EMBEDDED_VERSION as i64,
            );
            XSync(state.dpy, False);
            resizebarwin(state, state.selmon);
            updatesystray(state);
            setclientstate(state, c, NORMAL_STATE);

            return;
        }

        if c.is_null() {
            return;
        }
        if cme.message_type == state.netatom[Net::WMState as usize] {
            if cme.data.get_long(1)
                == state.netatom[Net::WMFullscreen as usize] as i64
                || cme.data.get_long(2)
                    == state.netatom[Net::WMFullscreen as usize] as i64
            {
                setfullscreen(
                    state,
                    c,
                    cme.data.get_long(0) == 1 // _NET_WM_STATE_ADD
                        || (cme.data.get_long(0) == 2 // _NET_WM_STATE_TOGGLE
                            && !(*c).isfullscreen),
                );
            }
        } else if cme.message_type == state.netatom[Net::ActiveWindow as usize]
            && c != (*state.selmon).sel
            && !(*c).isurgent
        {
            seturgent(state, c, true);
        }
    }
}

pub(crate) fn configurerequest(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).configure_request;
        let c = wintoclient(state, ev.window);
        if !c.is_null() {
            if (ev.value_mask & CWBorderWidth as u64) != 0 {
                (*c).bw = ev.border_width;
            } else if (*c).isfloating
                || (*(*state.selmon).lt[(*state.selmon).sellt as usize])
                    .arrange
                    .is_none()
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
                if (c.x + c.w) > m.mx + m.mw && c.isfloating {
                    c.x = m.mx + (m.mw / 2 - width(c) / 2); // center x
                }
                if (c.y + c.h) > m.my + m.mh && c.isfloating {
                    c.y = m.my + (m.mh / 2 - height(c) / 2); // center y
                }
                if (ev.value_mask & (CWX | CWY) as u64) != 0
                    && (ev.value_mask & (CWWidth | CWHeight) as u64) == 0
                {
                    configure(state, c);
                }
                if is_visible(c) {
                    xlib::XMoveResizeWindow(
                        state.dpy, c.win, c.x, c.y, c.w as u32, c.h as u32,
                    );
                }
            } else {
                configure(state, c);
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
                state.dpy,
                ev.window,
                ev.value_mask as u32,
                &mut wc,
            );
        }
        xlib::XSync(state.dpy, False);
    }
}

pub(crate) fn configurenotify(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).configure;
        /* TODO: updategeom handling sucks, needs to be simplified */
        if ev.window == state.root {
            let dirty = state.sw != ev.width || state.sh != ev.height;
            state.sw = ev.width;
            state.sh = ev.height;
            if updategeom(state) != 0 || dirty {
                drw::resize(
                    &mut state.drw,
                    state.sw as c_uint,
                    state.bh as c_uint,
                );
                updatebars(state);
                let mut m = state.mons;
                while !m.is_null() {
                    let mut c = (*m).clients;
                    while !c.is_null() {
                        if (*c).isfullscreen {
                            resizeclient(
                                state,
                                c,
                                (*m).mx,
                                (*m).my,
                                (*m).mw,
                                (*m).mh,
                            );
                        }
                        c = (*c).next;
                    }
                    resizebarwin(state, m);
                    m = (*m).next;
                }
                focus(state, null_mut());
                arrange(state, null_mut());
            }
        }
    }
}

pub(crate) fn destroynotify(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).destroy_window;
        let mut c = wintoclient(state, ev.window);
        if !c.is_null() {
            unmanage(state, c, 1);
        } else {
            c = swallowingclient(state, ev.window);
            if !c.is_null() {
                unmanage(state, (*c).swallowing, 1);
            } else {
                c = wintosystrayicon(state, ev.window);
                if !c.is_null() {
                    removesystrayicon(state, c);
                    resizebarwin(state, state.selmon);
                    updatesystray(state);
                }
            }
        }
    }
}

pub(crate) fn enternotify(state: &mut State, e: *mut XEvent) {
    log::trace!("enternotify");
    unsafe {
        let ev = &mut (*e).crossing;
        if (ev.mode != NotifyNormal || ev.detail == NotifyInferior)
            && ev.window != state.root
        {
            return;
        }
        let c = wintoclient(state, ev.window);
        let m =
            if !c.is_null() { (*c).mon } else { wintomon(state, ev.window) };
        if m != state.selmon {
            unfocus(state, (*state.selmon).sel, true);
            state.selmon = m;
        } else if c.is_null() || c == (*state.selmon).sel {
            return;
        }
        focus(state, c)
    }
}

pub(crate) fn expose(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).expose;
        if ev.count == 0 {
            let m = wintomon(state, ev.window);
            if !m.is_null() {
                drawbar(state, m);
                if m == state.selmon {
                    updatesystray(state);
                }
            }
        }
    }
}

/* there are some broken focus acquiring clients needing extra handling */
pub(crate) fn focusin(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &(*e).focus_change;
        if !(*state.selmon).sel.is_null()
            && ev.window != (*(*state.selmon).sel).win
        {
            setfocus(state, (*state.selmon).sel);
        }
    }
}

pub(crate) fn keypress(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).key;
        let keysym =
            xlib::XKeycodeToKeysym(state.dpy, ev.keycode as KeyCode, 0);
        for i in 0..state.config.keys.len() {
            if keysym == state.config.keys[i].keysym
                && cleanmask(state, state.config.keys[i].mod_)
                    == cleanmask(state, ev.state)
                && state.config.keys[i].func.0.is_some()
            {
                state.config.keys[i].func.0.unwrap()(
                    state,
                    &(state.config.keys[i].arg),
                );
            }
        }
    }
}

pub(crate) fn mappingnotify(state: &mut State, e: *mut XEvent) {
    unsafe {
        let ev = &mut (*e).mapping;
        xlib::XRefreshKeyboardMapping(ev);
        if ev.request == MappingKeyboard {
            grabkeys(state);
        }
    }
}

pub(crate) fn maprequest(state: &mut State, e: *mut XEvent) {
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
        let i = wintosystrayicon(state, ev.window);
        if !i.is_null() {
            sendevent(
                state,
                (*i).win,
                state.netatom[XEmbed::XEmbed as usize],
                StructureNotifyMask as i32,
                CurrentTime as i64,
                XEMBED_WINDOW_ACTIVATE as i64,
                0,
                state.systray().win as i64,
                XEMBED_EMBEDDED_VERSION as i64,
            );
            resizebarwin(state, state.selmon);
            updatesystray(state);
        }
        log::trace!("maprequest: XGetWindowAttributes");
        let res = xlib::XGetWindowAttributes(state.dpy, ev.window, &raw mut WA);
        // XGetWindowAttributes returns a zero if the function fails
        if res == 0 || WA.override_redirect != 0 {
            return;
        }
        if wintoclient(state, ev.window).is_null() {
            manage(state, ev.window, &raw mut WA);
        }
    }
}

pub(crate) fn motionnotify(state: &mut State, e: *mut XEvent) {
    log::trace!("motionnotify");
    static mut MON: *mut Monitor = null_mut();
    unsafe {
        let ev = &(*e).motion;
        if ev.window != state.root {
            return;
        }
        let m = recttomon(state, ev.x_root, ev.y_root, 1, 1);
        if m != MON && !MON.is_null() {
            unfocus(state, (*state.selmon).sel, true);
            state.selmon = m;
            focus(state, null_mut());
        }
        MON = m;
    }
}

pub(crate) fn propertynotify(state: &mut State, e: *mut XEvent) {
    log::trace!("propertynotify");
    unsafe {
        let mut trans: Window = 0;
        let ev = &mut (*e).property;

        let c = wintosystrayicon(state, ev.window);
        if !c.is_null() {
            if ev.atom == XA_WM_NORMAL_HINTS {
                updatesizehints(state, c);
                updatesystrayicongeom(state, c, (*c).w, (*c).h);
            } else {
                updatesystrayiconstate(state, c, ev);
            }
            resizebarwin(state, state.selmon);
            updatesystray(state);
        }

        if ev.window == state.root && ev.atom == XA_WM_NAME {
            updatestatus(state);
        } else if ev.state == PropertyDelete {
            return; // ignore
        } else {
            let c = wintoclient(state, ev.window);
            if c.is_null() {
                return;
            }
            let c = &mut *c;
            match ev.atom {
                XA_WM_TRANSIENT_FOR => {
                    if !c.isfloating
                        && (xlib::XGetTransientForHint(
                            state.dpy, c.win, &mut trans,
                        ) != 0)
                    {
                        c.isfloating = !wintoclient(state, trans).is_null();
                        if c.isfloating {
                            arrange(state, c.mon);
                        }
                    }
                }
                XA_WM_NORMAL_HINTS => {
                    c.hintsvalid = 0;
                }
                XA_WM_HINTS => {
                    updatewmhints(state, c);
                    drawbars(state);
                }
                _ => {}
            }
            if ev.atom == XA_WM_NAME
                || ev.atom == state.netatom[Net::WMName as usize]
            {
                updatetitle(state, c);
                if c as *mut _ == (*c.mon).sel {
                    drawbar(state, c.mon);
                }
            }
            if ev.atom == state.netatom[Net::WMWindowType as usize] {
                updatewindowtype(state, c);
            }
        }
    }
}

pub(crate) fn unmapnotify(state: &mut State, e: *mut XEvent) {
    log::trace!("unmapnotify");
    unsafe {
        let ev = &(*e).unmap;
        let mut c = wintoclient(state, ev.window);
        if !c.is_null() {
            if ev.send_event != 0 {
                setclientstate(state, c, WITHDRAWN_STATE);
            } else {
                unmanage(state, c, 0);
            }
        } else {
            c = wintosystrayicon(state, ev.window);
            if !c.is_null() {
                // KLUDGE (systray author) sometimes icons occasionally unmap
                // their windows but do _not_ destroy them. we map those windows
                // back
                XMapRaised(state.dpy, (*c).win);
                updatesystray(state);
            }
        }
    }
}

pub(crate) fn resizerequest(state: &mut State, e: *mut XEvent) {
    log::trace!("resizerequest");
    unsafe {
        let ev = &(*e).resize_request;
        let i = wintosystrayicon(state, ev.window);
        if !i.is_null() {
            updatesystrayicongeom(state, i, ev.width, ev.height);
            resizebarwin(state, state.selmon);
            updatesystray(state);
        }
    }
}
