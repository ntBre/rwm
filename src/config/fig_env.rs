//! Build an initial Fig environment containing useful symbols for writing
//! your config

use std::{collections::HashMap, sync::LazyLock};

use fig::Value;
use rwm::enums::Clk;
use x11::xlib::{
    Button1, Button2, Button3, ControlMask, Mod2Mask, Mod3Mask, Mod4Mask,
    Mod5Mask, ShiftMask,
};

use x11::keysym::*;

macro_rules! handler_fns {
    ($($id:ident$(,)*)*) => {
        [
            $((stringify!($id).into(),
                Value::Str(stringify!($id).into())),)*
        ]
    }
}

macro_rules! keys {
    ($($id:ident$(,)*)*) => {
        [
            $((stringify!($id).into(),
                Value::Int($id as i64)),)*
        ]
    }
}

/// Turn [Clk] variants like `Clk::TagBar` into string (`ClkTagBar`), `Int`
/// pairs
macro_rules! clicks {
    ($($id:ident$(,)*)*) => {
        [
            $((concat!("Clk", stringify!($id)).into(),
                Value::Int(Clk::$id as i64)),)*
        ]
    }
}

pub static FIG_ENV: LazyLock<HashMap<String, fig::Value>> = LazyLock::new(
    || {
        let handlers = handler_fns! {
            focusmon, focusstack, pushstack, incnmaster, killclient, quit, setlayout, setmfact,
            spawn, tag, tagmon, togglebar, togglefloating, toggletag, toggleview,
            view, zoom, movemouse, resizemouse, tile, monocle, fullscreen,
            togglescratch,
        };
        let keys = keys! {
            Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask,
            XK_a, XK_b, XK_c, XK_d, XK_e, XK_f, XK_g, XK_h, XK_i, XK_j, XK_k,
            XK_l, XK_m, XK_n, XK_o, XK_p, XK_q, XK_r, XK_s, XK_t, XK_u, XK_v,
            XK_w, XK_x, XK_y, XK_z, XK_0, XK_1, XK_2, XK_3, XK_4, XK_5, XK_6,
            XK_7, XK_8, XK_9, XK_Return, XK_Tab, XK_space, XK_comma, XK_period,
            XK_grave, ShiftMask, ControlMask,
        };
        let clicks = clicks! {
            TagBar, LtSymbol, StatusText, WinTitle, ClientWin, RootWin,
        };
        let buttons = keys! { Button1, Button2, Button3 };
        let mut ret = HashMap::new();
        ret.extend(handlers);
        ret.extend(keys);
        ret.extend(clicks);
        ret.extend(buttons);
        ret
    },
);
