[![check](https://github.com/ntBre/rwm/actions/workflows/check.yml/badge.svg)](https://github.com/ntBre/rwm/actions/workflows/check.yml)
[![test](https://github.com/ntBre/rwm/actions/workflows/test.yml/badge.svg)](https://github.com/ntBre/rwm/actions/workflows/test.yml)

# rwm
rust window manager ported from [dwm](https://dwm.suckless.org/)

For now this is a line-for-line port of dwm 6.4 with tons of unsafe code. Now
that it's working in this state, I'll start moving toward a safe Rust version
where possible. Linked lists are pretty cool, though.

## Screenshot
As you can see, it looks just like dwm, with the addition of a simple bar from
the `blocks` subdirectory! You can spawn windows with the default dwm
keybindings. Try out Mod+Shift+Enter for st or Mod+p for dmenu_run!

![Screenshot](screenshot.png)

## Installation
The following command should work:

``` shell
make install
```

This defers to `cargo install` and thus will place the resulting binary in
`~/.cargo/bin` by default.

I've been building with a Rust 1.81.0 nightly compiler from 2024-06-11 and test
in CI with nightly, but I don't use any nightly features, so it will likely
build with a stable toolchain too.

You'll also need the X11, Xft, Xinerama, and fontconfig libraries installed on
your system where rustc can find them, but the `x11` and `fontconfig-sys` crates
should help with detecting and linking against these.

Finally, if you're installing an experimental window manager based on dwm and
written in Rust, it's probably safe to assume you know how to start a window
manager. But just to be safe, I recommend putting something like the following
in your `$HOME/.xinitrc` script and launching with `startx`:

``` shell
while true
do
	rwm
done
```

Wrapping it with this loop allows smoother restarting, but you can also use the
simpler `exec rwm` if you prefer. 

You can optionally set the log level with the `RUST_LOG` environment variable. I
think I only used `log::trace!`, so you'll need `RUST_LOG=trace` if you need to
debug anything.

If you use a display manager, you can also include an `rwm.desktop` file
wherever your distro keeps those (maybe `/usr/share/xsessions/`?) that looks
like the one below. I've had success with this approach on an Ubuntu VM.

``` shell
[Desktop Entry]
Encoding=UTF-8
Name=Rwm
Comment=Rust window manager
Exec=rwm
Icon=rwm
Type=XSession
```

