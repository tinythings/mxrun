SHELL := /bin/sh

CARGO ?= cargo
RUSTUP ?= rustup
MXRUN ?= mxrun
MXRUN_MANIFEST ?= Cargo.toml
CLIPPY_FLAGS := --workspace --all-targets --all-features -- -D warnings
NEXTTEST_FLAGS := --workspace --all-features

define maybe_mxrun
	@if [ -n "$$MXRUN_CONFIG" ]; then \
		if command -v $(MXRUN) >/dev/null 2>&1; then \
			exec $(MXRUN) run $@; \
		elif [ -f "$(MXRUN_MANIFEST)" ]; then \
			exec $(CARGO) run --manifest-path $(MXRUN_MANIFEST) -- run $@; \
		else \
			printf '%s\n' 'mxrun is not available and no Cargo manifest was found for fallback' >&2; \
			exit 127; \
		fi; \
	fi
endef

.PHONY: dev release check fix test setup clean

dev:
	$(call maybe_mxrun)
	$(CARGO) build -vv

release:
	$(call maybe_mxrun)
	$(CARGO) build --release

check:
	$(call maybe_mxrun)
	$(CARGO) clippy $(CLIPPY_FLAGS)

fix:
	$(call maybe_mxrun)
	$(CARGO) clippy --fix --allow-dirty --allow-staged --workspace --all-targets --all-features -- -D warnings

test:
	$(call maybe_mxrun)
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