use std::{
    ffi::{c_int, CString},
    mem::MaybeUninit,
};

use fontconfig_sys::{
    constants::{FC_CHARSET, FC_SCALABLE},
    FcChar8, FcCharSetAddChar, FcCharSetCreate, FcCharSetDestroy,
    FcConfigSubstitute, FcDefaultSubstitute, FcMatchPattern, FcNameParse,
    FcPatternAddBool, FcPatternAddCharSet, FcPatternDestroy,
    FcPatternDuplicate,
};
use x11::{
    xft::{
        FcPattern, XftCharExists, XftColor, XftColorAllocName, XftDraw,
        XftDrawCreate, XftDrawDestroy, XftDrawStringUtf8, XftFont,
        XftFontClose, XftFontMatch, XftFontOpenName, XftFontOpenPattern,
        XftTextExtentsUtf8,
    },
    xlib::{
        CapButt, Drawable, False, JoinMiter, LineSolid, XCopyArea,
        XCreateFontCursor, XCreateGC, XCreatePixmap, XDefaultColormap,
        XDefaultDepth, XDefaultVisual, XDrawRectangle, XFillRectangle,
        XFreePixmap, XGCValues, XSetForeground, XSetLineAttributes, XSync, GC,
    },
};

use crate::{Clr, Col, Cursor, Display, LRPAD};

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

    /// this is the first case of xfont_create, where a font name is provided
    /// rather than an existing *mut Fnt
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

    fn xfont_create2(&self, fontpattern: *mut FcPattern) -> *mut Fnt {
        let pattern = std::ptr::null_mut();
        unsafe {
            let xfont = XftFontOpenPattern((*self.dpy).inner, fontpattern);
            if xfont.is_null() {
                panic!("cannot load font from pattern");
            }
            let font = Fnt {
                dpy: self.dpy,
                h: ((*xfont).ascent + (*xfont).descent) as usize,
                xfont,
                pattern,
                next: std::ptr::null_mut(),
            };
            Box::into_raw(Box::new(font))
        }
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
            let cmap = XDefaultColormap((*self.dpy).inner, self.screen);
            let ret = XftColorAllocName(
                (*self.dpy).inner,
                XDefaultVisual((*self.dpy).inner, self.screen),
                cmap,
                name.as_ptr(),
                dest.as_mut_ptr(),
            );
            if ret == 0 {
                panic!("cannot allocate color {clrname} with status {ret}");
            }
            dest.assume_init()
        }
    }

    pub(crate) fn setscheme(&mut self, scm: &mut [XftColor]) {
        self.scheme = scm.as_mut_ptr();
    }

    pub(crate) fn textw(&self, stext: *const String) -> usize {
        unsafe { self.fontset_getwidth(stext) + LRPAD }
    }

    fn fontset_getwidth(&self, text: *const String) -> usize {
        unsafe {
            if self.fonts.is_null() || (*text).is_empty() {
                return 0;
            }
        }
        self.text(0, 0, 0, 0, 0, text, 0)
    }
}

// these are statics defined inside text
static mut ELLIPSIS_WIDTH: usize = 0;
const NOMATCHES_LEN: usize = 64;
struct NoMatches {
    codepoint: [usize; NOMATCHES_LEN],
    idx: usize,
}
static mut NOMATCHES: NoMatches = NoMatches {
    codepoint: [0; NOMATCHES_LEN],
    idx: 0,
};

