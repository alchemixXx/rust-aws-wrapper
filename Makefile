# Name of your binary
BINARY=rust-aws-wrapper

# Where to symlink it for global dev use
SYMLINK_PATH=/usr/local/bin/$(BINARY)

.PHONY: build release install symlink dev uninstall clean test check

## Build in debug mode
build:
	cargo build

## Build in release mode (optimized)
release:
	cargo build --release

## Install globally using cargo install (release build)
install:
	cargo install --path . --force

## Symlink debug binary to /usr/local/bin (for local dev)
symlink:
	ln -sf $(PWD)/target/debug/$(BINARY) $(SYMLINK_PATH)

## Build and symlink for development
dev:
	cargo build && ln -sf $(PWD)/target/debug/$(BINARY) $(SYMLINK_PATH)

## Run tests
test:
	cargo test

## Run format, clippy lint, and build check
check:
	cargo fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo check

## Remove symlink or installed binary from /usr/local/bin
uninstall:
	rm -f $(SYMLINK_PATH)

## Clean build artifacts
clean:
	cargo clean
