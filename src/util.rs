use libc::{c_void, size_t};

pub(crate) fn die(msg: &str) {
    eprintln!("{}", msg);
    std::process::exit(1);
}

pub(crate) fn ecalloc(nmemb: size_t, size: size_t) -> *mut c_void {
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
