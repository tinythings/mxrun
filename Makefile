SHELL := /bin/sh

CARGO ?= cargo
RUSTUP ?= rustup
XRUN ?= xrun
XRUN_MANIFEST ?= Cargo.toml
CLIPPY_FLAGS := --workspace --all-targets --all-features -- -D warnings
NEXTTEST_FLAGS := --workspace --all-features

define maybe_xrun
	@if [ -n "$$XRUN_CONFIG" ]; then \
		if command -v $(XRUN) >/dev/null 2>&1; then \
			exec $(XRUN) run $@; \
		elif [ -f "$(XRUN_MANIFEST)" ]; then \
			exec $(CARGO) run --manifest-path $(XRUN_MANIFEST) -- run $@; \
		else \
			printf '%s\n' 'xrun is not available and no Cargo manifest was found for fallback' >&2; \
			exit 127; \
		fi; \
	fi
endef

.PHONY: dev release check fix test setup clean

dev:
	$(call maybe_xrun)
	$(CARGO) build -vv

release:
	$(call maybe_xrun)
	$(CARGO) build --release

check:
	$(call maybe_xrun)
	$(CARGO) clippy $(CLIPPY_FLAGS)

fix:
	$(call maybe_xrun)
	$(CARGO) clippy --fix --allow-dirty --allow-staged --workspace --all-targets --all-features -- -D warnings

test:
	$(call maybe_xrun)
	$(CARGO) nextest run $(NEXTTEST_FLAGS)

clean:
	$(CARGO) clean

setup:
	@$(RUSTUP) component add clippy
	@if $(CARGO) nextest --version >/dev/null 2>&1; then \
		printf '%s\n' 'cargo-nextest already installed'; \
	else \
		$(CARGO) install cargo-nextest --locked; \
	fi
	@$(CARGO) fetch