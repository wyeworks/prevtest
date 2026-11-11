# Nextest caching: High level design

## Objective

Integrate an optional test caching solution with nextest.

## Naming

`prevtest` plays on the "next" in nextest and refers to reusing previous test results.

I may use this name elsewhere in the document.

## Requirements

- Support Linux and macOS.
- Store results per test.
- Support saving and restoring the cache between CI runs.
- Determine cache inputs before executing the test.
- Allow users to bypass caching and force execution.

## Increments

Each increment provides usable value with explicit limitations and can be tried
without waiting for later increments. Caching uses nextest's existing run-wrapper
interface. Displaying cache labels and category counts requires
[wrapper reporting](https://github.com/nextest-rs/nextest/pull/3650). This and other
proposed integrations may require a patched build of nextest until released.
Implementation order is flexible.

### Binary-based caching

A nextest run wrapper stores and reuses results by test and binary identity.
Initially, only successful test results would be cached.

> More details: [Binary identity](design-notes.md#binary-identity).

> [!IMPORTANT]
> A binary contains many tests, so changes to crate code invalidate all cached
> results for tests in that binary.

We can start with SQLite as a safe, balanced storage choice and move to another
mechanism if needed.

The cache storage path should be configurable so CI jobs running different
subsets of tests can maintain separate caches.

### Filesystem isolation

Initially, tests would run in a temporary working directory containing symlinks
to the binary and required runtime files, with space for temporary files.
This helps identify dependencies on files in the original working directory
without requiring input declarations. Absolute paths would remain accessible.

If stronger restrictions are needed, we could add `bwrap` on Linux and
`sandbox-exec` on macOS.

Since nextest starts a process per test, setting up and tearing down an environment
with each test's own dependencies could be costly for short tests. Prevtest could
retain the prepared directory and its setup state, reusing it when it still matches
the required inputs and isolation settings. Writable test state would need to stay
separate or be cleaned between uses so tests do not interfere with each other.

With declarations, nextest filters could assign inputs to individual tests or a
whole binary. Giving every test in a binary the same inputs could allow a shared
environment. To keep cache reuse correct, changes to any of those inputs would
need to invalidate all cached test results for that binary.

A failure under isolation may also be caused by flakiness, so users are still
responsible for investigating its cause.

### Declared inputs

Users can declare fixtures and other inputs needed by their tests. These inputs
are made available during restricted execution and included in cache validation.
Changes invalidate the results of the tests that depend on them.

Declarations would initially live in a single `prevtest.toml` at the workspace
root, following nextest's [filtersets](https://nexte.st/docs/filtersets/reference/)
and [profile overrides](https://nexte.st/docs/configuration/per-test-overrides/).
Rules could select individual tests, modules, or entire binaries. For example:

```toml
[profile.default]
filesystem = { isolation = "none" }

[[profile.default.overrides]]
filter = 'binary_id(=my-crate) and test(=tests::loads_defaults)'
files = ["fixtures/defaults.json"]
filesystem = { isolation = "sandbox" }

[[profile.ci.overrides]]
filter = 'all()'
filesystem = { isolation = "sandbox" }
```

The wrapper would use `NEXTEST_PROFILE` to select the profile, following nextest's
inheritance and override precedence per setting. Here, CI enables isolation for
all tests while retaining the declared files
([inherited](https://nexte.st/docs/configuration/#profile-inheritance) from `default`). For
`files`, the first matching rule that defines it would supply the complete list,
without merging other rules' lists.

> [!NOTE]
> Pending decision:
>
> Filters reduce repetition, but keeping declarations in sync with test renames
> and moves could still become cumbersome. We may prefer
> [using macros](#input-declarations-through-macros) for input declarations,
> keeping profiles and general settings in TOML.

## Optional increments to consider

### Client-server wrapper

Nextest would still start a wrapper process per test. Each wrapper would act as
a client, using IPC to share cache state and coordinate writes through a prevtest
server.

This fits the existing wrapper interface, but needs server startup and shutdown
management. A nextest teardown hook could signal the end of a run, requiring
an additional upstream change.

### Persistent wrapper managed by nextest

Nextest could instead start prevtest once per run and send test execution
requests over stdio. Nextest would manage the process's lifecycle, avoiding a
wrapper process per test.

This requires a new execution interface in nextest beyond the
[wrapper reporting PR](https://github.com/nextest-rs/nextest/pull/3650), which adds
file-based reports to the existing per-test wrappers.

### Network isolation

Blocking external network access helps identify tests that depend on remote
services. This restriction can be enabled independently of filesystem isolation.

### Input declarations through macros

A Rust attribute macro could emit a record with the test's full name and declared
inputs. The linker would collect these records in a dedicated executable section.
The wrapper would then read it to find the test's declarations before execution.
This preserves the standard test harness, avoiding a custom main with `harness = false`.

Users would need a macro dependency, and declaration changes would require
recompilation. We would need to support ELF and Mach-O metadata sections and ensure
records survive linker optimizations and symbol stripping.

### Finer-grained code-change invalidation

Finer invalidation could distinguish changes confined to one test from changes
to shared code.

### Declaration assistance

An optional tracing tool could generate candidate input declarations for users
to review.

> More details: [Automatic dependency tracing](design-notes.md#automatic-dependency-tracing).

## Cache invalidation

Cache validation uses the content hashes of the binary and declared files.
A different hash invalidates the cached result.

A possible shortcut is to reuse file hashes while mtime and size match. This
metadata reflects local filesystem state, so the hash cache could be stored
separately from test results reused in CI.

If both the stored and current mtimes have zero nanoseconds, we would assume
whole-second precision and recompute the hash anyway. That assumption may be
wrong, but then we only pay for an unnecessary hash calculation.

The shortcut would matter less with a persistent server that hashes each binary
once per run and shares the result across tests.

Inputs are assumed to remain stable during test execution.

Changes to declared input paths or effective execution settings invalidate the
result.

> [!NOTE]
> Pending decision:
>
> Which environment variables and invocation arguments affect reuse.

## Challenges

Nextest invokes the run wrapper separately for each test. This makes optimizations
such as grouping cached results by binary harder to coordinate, and may require
each invocation to reload declarations and cache metadata.

## Benchmarks

We would compare total suite time with and without prevtest, measuring cache hits,
misses, and isolation overhead separately. Short tests are worth checking because
validation and setup could cost more than running the test.

A small fixture would provide controlled cases. Candidates for larger trials are
rust-analyzer for its many tests, and ripgrep for tests that create files and
launch subprocesses, exercising filesystem isolation and input declarations.
Measurements would exclude compilation and cover reuse after restoring the cache
in CI.
