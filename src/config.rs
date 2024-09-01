use std::{
    collections::HashMap,
    error::Error,
    ffi::{c_float, c_int, c_uint, CString},
    path::Path,
    ptr::null,
    sync::LazyLock,
};

use fig::{Fig, FigError, Value};
use key::{get_arg, FUNC_MAP};
use libc::c_char;
use x11::keysym::{
    XK_Return, XK_Tab, XK_b, XK_c, XK_comma, XK_d, XK_f, XK_grave, XK_h, XK_i,
    XK_j, XK_k, XK_l, XK_m, XK_p, XK_period, XK_q, XK_space, XK_t, XK_0, XK_1,
    XK_2, XK_3, XK_4, XK_5, XK_6, XK_7, XK_8, XK_9,
};

use x11::xlib::{Button1, Button2, Button3, ControlMask, Mod4Mask, ShiftMask};

use crate::{
    config::key::{conv, Key},
    enums::{Clk, Scheme},
    key_handlers::*,
    layouts::{monocle, tile},
};
use rwm::{Arg, Button, Layout, Monitor, Rule};

mod fig_env;
pub mod key;

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
            keys: default_keys().to_vec(),
            dmenucmd: DMENUCMD.to_vec(),
            rules: RULES.to_vec(),
            swallowfloating: SWALLOWFLOATING,
            systraypinning: SYSTRAYPINNING,
            systrayonleft: SYSTRAYONLEFT,
            systrayspacing: SYSTRAYSPACING,
            systraypinningfailfirst: SYSTRAYPINNINGFAILFIRST,
            showsystray: SHOWSYSTRAY,
            buttons: BUTTONS.to_vec(),
            layouts: LAYOUTS.to_vec(),
            scratchpadname: SCRATCHPADNAME.into(),
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

    pub keys: Vec<Key>,

    pub dmenucmd: Vec<String>,

    pub rules: Vec<Rule>,

    /// Swallow floating windows by default
    pub swallowfloating: bool,

    /// 0: sloppy systray follows selected monitor, >0: pin systray to monitor x
    pub systraypinning: c_uint,

    pub systrayonleft: bool,

    pub systrayspacing: c_uint,

    /// if pinning fails and this is true, display systray on the first monitor,
    /// else display systray on the last monitor
    pub systraypinningfailfirst: bool,

    pub showsystray: bool,

    pub buttons: Vec<Button>,

    pub layouts: Vec<Layout>,

    pub scratchpadname: String,
}

unsafe impl Send for Config {}
unsafe impl Sync for Config {}

fn get(
    v: &mut HashMap<String, fig::Value>,
    name: &str,
) -> Result<Value, String> {
    v.remove(name).ok_or(format!("failed to find {name}"))
}

/// Extract colors from a fig Map<Str, List<Str>>. The two Str keys should be
/// `SchemeNorm` and `SchemeSel`, and the two Lists should be of length 3.
fn get_colors(
    v: &mut HashMap<String, fig::Value>,
) -> Result<[[CString; 3]; 2], Box<dyn Error>> {
    let colors = get(v, "colors")?;
    let mut colors =
        colors.try_into_map().map_err(|_| "colors must be a map")?;
    let norm: Vec<String> = get(&mut colors, "SchemeNorm")?.try_into()?;
    let sel: Vec<String> = get(&mut colors, "SchemeSel")?.try_into()?;
    let mut ret = default_colors();
    let [n0, n1, n2] = &norm[..] else {
        return Err("not enough colors for SchemeNorm".into());
    };
    ret[Scheme::Norm as usize] = [
        CString::new(n0.clone())?,
        CString::new(n1.clone())?,
        CString::new(n2.clone())?,
    ];
    let [s0, s1, s2] = &sel[..] else {
        return Err("not enough colors for SchemeSel".into());
    };
    ret[Scheme::Sel as usize] = [
        CString::new(s0.clone())?,
        CString::new(s1.clone())?,
        CString::new(s2.clone())?,
    ];
    Ok(ret)
}

fn get_keys(v: &mut HashMap<String, Value>) -> Result<Vec<Key>, FigError> {
    let err = Err(FigError::Conversion);
    let Some(keys) = v.get("keys") else {
        log::trace!("failed to get keys from config");
        return err;
    };
    let keys = conv(keys.as_list())?;
    keys.into_iter().map(Key::try_from).collect()
}