impl Drw {
    pub(crate) fn text(
        &self,
        mut x: i32,
        y: i32,
        mut w: usize,
        h: usize,
        lpad: usize,
        text: *const String,
        invert: c_int,
    ) -> usize {
        let mut ty;
        let mut ellipsis_x = 0;
        let mut tmpw: usize = 0;
        let mut ew;
        let mut ellipsis_w = 0;
        let mut ellipsis_len;
        let mut d: *mut XftDraw = std::ptr::null_mut();
        let mut usedfont;
        let mut curfont;
        let mut nextfont;
        let mut utf8strlen;
        let mut utf8charlen;
        let render = x != 0 || y != 0 || w != 0 || h != 0;
        let mut utf8codepoint = 0;
        let mut utf8str;
        let mut fccharset;
        let mut fcpattern;
        let mut match_;
        let mut result = MaybeUninit::uninit();
        let mut charexists = false;
        let mut overflow = false;

        let ellipsis = String::from("...");

        unsafe {
            if (render && (self.scheme.is_null() || w == 0))
                || (*text).is_empty()
                || self.fonts.is_null()
            {
                return 0;
            }

            if !render {
                w = if invert != 0 { invert } else { !invert } as usize;
            } else {
                XSetForeground(
                    (*self.dpy).inner,
                    self.gc,
                    (*self.scheme.offset(if invert != 0 {
                        Col::Fg
                    } else {
                        Col::Bg
                    } as isize))
                    .pixel,
                );
                XFillRectangle(
                    (*self.dpy).inner,
                    self.drawable,
                    self.gc,
                    x,
                    y,
                    w as u32,
                    h as u32,
                );
                d = XftDrawCreate(
                    (*self.dpy).inner,
                    self.drawable,
                    XDefaultVisual((*self.dpy).inner, self.screen),
                    XDefaultColormap((*self.dpy).inner, self.screen),
                );
                x += lpad as i32;
                w -= lpad;
            }

            usedfont = self.fonts;
            if ELLIPSIS_WIDTH == 0 && render {
                ELLIPSIS_WIDTH = self.fontset_getwidth(&ellipsis);
            }

            let mut text_idx = 0;
            'outer: loop {
                ew = 0;
                ellipsis_len = 0;
                utf8strlen = 0;
                utf8str = text;
                nextfont = std::ptr::null_mut();
                for ch in (*text).chars() {
                    utf8charlen = ch.len_utf8();
                    utf8codepoint = ch as usize;
                    curfont = self.fonts;
                    while !curfont.is_null() {
                        charexists = charexists
                            || XftCharExists(
                                (*self.dpy).inner,
                                (*curfont).xfont,
                                utf8codepoint as u32,
                            ) != 0;
                        if charexists {
                            let text = (*text).clone();
                            self.font_getexts(
                                curfont,
                                &text,
                                utf8charlen,
                                &mut tmpw,
                                std::ptr::null_mut(),
                            );
                            if ew + ELLIPSIS_WIDTH <= w {
                                // keep track where the ellipsis still fits
                                ellipsis_x = x + ew as i32;
                                ellipsis_w = w - ew;
                                ellipsis_len = utf8strlen;
                            }

                            if ew + tmpw > w {
                                overflow = true;
                                // called from drw_fontset_getwidth_clamp():
                                // it wants the width AFTER the overflow
                                if !render {
                                    x += tmpw as i32;
                                } else {
                                    utf8strlen = ellipsis_len;
                                }
                            } else if curfont == usedfont {
                                utf8strlen += utf8charlen;
                                text_idx += utf8charlen;
                                ew += tmpw;
                            } else {
                                nextfont = curfont;
                            }
                            break;
                        }
                        curfont = (*curfont).next;
                    }

                    if overflow || !charexists || !nextfont.is_null() {
                        break;
                    } else {
                        charexists = false;
                    }
                } // end while text

                if utf8strlen != 0 {
                    if render {
                        ty = y as usize
                            + (h - (*usedfont).h) / 2
                            + (*(*usedfont).xfont).ascent as usize;
                        let s = CString::new((*utf8str).clone()).unwrap();
                        XftDrawStringUtf8(
                            d,
                            self.scheme.offset(if invert != 0 {
                                Col::Bg
                            } else {
                                Col::Fg
                            }
                                as isize),
                            (*usedfont).xfont,
                            x,
                            ty as i32,
                            s.as_ptr().cast(),
                            utf8strlen as i32,
                        );
                    }
                    x += ew as i32;
                    w -= ew;
                }

                if render && overflow {
                    self.text(
                        ellipsis_x, y, ellipsis_w, h, 0, &ellipsis, invert,
                    );
                }

                if text_idx >= (*text).len() || overflow {
                    break;
                } else if !nextfont.is_null() {
                    charexists = false;
                    usedfont = nextfont;
                } else {
                    // regardless of whether or not a fallback font is found,
                    // the character must be drawn
                    charexists = true;

                    for i in 0..NOMATCHES_LEN {
                        // avoid calling XftFontMatch if we know we won't find a
                        // match
                        if utf8codepoint == NOMATCHES.codepoint[i] {
                            usedfont = self.fonts;
                            continue 'outer;
                        }
                    }

                    fccharset = FcCharSetCreate();
                    FcCharSetAddChar(fccharset, utf8codepoint as u32);

                    if (*self.fonts).pattern.is_null() {
                        // refer to the comment in xfont_create for more information
                        panic!("the first font in the cache must be loaded from a font string");
                    }

                    fcpattern =
                        FcPatternDuplicate((*self.fonts).pattern.cast());
                    FcPatternAddCharSet(
                        fcpattern,
                        FC_CHARSET.as_ptr(),
                        fccharset,
                    );
                    // this 1 is supposed to be FcTrue, but the type FcBool is
                    // an alias for a c_int. confirmed in fontconfig.h
                    FcPatternAddBool(fcpattern, FC_SCALABLE.as_ptr(), 1);

                    FcConfigSubstitute(
                        std::ptr::null_mut(),
                        fcpattern,
                        FcMatchPattern,
                    );
                    FcDefaultSubstitute(fcpattern);
                    match_ = XftFontMatch(
                        (*self.dpy).inner,
                        self.screen,
                        fcpattern.cast(),
                        result.as_mut_ptr(),
                    );

                    FcCharSetDestroy(fccharset);
                    FcPatternDestroy(fcpattern);

                    if !match_.is_null() {
                        usedfont = self.xfont_create2(match_);
                        if !usedfont.is_null()
                            && XftCharExists(
                                (*self.dpy).inner,
                                (*usedfont).xfont,
                                utf8codepoint as u32,
                            ) != 0
                        {
                            // get to the end of the linked list
                            let mut curfont = self.fonts;
                            while !(*curfont).next.is_null() {
                                curfont = (*curfont).next;
                            }

                            // and append
                            (*curfont).next = usedfont;
                        } else {
                            xfont_free(usedfont);
                            NOMATCHES.idx += 1;
                            NOMATCHES.codepoint
                                [NOMATCHES.idx % NOMATCHES_LEN] = utf8codepoint;
                            usedfont = self.fonts;
                        }
                    }
                }
            } // close loop
            if !d.is_null() {
                XftDrawDestroy(d);
            }
        }
        x as usize + if render { w } else { 0 }
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

