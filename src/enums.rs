/// Cursor
pub enum Cur {
    Normal,
    Resize,
    Move,
    Last,
}

/// Clicks
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Clk {
    TagBar,
    LtSymbol,
    StatusText,
    WinTitle,
    ClientWin,
    RootWin,
    #[allow(unused)]
    Last,
}

/// Default atoms
pub enum WM {
    Protocols,
    Delete,
    State,
    TakeFocus,
    #[allow(unused)]
    Last,
}

/// EWMH atoms
pub enum Net {
    Supported,
    WMName,
    WMState,
    WMCheck,
    WMFullscreen,
    ActiveWindow,
    WMWindowType,
    WMWindowTypeDialog,
    ClientList,
    Last,
}
