# prevtest

prevtest (previous test) is a test caching solution for nextest.

## Status

Work in progress.

The main proposal is described in the [high-level design document](docs/high-level-design.md).

So far, prevtest implements only the first increment (binary-based caching).

## Installation

From this crate's directory:

```sh
cargo install --path . --locked
```

## Usage

Add this to your project's `.config/nextest.toml`:

```toml
experimental = ["wrapper-scripts"]

[scripts.wrapper.prevtest]
command = "prevtest"
target-runner = "overrides-wrapper"

[[profile.default.scripts]]
filter = "all()"
run-wrapper = "prevtest"
```

Then run:

```sh
cargo nextest run
```

## Options

| Environment variable | Default | Description |
| --- | --- | --- |
| `PREVTEST_FORCE` | `false` | Run tests even when cached, still updating the cache. Same as `--force`. |
| `PREVTEST_CACHE_DIR` | `.prevtest/cache` | Test results. |
| `PREVTEST_STATE_DIR` | `.prevtest/state` | Machine-local state. |

- Relative directories resolve from the workspace root.
- **Only the cache directory is meant to be saved and restored in CI.** The state directory holds information that only applies to the machine that wrote it.
