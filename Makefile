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
