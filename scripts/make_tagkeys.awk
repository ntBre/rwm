#!/usr/bin/awk -f

BEGIN {
    for (i = 1; i <= 9; i++) {
	printf "Key::new(MODKEY, XK_%d, bindgen::view, Arg{ u: 1 << %d }),\n", i, i-1
	printf "Key::new(MODKEY|ControlMask, XK_%d, bindgen::toggleview, Arg{ u: 1 << %d }),\n", i, i-1
	printf "Key::new(MODKEY|ShiftMask, XK_%d, bindgen::tag, Arg{u: 1 << %d}),\n", i, i-1
	printf "Key::new(MODKEY|ControlMask|ShiftMask, XK_%d, bindgen::toggletag, Arg{ u: 1 << %d }),\n", i, i-1
    }
}
