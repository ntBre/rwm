use libc::{c_void, size_t};

pub(crate) fn die(msg: &str) {
    eprintln!("{}", msg);
    std::process::exit(1);
}

/// Attempt to allocate with `libc::calloc` and die if the result is null
pub(crate) fn ecalloc(nmemb: size_t, size: size_t) -> *mut c_void {
    log::trace!("ecalloc: nmemb = {nmemb}, size = {size}");
    let ret = unsafe { libc::calloc(nmemb, size) };
    if ret.is_null() {
        die("calloc:");
    }
    ret
}

#[inline]
pub(crate) fn between<T: PartialOrd>(x: T, a: T, b: T) -> bool {
    a <= x && x <= b
}
