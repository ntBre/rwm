use std::{ffi::CString, mem::MaybeUninit};

use fontconfig_sys::{FcChar8, FcNameParse};
use x11::{
    xft::{
        FcPattern, XftColor, XftColorAllocName, XftFont, XftFontClose,
        XftFontOpenName,
    },
    xlib::{
        CapButt, Drawable, JoinMiter, LineSolid, XCreateFontCursor, XCreateGC,
        XCreatePixmap, XDefaultColormap, XDefaultDepth, XDefaultVisual,
        XGCValues, XSetLineAttributes, GC,
    },
};

use crate::{Clr, Cursor, Display};

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

    pub(crate) fn cur_create(&self, shape: u8) -> Cursor {
        unsafe { XCreateFontCursor(self.dpy.inner, shape as u32) }
    }

    // TODO clrcount is the length of clrnames
    pub(crate) fn scm_create(
        &self,
        clrnames: [&str; 3],
        clrcount: i32,
    ) -> Vec<Clr> {
        let mut ret = Vec::new();
        for i in 0..clrcount {
            ret.push(self.clr_create(clrnames[i as usize]));
        }
        ret
    }

    fn clr_create(&self, clrname: &str) -> Clr {
        unsafe {
            let name = CString::new(clrname).unwrap();
            let mut dest = MaybeUninit::uninit();
            let ret = XftColorAllocName(
                self.dpy.inner,
                XDefaultVisual(self.dpy.inner, self.screen),
                XDefaultColormap(self.dpy.inner, self.screen),
                name.as_ptr(),
                dest.as_mut_ptr(),
            );
            if ret != 0 {
                panic!("cannot allocate color {clrname}");
            }
            dest.assume_init()
        }
    }
}
