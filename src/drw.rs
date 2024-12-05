use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, CStr, CString};
use std::mem::MaybeUninit;
use std::ptr::null_mut;

use fontconfig_sys as fcfg;
use fontconfig_sys::constants::{FC_CHARSET, FC_SCALABLE};
use fontconfig_sys::{
    FcChar8, FcCharSet, FcCharSetAddChar, FcCharSetCreate, FcMatchPattern,
    FcNameParse, FcPattern, FcPatternDestroy, FcPatternDuplicate,
};
use x11::xft::{self, XftFont};
use x11::xlib::{
    self, CapButt, Display, Drawable, False, JoinMiter, LineSolid, GC,
};

use crate::die;
use crate::enums::Col;
use crate::util::{between, ecalloc};
use crate::Clr;
use rwm::Cursor as Cur;
use rwm::Window;

// defined in drw.c
const UTF_SIZ: usize = 4;
const UTF_INVALID: usize = 0xFFFD;
const UTFBYTE: [c_uchar; UTF_SIZ + 1] = [0x80, 0, 0xC0, 0xE0, 0xF0];
const UTFMASK: [c_uchar; UTF_SIZ + 1] = [0xC0, 0x80, 0xE0, 0xF0, 0xF8];
const UTFMIN: [c_long; UTF_SIZ + 1] = [0, 0, 0x80, 0x800, 0x10000];
const UTFMAX: [c_long; UTF_SIZ + 1] = [0x10FFFF, 0x7F, 0x7FF, 0xFFFF, 0x10FFFF];

