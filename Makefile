.PHONY: build test lint fmt clean dev

build:
	cargo build

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

clean:
	cargo clean

dev: fmt lint test build
ci: fmt-check lint test build
