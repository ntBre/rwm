use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, CStr};
use std::mem::MaybeUninit;
use std::ptr::null_mut;

use fontconfig_sys::FcMatchPattern;
use x11::xlib::{CapButt, False, JoinMiter, LineSolid};

use crate::bindgen::{
    self, Clr, Cur, Display, Drw, FcChar8, FcCharSet, FcPattern, Fnt, Window,
    XftFont,
};
use crate::die;
use crate::enums::Col;
use crate::util::{between, ecalloc};

// defined in drw.c
const UTF_SIZ: usize = 4;
const UTF_INVALID: usize = 0xFFFD;
const UTFBYTE: [c_uchar; UTF_SIZ + 1] = [0x80, 0, 0xC0, 0xE0, 0xF0];
const UTFMASK: [c_uchar; UTF_SIZ + 1] = [0xC0, 0x80, 0xE0, 0xF0, 0xF8];
const UTFMIN: [c_long; UTF_SIZ + 1] = [0, 0, 0x80, 0x800, 0x10000];
const UTFMAX: [c_long; UTF_SIZ + 1] = [0x10FFFF, 0x7F, 0x7FF, 0xFFFF, 0x10FFFF];

// defined in /usr/include/fontconfig/fontconfig.h
const FC_TRUE: i32 = 1;

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
) -> *mut Drw {
    unsafe {
        let drw: *mut Drw = crate::util::ecalloc(1, size_of::<Drw>()).cast();
        (*drw).dpy = dpy;
        (*drw).screen = screen;
        (*drw).root = root;
        (*drw).w = w;
        (*drw).h = h;
        (*drw).drawable = bindgen::XCreatePixmap(
            dpy,
            root,
            w,
            h,
            bindgen::XDefaultDepth(dpy, screen) as u32,
        );
        (*drw).gc = bindgen::XCreateGC(dpy, root, 0, std::ptr::null_mut());
        bindgen::XSetLineAttributes(
            dpy,
            (*drw).gc,
            1,
            LineSolid as i32,
            CapButt as i32,
            JoinMiter as i32,
        );
        drw
    }
}

pub(crate) fn free(drw: *mut Drw) {
    unsafe {
        bindgen::XFreePixmap((*drw).dpy, (*drw).drawable);
        bindgen::XFreeGC((*drw).dpy, (*drw).gc);
        fontset_free((*drw).fonts);
        libc::free(drw.cast());
    }
}

pub(crate) fn rect(
    drw: *mut Drw,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
    filled: c_int,
    invert: c_int,
) {
    unsafe {
        if drw.is_null() || (*drw).scheme.is_null() {
            return;
        }
        bindgen::XSetForeground(
            (*drw).dpy,
            (*drw).gc,
            if invert != 0 {
                (*(*drw).scheme.offset(Col::Bg as isize)).pixel
            } else {
                (*(*drw).scheme.offset(Col::Fg as isize)).pixel
            },
        );
        if filled != 0 {
            bindgen::XFillRectangle(
                (*drw).dpy,
                (*drw).drawable,
                (*drw).gc,
                x,
                y,
                w,
                h,
            );
        } else {
            bindgen::XDrawRectangle(
                (*drw).dpy,
                (*drw).drawable,
                (*drw).gc,
                x,
                y,
                w - 1,
                h - 1,
            );
        }
    }
}

pub(crate) fn cur_create(drw: *mut Drw, shape: c_int) -> *mut Cur {
    if drw.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let cur: *mut Cur = crate::util::ecalloc(1, size_of::<Cur>()).cast();
        if cur.is_null() {
            return std::ptr::null_mut();
        }
        (*cur).cursor = bindgen::XCreateFontCursor((*drw).dpy, shape as c_uint);
        cur
    }
}

pub(crate) fn cur_free(drw: *mut Drw, cursor: *mut Cur) {
    if cursor.is_null() {
        return;
    }

    unsafe {
        bindgen::XFreeCursor((*drw).dpy, (*cursor).cursor);
        libc::free(cursor.cast());
    }
}

pub(crate) fn setscheme(drw: *mut Drw, scm: *mut Clr) {
    if !drw.is_null() {
        unsafe {
            (*drw).scheme = scm;
        }
    }
}

