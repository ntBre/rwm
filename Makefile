install:
	cargo install --path .

test:
	cargo test

clippy_args :=
ifdef FIX
    clippy_args += --fix --allow-dirty
endif

clippy:
	cargo +nightly clippy --workspace --all-targets --all-features --tests $(clippy_args)

doc:
	cargo doc --open

fmt:
	cargo fmt

rust-src := $(shell find src -name '*.rs')

target/release/rwm: $(rust-src)
	cargo build --release

build: target/release/rwm

.PHONY: build

xephyr:
	Xephyr :1 -screen 960x540 & DISPLAY=:1.0 cargo run && kill %1