// defined in /usr/include/fontconfig/fontconfig.h
const FC_TRUE: i32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Fnt {
    pub dpy: *mut Display,
    pub h: c_uint,
    pub xfont: *mut XftFont,
    pub pattern: *mut FcPattern,
    pub next: *mut Fnt,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Drw {
    pub w: c_uint,
    pub h: c_uint,
    pub dpy: *mut Display,
    pub screen: c_int,
    pub root: Window,
    pub drawable: Drawable,
    pub gc: GC,
    pub scheme: *mut Clr,
    pub fonts: *mut Fnt,
}

fn utf8decodebyte(c: c_char, i: *mut usize) -> c_long {
    unsafe {
        *i = 0;
        while *i < UTF_SIZ + 1 {
            if c as c_uchar & UTFMASK[*i] == UTFBYTE[*i] {
                return (c as c_uchar & !UTFMASK[*i]) as c_long;
            }
            *i += 1;
        }
        0
    }
}

fn utf8validate(u: *mut c_long, i: usize) -> usize {
    unsafe {
        if !between(*u, UTFMIN[i], UTFMAX[i]) || between(*u, 0xD800, 0xDFFF) {
            *u = UTF_INVALID as c_long;
        }
        let mut i = 1;
        while *u > UTFMAX[i] {
            i += 1;
        }
        i
    }
}

fn utf8decode(c: *const i8, u: *mut c_long, clen: usize) -> usize {
    unsafe {
        *u = UTF_INVALID as c_long;
        if clen == 0 {
            return 0;
        }
        let mut len = 0;
        let mut udecoded = utf8decodebyte(*c, &mut len);
        if !between(len, 1, UTF_SIZ) {
            return 1;
        }
        let mut i = 1;
        let mut j = 1;
        let mut type_ = 0;
        while i < clen && j < len {
            udecoded = (udecoded << 6) | utf8decodebyte(*c.add(i), &mut type_);
            if type_ != 0 {
                return j;
            }
            i += 1;
            j += 1;
        }
        if j < len {
            return 0;
        }
        *u = udecoded;
        utf8validate(u, len);

        len
    }
}

pub(crate) fn create(
    dpy: *mut Display,
    screen: c_int,
    root: Window,
    w: c_uint,
    h: c_uint,
) -> Drw {
    unsafe {
        let drw = Drw {
            drawable: xlib::XCreatePixmap(
                dpy,
                root,
                w,
                h,
                xlib::XDefaultDepth(dpy, screen) as u32,
            ),
            w,
            h,
            dpy,
            screen,
            root,
            gc: xlib::XCreateGC(dpy, root, 0, null_mut()),
            scheme: null_mut(),
            fonts: null_mut(),
        };
        xlib::XSetLineAttributes(dpy, drw.gc, 1, LineSolid, CapButt, JoinMiter);
        drw
    }
}

impl Drop for Drw {
    fn drop(&mut self) {
        unsafe {
            xlib::XFreePixmap(self.dpy, self.drawable);
            xlib::XFreeGC(self.dpy, self.gc);
            fontset_free(self.fonts);
        }
    }
}

pub(crate) fn rect(
    drw: Option<&mut Drw>,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
    filled: c_int,
    invert: c_int,
) {
    unsafe {
        let Some(drw) = drw else {
            return;
        };
        if drw.scheme.is_null() {
            return;
        }
        xlib::XSetForeground(
            drw.dpy,
            drw.gc,
            if invert != 0 {
                (*drw.scheme.offset(Col::Bg as isize)).pixel
            } else {
                (*drw.scheme.offset(Col::Fg as isize)).pixel
            },
        );
        if filled != 0 {
            xlib::XFillRectangle(drw.dpy, drw.drawable, drw.gc, x, y, w, h);
        } else {
            xlib::XDrawRectangle(
                drw.dpy,
                drw.drawable,
                drw.gc,
                x,
                y,
                w - 1,
                h - 1,
            );
        }
    }
}

pub(crate) fn cur_create(drw: &mut Drw, shape: c_int) -> *mut Cur {
    unsafe {
        let cur: *mut Cur = crate::util::ecalloc(1, size_of::<Cur>()).cast();
        if cur.is_null() {
            return std::ptr::null_mut();
        }
        (*cur).cursor = xlib::XCreateFontCursor(drw.dpy, shape as c_uint);
        cur
    }
}

pub(crate) fn cur_free(drw: &mut Drw, cursor: *mut Cur) {
    if cursor.is_null() {
        return;
    }

    unsafe {
        xlib::XFreeCursor(drw.dpy, (*cursor).cursor);
        libc::free(cursor.cast());
    }
}

pub(crate) fn setscheme(drw: Option<&mut Drw>, scm: *mut Clr) {
    if let Some(drw) = drw {
        drw.scheme = scm;
    }
}

pub(crate) fn fontset_create(drw: &mut Drw, fonts: &[CString]) -> *mut Fnt {
    log::trace!("fontset_create");
    unsafe {
        let mut ret: *mut Fnt = null_mut();

        // since fonts is a & not a *, it can't be null, but it could be empty
        if fonts.is_empty() {
            return null_mut();
        }

        for font in fonts.iter().rev() {
            let cur = xfont_create(drw, font.as_ptr(), null_mut());
            if !cur.is_null() {
                (*cur).next = ret;
                ret = cur;
            }
        }
        drw.fonts = ret;
        ret
    }
}

fn fontset_free(font: *mut Fnt) {
    if font.is_null() {
        return;
    }
    unsafe {
        fontset_free((*font).next);
        xfont_free(font);
    }
}

fn xfont_create(
    drw: *mut Drw,
    fontname: *const i8,
    fontpattern: *mut FcPattern,
) -> *mut Fnt {
    log::trace!("xfont_create");
    unsafe {
        let xfont: *mut XftFont;
        let mut pattern: *mut FcPattern = null_mut();
        if !fontname.is_null() {
            /* Using the pattern found at font->xfont->pattern does not yield
             * the same substitution results as using the pattern returned by
             * FcNameParse; using the latter results in the desired fallback
             * behaviour whereas the former just results in missing-character
             * rectangles being drawn, at least with some fonts. */
            log::trace!("xfont_create: XftFontOpenName");
            xfont = xft::XftFontOpenName((*drw).dpy, (*drw).screen, fontname);
            if xfont.is_null() {
                eprintln!(
                    "error, cannot load font from name: '{:?}'",
                    CStr::from_ptr(fontname)
                );
                return null_mut();
            }
            log::trace!("xfont_create: FcNameParse");
            pattern = FcNameParse(fontname as *mut FcChar8);
            if pattern.is_null() {
                eprintln!(
                    "error, cannot parse font name to pattern: '{:?}'",
                    CStr::from_ptr(fontname)
                );
                log::trace!("xfont_create: null pattern, XftFontClose");
                xft::XftFontClose((*drw).dpy, xfont);
                return null_mut();
            }
        } else if !fontpattern.is_null() {
            log::trace!("xfont_create: XftFontOpenPattern");
            xfont = xft::XftFontOpenPattern((*drw).dpy, fontpattern.cast());
            if xfont.is_null() {
                eprintln!("error, cannot load font from pattern");
                return null_mut();
            }
        } else {
            die("no font specified");
        }

        // could just Box::into_raw after constructing a Fnt here, I think. the
        // only field not initialized is the next ptr
        let font: *mut Fnt = ecalloc(1, size_of::<Fnt>()).cast();
        (*font).xfont = xfont;
        (*font).pattern = pattern;
        assert!(!xfont.is_null());
        (*font).h = (*xfont).ascent as u32 + (*xfont).descent as u32;
        (*font).dpy = (*drw).dpy;

        log::trace!("returning font: {font:?}");
        font
    }
}

fn xfont_free(font: *mut Fnt) {
    log::trace!("xfont_free");
    if font.is_null() {
        return;
    }
    unsafe {
        if !(*font).pattern.is_null() {
            log::trace!("xfont_free: FcPatternDestroy");
            FcPatternDestroy((*font).pattern.cast());
        }
        log::trace!("xfont_free: XftFontClose");
        xft::XftFontClose((*font).dpy, (*font).xfont);
        log::trace!("xfont_free: free {font:?}");
        libc::free(font.cast());
    }
}

fn clr_create(drw: *mut Drw, dest: *mut Clr, clrname: *const c_char) {
    if drw.is_null() || dest.is_null() || clrname.is_null() {
        return;
    }
    unsafe {
        if xft::XftColorAllocName(
            (*drw).dpy,
            xlib::XDefaultVisual((*drw).dpy, (*drw).screen),
            xlib::XDefaultColormap((*drw).dpy, (*drw).screen),
            clrname,
            dest,
        ) == 0
        {
            die(&format!(
                "error, cannot allocate color '{:?}'",
                CStr::from_ptr(clrname)
            ));
        }
    }
}

pub(crate) fn scm_create(
    drw: &mut Drw,
    clrnames: &[CString],
    clrcount: usize,
) -> *mut Clr {
    if clrnames.is_empty() || clrcount < 2 {
        return null_mut();
    }
    let ret: *mut Clr = ecalloc(clrcount, size_of::<xft::XftColor>()).cast();
    if ret.is_null() {
        return null_mut();
    }
    for (i, clr) in clrnames.iter().enumerate() {
        unsafe {
            clr_create(drw, ret.add(i), clr.as_ptr());
        }
    }
    ret
}

pub(crate) fn fontset_getwidth(
    drw: Option<&mut Drw>,
    text: *const c_char,
) -> c_uint {
    if drw.as_ref().is_none_or(|drw| drw.fonts.is_null()) || text.is_null() {
        return 0;
    }
    self::text(drw, 0, 0, 0, 0, 0, text, 0) as c_uint
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    drw: Option<&mut Drw>,
    mut x: c_int,
    y: c_int,
    mut w: c_uint,
    h: c_uint,
    lpad: c_uint,
    mut text: *const c_char,
    invert: c_int,
) -> c_int {
    // this function is very confusing and likely can be dramatically simplified
    // with Rust's native utf8 handling. For now, I'm declaring all of the
    // variables at the top to match C as much as possible.
    unsafe {
        log::trace!(
            "text: {drw:?}, {x}, {y}, {w}, {h}, {lpad}, {:?}, {invert}",
            std::ffi::CStr::from_ptr(text)
        );
        let mut ty: c_int;
        let mut ellipsis_x: c_int = 0;

        let mut tmpw: c_uint = 0;
        let mut ew: c_uint;
        let mut ellipsis_w: c_uint = 0;
        let mut ellipsis_len: c_uint;

        let mut d: *mut xft::XftDraw = null_mut();

        let mut usedfont: *mut Fnt;
        let mut curfont: *mut Fnt;
        let mut nextfont: *mut Fnt;

        let mut utf8strlen: c_int;
        let mut utf8charlen: c_int;
        let render: c_int = (x != 0 || y != 0 || w != 0 || h != 0) as c_int;

        let mut utf8codepoint: c_long = 0;

        let mut utf8str: *const c_char;

        let mut fccharset: *mut FcCharSet;
        let mut fcpattern: *mut FcPattern;
        let mut match_: *mut FcPattern;

        let mut result: xft::FcResult = xft::FcResult::NoMatch;

        let mut charexists: c_int = 0;
        let mut overflow: c_int = 0;

        // keep track of a couple codepoints for which we have no match
        const NOMATCHES_LEN: usize = 64;
        struct NoMatches {
            codepoint: [c_long; NOMATCHES_LEN],
            idx: usize,
        }
        static mut NOMATCHES: NoMatches =
            NoMatches { codepoint: [0; NOMATCHES_LEN], idx: 0 };
        static mut ELLIPSIS_WIDTH: c_uint = 0;

        let Some(drw) = drw else {
            return 0;
        };

        if (render != 0 && (drw.scheme.is_null() || w == 0))
            || text.is_null()
            || drw.fonts.is_null()
        {
            return 0;
        }

        let drw = &mut *drw;
        if render == 0 {
            w = if invert != 0 { invert } else { !invert } as u32;
        } else {
            log::trace!("text: XSetForeground");
            xlib::XSetForeground(
                drw.dpy,
                drw.gc,
                (*drw
                    .scheme
                    .add(if invert != 0 { Col::Fg } else { Col::Bg } as usize))
                .pixel,
            );
            log::trace!("text: XFillRectangle");
            xlib::XFillRectangle(drw.dpy, drw.drawable, drw.gc, x, y, w, h);
            log::trace!("text: XftDrawCreate");
            d = xft::XftDrawCreate(
                drw.dpy,
                drw.drawable,
                xlib::XDefaultVisual(drw.dpy, drw.screen),
                xlib::XDefaultColormap(drw.dpy, drw.screen),
            );
            x += lpad as i32;
            w -= lpad;
        }

        usedfont = drw.fonts;
        if ELLIPSIS_WIDTH == 0 && render != 0 {
            ELLIPSIS_WIDTH = fontset_getwidth(Some(drw), c"...".as_ptr());
        }
        log::trace!("text: entering loop");
        'no_match: loop {
            ew = 0;
            ellipsis_len = 0;
            utf8strlen = 0;
            utf8str = text;
            nextfont = null_mut();

            // I believe this loop is just walking along the characters in text
            // and computing their width. text += utf8charlen at the end just
            // advances the pointer to the next codepoint, so in Rust I should
            // be able to do something like text.chars() and replace almost all
            // of this.
            //
            // NOTE this was almost an infinite loop because I translated
            // `while(*text)` to `while !text.is_null()`, but we actually need
            // to check if we're at the null byte at the end of the string, NOT
            // if text is a null pointer
            while *text != b'\0' as i8 {
                utf8charlen =
                    utf8decode(text, &mut utf8codepoint, UTF_SIZ) as c_int;
                curfont = drw.fonts;
                while !curfont.is_null() {
                    charexists = (charexists != 0
                        || xft::XftCharExists(
                            drw.dpy,
                            (*curfont).xfont,
                            utf8codepoint as u32,
                        ) != 0) as c_int;
                    if charexists != 0 {
                        font_getexts(
                            curfont,
                            text,
                            utf8charlen as u32,
                            &mut tmpw,
                            null_mut(),
                        );
                        if ew + ELLIPSIS_WIDTH <= w {
                            // keep track where the ellipsis still fits
                            ellipsis_x = x + ew as i32;
                            ellipsis_w = w - ew;
                            ellipsis_len = utf8strlen as c_uint;
                        }

                        if ew + tmpw > w {
                            overflow = 1;
                            // called from drw_fontset_getwidth_clamp():
                            // it wants the width AFTER the overflow
                            if render == 0 {
                                x += tmpw as i32;
                            } else {
                                utf8strlen = ellipsis_len as c_int;
                            }
                        } else if curfont == usedfont {
                            utf8strlen += utf8charlen;
                            text = text.add(utf8charlen as usize);
                            ew += tmpw;
                        } else {
                            nextfont = curfont;
                        }
                        break;
                    }
                    curfont = (*curfont).next;
                }

                if overflow != 0 || charexists == 0 || !nextfont.is_null() {
                    break;
                } else {
                    charexists = 0;
                }
            } // end while(*text)

            log::trace!("text: end loop");

            if utf8strlen != 0 {
                if render != 0 {
                    ty = y
                        + (h as i32 - (*usedfont).h as i32) / 2
                        + (*(*usedfont).xfont).ascent;
                    log::trace!("text: XftDrawStringUtf8");
                    xft::XftDrawStringUtf8(
                        d,
                        drw.scheme.add(if invert != 0 {
                            Col::Bg
                        } else {
                            Col::Fg
                        } as usize),
                        (*usedfont).xfont,
                        x,
                        ty,
                        utf8str as *const c_uchar,
                        utf8strlen,
                    );
                    log::trace!("text: XftDrawStringUtf8 finished");
                }
                x += ew as i32;
                w -= ew;
            }
            if render != 0 && overflow != 0 {
                self::text(
                    Some(drw),
                    ellipsis_x,
                    y,
                    ellipsis_w,
                    h,
                    0,
                    c"...".as_ptr(),
                    invert,
                );
            }

            if *text == b'\0' as i8 || overflow != 0 {
                break;
            } else if !nextfont.is_null() {
                charexists = 0;
                usedfont = nextfont;
            } else {
                // regardless of whether or not a fallback font is found, the
                // character must be drawn
                charexists = 1;

                for i in 0..NOMATCHES_LEN {
                    // avoid calling XftFontMatch if we know we won't find a
                    // match
                    if utf8codepoint == NOMATCHES.codepoint[i] {
                        // goto no_match
                        usedfont = drw.fonts;
                        continue 'no_match;
                    }
                }

                log::trace!("text: FcCharSetCreate");
                fccharset = FcCharSetCreate();
                log::trace!("text: FcCharSetAddChar: {utf8codepoint}");
                FcCharSetAddChar(fccharset, utf8codepoint as u32);

                if (*drw.fonts).pattern.is_null() {
                    // refer to the comment in xfont_create for more information
                    die("the first font in the cache must be loaded from a font string");
                }

                log::trace!("text: FcPatternDuplicate");
                fcpattern = FcPatternDuplicate((*drw.fonts).pattern);
                log::trace!("text: FcPatternAddCharSet");
                fcfg::FcPatternAddCharSet(
                    fcpattern,
                    // cast &[u8] to *u8 and then to *i8, hopefully okay
                    FC_CHARSET.as_ptr() as *const _,
                    fccharset,
                );
                log::trace!("text: FcPatternAddBool");
                fcfg::FcPatternAddBool(
                    fcpattern,
                    // same as above: &[u8] -> *u8 -> *i8
                    FC_SCALABLE.as_ptr() as *const _,
                    FC_TRUE,
                );

                log::trace!("text: FcConfigSubstitute");
                fcfg::FcConfigSubstitute(null_mut(), fcpattern, FcMatchPattern);
                log::trace!("text: FcDefaultSubstitute");
                fcfg::FcDefaultSubstitute(fcpattern);
                log::trace!("text: XftFontMatch");
                match_ = xft::XftFontMatch(
                    drw.dpy,
                    drw.screen,
                    fcpattern.cast(),
                    &mut result,
                )
                .cast();

                log::trace!("text: FcCharSetDestroy");
                fcfg::FcCharSetDestroy(fccharset);
                log::trace!("text: FcPatternDestroy");
                fcfg::FcPatternDestroy(fcpattern);

                if !match_.is_null() {
                    log::trace!("text: xfont_create");
                    usedfont = xfont_create(drw, null_mut(), match_);
                    log::trace!("text: XftCharExists");
                    if !usedfont.is_null()
                        && xft::XftCharExists(
                            drw.dpy,
                            (*usedfont).xfont,
                            utf8codepoint as u32,
                        ) != 0
                    {
                        curfont = drw.fonts;
                        while !(*curfont).next.is_null() {
                            curfont = (*curfont).next;
                        }
                        (*curfont).next = usedfont;
                    } else {
                        xfont_free(usedfont);
                        NOMATCHES.idx += 1;
                        NOMATCHES.codepoint[NOMATCHES.idx % NOMATCHES_LEN] =
                            utf8codepoint;
                        // no_match label
                        usedfont = drw.fonts;
                    }
                }
            }
        }
        if !d.is_null() {
            log::trace!("text: XftDrawDestroy");
            xft::XftDrawDestroy(d);
        }

        x + if render != 0 { w } else { 0 } as i32
    }
}

