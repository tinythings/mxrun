# mxrun

`mxrun` is a terminal program for running the same build on more than one machine at the same time. A typical use case is a project that builds locally, but also needs to be built on a FreeBSD or Linux VM. Instead of opening several terminals, syncing files by hand, and trying to remember what was run where, `mxrun` does the synchronization, starts the builds, and shows the output in one screen.

The important thing to understand is that `mxrun` is not supposed to know anything about the private structure of a particular project. It should know how to send a project tree to a target machine, how to run one build entry there, and, if requested, how to copy back the final results. The project itself remains responsible for deciding what those final results are.

## The basic idea

Every `mxrun` run has three parts. First, it reads a small text file that describes the build targets. Second, it starts the selected build entry on every target. Third, if result mirroring is enabled, it copies back only the files that the project explicitly listed as final deliverables.

In practice, the command usually looks like this:

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel
```

If the finished artifacts should also be copied back to the local machine, the command becomes:

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel --mirror-results
```

That is the whole workflow. The rest of the documentation explains what the config file looks like and what the producer project must provide.

## The target config file

The target config file is YAML. Targets retain their compact existing format:

```yaml
targets:
  - local
  - FreeBSD amd64 builder@freebsd-vm:work/example-mxrun
  - GNU/Linux x86_64 builder@linux-vm:work/example-mxrun

project:
  ignore:
    - /generated/
    - '*.cache'
  build:
    src:
      - target/debug/example
    dst: target/platforms
```

Each remote target entry has this form:

```text
<uname -o> <uname -m> [user@]host:/destination
```

The special word `local` means that the current machine should also participate in the run. The other lines describe remote machines. For example, the FreeBSD line says that the project should be synchronized to `builder@freebsd-vm:work/example-mxrun` and built there.

This means that one `mxrun` run can cover the local machine and one or more remote systems with the same build entry.

`project.ignore` contains `rsync` exclusion patterns. Entries may name a file, directory, or glob. A leading `/` anchors the pattern at the project root. The syntax is gitignore-like but uses rsync matching rules, and an exclusion does not remove files already present on a remote target. Unknown `project` keys are accepted for future options.

`project.build` collects build outputs after each target succeeds. `src` is a list of paths relative to the project workspace. `dst` is the local output root. mxrun copies each source into `<dst>/<platform>-<arch>/`; for example, `target/debug/example` becomes `target/platforms/GNU_Linux-x86_64/example`. Failed targets do not contribute collected output.

To select files from directory sources instead of copying them recursively, add `files`. Its paths are relative to each directory source. Direct file sources continue to be copied normally:

```yaml
project:
  build:
    src:
      - target/debug
      - target/release/helper
    files:
      - app
    dst: target/platforms
```

This collects `target/debug/app` as `target/platforms/<platform>-<arch>/debug/app`. Missing source paths and selected files are logged as skipped; existing paths that fail to transfer still fail collection for that target.

Existing target-only config files remain valid:

```text
local
FreeBSD amd64 builder@freebsd-vm:work/example-mxrun
```

If `MXRUN_CONFIG` or `--config` points to a file that does not exist yet, `mxrun` creates it automatically with a local target and no exclusions:

```yaml
targets:
  - local

project:
  ignore: []
  build:
    src:
      - build/stage/hello
    dst: target/platforms
```

That lets a first run start with a local-only config instead of aborting on a missing file.

To add a remote host to the config, use:

```bash
mxrun --add-host 203.0.113.10
mxrun -a 203.0.113.10
```

This currently does four things in order:

1. detects the current local user name
2. runs `ssh-copy-id <user>@<host>`
3. connects again over SSH and reads `uname -o` and `uname -m`
4. appends a config line using the same project path as the current local working directory

So if the current project root is `/home/alice/work/demo`, the added target line will use that same destination path on the remote side.

## What the project must provide

The producer project must provide two things. First, the selected build entry must actually exist and work on the target machine. Second, if result mirroring is enabled, the project must write a manifest file that lists the files that should be copied back.

