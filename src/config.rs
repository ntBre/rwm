use std::{
    error::Error,
    ffi::{c_float, c_int, c_uint, CStr, CString},
    path::Path,
    ptr::{null, null_mut},
    sync::LazyLock,
};

use fig::{Fig, Value};
use libc::c_char;
use x11::xlib::{Button1, Button2, Button3, ControlMask, Mod4Mask, ShiftMask};

use crate::{
    enums::{Clk, Scheme},
    key_handlers::*,
    layouts::{monocle, tile},
};
use rwm::{Arg, Button, Key, Layout, Rule};

impl Default for Config {
    fn default() -> Self {
        Self {
            borderpx: 3,
            snap: 32,
            showbar: true,
            topbar: true,
            mfact: 0.5,
            nmaster: 1,
            resize_hints: false,
            lock_fullscreen: true,
            fonts: vec![c"monospace:size=10".into()],
            tags: [c"1", c"2", c"3", c"4", c"5", c"6", c"7", c"8", c"9"]
                .map(CString::from)
                .to_vec(),
            colors: default_colors(),
        }
    }
}

pub struct Config {
    /// Border pixel of windows
    pub borderpx: c_uint,

    /// Snap pixel
    pub snap: c_uint,

    /// Whether to show the bar
    pub showbar: bool,

    /// Whether to show the bar at the top or bottom
    pub topbar: bool,

    /// Factor of master area size [0.05..0.95]
    pub mfact: c_float,

    /// Number of clients in master area
    pub nmaster: c_int,

    /// Respect size hints in tiled resizals
    pub resize_hints: bool,

    /// Force focus on the fullscreen window
    pub lock_fullscreen: bool,

    pub fonts: Vec<CString>,

    pub tags: Vec<CString>,

    pub colors: [[CString; 3]; 2],
}

fn get(
    v: &mut std::collections::HashMap<String, fig::Value>,
    name: &str,
) -> Result<Value, String> {
    v.remove(name).ok_or(format!("failed to find {name}"))
}

fn get_colors(
    v: &mut std::collections::HashMap<String, fig::Value>,
) -> Result<[[CString; 3]; 2], Box<dyn Error>> {
    let colors = get(v, "colors")?;
    let mut colors =
        colors.try_into_map().map_err(|_| "colors must be a map")?;
    let norm = get(&mut colors, "SchemeNorm")?
        .try_into_list()
        .map_err(|_| "SchemeNorm must be a list")?;
    let sel = get(&mut colors, "SchemeSel")?
        .try_into_list()
        .map_err(|_| "SchemeSel must be a list")?;
    let mut ret = default_colors();
    let str_err = |_| "failed to convert color";
    let [n0, n1, n2] = &norm[..] else {
        return Err("not enough colors for SchemeNorm".into());
    };
    ret[Scheme::Norm as usize] = [
        CString::new(n0.clone().try_into_str().map_err(str_err)?)?,
        CString::new(n1.clone().try_into_str().map_err(str_err)?)?,
        CString::new(n2.clone().try_into_str().map_err(str_err)?)?,
    ];
    let [s0, s1, s2] = &sel[..] else {
        return Err("not enough colors for SchemeSel".into());
    };
    ret[Scheme::Sel as usize] = [
        CString::new(s0.clone().try_into_str().map_err(str_err)?)?,
        CString::new(s1.clone().try_into_str().map_err(str_err)?)?,
        CString::new(s2.clone().try_into_str().map_err(str_err)?)?,
    ];
    Ok(ret)
}

impl TryFrom<Fig> for Config {
    type Error = Box<dyn Error>;

