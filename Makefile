install: dwm/libdwm.so
	cargo install --path .

test: dwm/libdwm.so
	cargo test

clippy_args :=
ifdef FIX
    clippy_args += --fix
endif

clippy:
	cargo clippy --workspace $(clippy_args)

doc:
	cargo doc --open

fmt:
	cargo fmt

include config.mk

SRC = $(addprefix dwm/,drw.c dwm.c util.c)

dwm/libdwm.so: $(SRC) dwm/config.h dwm/dwm.h
	cd dwm ; \
	clang -fPIC -shared -o $(notdir $@ $(SRC)) $(CPPFLAGS) $(LDFLAGS)  $(INCS)

rust-src := $(shell find src -name '*.rs')

target/release/rwm: dwm/libdwm.so $(rust-src)
	cargo build --release

build: target/release/rwm

.PHONY: build
