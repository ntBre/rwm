//! Safe bindings to X11 functions

use std::os::raw::c_int;

use x11::xlib::{self, Atom, Display, XFree};

use crate::Window;

pub struct WmProtocols<'a> {
    atoms: *mut Atom,
    slice: &'a [Atom],
}

impl WmProtocols<'_> {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Atom> {
        self.slice.iter()
    }
}

impl Drop for WmProtocols<'_> {
    fn drop(&mut self) {
        unsafe {
            XFree(self.atoms.cast());
        }
    }
}

/// Return the list of atoms stored in the `WM_PROTOCOLS` property on `w`.
///
/// See `XGetWMProtocols(3)` for more details.
pub fn get_wm_protocols<'a>(
    display: *mut Display,
    w: Window,
) -> Result<WmProtocols<'a>, c_int> {
    let mut protocols = std::ptr::null_mut();
    let mut n = 0;
    unsafe {
        let status = xlib::XGetWMProtocols(display, w, &mut protocols, &mut n);

        if status == 0 {
            return Err(status);
        }

        Ok(WmProtocols {
            atoms: protocols,
            slice: std::slice::from_raw_parts(
                protocols,
                usize::try_from(n).unwrap(),
            ),
        })
    }
}