fn font_getexts(
    font: *mut Fnt,
    text: *const i8,
    len: u32,
    w: *mut c_uint,
    h: *mut c_uint,
) {
    unsafe {
        if font.is_null() || text.is_null() {
            return;
        }
        let mut ext = MaybeUninit::uninit();
        xft::XftTextExtentsUtf8(
            (*font).dpy,
            (*font).xfont,
            text as *const c_uchar,
            len as i32,
            ext.as_mut_ptr(),
        );
        let ext = ext.assume_init();
        if !w.is_null() {
            *w = ext.xOff as u32;
        }
        if !h.is_null() {
            *h = (*font).h;
        }
    }
}

pub(crate) fn map(
    drw: Option<&mut Drw>,
    win: Window,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
) {
    let Some(drw) = drw else {
        return;
    };
    unsafe {
        xlib::XCopyArea(drw.dpy, drw.drawable, win, drw.gc, x, y, w, h, x, y);
        xlib::XSync(drw.dpy, False);
    }
}

pub(crate) fn resize(drw: Option<&mut Drw>, w: c_uint, h: c_uint) {
    unsafe {
        let Some(drw) = drw else {
            return;
        };
        drw.w = w;
        drw.h = h;
        if drw.drawable != 0 {
            xlib::XFreePixmap(drw.dpy, drw.drawable);
        }
        drw.drawable = xlib::XCreatePixmap(
            drw.dpy,
            drw.root,
            w,
            h,
            xlib::XDefaultDepth(drw.dpy, drw.screen) as c_uint,
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_utf8decode() {
        let tests = [
            (c"GNU Emacs at omsf", 71, 1),
            (c"NU Emacs at omsf", 78, 1),
            (c"U Emacs at omsf", 85, 1),
            (c"🕔", 0x1f554, 4),
        ];

        for (inp, want_u, ret) in tests {
            let mut u = 0;
            let got = super::utf8decode(inp.as_ptr(), &mut u, super::UTF_SIZ);
            assert_eq!(got, ret);
            assert_eq!(u, want_u);
        }
    }
}
