SHELL := /bin/sh

CC ?= cc
CARGO ?= cargo
XRUN ?= xrun
XRUN_MANIFEST ?= ../Cargo.toml
DEBUG_CFLAGS ?= -O0 -g -Wall -Wextra
RELEASE_CFLAGS ?= -O2 -DNDEBUG -Wall -Wextra
BUILD_DIR := build
STAGE_DIR := $(BUILD_DIR)/stage
DIST_DIR := $(BUILD_DIR)/dist
XRUN_DIR := $(BUILD_DIR)/.xrun
APP := hello

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

.PHONY: devel release clean

devel:
	$(call maybe_xrun)
	@mkdir -p $(STAGE_DIR) $(XRUN_DIR)
	$(CC) $(DEBUG_CFLAGS) -v hello.c -o $(STAGE_DIR)/$(APP)
	@printf '%s\n' \
		'build/stage/hello' \
		> $(XRUN_DIR)/devel.paths

release:
	$(call maybe_xrun)
	@mkdir -p $(DIST_DIR) $(XRUN_DIR)
	$(CC) $(RELEASE_CFLAGS) hello.c -o $(DIST_DIR)/$(APP)
	@printf '%s\n' \
		'build/dist/hello' \
		> $(XRUN_DIR)/release.paths

clean:
	rm -rf $(BUILD_DIR)