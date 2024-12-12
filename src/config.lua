-- Constructor functions
function key (mod, keysym, func, arg)
   return {
	  mod_ = mod,
	  keysym = keysym,
	  func = func,
	  arg = arg,
   }
end

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
modkey = Mod4Mask
s_mod = ShiftMask | modkey

-- TODO
keys = {
   key(modkey, XK_p, spawn, {V = dmenucmd}),
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
   -- TODO
   rules = {},
   swallowfloating = false,
   systraypinning = 0,
   systrayonleft = false,
   systrayspacing = 2,
   systraypinningfailfirst = true,
   showsystray = true,
   -- TODO
   buttons = {
	  {
		 click = ClkLtSymbol,
		 mask = 0,
		 button = Button1,
		 func = setlayout,
	  },
   },
   layouts = {
	  {symbol = "[]=", arrange = tile },
	  {symbol = "><>", arrange = nil },
	  {symbol = "[M]", arrange = monocle },
   },
   scratchpadname = "scratchpad",
}
