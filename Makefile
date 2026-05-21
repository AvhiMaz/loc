.PHONY: build check clean

build:
	cargo build

check:
	cargo clippy

clean:
	cargo clean
