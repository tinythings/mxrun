SHELL := /bin/sh

CC ?= cc
CARGO ?= cargo
MXRUN ?= mxrun
MXRUN_MANIFEST ?= ../Cargo.toml
DEBUG_CFLAGS ?= -O0 -g -Wall -Wextra
RELEASE_CFLAGS ?= -O2 -DNDEBUG -Wall -Wextra
BUILD_DIR := build
STAGE_DIR := $(BUILD_DIR)/stage
DIST_DIR := $(BUILD_DIR)/dist
MXRUN_DIR := $(BUILD_DIR)/.mxrun
APP := hello

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

.PHONY: devel release clean

devel:
	$(call maybe_mxrun)
	@mkdir -p $(STAGE_DIR) $(MXRUN_DIR)
	$(CC) $(DEBUG_CFLAGS) -v hello.c -o $(STAGE_DIR)/$(APP)
	@printf '%s\n' \
		'build/stage/hello' \
		> $(MXRUN_DIR)/devel.paths

release:
	$(call maybe_mxrun)
	@mkdir -p $(DIST_DIR) $(MXRUN_DIR)
	$(CC) $(RELEASE_CFLAGS) hello.c -o $(DIST_DIR)/$(APP)
	@printf '%s\n' \
		'build/dist/hello' \
		> $(MXRUN_DIR)/release.paths

clean:
	rm -rf $(BUILD_DIR)