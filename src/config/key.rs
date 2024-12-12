use std::{collections::HashMap, ffi::c_uint, fmt::Debug, sync::LazyLock};

use crate::{Arg, State};
use x11::xlib::KeySym;

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct KeyFn(pub Option<fn(&mut State, *const Arg)>);

impl TryFrom<String> for KeyFn {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(KeyFn(Some(
            FUNC_MAP
                .get(value.as_str())
                .cloned()
                .ok_or_else(|| format!("no key `{value}`"))?,
        )))
    }
}

#[repr(C)]
#[derive(Clone, serde::Deserialize)]
pub struct Key {
    pub mod_: c_uint,
    pub keysym: KeySym,
    pub func: KeyFn,
    pub arg: Arg,
}

impl Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key")
            .field("mod_", &self.mod_)
            .field("keysym", &self.keysym)
            .field("func", &self.func.0.map(|_| "[func]"))
            .field("arg", &self.arg)
            .finish()
    }
}

impl Key {
    pub const fn new(
        mod_: c_uint,
        keysym: u32,
        func: fn(&mut State, *const Arg),
        arg: Arg,
    ) -> Self {
        Self { mod_, keysym: keysym as KeySym, func: KeyFn(Some(func)), arg }
    }
}

unsafe impl Sync for Key {}

type FnMap = HashMap<&'static str, fn(&mut State, *const Arg)>;
static FUNC_MAP: LazyLock<FnMap> = LazyLock::new(|| {
    use crate::key_handlers::*;
    type FN = fn(&mut State, *const Arg);
    HashMap::from([
        ("focusmon", focusmon as FN),
        ("focusstack", focusstack as FN),
        ("pushstack", pushstack as FN),
        ("incnmaster", incnmaster as FN),
        ("killclient", killclient as FN),
        ("quit", quit as FN),
        ("setlayout", setlayout as FN),
        ("setmfact", setmfact as FN),
        ("spawn", spawn as FN),
        ("togglescratch", togglescratch as FN),
        ("tag", tag as FN),
        ("tagmon", tagmon as FN),
        ("togglebar", togglebar as FN),
        ("togglefloating", togglefloating as FN),
        ("toggletag", toggletag as FN),
        ("toggleview", toggleview as FN),
        ("view", view as FN),
        ("zoom", zoom as FN),
        ("fullscreen", fullscreen as FN),
        // mouse handlers
        ("movemouse", movemouse as FN),
        ("resizemouse", resizemouse as FN),
    ])
});
