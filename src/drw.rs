use std::{ffi::CString, mem::MaybeUninit};

use fontconfig_sys::{FcChar8, FcNameParse};
use x11::{
    xft::{
        FcPattern, XftColor, XftColorAllocName, XftFont, XftFontClose,
        XftFontOpenName,
    },
    xlib::{
        CapButt, Drawable, False, JoinMiter, LineSolid, XCopyArea,
        XCreateFontCursor, XCreateGC, XCreatePixmap, XDefaultColormap,
        XDefaultDepth, XDefaultVisual, XDrawRectangle, XFillRectangle,
        XGCValues, XSetForeground, XSetLineAttributes, XSync, GC,
    },
};

use crate::{Clr, Col, Cursor, Display};

pub struct Fnt {
    dpy: *mut Display,
    pub h: usize,
    xfont: *mut XftFont,
    pattern: *mut FcPattern,
    next: *mut Fnt,
}

pub struct Drw {
    w: usize,
    h: usize,
    dpy: *mut Display,
    screen: i32,
    root: u64,
    drawable: Drawable,
    gc: GC,
    scheme: *mut Clr,
    pub fonts: *mut Fnt,
}

impl Drw {
    pub fn new(
        dpy: &mut Display,
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
            scheme: std::ptr::null_mut(),
            fonts: std::ptr::null_mut(),
        }
    }

    pub(crate) fn fontset_create(
        &mut self,
        fonts: [&str; 1],
    ) -> Result<(), ()> {
        unsafe {
            let mut ret = std::ptr::null_mut();
            for font in fonts {
                let cur = self.xfont_create(font);
                if !cur.is_null() {
                    (*cur).next = ret;
                    ret = cur;
                }
            }
            self.fonts = ret;
        }
        Ok(())
    }

    fn xfont_create(&mut self, font: &str) -> *mut Fnt {
        let s = CString::new(font).unwrap();
        let xfont = unsafe {
            XftFontOpenName((*self.dpy).inner, self.screen, s.as_ptr())
        };

        let pattern = unsafe { FcNameParse(s.as_ptr() as *const FcChar8) };

        unsafe {
            XftFontClose((*self.dpy).inner, xfont);
        }

        let font = Fnt {
            dpy: self.dpy,
            h: unsafe { (*xfont).ascent + (*xfont).descent } as usize,
            xfont,
            pattern: pattern as *mut FcPattern,
            next: std::ptr::null_mut(),
        };
        Box::into_raw(Box::new(font))
    }

    pub(crate) fn cur_create(&self, shape: u8) -> Cursor {
        unsafe { XCreateFontCursor((*self.dpy).inner, shape as u32) }
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
                (*self.dpy).inner,
                XDefaultVisual((*self.dpy).inner, self.screen),
                XDefaultColormap((*self.dpy).inner, self.screen),
                name.as_ptr(),
                dest.as_mut_ptr(),
            );
            if ret != 0 {
                panic!("cannot allocate color {clrname}");
            }
            dest.assume_init()
        }
    }

    pub(crate) fn setscheme(&mut self, scm: &mut [XftColor]) {
        self.scheme = scm.as_mut_ptr();
    }

    pub(crate) fn textw(&self, stext: &str, lrpad: usize) -> usize {
        self.fontset_getwidth(stext) + lrpad
    }

    fn fontset_getwidth(&self, stext: &str) -> usize {
        self.text(0, 0, 0, 0, 0, stext, false)
    }

    pub(crate) fn text(
        &self,
        x: i32,
        y: i32,
        w: usize,
        h: usize,
        lpad: usize,
        text: &str,
        invert: bool,
    ) -> usize {
        todo!()
    }

    pub(crate) fn rect(
        &self,
        x: i32,
        y: usize,
        w: usize,
        h: usize,
        filled: bool,
        invert: bool,
    ) {
        if self.scheme.is_null() {
            return;
        }
        unsafe {
            XSetForeground(
                (*self.dpy).inner,
                self.gc,
                (*if invert {
                    self.scheme.offset(Col::Bg as isize)
                } else {
                    self.scheme.offset(Col::Fg as isize)
                })
                .pixel,
            );
            if filled {
                XFillRectangle(
                    (*self.dpy).inner,
                    self.drawable,
                    self.gc,
                    x,
                    y as i32,
                    w as u32,
                    h as u32,
                );
            } else {
                XDrawRectangle(
                    (*self.dpy).inner,
                    self.drawable,
                    self.gc,
                    x,
                    y as i32,
                    w as u32 - 1,
                    h as u32 - 1,
                );
            }
        }
    }

    pub(crate) fn map(&self, win: u64, x: i32, y: i32, w: i16, h: i16) {
        unsafe {
            XCopyArea(
                (*self.dpy).inner,
                self.drawable,
                win,
                self.gc,
                x,
                y,
                w as u32,
                h as u32,
                x,
                y,
            );
            XSync((*self.dpy).inner, False);
        }
    }
}
