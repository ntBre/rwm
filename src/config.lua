-- Constructor functions
function key (mod, keysym, func, arg)
   return {
	  mod_ = mod,
	  keysym = keysym,
	  func = func,
	  arg = arg,
   }
end

function button (click, mask, button, func, arg)
   return {
	  click = click,
	  mask = mask,
	  button = button,
	  func = func,
	  arg = arg,
   }
end

function rule (class, instance, title, tags, isfloating, isterminal, noswallow, monitor)
   return {
	  class = class,
	  instance = instance,
	  title = title,
	  tags = tags,
	  isfloating = isfloating,
	  isterminal = isterminal,
	  noswallow = noswallow,
	  monitor = monitor,
   }
end

-- For each tag key (N = 1, 2, ...) you generally want to set four bindings:
--
-- * MOD+N to view the tag
-- * MOD+Ctrl+N to toggle viewing the tag
-- * MOD+Shift+N to send a window to the tag
-- * MOD+Shift+Control+N to toggle the display of a window on a tag
--
-- This function inserts each of those into `keys` for the default tag keys XK_1
-- through XK_9.
function tagkeys (keys)
   for i, xk in ipairs({XK_1, XK_2, XK_3, XK_4, XK_5, XK_6, XK_7, XK_8, XK_9}) do
	  arg = {Ui = 1 << (i - 1)}
	  table.insert(keys, key(modkey,             xk, view,       arg))
	  table.insert(keys, key(modkey|ControlMask, xk, toggleview, arg))
	  table.insert(keys, key(s_mod,              xk, tag,        arg))
	  table.insert(keys, key(s_mod|ControlMask,  xk, toggletag,  arg))
   end
end

-- Default colors
gray1 = "#222222"
gray2 = "#444444"
gray3 = "#bbbbbb"
gray4 = "#eeeeee"
cyan = "#005577"

dmenufont =  "monospace:size=10"
dmenucmd = {
   "dmenu_run",
   "-fn",
   dmenufont,
   "-nb",
   gray1,
   "-nf",
   gray3,
   "-sb",
   cyan,
   "-sf",
   gray4,
}

termcmd = {"st", "-e", "zsh"}

scratchpadname = "scratchpad"
scratchpadcmd = {"st", "-t", scratchpadname, "-g", "120x34"}

modkey = Mod4Mask
s_mod = ShiftMask | modkey

-- Lua doesn't have true integers, so ~0 gives -1. We're just trying to get all
-- of the bits in a u32 set, so this works too. math.floor is required for the
-- deserializer to detect this as an int
all_tags = math.floor(2^32 - 1)

keys = {
   key(modkey, XK_p, spawn, {V = dmenucmd}),
   key(s_mod, XK_Return, spawn, {V = termcmd}),
   key(modkey, XK_grave, togglescratch, {V = scratchpadcmd}),
   key(modkey, XK_b, togglebar, {I = 0}),
   key(modkey, XK_j, focusstack, {I = 1}),
   key(modkey, XK_k, focusstack, {I = -1}),
   key(s_mod, XK_j, pushstack, {I = 1}),
   key(s_mod, XK_k, pushstack, {I = -1}),
   key(modkey, XK_i, incnmaster, {I = 1}),
   key(modkey, XK_d, incnmaster, {I = -1}),
   key(modkey, XK_h, setmfact, {F = -0.05}),
   key(modkey, XK_l, setmfact, {F = 0.05}),
   key(modkey, XK_Return, zoom, {I = 0}),
   key(modkey, XK_Tab, view, {Ui = 0}),
   key(s_mod, XK_c, killclient, {I = 0}),
   key(modkey, XK_t, setlayout, {L = 0}),
   key(modkey, XK_f, setlayout, {L = 1}),
   key(modkey, XK_m, setlayout, {L = 2}),
   key(modkey, XK_space, setlayout),
   key(s_mod, XK_space, togglefloating, {I = 0}),
   key(modkey, XK_0, view, {Ui = all_tags}),
   key(s_mod, XK_0, tag, {Ui = all_tags}),
   key(modkey, XK_comma, focusmon, {I = -1}),
   key(modkey, XK_period, focusmon, {I = 1}),
   key(s_mod, XK_comma, tagmon, {I = -1}),
   key(s_mod, XK_period, tagmon, {I = 1}),
   key(s_mod, XK_q, quit, {I = 0}),
}
tagkeys(keys)

rwm = {
   borderpx = 3,
   snap = 32,
   showbar = true,
   topbar = true,
   mfact = 0.5,
   nmaster = 1,
   resize_hints = false,
   lock_fullscreen = true,
   fonts = {"monospace:size=10"},
   tags = {"1", "2", "3", "4", "5", "6", "7", "8", "9"},
   colors = {
	  norm = {gray3, gray1, gray2},
	  sel = {gray4, cyan, cyan},
   },
   keys = keys,
   dmenucmd = dmenucmd,
   rules = {
	  rule("st-256color", "", "", 0, false, true, false, -1),
   },
   -- Whether to swallow floating windows
   swallowfloating = false,
   -- If 0, sloppy systray follows monitor, otherwise pin systray to monitor n
   systraypinning = 0,
   -- Pin the systray to the left size of the bar
   systrayonleft = false,
   -- Spacing in pixels between icons in the systray
   systrayspacing = 2,
   -- If pinning the systray to the active monitor fails, display it on first
   -- monitor. If `false`, display it instead on the last monitor
   systraypinningfailfirst = true,
   -- Show the systray
   showsystray = true,
   buttons = {
	  button(ClkLtSymbol, 0, Button1, setlayout),
	  button(ClkLtSymbol, 0, Button3, setlayout, {L = 2}),
	  button(ClkWinTitle, 0, Button2, zoom, {I = 0}),
	  button(ClkStatusText, 0, Button2, spawn, {V = termcmd}),
	  button(ClkClientWin, modkey, Button1, movemouse, {I = 0}),
	  button(ClkClientWin, modkey, Button2, togglefloating, {I = 0}),
	  button(ClkClientWin, modkey, Button3, resizemouse, {I = 0}),
	  button(ClkTagBar, 0, Button1, view, {I = 0}),
	  button(ClkTagBar, 0, Button3, toggleview, {I = 0}),
	  button(ClkTagBar, modkey, Button1, tag, {I = 0}),
	  button(ClkTagBar, modkey, Button3, toggletag, {I = 0}),
   },
   layouts = {
	  {symbol = "[]=", arrange = tile },
	  {symbol = "><>", arrange = nil },
	  {symbol = "[M]", arrange = monocle },
   },
   scratchpadname = scratchpadname,
}