    fn try_from(Fig { variables: mut v }: Fig) -> Result<Self, Self::Error> {
        let float = |val: fig::Value| {
            val.as_float().cloned().ok_or("unable to parse number")
        };
        let int = |val: fig::Value| {
            val.try_into_int().map_err(|_| "unable to parse int")
        };
        let bool = |val: fig::Value| {
            val.as_bool().cloned().ok_or("unable to parse bool")
        };
        let str_list = |val: fig::Value| {
            val.as_list()
                .cloned()
                .ok_or("unable to parse list")
                .map(|vec| {
                    vec.into_iter()
                        .map(|v| {
                            v.as_str().cloned().map(|s| {
                                CString::new(s)
                                    .expect("failed to convert to CString")
                            })
                        })
                        .collect::<Option<Vec<CString>>>()
                        .ok_or("unable to convert to strings")
                })?
        };
        Ok(Self {
            borderpx: int(get(&mut v, "borderpx")?)? as c_uint,
            snap: int(get(&mut v, "snap")?)? as c_uint,
            showbar: bool(get(&mut v, "showbar")?)?,
            topbar: bool(get(&mut v, "topbar")?)?,
            mfact: float(get(&mut v, "mfact")?)?,
            nmaster: int(get(&mut v, "nmaster")?)? as c_int,
            resize_hints: bool(get(&mut v, "resize_hints")?)?,
            lock_fullscreen: bool(get(&mut v, "lock_fullscreen")?)?,
            fonts: str_list(get(&mut v, "fonts")?)?,
            tags: str_list(get(&mut v, "tags")?)?,
            colors: get_colors(&mut v)?,
        })
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(path)?;
        let f = fig::Fig::parse(&s)?;
        Self::try_from(f)
    }
}

/// Attempt to load a config file on first usage from `$XDG_CONFIG_HOME`, then
/// `$HOME`, before falling back to the default config.
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let mut home = std::env::var("XDG_CONFIG_HOME");
    if home.is_err() {
        home = std::env::var("HOME");
    }
    if home.is_err() {
        log::warn!("unable to determine config directory");
        return Config::default();
    }
    let config_path = Path::new(&home.unwrap())
        .join(".config")
        .join("rwm")
        .join("config.fig");

    Config::load(config_path).unwrap_or_else(|e| {
        log::error!("failed to read config file: {e:?}");
        Config::default()
    })
});

// appearance

const DMENUFONT: &CStr = c"monospace:size=12";

const COL_GRAY1: &CStr = c"#222222";
const COL_GRAY2: &CStr = c"#444444";
const COL_GRAY3: &CStr = c"#bbbbbb";
const COL_GRAY4: &CStr = c"#eeeeee";
const COL_CYAN: &CStr = c"#005577";

fn default_colors() -> [[CString; 3]; 2] {
    let mut ret = std::array::from_fn(|_| {
        std::array::from_fn(|_| CString::new("").unwrap())
    });
    ret[Scheme::Norm as usize] =
        [COL_GRAY3, COL_GRAY1, COL_GRAY2].map(CString::from);
    ret[Scheme::Sel as usize] =
        [COL_GRAY4, COL_CYAN, COL_CYAN].map(CString::from);
    ret
}

pub const RULES: [Rule; 2] = [
    Rule::new(c"Gimp", null(), null(), 0, 1, -1),
    Rule::new(c"Firefox", null(), null(), 1 << 8, 0, -1),
];

// layouts

pub const LAYOUTS: [Layout; 3] = [
    Layout { symbol: c"[]=".as_ptr(), arrange: Some(tile) },
    Layout { symbol: c"><>".as_ptr(), arrange: None },
    Layout { symbol: c"[M]".as_ptr(), arrange: Some(monocle) },
];

// key definitions
pub const MODKEY: c_uint = Mod4Mask;

/// This type is needed just to implement Sync for this raw pointer
pub struct DmenuCmd(pub [*const c_char; 14]);

unsafe impl Sync for DmenuCmd {}

pub static mut DMENUMON: [c_char; 2] = ['0' as c_char, '\0' as c_char];

// commands
pub static DMENUCMD: DmenuCmd = DmenuCmd([
    c"dmenu_run".as_ptr(),
    c"-m".as_ptr(),
    unsafe { DMENUMON.as_ptr() },
    c"-fn".as_ptr(),
    DMENUFONT.as_ptr(),
    c"-nb".as_ptr(),
    COL_GRAY1.as_ptr(),
    c"-nf".as_ptr(),
    COL_GRAY3.as_ptr(),
    c"-sb".as_ptr(),
    COL_CYAN.as_ptr(),
    c"-sf".as_ptr(),
    COL_GRAY4.as_ptr(),
    null_mut(),
]);
pub const TERMCMD: [*const c_char; 2] = [c"st".as_ptr(), null_mut()];

