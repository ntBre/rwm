install: dwm/libdwm.so
	cargo install --path .

test:
	cargo build --release
	-ssh omsf -t 'pkill rwm'
	scp target/release/rwm 'omsf:.cargo/bin/rwm'

clippy:
	cargo clippy --workspace

doc:
	cargo doc --open

include config.mk

SRC = $(addprefix dwm/,drw.c dwm.c util.c)

dwm/libdwm.so: $(SRC) dwm/config.h dwm/dwm.h
	cd dwm ; \
	clang -fPIC -shared -o $(notdir $@ $(SRC)) $(CPPFLAGS) $(LDFLAGS)  $(INCS)

rust-src := $(shell find src -name '*.rs')

target/release/rwm: dwm/libdwm.so $(rust-src)

build: target/release/rwm

.PHONY: build
