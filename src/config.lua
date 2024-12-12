-- Constructor functions
function key (mod, keysym, func, arg)
   return {
	  mod_ = mod,
	  keysym = keysym,
	  func = func,
	  arg = arg,
   }
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
   -- TODO
   keys = {key(modkey, XK_p, spawn, {V = dmenucmd})},
   dmenucmd = dmenucmd,
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
