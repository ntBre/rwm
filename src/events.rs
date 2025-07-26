use x11::xlib::{_XDisplay, XButtonEvent, XEvent};

pub enum Event {
    Button(XButtonEvent),
}

impl Event {
    pub fn button(
        window: u64,
        button: u32,
        x: i32,
        display: *mut _XDisplay,
        state: u32,
    ) -> Self {
        Self::Button(XButtonEvent {
            type_: 0,
            serial: 0,
            send_event: 0,
            display,
            window,
            root: 0,
            subwindow: 0,
            time: 0,
            x,
            y: 0,
            x_root: 0,
            y_root: 0,
            state,
            button,
            same_screen: 0,
        })
    }

    pub fn into_button(self) -> XEvent {
        match self {
            Event::Button(button) => XEvent { button },
        }
    }
}