fn get_rules(v: &mut HashMap<String, Value>) -> Result<Vec<Rule>, FigError> {
    let err = Err(FigError::Conversion);
    let Some(rules) = v.get("rules") else {
        log::trace!("failed to get rules from config");
        return err;
    };
    let rules: Vec<Value> = conv(rules.as_list())?;

    // NOTE each into_raw call is a leak, but these should live as long as the
    // program runs anyway
    let maybe_string = |val: Value| -> Result<*const c_char, FigError> {
        if let Ok(Ok(s)) = String::try_from(val.clone()).map(CString::new) {
            Ok(s.into_raw())
        } else if val.is_nil() {
            Ok(null())
        } else {
            log::error!("expected Str or Nil");
            Err(FigError::Conversion)
        }
    };

    let mut ret = Vec::new();
    for rule in rules {
        let rule: Vec<Value> = rule.try_into()?;
        if rule.len() != 8 {
            log::error!("invalid rule: {rule:?}");
            return err;
        }
        ret.push(Rule {
            class: maybe_string(rule[0].clone())?,
            instance: maybe_string(rule[1].clone())?,
            title: maybe_string(rule[2].clone())?,
            tags: i64::try_from(rule[3].clone())? as u32,
            isfloating: rule[4].clone().try_into()?,
            isterminal: rule[5].clone().try_into()?,
            noswallow: rule[6].clone().try_into()?,
            monitor: i64::try_from(rule[7].clone())? as i32,
        });
    }

    Ok(ret)
}

fn get_buttons(
    v: &mut HashMap<String, Value>,
) -> Result<Vec<Button>, FigError> {
    let err = Err(FigError::Conversion);
    let Some(buttons) = v.get("buttons") else {
        log::trace!("failed to get buttons from config");
        return err;
    };
    let buttons: Vec<Value> = conv(buttons.as_list())?;

    // each entry should be a list of
    // Clk (int), mask (int), button (int), func (str or nil), arg (Map)
    let mut ret = Vec::new();
    for button in buttons {
        let button: Vec<Value> = button.try_into()?;
        if button.len() != 5 {
            log::error!("Expected 5 fields for button");
            return err;
        }
        let func = match &button[3] {
            Value::Str(s) => match FUNC_MAP.get(s.as_str()) {
                res @ Some(_) => res.cloned(),
                None => {
                    log::error!("unrecognized func name for button: {s}");
                    return err;
                }
            },
            Value::Nil => None,
            _ => {
                log::error!("func field on button should be Str or nil");
                return err;
            }
        };
        ret.push(Button {
            click: i64::try_from(button[0].clone())? as u32,
            mask: i64::try_from(button[1].clone())? as u32,
            button: i64::try_from(button[2].clone())? as u32,
            func,
            arg: get_arg(conv(button[4].as_map())?)?,
        });
    }

    Ok(ret)
}

fn get_layouts(
    v: &mut HashMap<String, Value>,
) -> Result<Vec<Layout>, FigError> {
    let err = Err(FigError::Conversion);
    let Some(layouts) = v.get("layouts") else {
        log::trace!("failed to get layouts from config");
        return err;
    };
    let layouts: Vec<Value> = conv(layouts.as_list())?;

    // each entry should be a list of
    // Clk (int), mask (int), layout (int), func (str or nil), arg (Map)
    let mut ret = Vec::new();
    for layout in layouts {
        let layout: Vec<Value> = layout.try_into()?;
        if layout.len() != 2 {
            log::error!("Expected 2 fields for layout");
            return err;
        }

        let symbol: String = layout[0].clone().try_into()?;
        let symbol = CString::new(symbol).map_err(|_| FigError::Conversion)?;
        // LEAK but okay since this should hang around for the whole program
        let symbol = symbol.into_raw();

        type F = fn(*mut Monitor);
        let arrange = match &layout[1] {
            Value::Str(s) if s == "tile" => Some(tile as F),
            Value::Str(s) if s == "monocle" => Some(monocle as F),
            Value::Nil => None,
            _ => {
                log::error!("func field on layout should be Str or nil");
                return err;
            }
        };

        ret.push(Layout { symbol, arrange });
    }

    Ok(ret)
}

