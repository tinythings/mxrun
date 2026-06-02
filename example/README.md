# mxrun Example

This is a minimal producer project for mxrun.

The project supplies:

- a tiny hello world program in `hello.c`
- a local-only target config in `mxrun.conf`
- a producer Makefile that builds an artifact and writes the manifest file mxrun expects under `build/.mxrun/`

## Quick start

From this directory:

```bash
export MXRUN_CONFIG=mxrun.conf
make devel
```

If `MXRUN_CONFIG` is exported, the example Makefile delegates `devel` and `release` through `mxrun`. If `mxrun` is not installed in your PATH, it falls back to `cargo run --manifest-path ../Cargo.toml -- run ...`. Without `MXRUN_CONFIG`, the same targets run locally as plain Makefile entries.

To mirror results back through mxrun, use:

```bash
export MXRUN_CONFIG=mxrun.conf
mxrun run devel --mirror-results
```

The explicit Cargo form is still:

```bash
cd ..
cargo run -- run devel --mirror-results
```

The `devel` target builds `build/stage/hello` with verbose compiler output and writes:

```text
build/.mxrun/devel.paths
```

The `release` target builds `build/dist/hello` and writes:

```text
build/.mxrun/release.paths
```

With result mirroring enabled, mxrun copies the listed outputs back under:

```text
target/mxrun/<OS-LABEL>/...
```