pub(crate) fn fontset_create(
    drw: *mut Drw,
    fonts: &mut [*const c_char],
    fontcount: usize,
) -> *mut Fnt {
    unsafe {
        let mut ret: *mut Fnt = null_mut();

        // since fonts is a & not a *, it can't be null, but it could be empty
        if drw.is_null() || fonts.is_empty() {
            return null_mut();
        }

        for i in 1..=fontcount {
            let cur = xfont_create(drw, fonts[fontcount - i], null_mut());
            if !cur.is_null() {
                (*cur).next = ret;
                ret = cur;
            }
        }
        (*drw).fonts = ret;
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
    unsafe {
        let mut xfont: *mut XftFont = null_mut();
        let mut pattern: *mut FcPattern = null_mut();
        if !fontname.is_null() {
            /* Using the pattern found at font->xfont->pattern does not yield
             * the same substitution results as using the pattern returned by
             * FcNameParse; using the latter results in the desired fallback
             * behaviour whereas the former just results in missing-character
             * rectangles being drawn, at least with some fonts. */
            xfont =
                bindgen::XftFontOpenName((*drw).dpy, (*drw).screen, fontname);
            if xfont.is_null() {
                eprintln!(
                    "error, cannot load font from name: '{:?}'",
                    CStr::from_ptr(fontname)
                );
                return null_mut();
            }
            pattern = bindgen::FcNameParse(fontname as *mut FcChar8);
            if pattern.is_null() {
                eprintln!(
                    "error, cannot parse font name to pattern: '{:?}'",
                    CStr::from_ptr(fontname)
                );
                bindgen::XftFontClose((*drw).dpy, xfont);
                return null_mut();
            }
        } else if !fontpattern.is_null() {
            let xfont = bindgen::XftFontOpenPattern((*drw).dpy, fontpattern);
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
        (*font).h = (*xfont).ascent as u32 + (*xfont).descent as u32;
        (*font).dpy = (*drw).dpy;

        font
    }
}

fn xfont_free(font: *mut Fnt) {
    if font.is_null() {
        return;
    }
    unsafe {
        if !(*font).pattern.is_null() {
            bindgen::FcPatternDestroy((*font).pattern);
        }
        bindgen::XftFontClose((*font).dpy, (*font).xfont);
        libc::free(font.cast());
    }
}

fn clr_create(drw: *mut Drw, dest: *mut Clr, clrname: *const c_char) {
    if drw.is_null() || dest.is_null() || clrname.is_null() {
        return;
    }
    unsafe {
        if bindgen::XftColorAllocName(
            (*drw).dpy,
            bindgen::XDefaultVisual((*drw).dpy, (*drw).screen),
            bindgen::XDefaultColormap((*drw).dpy, (*drw).screen),
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
    drw: *mut Drw,
    clrnames: &mut [*const c_char],
    clrcount: usize,
) -> *mut Clr {
    if drw.is_null() || clrnames.is_empty() || clrcount < 2 {
        return null_mut();
    }
    let ret: *mut Clr =
        ecalloc(clrcount, size_of::<bindgen::XftColor>()).cast();
    if ret.is_null() {
        return null_mut();
    }
    for i in 0..clrcount {
        unsafe {
            clr_create(drw, ret.add(i), clrnames[i]);
        }
    }
    ret
}

pub(crate) fn fontset_getwidth(drw: *mut Drw, text: *const c_char) -> c_uint {
    unsafe {
        if drw.is_null() || (*drw).fonts.is_null() || text.is_null() {
            return 0;
        }
    }
    self::text(drw, 0, 0, 0, 0, 0, text, 0) as c_uint
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    drw: *mut Drw,
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

        let mut d: *mut bindgen::XftDraw = null_mut();

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

        let mut result: bindgen::XftResult = 0;

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

        if drw.is_null()
            || (render != 0 && ((*drw).scheme.is_null() || w == 0))
            || text.is_null()
            || (*drw).fonts.is_null()
        {
            return 0;
        }

        let drw = &mut *drw;
        if render == 0 {
            w = if invert != 0 { invert } else { !invert } as u32;
        } else {
            bindgen::XSetForeground(
                drw.dpy,
                drw.gc,
                (*drw
                    .scheme
                    .add(if invert != 0 { Col::Fg } else { Col::Bg } as usize))
                .pixel,
            );
            bindgen::XFillRectangle(drw.dpy, drw.drawable, drw.gc, x, y, w, h);
            d = bindgen::XftDrawCreate(
                drw.dpy,
                drw.drawable,
                bindgen::XDefaultVisual(drw.dpy, drw.screen),
                bindgen::XDefaultColormap(drw.dpy, drw.screen),
            );
            x += lpad as i32;
            w -= lpad;
        }

        usedfont = drw.fonts;
        if ELLIPSIS_WIDTH == 0 && render != 0 {
            ELLIPSIS_WIDTH = fontset_getwidth(drw, c"...".as_ptr());
        }
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
                        || bindgen::XftCharExists(
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

            if utf8strlen != 0 {
                if render != 0 {
                    ty = y
                        + (h - (*usedfont).h) as i32 / 2
                        + (*(*usedfont).xfont).ascent;
                    bindgen::XftDrawStringUtf8(
                        d,
                        drw.scheme.add(if invert != 0 {
                            Col::Bg
                        } else {
                            Col::Fg
                        } as usize),
                        (*usedfont).xfont,
                        x,
                        ty,
                        utf8str as *const bindgen::XftChar8,
                        utf8strlen,
                    );
                }
                x += ew as i32;
                w -= ew;
            }
            if render != 0 && overflow != 0 {
                self::text(
                    drw,
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

                fccharset = bindgen::FcCharSetCreate();
                bindgen::FcCharSetAddChar(fccharset, utf8codepoint as u32);

                if (*drw.fonts).pattern.is_null() {
                    // refer to the comment in xfont_create for more information
                    die("the first font in the cache must be loaded from a font string");
                }

                fcpattern = bindgen::FcPatternDuplicate((*drw.fonts).pattern);
                bindgen::FcPatternAddCharSet(
                    fcpattern,
                    // cast &[u8] to *u8 and then to *i8, hopefully okay
                    bindgen::FC_CHARSET.as_ptr() as *const _,
                    fccharset,
                );
                bindgen::FcPatternAddBool(
                    fcpattern,
                    // same as above: &[u8] -> *u8 -> *i8
                    bindgen::FC_SCALABLE.as_ptr() as *const _,
                    FC_TRUE,
                );

                bindgen::FcConfigSubstitute(
                    null_mut(),
                    fcpattern,
                    FcMatchPattern,
                );
                bindgen::FcDefaultSubstitute(fcpattern);
                match_ = bindgen::XftFontMatch(
                    drw.dpy,
                    drw.screen,
                    fcpattern,
                    &mut result,
                );

                bindgen::FcCharSetDestroy(fccharset);
                bindgen::FcPatternDestroy(fcpattern);

                if !match_.is_null() {
                    usedfont = xfont_create(drw, null_mut(), match_);
                    if !usedfont.is_null()
                        && bindgen::XftCharExists(
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
            bindgen::XftDrawDestroy(d);
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
        bindgen::XftTextExtentsUtf8(
            (*font).dpy,
            (*font).xfont,
            text as *const bindgen::XftChar8,
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
    drw: *mut Drw,
    win: Window,
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
) {
    if drw.is_null() {
        return;
    }
    unsafe {
        bindgen::XCopyArea(
            (*drw).dpy,
            (*drw).drawable,
            win,
            (*drw).gc,
            x,
            y,
            w,
            h,
            x,
            y,
        );
        bindgen::XSync((*drw).dpy, False);
    }
}

pub(crate) fn resize(drw: *mut Drw, w: c_uint, h: c_uint) {
    unsafe {
        if drw.is_null() {
            return;
        }
        (*drw).w = w;
        (*drw).h = h;
        if (*drw).drawable != 0 {
            bindgen::XFreePixmap((*drw).dpy, (*drw).drawable);
        }
        (*drw).drawable = bindgen::XCreatePixmap(
            (*drw).dpy,
            (*drw).root,
            w,
            h,
            bindgen::XDefaultDepth((*drw).dpy, (*drw).screen) as c_uint,
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
        ];

        for (inp, want_u, ret) in tests {
            let mut u = 0;
            let got = super::utf8decode(inp.as_ptr(), &mut u, super::UTF_SIZ);
            assert_eq!(got, ret);
            assert_eq!(u, want_u);
        }
    }
}
