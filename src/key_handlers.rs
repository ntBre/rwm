use crate::bindgen::{self, Arg};

pub(crate) unsafe extern "C" fn togglebar(arg: *const Arg) {
    unsafe { bindgen::togglebar(arg) }
}

pub(crate) unsafe extern "C" fn focusstack(arg: *const Arg) {
    unsafe { bindgen::focusstack(arg) }
}

pub(crate) unsafe extern "C" fn incnmaster(arg: *const Arg) {
    unsafe { bindgen::incnmaster(arg) }
}

pub(crate) unsafe extern "C" fn setmfact(arg: *const Arg) {
    unsafe { bindgen::setmfact(arg) }
}

pub(crate) unsafe extern "C" fn zoom(arg: *const Arg) {
    unsafe { bindgen::zoom(arg) }
}

pub(crate) unsafe extern "C" fn view(arg: *const Arg) {
    unsafe { bindgen::view(arg) }
}

pub(crate) unsafe extern "C" fn killclient(arg: *const Arg) {
    unsafe { bindgen::killclient(arg) }
}

pub(crate) unsafe extern "C" fn setlayout(arg: *const Arg) {
    unsafe { bindgen::setlayout(arg) }
}

pub(crate) unsafe extern "C" fn togglefloating(arg: *const Arg) {
    unsafe { bindgen::togglefloating(arg) }
}

pub(crate) unsafe extern "C" fn tag(arg: *const Arg) {
    unsafe { bindgen::tag(arg) }
}

pub(crate) unsafe extern "C" fn focusmon(arg: *const Arg) {
    unsafe { bindgen::focusmon(arg) }
}

pub(crate) unsafe extern "C" fn tagmon(arg: *const Arg) {
    unsafe { bindgen::tagmon(arg) }
}

pub(crate) unsafe extern "C" fn toggleview(arg: *const Arg) {
    unsafe { bindgen::toggleview(arg) }
}

pub(crate) unsafe extern "C" fn quit(arg: *const Arg) {
    unsafe { bindgen::quit(arg) }
}
