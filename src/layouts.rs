use crate::{
    bindgen::{self, Monitor},
    is_visible, nexttiled, resize,
};

pub(crate) unsafe extern "C" fn monocle(m: *mut Monitor) {
    unsafe {
        let mut n = 0;
        let mut c;
        cfor!((c = (*m).clients; !c.is_null(); c = (*c).next) {
            if is_visible(c) {
                n += 1;
            }
        });
        if n > 0 {
            // override layout symbol
            libc::snprintf(
                (*m).ltsymbol.as_mut_ptr(),
                size_of_val(&(*m).ltsymbol),
                c"[%d]".as_ptr(),
                n,
            );
        }
        cfor!((c = nexttiled((*m).clients); !c.is_null(); c = nexttiled((*c).next)) {
            resize(c, (*m).wx, (*m).wy, (*m).ww - 2 * (*c).bw, (*m).wh - 2 * (*c).bw, 0);
        });
    }
}

pub(crate) unsafe extern "C" fn tile(m: *mut Monitor) {
    unsafe { bindgen::tile(m) }
}
