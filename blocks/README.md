# rwmblocks
status bar for dwm or rwm ported from
[dwmblocks](https://github.com/torrinfail/dwmblocks)

## Installation
Unlike `rwm` itself, `rwmblocks` works pretty well, and I plan to replace
dwmblocks in my config. Install with make:

``` shell
make install
```

I don't think this one requires nightly, but you will probably need X11
installed on your system.

## Usage
Before installation, configure your status bar by editing `config.rs`. Each
`Block` entry is one section of the status bar. Unfortunately, Rust's `Command`
API doesn't expand special shell characters like `~`, so the `command` paths
need to be absolute. I recommend storing the command in a script so that you can
edit the command without recompiling `rwmblocks`. For example, my `time` script
is a very thin wrapper around `date`:

``` shell
#!/bin/sh

date '+%a %b %d %Y %H:%M |'
```

while my `mail` and `weather` scripts are a bit more involved.

The `icon` is displayed before command output, the `interval` is how often the
block is updated in seconds, and `signal` is a signal sent to `rwmblocks` to
trigger an immediate update for the block. This can be useful if you have
something like a volume icon, for example, that doesn't need to update on an
interval but should update when you run some other script. To send such a
signal, you can use `pkill` as shown below.

``` shell
pkill -RTMIN+12 rwmblocks
```

`SIGRTMIN` is the minimum real-time signal reserved for applications, so the
`signal` fields in `Blocks` are relative to this minimum. Fortunately, `pkill`
also knows about this convention and allows you to use the syntax shown above.
This command would trigger an immediate update of my weather block.

After this configuration and installation, simply spawn `rwmblocks` before
running your window manager. I do this in my `.xinitrc` with `rwmblocks &>
~/.log/blocks.log &`. If you copy this command, make sure the `.log` directory
exists in your `$HOME` directory.

