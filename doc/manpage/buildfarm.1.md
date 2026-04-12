% xrun(1)
% Sysinspect Project
% April 2026

# NAME

xrun - run one build entry across local and remote machines in parallel

# SYNOPSIS

```text
xrun [--add-host|-a <host>]
xrun [OPTIONS] init
xrun [OPTIONS] run <entry> [--mirror-results] [--mirror-root <dir>]
```

# DESCRIPTION

`xrun` is a terminal UI tool for projects that need to run the same build on more than one machine. It is intended for situations where a project builds locally, but also has to be built on remote systems such as FreeBSD or Linux virtual machines.

During a run, `xrun` reads a target list, synchronizes the project tree to remote targets, starts the selected build entry on every target, and shows the output in one screen. If result mirroring is enabled, it copies back only the files listed by the producer project’s manifest.

The tool is intentionally separate from project-specific build logic. It should know how to run builds and copy results, but the producer project remains responsible for deciding what the final deliverables are.

# TARGET CONFIG

The target config file is plain text. Each line describes one target in the following form:

```text
<uname -o> <uname -m> [user@]host:/destination
```

Example:

```text
local
FreeBSD amd64 builder@freebsd-vm:work/example-xrun
GNU/Linux x86_64 builder@linux-vm:work/example-xrun
```

The special value `local` means that the current machine should also take part in the run.

# PRODUCER CONTRACT

The producer project must provide a working build entry and, when mirroring is enabled, a manifest that lists the files to copy back.

The standard manifest path is:

```text
build/.xrun/<entry>.paths
```

For example:

```text
build/.xrun/devel.paths
```

The manifest is a line-based list of relative paths:

```text
build/stage/mytool
build/stage/myhelper
build/dist/example.wasm
```

Blank lines are ignored. Lines beginning with `#` are treated as comments.

# COMMANDS

## init

Validate and load the configured targets.

Today this command reads `XRUN_CONFIG`, parses the target list, and reports how many targets were loaded.

It is useful for checking that the config file exists and that target lines parse correctly.

It does not yet create remote directories, synchronize the project tree, or act as a standalone remote bootstrap command. Those steps still happen during `run <entry>`.

## run <entry>

Run one build entry across all configured targets.

Examples:

```bash
xrun run devel
xrun run release
xrun run devel --mirror-results
xrun run devel --mirror-results --mirror-root /tmp/xrun-out
```

# OPTIONS

## -c, --config <file>

Use the given config file instead of `XRUN_CONFIG`.

## -a, --add-host <host>

Add a remote host to the xrun config using the current local user and the current local project path as the remote destination.

## --mirror-results

After a successful build, read `build/.xrun/<entry>.paths` and copy back only the listed files.

Mirroring is disabled by default.

## --mirror-root <dir>

Override the default local destination for mirrored results:

```text
./target/xrun
```

This option requires `--mirror-results`.

## -d, --debug

Increase debug verbosity.

# HOW A RUN WORKS

For each configured target, `xrun` synchronizes the project tree if the target is remote, then starts the selected build entry on that machine. Linux-like targets use `make`. FreeBSD targets use `gmake`.

If mirroring is enabled and the build succeeded, `xrun` reads the manifest for the selected entry and copies back only the listed files. Mirrored results are stored under:

```text
target/xrun/<OS-LABEL>/...
```

Targets are handled independently. One target may already be mirroring results while another is still compiling.

# SIMPLE MAKEFILE EXAMPLE

```make
STAGE_DIR := build/stage
XRUN_MANIFEST_DIR := build/.xrun

.PHONY: devel

devel:
 @mkdir -p $(STAGE_DIR)
 @cc -O0 -g src/main.c -o $(STAGE_DIR)/hello
 @mkdir -p $(XRUN_MANIFEST_DIR)
 @printf '%s\n' \
  'build/stage/hello' \
  > $(XRUN_MANIFEST_DIR)/devel.paths
```

This example produces one executable and writes a manifest that lists it as the result to copy back.

# ENVIRONMENT

## XRUN_CONFIG

Path to the xrun config file.

## XRUN_LOCAL_MAKE

Override the command used for the local target.

Default:

```text
make
```

# FILES

## Producer manifest

```text
build/.xrun/<entry>.paths
```

This file belongs to the producer project and is required when `--mirror-results` is used.

# NOTES

While the TUI is running, `xrun` stores temporary logs in `.xrun/`. These logs are runtime scratch data only.

At the finish popup, `Ctrl-C` quits and deletes the logs. Pressing `p` quits and preserves them. Any other key dismisses the popup and keeps the TUI open.

# EXIT STATUS

`0` on success.

Non-zero if any target build or mirror step fails, or if the config or CLI arguments are invalid.

# SEE ALSO

`make(1)`, `gmake(1)`, `ssh(1)`, `rsync(1)`
