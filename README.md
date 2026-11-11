# prevtest

Work in progress test caching solution for nextest.

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

Set `PREVTEST_FORCE=1` to force test execution while still updating the cache.
