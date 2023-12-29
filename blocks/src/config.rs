use crate::Block;

pub(crate) const DELIM: &str = "|";

pub(crate) const BLOCKS: [Block; 3] = [
    //
    Block {
        icon: "",
        command: "/home/brent/configs/bar_scripts/mail",
        interval: 60,
        signal: 11,
    },
    Block {
        icon: " ",
        command: "/home/brent/configs/bar_scripts/weather",
        interval: 600,
        signal: 12,
    },
    Block {
        icon: " 🕔 ",
        command: "/home/brent/configs/bar_scripts/time",
        interval: 1,
        signal: 0,
    },
];
