#!/usr/bin/awk -f

BEGIN {
    for (i = 1; i <= 9; i++) {
	printf "Key::new(MODKEY, XK_%d, view, Arg::Uint(1 << %d)),\n", i, i-1
	printf "Key::new(MODKEY|ControlMask, XK_%d, toggleview, Arg::Uint(1 << %d)),\n", i, i-1
	printf "Key::new(MODKEY|ShiftMask, XK_%d, tag, Arg::Uint(1 << %d)),\n", i, i-1
	printf "Key::new(MODKEY|ControlMask|ShiftMask, XK_%d, toggletag, Arg::Uint(1 << %d)),\n", i, i-1
    }
}
