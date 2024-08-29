/// Cursor
pub enum Cur {
    Normal,
    Resize,
    Move,
    Last,
}

/// Color schemes
pub enum Scheme {
    Norm,
    Sel,
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
    Last,
}

/// Default atoms
pub enum WM {
    Protocols,
    Delete,
    State,
    TakeFocus,
    Last,
}

/// EWMH atoms
pub enum Net {
    Supported,
    WMName,
    WMState,
    WMCheck,
    SystemTray,
    SystemTrayOP,
    SystemTrayOrientation,
    SystemTrayOrientationHorz,
    WMFullscreen,
    ActiveWindow,
    WMWindowType,
    WMWindowTypeDialog,
    ClientList,
    Last,
}

pub enum XEmbed {
    Manager,
    XEmbed,
    XEmbedInfo,
    Last,
}

/// Clr scheme index
pub enum Col {
    Fg,
    Bg,
    Border,
}