    fn font_getexts(
        &self,
        font: *mut Fnt,
        text: &str,
        len: usize,
        w: *mut usize,
        h: *mut usize,
    ) {
        if font.is_null() || text.is_empty() {
            return;
        }
        let mut ext = MaybeUninit::uninit();
        let s = CString::new(text).unwrap();
        unsafe {
            XftTextExtentsUtf8(
                (*(*font).dpy).inner,
                (*font).xfont,
                s.as_ptr().cast(),
                len as i32,
                ext.as_mut_ptr(),
            );
            let ext = ext.assume_init();
            if !w.is_null() {
                *w = ext.xOff as usize;
            }
            if !h.is_null() {
                *h = (*font).h;
            }
        }
    }

    /// no-op, I'm pretty sure Cursors get cleaned up naturally
    pub(crate) fn cur_free(&self, _cursor: Cursor) {}

    pub(crate) fn resize(&mut self, w: i16, h: i16) {
        unsafe {
            self.w = w as usize;
            self.h = h as usize;
            if self.drawable != 0 {
                XFreePixmap((*self.dpy).inner, self.drawable);
            }
            self.drawable = XCreatePixmap(
                (*self.dpy).inner,
                self.root,
                w as u32,
                h as u32,
                XDefaultDepth((*self.dpy).inner, self.screen) as u32,
            );
        }
    }
}

fn xfont_free(font: *mut Fnt) {
    if font.is_null() {
        return;
    }
    unsafe {
        if !(*font).pattern.is_null() {
            FcPatternDestroy((*font).pattern.cast());
        }
        XftFontClose((*(*font).dpy).inner, (*font).xfont);
        drop(Box::from_raw(font)); // free
    }
}