The standard manifest path is:

```text
build/.mxrun/<entry>.paths
```

For a build entry named `devel`, the manifest path would therefore be:

```text
build/.mxrun/devel.paths
```

The manifest format is intentionally simple. It is just a line-based list of relative paths:

```text
build/stage/hello
build/stage/hello-helper
build/dist/example.wasm
```

Each line names one file or directory that should be copied back after a successful build. Blank lines are ignored. Lines beginning with `#` are treated as comments. Paths are interpreted relative to the project root.

This manifest is the boundary between `mxrun` and the producer project. The project decides what counts as a final deliverable. `mxrun` simply copies what the manifest names.

## What init does

`mxrun init` currently acts as a config-loading probe.

It reads `MXRUN_CONFIG`, parses the target file, and reports how many targets were loaded. That makes it useful for checking that the config file exists and that its lines parse as valid local or remote targets.

What it does not do yet is just as important: it does not create remote directories, it does not run `rsync`, and it does not bootstrap remote hosts as a separate standalone step. Remote preparation still happens as part of `mxrun run ...`.

Today `mxrun init` is therefore a validation-oriented command rather than a full remote setup command.

## What happens during a run

For a remote target, `mxrun` first synchronizes the current project tree to the destination directory with `rsync`. After that, it starts the selected build entry on the remote machine. On Linux-like systems it uses `make`. On FreeBSD it uses `gmake`. For the local target it simply runs the local build command without SSH.

If mirroring is enabled and the build succeeds, `mxrun` reads the manifest file for that entry and copies back only the listed paths. The copied files land under:

```text
target/mxrun/<OS-LABEL>/...
```

So, for example, a successful run might produce:

```text
target/mxrun/freebsd_14.2/build/stage/hello
target/mxrun/linux_6_glibc_2.39/build/stage/hello
```

Each target is handled independently. If one machine finishes before another, it can already start mirroring while the other machine is still compiling.

## A minimal Makefile example

The example below shows the smallest useful producer integration. It builds one executable into `build/stage` and then writes a manifest that lists that executable as the result worth copying back.

```make
STAGE_DIR := build/stage
MXRUN_MANIFEST_DIR := build/.mxrun

.PHONY: devel

devel:
 @mkdir -p $(STAGE_DIR)
 @cc -O0 -g src/main.c -o $(STAGE_DIR)/hello
 @mkdir -p $(MXRUN_MANIFEST_DIR)
 @printf '%s\n' \
  'build/stage/hello' \
  > $(MXRUN_MANIFEST_DIR)/devel.paths
```

With that Makefile in place, a matching `mxrun.conf` could look like this:

```yaml
targets:
  - local
  - FreeBSD amd64 builder@freebsd-vm:work/hello-mxrun

project:
  ignore: []
```

Now a full run becomes:

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel --mirror-results
```

In this example, the local machine runs `make devel`, the FreeBSD target runs `gmake devel`, and the file listed in `build/.mxrun/devel.paths` is copied back into the local `target/mxrun/...` tree.

## Common commands

The most common form is:

```bash
mxrun run devel
```

To validate that `MXRUN_CONFIG` loads and parses:

```bash
mxrun init
```

At the moment, `init` is a narrow validation command. It does not perform remote bootstrap on its own.

To enable result mirroring:

```bash
mxrun run devel --mirror-results
```

To override the default local destination for mirrored results:

```bash
mxrun run devel --mirror-results --mirror-root /tmp/mxrun-out
```

To use an explicit config file instead of `MXRUN_CONFIG`:

```bash
mxrun --config mxrun.conf run devel
```

## Runtime logs

While the TUI is running, `mxrun` keeps temporary logs in `.mxrun/`. These logs are only runtime scratch data. They are not part of the producer contract and they are not where the manifest lives.

At the finish popup, `Ctrl-C` quits and deletes those logs. Pressing `p` quits and preserves them so they can still be inspected afterwards. Any other key dismisses the popup and leaves the TUI open.
