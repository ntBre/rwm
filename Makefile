install:
	cargo install --path .

test:
	cargo build --release
	-ssh omsf -t 'pkill rwm'
	scp target/release/rwm 'omsf:.cargo/bin/rwm'

clippy:
	cargo clippy

doc:
	cargo doc --open

include config.mk

SRC = $(addprefix dwm/,drw.c dwm.c util.c)

dwm/libdwm.so: $(SRC)
	cd dwm ; \
	clang -fPIC -shared -o $(notdir $@ $^) $(CPPFLAGS) $(LDFLAGS)  $(INCS)
