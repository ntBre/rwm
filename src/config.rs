use std::{
    collections::HashMap,
    error::Error,
    ffi::{c_float, c_int, c_uint, CString},
    fs::read_to_string,
    path::Path,
};

use fig_env::{CLICKS, HANDLERS, KEYS, XKEYS};
use mlua::{Lua, LuaSerdeExt as _, Table};

use crate::{config::key::Key, enums::Scheme, Button, Layout, Rule};

mod fig_env;
pub mod key;

#[derive(Debug, serde::Deserialize)]
#[serde(try_from = "HashMap<String, Vec<String>>")]
pub struct ColorMap(pub Vec<Vec<CString>>);

impl TryFrom<HashMap<String, Vec<String>>> for ColorMap {
    type Error = Box<dyn Error>;

    fn try_from(
        mut value: HashMap<String, Vec<String>>,
    ) -> Result<Self, Self::Error> {
        let mut ret = vec![vec![], vec![]];
        let norm = value.remove("norm").ok_or("missing key norm")?;
        let sel = value.remove("sel").ok_or("missing key sel")?;

        if norm.len() != 3 {
            return Err("not enough colors for SchemeNorm".into());
        };
        ret[Scheme::Norm as usize] = norm
            .into_iter()
            .map(CString::new)
            .collect::<Result<_, _>>()?;

        if sel.len() != 3 {
            return Err("not enough colors for SchemeSel".into());
        };
        ret[Scheme::Sel as usize] = sel
            .into_iter()
            .map(CString::new)
            .collect::<Result<_, _>>()?;

        Ok(Self(ret))
    }
}

#[derive(Debug, serde::Deserialize)]
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

    pub tags: Vec<String>,

    pub colors: ColorMap,

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

struct ConfigBuilder {
    lua: Lua,
    globals: Table,
}

impl ConfigBuilder {
    fn new() -> Self {
        let lua = Lua::new();
        let globals = lua.globals();

        // install handler functions for keybindings
        for (k, v) in HANDLERS {
            globals.set(k, v).unwrap();
        }

        // install key definitions
        for (k, v) in KEYS.iter().chain(XKEYS.iter()) {
            globals.set(*k, *v).unwrap();
        }

        // install click and button definitions
        for (k, v) in CLICKS {
            globals.set(k, v).unwrap();
        }

        for (k, v) in fig_env::BUTTONS {
            globals.set(k, v).unwrap();
        }

        lua.load(include_str!("config.lua")).exec().unwrap();

        Self { lua, globals }
    }

    /// Load and eval `path` into the Lua interpreter in `self`.
    fn load(self, path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        self.lua.load(read_to_string(path)?).exec()?;
        Ok(self)
    }

    fn finish(self) -> Result<Config, Box<dyn Error>> {
        Ok(self.lua.from_value(self.globals.get("rwm")?)?)
    }
}

impl Default for Config {
    fn default() -> Self {
        ConfigBuilder::new().finish().unwrap()
    }
}

impl Config {
    pub fn from_lua(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        ConfigBuilder::new().load(path)?.finish()
    }

    /// Attempt to load a config file on first usage from `$XDG_CONFIG_HOME`,
    /// then `$HOME`, before falling back to the default config.
    pub fn load_home() -> Self {
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
            .join("config.lua");

        Config::from_lua(config_path).unwrap_or_else(|e| {
            log::error!("failed to read config file: {e:?}");
            Config::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn from_lua() {
        let got = Config::from_lua("example.lua").unwrap();
        assert_debug_snapshot!(got)
    }
}