impl TryFrom<Fig> for Config {
    type Error = Box<dyn Error>;

    fn try_from(Fig { variables: mut v }: Fig) -> Result<Self, Self::Error> {
        let float = |val: fig::Value| val.try_into();
        let int = |val: fig::Value| {
            val.try_into_int().map_err(|_| "unable to parse int")
        };
        let bool = |val: fig::Value| val.try_into();
        let str_list = |val: fig::Value| -> Result<Vec<CString>, _> {
            let strs: Vec<String> = val.try_into()?;
            strs.into_iter()
                .map(CString::new)
                .collect::<Result<Vec<CString>, _>>()
                .map_err(|_| Box::new(FigError::Conversion))
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
            keys: get_keys(&mut v)?,
            dmenucmd: get(&mut v, "dmenucmd")?.try_into()?,
            rules: get_rules(&mut v)?,
            swallowfloating: get(&mut v, "swallowfloating")?.try_into()?,
            systraypinning: i64::try_from(get(&mut v, "systraypinning")?)?
                as u32,
            systrayonleft: get(&mut v, "systrayonleft")?.try_into()?,
            systrayspacing: i64::try_from(get(&mut v, "systrayspacing")?)?
                as u32,
            systraypinningfailfirst: get(&mut v, "systraypinningfailfirst")?
                .try_into()?,
            showsystray: get(&mut v, "showsystray")?.try_into()?,
            buttons: get_buttons(&mut v)?,
            layouts: get_layouts(&mut v)?,
            scratchpadname: get(&mut v, "scratchpadname")?.try_into()?,
        })
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(path)?;
        let mut f = fig::Fig::new();
        f.variables = fig_env::FIG_ENV.clone();
        f.parse(&s)?;
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

/// Swallow floating windows by default
const SWALLOWFLOATING: bool = false;

/// 0: sloppy systray follows selected monitor, >0: pin systray to monitor x
static SYSTRAYPINNING: c_uint = 0;
const SYSTRAYONLEFT: bool = false;
/// systray spacing
const SYSTRAYSPACING: c_uint = 2;
/// if pinning fails and this is true, display systray on the first monitor,
/// else display systray on the last monitor
const SYSTRAYPINNINGFAILFIRST: bool = true;
const SHOWSYSTRAY: bool = true;

const COL_GRAY1: &str = "#222222";
const COL_GRAY2: &str = "#444444";
const COL_GRAY3: &str = "#bbbbbb";
const COL_GRAY4: &str = "#eeeeee";
const COL_CYAN: &str = "#005577";

fn default_colors() -> [[CString; 3]; 2] {
    let mut ret = std::array::from_fn(|_| {
        std::array::from_fn(|_| CString::new("").unwrap())
    });
    ret[Scheme::Norm as usize] =
        [COL_GRAY3, COL_GRAY1, COL_GRAY2].map(|s| CString::new(s).unwrap());
    ret[Scheme::Sel as usize] =
        [COL_GRAY4, COL_CYAN, COL_CYAN].map(|s| CString::new(s).unwrap());
    ret
}

const RULES: [Rule; 3] = [
    Rule {
        class: c"Gimp".as_ptr(),
        instance: null(),
        title: null(),
        tags: 0,
        isfloating: true,
        isterminal: false,
        noswallow: false,
        monitor: -1,
    },
    Rule {
        class: c"Firefox".as_ptr(),
        instance: null(),
        title: null(),
        tags: 1 << 8,
        isfloating: false,
        isterminal: false,
        noswallow: true,
        monitor: -1,
    },
    Rule {
        class: c"st-256color".as_ptr(),
        instance: null(),
        title: null(),
        tags: 0,
        isfloating: false,
        isterminal: true,
        noswallow: false,
        monitor: -1,
    },
];

// layouts

const LAYOUTS: [Layout; 3] = [
    Layout { symbol: c"[]=".as_ptr(), arrange: Some(tile) },
    Layout { symbol: c"><>".as_ptr(), arrange: None },
    Layout { symbol: c"[M]".as_ptr(), arrange: Some(monocle) },
];

// key definitions
const MODKEY: c_uint = Mod4Mask;

// commands

const S_MOD: c_uint = MODKEY | ShiftMask;

static DMENUFONT: &str = "monospace:size=10";
static DMENUCMD: LazyLock<Vec<String>> = LazyLock::new(|| {
    vec![
        "dmenu_run".into(),
        "-fn".into(),
        DMENUFONT.into(),
        "-nb".into(),
        COL_GRAY1.into(),
        "-nf".into(),
        COL_GRAY3.into(),
        "-sb".into(),
        COL_CYAN.into(),
        "-sf".into(),
        COL_GRAY4.into(),
    ]
});
static TERMCMD: LazyLock<Vec<String>> = LazyLock::new(|| vec!["st".into()]);

const SCRATCHPADNAME: &str = "scratchpad";
static SCRATCHPADCMD: LazyLock<Vec<String>> = LazyLock::new(|| {
    vec![
        "st".into(),
        "-t".into(),
        SCRATCHPADNAME.into(),
        "-g".into(),
        "120x34".into(),
    ]
});

fn default_keys() -> [Key; 61] {
    [
        Key::new(MODKEY, XK_p, spawn, Arg::V(DMENUCMD.clone())),
        Key::new(S_MOD, XK_Return, spawn, Arg::V(TERMCMD.to_vec())),
        Key::new(
            MODKEY,
            XK_grave,
            togglescratch,
            Arg::V(SCRATCHPADCMD.to_vec()),
        ),
        Key::new(MODKEY, XK_b, togglebar, Arg::I(0)),
        Key::new(MODKEY, XK_j, focusstack, Arg::I(1)),
        Key::new(MODKEY, XK_k, focusstack, Arg::I(-1)),
        Key::new(MODKEY, XK_i, incnmaster, Arg::I(1)),
        Key::new(MODKEY, XK_d, incnmaster, Arg::I(-1)),
        Key::new(MODKEY, XK_h, setmfact, Arg::F(-0.05)),
        Key::new(MODKEY, XK_l, setmfact, Arg::F(0.05)),
        Key::new(MODKEY, XK_Return, zoom, Arg::I(0)),
        Key::new(MODKEY, XK_Tab, view, Arg::Ui(0)),
        Key::new(S_MOD, XK_c, killclient, Arg::I(0)),
        Key::new(MODKEY, XK_t, setlayout, Arg::L(Some(0))),
        Key::new(MODKEY, XK_f, setlayout, Arg::L(Some(1))),
        Key::new(MODKEY, XK_m, setlayout, Arg::L(Some(2))),
        Key::new(MODKEY, XK_space, setlayout, Arg::L(None)),
        Key::new(S_MOD, XK_space, togglefloating, Arg::I(0)),
        Key::new(MODKEY, XK_0, view, Arg::Ui(!0)),
        Key::new(S_MOD, XK_0, tag, Arg::Ui(!0)),
        Key::new(MODKEY, XK_comma, focusmon, Arg::I(-1)),
        Key::new(MODKEY, XK_period, focusmon, Arg::I(1)),
        Key::new(S_MOD, XK_comma, tagmon, Arg::I(-1)),
        Key::new(S_MOD, XK_period, tagmon, Arg::I(1)),
        Key::new(MODKEY, XK_1, view, Arg::Ui(1 << 0)),
        Key::new(MODKEY | ControlMask, XK_1, toggleview, Arg::Ui(1 << 0)),
        Key::new(S_MOD, XK_1, tag, Arg::Ui(1 << 0)),
        Key::new(S_MOD | ControlMask, XK_1, toggletag, Arg::Ui(1 << 0)),
        Key::new(MODKEY, XK_2, view, Arg::Ui(1 << 1)),
        Key::new(MODKEY | ControlMask, XK_2, toggleview, Arg::Ui(1 << 1)),
        Key::new(S_MOD, XK_2, tag, Arg::Ui(1 << 1)),
        Key::new(S_MOD | ControlMask, XK_2, toggletag, Arg::Ui(1 << 1)),
        Key::new(MODKEY, XK_3, view, Arg::Ui(1 << 2)),
        Key::new(MODKEY | ControlMask, XK_3, toggleview, Arg::Ui(1 << 2)),
        Key::new(S_MOD, XK_3, tag, Arg::Ui(1 << 2)),
        Key::new(S_MOD | ControlMask, XK_3, toggletag, Arg::Ui(1 << 2)),
        Key::new(MODKEY, XK_4, view, Arg::Ui(1 << 3)),
        Key::new(MODKEY | ControlMask, XK_4, toggleview, Arg::Ui(1 << 3)),
        Key::new(S_MOD, XK_4, tag, Arg::Ui(1 << 3)),
        Key::new(S_MOD | ControlMask, XK_4, toggletag, Arg::Ui(1 << 3)),
        Key::new(MODKEY, XK_5, view, Arg::Ui(1 << 4)),
        Key::new(MODKEY | ControlMask, XK_5, toggleview, Arg::Ui(1 << 4)),
        Key::new(S_MOD, XK_5, tag, Arg::Ui(1 << 4)),
        Key::new(S_MOD | ControlMask, XK_5, toggletag, Arg::Ui(1 << 4)),
        Key::new(MODKEY, XK_6, view, Arg::Ui(1 << 5)),
        Key::new(MODKEY | ControlMask, XK_6, toggleview, Arg::Ui(1 << 5)),
        Key::new(S_MOD, XK_6, tag, Arg::Ui(1 << 5)),
        Key::new(S_MOD | ControlMask, XK_6, toggletag, Arg::Ui(1 << 5)),
        Key::new(MODKEY, XK_7, view, Arg::Ui(1 << 6)),
        Key::new(MODKEY | ControlMask, XK_7, toggleview, Arg::Ui(1 << 6)),
        Key::new(S_MOD, XK_7, tag, Arg::Ui(1 << 6)),
        Key::new(S_MOD | ControlMask, XK_7, toggletag, Arg::Ui(1 << 6)),
        Key::new(MODKEY, XK_8, view, Arg::Ui(1 << 7)),
        Key::new(MODKEY | ControlMask, XK_8, toggleview, Arg::Ui(1 << 7)),
        Key::new(S_MOD, XK_8, tag, Arg::Ui(1 << 7)),
        Key::new(S_MOD | ControlMask, XK_8, toggletag, Arg::Ui(1 << 7)),
        Key::new(MODKEY, XK_9, view, Arg::Ui(1 << 8)),
        Key::new(MODKEY | ControlMask, XK_9, toggleview, Arg::Ui(1 << 8)),
        Key::new(S_MOD, XK_9, tag, Arg::Ui(1 << 8)),
        Key::new(S_MOD | ControlMask, XK_9, toggletag, Arg::Ui(1 << 8)),
        Key::new(S_MOD, XK_q, quit, Arg::I(0)),
    ]
}

// button definitions

static BUTTONS: LazyLock<[Button; 11]> = LazyLock::new(|| {
    [
        Button::new(Clk::LtSymbol, 0, Button1, setlayout, Arg::L(None)),
        Button::new(Clk::LtSymbol, 0, Button3, setlayout, Arg::L(Some(2))),
        Button::new(Clk::WinTitle, 0, Button2, zoom, Arg::I(0)),
        Button::new(
            Clk::StatusText,
            0,
            Button2,
            spawn,
            Arg::V(TERMCMD.to_vec()),
        ),
        Button::new(Clk::ClientWin, MODKEY, Button1, movemouse, Arg::I(0)),
        Button::new(Clk::ClientWin, MODKEY, Button2, togglefloating, Arg::I(0)),
        Button::new(Clk::ClientWin, MODKEY, Button3, resizemouse, Arg::I(0)),
        Button::new(Clk::TagBar, 0, Button1, view, Arg::I(0)),
        Button::new(Clk::TagBar, 0, Button3, toggleview, Arg::I(0)),
        Button::new(Clk::TagBar, MODKEY, Button1, tag, Arg::I(0)),
        Button::new(Clk::TagBar, MODKEY, Button3, toggletag, Arg::I(0)),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config() {
        let _ = env_logger::try_init();
        let conf = Config::load("example.fig").unwrap();

        assert_eq!(conf.tags.len(), 9);
    }
}
