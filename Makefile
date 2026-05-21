BIN := loc
INSTALL_DIR := /usr/local/bin

.PHONY: build release install uninstall check clean

build:
	cargo build

release:
	cargo build --release

install: release
	sudo cp target/release/$(BIN) $(INSTALL_DIR)/$(BIN)

uninstall:
	sudo rm -f $(INSTALL_DIR)/$(BIN)

check:
	cargo clippy

clean:
	cargo clean
