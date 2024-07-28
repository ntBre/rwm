use crate::bindgen::{self, XEvent};

// DUMMY
pub(crate) fn buttonpress(e: *mut XEvent) {
    unsafe { bindgen::buttonpress(e) }
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
