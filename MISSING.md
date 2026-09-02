# Missing Feature: Forward Make Variables to mxrun Build Entries

## Goal

Implement repeatable make-variable forwarding for mxrun run.

Example invocation:

mxrun run --make-var HOST=jackass _push-dev

## User-visible behavior

The current Kenpit development deployment builds through mxrun, then waits for the TUI to be quit before deployment runs locally. The desired behavior is one mxrun session: a worker builds Kenpit and that same worker runs deployment using explicit HOST.

The mxrun worker is the build executor. HOST is a separate deployment destination and must not be inferred from mxrun configuration.

The option is repeatable:

mxrun run --make-var HOST=deployer@example.test --make-var BUILD_KIND=debug _push-dev

Without make variables, existing behavior and generated command strings must remain exactly unchanged.

## CLI work

In src/clidef.rs, add a make-var argument to the existing run subcommand.

Required properties:

- long name: make-var
- value name: NAME=VALUE
- append action, allowing repeated use
- help: Forward a Make variable to every build entry; may be repeated

Add a CLI extraction helper beside existing entry and label helpers. It should return every supplied value from the run subcommand, or an empty vector when absent.

## Validation and security

Each option must be NAME=VALUE.

Validate NAME before synchronization, SSH, TUI startup, or Make execution:

- it is nonempty;
- first character is ASCII alphabetic or underscore;
- later characters are ASCII alphabetic, digit, or underscore;
- empty VALUE is valid;
- reject missing equals sign, numeric-leading names, hyphenated names, and shell syntax in names.

Recommended error text:

mxrun: --make-var must use NAME=VALUE
mxrun: --make-var NAME must be a Make identifier

Values must pass to Make as exactly one command-line assignment. Spaces, apostrophes, at signs, colons, and shell-significant characters must not turn into shell syntax.

Use POSIX single-quote escaping for each complete NAME=VALUE assignment. An apostrophe in a value uses the normal close-quote, escaped-apostrophe, reopen-quote pattern. Do not concatenate unvalidated raw user text into a command.

## Internal design

Add a small validated MakeVariable type near command construction in src/runner.rs.

It should own the complete validated assignment string, derive Clone, Debug, PartialEq, and Eq, and expose:

- parse: takes String and returns a result or the validation errors above;
- an internal method that converts the complete assignment into one safely quoted POSIX shell argument.

The implementation may use Rust split_once on equals sign, enumerate the name characters for identifier validation, and String replacement for apostrophe escaping.

In src/main.rs:

- import MakeVariable with BuildPlan;
- add a vector of MakeVariable to RunOptions;
- parse all clidef make-var values in RunOptions::from_matches using existing fatal error handling;
- pass the validated variable vector to BuildPlan::new.

This ensures invalid input fails before remote work begins.

## Runner wiring

Current flow:

BuildPlan::new -> BuildJob::build_with_config -> BuildCommand::for_target -> local or remote command constructor

Thread a slice of MakeVariable values through that chain.

Keep the public BuildJob::build test helper signature unchanged if practical. Its internal call should use an empty variable slice, preserving unrelated tests.

Add a BuildCommand helper that joins variables as a sequence of space-prefixed, safely quoted assignment arguments.

Local command must retain both recursion guards and append variables after entry:

MXRUN_CONFIG= MXRUN_LOCAL_MAKE= make _push-dev quoted-HOST-assignment

Remote command must append the same quoted variables after entry:

cd remote-path && remote-make _push-dev quoted-HOST-assignment

No variables must retain the existing local make dev and remote make dev command strings exactly.

## Test plan

Add focused tests in src/runner_ut.rs.

1. Parse an ordinary HOST assignment and an assignment containing an apostrophe plus spaces.
2. Build both remote and local commands with BuildCommand::for_target.
3. Assert each resulting command contains the assignments once, after the entry, as single safely escaped shell arguments.
4. Reject missing delimiter, numeric-leading name, and hyphenated name.
5. Accept empty value, underscore-leading name, and a digit after the first character.
6. Preserve the existing remote command test without variables, including its old gmake dev assertion.

Unit tests are the primary proof because normal mxrun execution launches a TUI and can require configured workers.

## mxrun validation

Run from the mxrun repository root:

cargo fmt --all -- --check
cargo test
cargo clippy -- -D warnings
cargo build --release
./target/debug/mxrun run --help

The help output must list make-var with NAME=VALUE.

## Kenpit follow-up, separate change

Do this only after mxrun support is implemented and validated.

Change Kenpit root Makefile so public push-dev does not depend on public dev. The current public dev dependency causes build through mxrun and local deployment only after the TUI exits.

Required design:

- push-dev verifies HOST is present;
- push-dev invokes maybe-mxrun with entry _push-dev;
- MXRUN_ARGS contains make-var HOST with the caller value;
- local fallback invokes make _push-dev with the same HOST assignment;
- _push-dev depends on existing non-mxrun build entry _dev;
- _push-dev calls dev/scripts/push-dev.sh.

Constraints:

- add _push-dev to .PHONY;
- preserve MXRUN_CONFIG and MXRUN_LOCAL_MAKE recursion controls;
- maybe-mxrun.sh already places MXRUN_ARGS before the requested entry;
- do not change its existing mxrun nonzero-status policy without a separate decision;
- worker-side deployment means SSH and rsync execute from every selected worker;
- each selected worker needs network access and credentials for HOST;
- multiple configured workers will deploy concurrently. Use one configured target or design explicit target selection separately; do not add implicit winner election.

After Kenpit integration, perform at least a Make dry-run with HOST defined, formatting checks, and git diff check. Do not use real deployment as the first proof unless worker topology and host access are intentional.

## Non-goals

- Do not store deployment hosts in mxrun configuration or source control.
- Do not forward all environment variables.
- Do not accept arbitrary shell fragments.
- Do not change synchronization, artifact mirroring, TUI behavior, target configuration, or existing recursion guards.
- Do not modify unrelated Kenpit frontend work.
