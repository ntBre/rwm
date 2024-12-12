dmenufont =  "monospace:size=10"

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
	  -- gray3, gray1, gray2
	  norm = {"#bbbbbb", "#222222",  "#444444"},
	  -- gray4, cyan, cyan
	  sel = {"#eeeeee", "#005577", "#005577"},
   },
   keys = {}, 					-- TODO
   dmenucmd = {"dmenu_run", "-fn", dmenufont, "-nb"}, -- TODO
   rules = {},										 -- TODO
   swallowfloating = false,
   systraypinning = 0,
   systrayonleft = false,
   systrayspacing = 2,
   systraypinningfailfirst = true,
   showsystray = true,
   buttons = {}, 				-- TODO
   layouts = {}, 				-- TODO
   scratchpadname = "scratchpad",
}
