.PHONY: all
all: test

.PHONY: build
check:
	@cargo check --all --all-features

.PHONY: doc
doc:
	@cargo doc --all --all-features

.PHONY: all
test:
	@cargo test --all -- --test-threads=1 < /dev/null

.PHONY: format
format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

.PHONY: format
format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

.PHONY: format
lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy
