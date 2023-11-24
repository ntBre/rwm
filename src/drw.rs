use std::ffi::CString;

use fontconfig_sys::{FcChar8, FcNameParse};
use x11::{
    xft::{FcPattern, XftColor, XftFont, XftFontClose, XftFontOpenName},
    xlib::{
        CapButt, Drawable, JoinMiter, LineSolid, XCreateGC, XCreatePixmap,
        XDefaultDepth, XGCValues, XSetLineAttributes, GC,
    },
};

use crate::Display;

pub struct Fnt<'a> {
    dpy: &'a Display,
    pub h: usize,
    xfont: *mut XftFont,
    pattern: *mut FcPattern,
}

pub struct Drw<'a> {
    w: usize,
    h: usize,
    dpy: &'a Display,
    screen: i32,
    root: u64,
    drawable: Drawable,
    gc: GC,

    /// initially unset, set later I guess
    scheme: Option<XftColor>,

    /// using a vec instead of a linked list
    pub fonts: Vec<Fnt<'a>>,
}

impl<'a> Drw<'a> {
    pub fn new(
        dpy: &'a Display,
        screen: i32,
        root: u64,
        w: usize,
        h: usize,
    ) -> Self {
        let gc = unsafe {
            XCreateGC(
                dpy.inner,
                root,
                0,
                std::ptr::null::<XGCValues>() as *mut _,
            )
        };

        let drawable = unsafe {
            XCreatePixmap(
                dpy.inner,
                root,
                w as u32,
                h as u32,
                XDefaultDepth(dpy.inner, screen) as u32,
            )
        };

        unsafe {
            XSetLineAttributes(dpy.inner, gc, 1, LineSolid, CapButt, JoinMiter);
        }

        Self {
            w,
            h,
            dpy,
            screen,
            root,
            drawable,
            gc,
            scheme: None,
            fonts: Vec::new(),
        }
    }

    pub(crate) fn fontset_create(
        &mut self,
        fonts: [&str; 1],
    ) -> Result<(), ()> {
        for font in fonts {
            self.xfont_create(font);
        }
        Ok(())
    }

    fn xfont_create(&mut self, font: &str) {
        let s = CString::new(font).unwrap();
        let xfont =
            unsafe { XftFontOpenName(self.dpy.inner, self.screen, s.as_ptr()) };

        let pattern = unsafe { FcNameParse(s.as_ptr() as *const FcChar8) };

        unsafe {
            XftFontClose(self.dpy.inner, xfont);
        }

        self.fonts.push(Fnt {
            dpy: self.dpy,
            h: unsafe { (*xfont).ascent + (*xfont).descent } as usize,
            xfont,
            pattern: pattern as *mut FcPattern,
        })
    }
}