use x11::keysym::{
    XK_Return, XK_Tab, XK_b, XK_c, XK_comma, XK_d, XK_f, XK_h, XK_i, XK_j,
    XK_k, XK_l, XK_m, XK_p, XK_period, XK_q, XK_space, XK_t, XK_0, XK_1, XK_2,
    XK_3, XK_4, XK_5, XK_6, XK_7, XK_8, XK_9,
};

pub static KEYS: [Key; 60] = [
    Key::new(MODKEY, XK_p, spawn, Arg { v: DMENUCMD.0.as_ptr().cast() }),
    Key::new(
        MODKEY | ShiftMask,
        XK_Return,
        spawn,
        Arg { v: TERMCMD.as_ptr().cast() },
    ),
    Key::new(MODKEY, XK_b, togglebar, Arg { i: 0 }),
    Key::new(MODKEY, XK_j, focusstack, Arg { i: 1 }),
    Key::new(MODKEY, XK_k, focusstack, Arg { i: -1 }),
    Key::new(MODKEY, XK_i, incnmaster, Arg { i: 1 }),
    Key::new(MODKEY, XK_d, incnmaster, Arg { i: -1 }),
    Key::new(MODKEY, XK_h, setmfact, Arg { f: -0.05 }),
    Key::new(MODKEY, XK_l, setmfact, Arg { f: 0.05 }),
    Key::new(MODKEY, XK_Return, zoom, Arg { i: 0 }),
    Key::new(MODKEY, XK_Tab, view, Arg { i: 0 }),
    Key::new(MODKEY | ShiftMask, XK_c, killclient, Arg { i: 0 }),
    Key::new(
        MODKEY,
        XK_t,
        setlayout,
        Arg { v: &LAYOUTS[0] as *const _ as *const _ },
    ),
    Key::new(
        MODKEY,
        XK_f,
        setlayout,
        Arg { v: &LAYOUTS[1] as *const _ as *const _ },
    ),
    Key::new(
        MODKEY,
        XK_m,
        setlayout,
        Arg { v: &LAYOUTS[2] as *const _ as *const _ },
    ),
    Key::new(MODKEY, XK_space, setlayout, Arg { i: 0 }),
    Key::new(MODKEY | ShiftMask, XK_space, togglefloating, Arg { i: 0 }),
    Key::new(MODKEY, XK_0, view, Arg { ui: !0 }),
    Key::new(MODKEY | ShiftMask, XK_0, tag, Arg { ui: !0 }),
    Key::new(MODKEY, XK_comma, focusmon, Arg { i: -1 }),
    Key::new(MODKEY, XK_period, focusmon, Arg { i: 1 }),
    Key::new(MODKEY | ShiftMask, XK_comma, tagmon, Arg { i: -1 }),
    Key::new(MODKEY | ShiftMask, XK_period, tagmon, Arg { i: 1 }),
    Key::new(MODKEY, XK_1, view, Arg { ui: 1 << 0 }),
    Key::new(MODKEY | ControlMask, XK_1, toggleview, Arg { ui: 1 << 0 }),
    Key::new(MODKEY | ShiftMask, XK_1, tag, Arg { ui: 1 << 0 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_1,
        toggletag,
        Arg { ui: 1 << 0 },
    ),
    Key::new(MODKEY, XK_2, view, Arg { ui: 1 << 1 }),
    Key::new(MODKEY | ControlMask, XK_2, toggleview, Arg { ui: 1 << 1 }),
    Key::new(MODKEY | ShiftMask, XK_2, tag, Arg { ui: 1 << 1 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_2,
        toggletag,
        Arg { ui: 1 << 1 },
    ),
    Key::new(MODKEY, XK_3, view, Arg { ui: 1 << 2 }),
    Key::new(MODKEY | ControlMask, XK_3, toggleview, Arg { ui: 1 << 2 }),
    Key::new(MODKEY | ShiftMask, XK_3, tag, Arg { ui: 1 << 2 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_3,
        toggletag,
        Arg { ui: 1 << 2 },
    ),
    Key::new(MODKEY, XK_4, view, Arg { ui: 1 << 3 }),
    Key::new(MODKEY | ControlMask, XK_4, toggleview, Arg { ui: 1 << 3 }),
    Key::new(MODKEY | ShiftMask, XK_4, tag, Arg { ui: 1 << 3 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_4,
        toggletag,
        Arg { ui: 1 << 3 },
    ),
    Key::new(MODKEY, XK_5, view, Arg { ui: 1 << 4 }),
    Key::new(MODKEY | ControlMask, XK_5, toggleview, Arg { ui: 1 << 4 }),
    Key::new(MODKEY | ShiftMask, XK_5, tag, Arg { ui: 1 << 4 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_5,
        toggletag,
        Arg { ui: 1 << 4 },
    ),
    Key::new(MODKEY, XK_6, view, Arg { ui: 1 << 5 }),
    Key::new(MODKEY | ControlMask, XK_6, toggleview, Arg { ui: 1 << 5 }),
    Key::new(MODKEY | ShiftMask, XK_6, tag, Arg { ui: 1 << 5 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_6,
        toggletag,
        Arg { ui: 1 << 5 },
    ),
    Key::new(MODKEY, XK_7, view, Arg { ui: 1 << 6 }),
    Key::new(MODKEY | ControlMask, XK_7, toggleview, Arg { ui: 1 << 6 }),
    Key::new(MODKEY | ShiftMask, XK_7, tag, Arg { ui: 1 << 6 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_7,
        toggletag,
        Arg { ui: 1 << 6 },
    ),
    Key::new(MODKEY, XK_8, view, Arg { ui: 1 << 7 }),
    Key::new(MODKEY | ControlMask, XK_8, toggleview, Arg { ui: 1 << 7 }),
    Key::new(MODKEY | ShiftMask, XK_8, tag, Arg { ui: 1 << 7 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_8,
        toggletag,
        Arg { ui: 1 << 7 },
    ),
    Key::new(MODKEY, XK_9, view, Arg { ui: 1 << 8 }),
    Key::new(MODKEY | ControlMask, XK_9, toggleview, Arg { ui: 1 << 8 }),
    Key::new(MODKEY | ShiftMask, XK_9, tag, Arg { ui: 1 << 8 }),
    Key::new(
        MODKEY | ControlMask | ShiftMask,
        XK_9,
        toggletag,
        Arg { ui: 1 << 8 },
    ),
    Key::new(MODKEY | ShiftMask, XK_q, quit, Arg { i: 0 }),
];

// button definitions

pub static BUTTONS: [Button; 11] = [
    Button::new(Clk::LtSymbol, 0, Button1, setlayout, Arg { i: 0 }),
    Button::new(
        Clk::LtSymbol,
        0,
        Button3,
        setlayout,
        Arg { v: &LAYOUTS[2] as *const _ as *const _ },
    ),
    Button::new(Clk::WinTitle, 0, Button2, zoom, Arg { i: 0 }),
    Button::new(
        Clk::StatusText,
        0,
        Button2,
        spawn,
        Arg { v: TERMCMD.as_ptr().cast() },
    ),
    Button::new(Clk::ClientWin, MODKEY, Button1, movemouse, Arg { i: 0 }),
    Button::new(
        Clk::ClientWin,
        MODKEY,
        Button2,
        togglefloating,
        Arg { i: 0 },
    ),
    Button::new(Clk::ClientWin, MODKEY, Button3, resizemouse, Arg { i: 0 }),
    Button::new(Clk::TagBar, 0, Button1, view, Arg { i: 0 }),
    Button::new(Clk::TagBar, 0, Button3, toggleview, Arg { i: 0 }),
    Button::new(Clk::TagBar, MODKEY, Button1, tag, Arg { i: 0 }),
    Button::new(Clk::TagBar, MODKEY, Button3, toggletag, Arg { i: 0 }),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config() {
        Config::load("example.fig").unwrap();
    }
}
