# mxrun

mxrun runs one build entry across local and remote machines in parallel.

It syncs the project tree, starts the same make target on each machine, and can mirror back only the final artefacts your project lists.

## Why

Projects often need one local build and one or more remote builds, such as FreeBSD or Linux VMs. mxrun keeps that flow in one terminal UI instead of many shells and ad-hoc rsync steps.

## What mxrun owns

- Target loading from `MXRUN_CONFIG` or `--config`
- Remote directory creation and project sync with `rsync`
- Running one build entry on every target
- Optional result mirroring from `build/.mxrun/<entry>.paths`

## What the project owns

- The actual `make` or `gmake` targets
- The manifest of final deliverables
- Any project-specific build logic

## Quick start

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel
```

Mirror back listed outputs:

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel --mirror-results
```

Validate the config file:

```bash
mxrun init
```

Add a remote host:

```bash
mxrun -a 203.0.113.10
```

## Config format

```text
local
FreeBSD amd64 builder@freebsd-vm:work/example-mxrun
GNU/Linux x86_64 builder@linux-vm:work/example-mxrun
```

If the config file does not exist yet, mxrun creates it with:

```text
local
```

## Producer contract

For an entry named `devel`, the project writes:

```text
build/.mxrun/devel.paths
```

The file is a line-based list of relative paths to mirror back after a successful build.

## Example

See [example/README.md](example/README.md) for a minimal producer project.

## More docs

- Full user guide: [doc/README.md](doc/README.md)
- Man page source: [doc/manpage/buildfarm.1.md](doc/manpage/buildfarm.1.md)

## Licence

MIT. See [LICENSE](LICENSE).
