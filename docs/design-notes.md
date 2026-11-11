## Binary identity

Results from a small fixture using Rust 1.95.0 on Linux and macOS:

- When Cargo skipped compilation and kept the existing executable, its contents,
  hash, size, mtime, ctime, and inode stayed unchanged.
- When Cargo recompiled and replaced the executable, mtime/ctime changed, even
  when the new executable had identical contents and hash.
- Behavior change: different hash and failing tests, but the same binary size.

Recompiling unchanged source produced identical binary contents in Linux `test`
and `release` builds, and macOS `release` builds. macOS `test` builds with
incremental compilation differed in debug metadata and the code signature.
Disabling incremental or running `strip -S` on copies removed those differences.

`mtime + size` may be enough for a quick check before rehashing. If a rebuild
changes mtime but produces the same hash, we can keep the cached test results
instead of invalidating them just because the executable was replaced.

Rapid rebuilds on macOS with Rust 1.99.1 took a median of 0.216 s. In 16 of
20 rebuilds, content changed while size and `mtime` truncated to seconds matched
the previous build. Full-precision timestamps differed in all 20.
Related discussion: [Cargo's timestamp and checksum trade-offs](https://github.com/rust-lang/cargo/issues/17626).

## Automatic dependency tracing

Without language/runtime cooperation like Go's standard-library hooks, we need another
way to observe the test's inputs.

- **Linux:** [ptrace](dependency-tracking.md#ptrace) observes syscalls, usually without root.
- **macOS:** ptrace lacks Linux's syscall-stop interface.
  [DTrace](dependency-tracking.md#dtrace) requires privileges.
  A session helper could avoid repeated authorization
  while keeping tests under the user's normal privileges.

Preloading offers cross-platform, unprivileged alternatives to system tracing
for compatible binaries. [Libc interposition](dependency-tracking.md#dynamic-library-injection-and-interposition)
intercepts filesystem functions but misses direct syscalls and calls bypassing
those hooks, a coverage limitation similar to Go's.
[Dynamic binary instrumentation](dependency-tracking.md#binary-rewriting-and-dynamic-instrumentation)
with an engine such as Frida Gadget with Stalker rewrites executed code and can
observe direct syscalls too.

See [dependency tracking](dependency-tracking.md) for other techniques evaluated.
