# mxrun Example

This is a minimal producer project for mxrun.

The project supplies:

- a tiny hello world program in `hello.c`
- a local-only target config in `mxrun.conf`
- a producer Makefile that builds an artifact

## Quick start

From this directory:

```bash
export MXRUN_CONFIG=mxrun.conf
make devel
```

If `MXRUN_CONFIG` is exported, the example Makefile delegates `devel` and `release` through `mxrun`. If `mxrun` is not installed in your PATH, it falls back to `cargo run --manifest-path ../Cargo.toml -- run ...`. Without `MXRUN_CONFIG`, the same targets run locally as plain Makefile entries.

The configured `project.build` section automatically collects successful outputs under:

```text
target/platforms/<platform>-<arch>/build/stage/hello
```
