# Builds offline from the crates already in ~/.cargo (pinned by Cargo.lock); never downloads.
BIN      ?= bin
VERSION  := $(shell grep -m1 -oP '^version = "\K[^"]+' Cargo.toml)
export GIT_HASH := $(shell git describe --always --dirty=+ --abbrev=7 2>/dev/null)
export GIT_DATE := $(shell git log -1 --format=%cs 2>/dev/null)
# Keep the home folder out of the binary (panic messages carry source paths).
export RUSTFLAGS := --remap-path-prefix=$(HOME)=~

all:
	cargo build --release --offline --locked
	@mkdir -p $(BIN)
	cp target/release/colourpicker $(BIN)/colourpicker
	cp target/release/colourpicker $(BIN)/ColourPicker_v$(VERSION).bin

test:
	cargo test --offline --locked

clean:
	cargo clean
	rm -f $(BIN)/colourpicker

.PHONY: all test clean
