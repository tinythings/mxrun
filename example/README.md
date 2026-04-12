# xrun Example

This is a minimal producer project for xrun.

The project supplies:

- a tiny hello world program in `hello.c`
- a local-only target config in `xrun.conf`
- a producer Makefile that builds an artifact and writes the manifest file xrun expects under `build/.xrun/`

## Quick start

From this directory:

```bash
export XRUN_CONFIG=xrun.conf
make devel
```

If `XRUN_CONFIG` is exported, the example Makefile delegates `devel` and `release` through `xrun`. If `xrun` is not installed in your PATH, it falls back to `cargo run --manifest-path ../Cargo.toml -- run ...`. Without `XRUN_CONFIG`, the same targets run locally as plain Makefile entries.

To mirror results back through xrun, use:

```bash
export XRUN_CONFIG=xrun.conf
xrun run devel --mirror-results
```

The explicit Cargo form is still:

```bash
cd ..
cargo run -- run devel --mirror-results
```

The `devel` target builds `build/stage/hello` with verbose compiler output and writes:

```text
build/.xrun/devel.paths
```

The `release` target builds `build/dist/hello` and writes:

```text
build/.xrun/release.paths
```

With result mirroring enabled, xrun copies the listed outputs back under:

```text
target/xrun/<OS-LABEL>/...
```
