use crate::bindgen::{self, Monitor};

pub(crate) unsafe extern "C" fn monocle(m: *mut Monitor) {
    unsafe { bindgen::monocle(m) }
}

pub(crate) unsafe extern "C" fn tile(m: *mut Monitor) {
    unsafe { bindgen::tile(m) }
}
