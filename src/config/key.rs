use std::{collections::HashMap, ffi::c_uint, sync::LazyLock};

use fig::{FigError, Value};
use rwm::Arg;
use x11::xlib::KeySym;

#[repr(C)]
#[derive(Clone)]
pub struct Key {
    pub mod_: c_uint,
    pub keysym: KeySym,
    pub func: Option<fn(*const Arg)>,
    pub arg: Arg,
}

impl Key {
    pub const fn new(
        mod_: c_uint,
        keysym: u32,
        func: fn(*const Arg),
        arg: Arg,
    ) -> Self {
        Self { mod_, keysym: keysym as KeySym, func: Some(func), arg }
    }
}

unsafe impl Sync for Key {}

/// convert an option from Value::as_* into a fig result
pub(crate) fn conv<T: Clone>(opt: Option<&T>) -> Result<T, FigError> {
    match opt {
        Some(v) => Ok(v.clone()),
        None => Err(FigError::Conversion),
    }
}

type FnMap = HashMap<&'static str, fn(*const Arg)>;
pub(super) static FUNC_MAP: LazyLock<FnMap> = LazyLock::new(|| {
    use crate::key_handlers::*;
    type FN = fn(*const Arg);
    HashMap::from([
        ("focusmon", focusmon as FN),
        ("focusstack", focusstack as FN),
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

impl TryFrom<Value> for Key {
    type Error = FigError;

    /// Convert a Value::List of length 4 into a Key. assumes the list entries
    /// are mod_, keysym, func (as a string name), and arg as a Map
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let err = Err(FigError::Conversion);
        let Value::List(l) = value else {
            log::error!("Key should be a list: {value:?}");
            return err;
        };
        if l.len() != 4 {
            log::error!("Key list should have 4 fields: {l:?}");
            return err;
        }
        let func_name = conv(l[2].as_str())?;
        let func = conv(FUNC_MAP.get(func_name.as_str()))?;

        let arg = get_arg(conv(l[3].as_map())?)?;
        Ok(Self {
            mod_: conv(l[0].as_int())? as u32,
            keysym: conv(l[1].as_int())? as u64,
            func: Some(func),
            arg,
        })
    }
}

/// Try to extract an Arg from a fig::Map
pub(super) fn get_arg(arg: HashMap<String, Value>) -> Result<Arg, FigError> {
    let err = Err(FigError::Conversion);
    if arg.len() != 1 {
        log::error!("Key arg map should have 1 entry: {arg:?}");
        return err;
    }
    let arg: Vec<(String, Value)> = arg.into_iter().collect();
    let key = arg[0].0.to_lowercase();
    let arg = match key.as_str() {
        "i" => Arg::I(conv(arg[0].1.as_int())? as i32),
        "ui" => Arg::Ui(conv(arg[0].1.as_int())? as u32),
        "f" => Arg::F(conv(arg[0].1.as_float())? as f32),
        "v" => {
            // the value will be a fig List[Value], so map over the
            // Vec<Value> and try turning them all into Strings
            let v = conv(arg[0].1.as_list())?;
            let v: Result<Vec<_>, _> =
                v.into_iter().map(|v| conv(v.as_str())).collect();
            Arg::V(v?)
        }
        "l" => Arg::L(arg[0].1.as_int().map(|i| *i as usize)),
        _ => {
            log::error!("Unrecognized Key arg type: {key:?}");
            return err;
        }
    };
    Ok(arg)
}
