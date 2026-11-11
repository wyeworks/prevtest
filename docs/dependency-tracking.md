# Dependency tracking

## Runtime and standard-library hooks

**Use and coverage:** Instrument APIs such as file opens and `getenv` while the
program executes. Hooks see high-level arguments/results, including environment
reads that do not enter the kernel.

**Limits:** Only instrumented paths are visible; foreign code, direct syscalls,
or children using another runtime can bypass hooks. Requires runtime cooperation
or rebuilding with instrumentation.

**Platforms and permissions:** Linux/macOS when the runtime supports them;
typically no privileges beyond running the program. Cost is paid at hooked calls.

**Sources:** [Go's hooks](https://github.com/golang/go/blob/go1.27.2/src/internal/testlog/log.go).

## Dynamic-library injection and interposition

**Use and coverage:** `LD_PRELOAD` (Linux) and `DYLD_INSERT_LIBRARIES` (macOS) load
code at startup. The library can replace selected functions with logging wrappers
(interposition), or start an engine such as Stalker to instrument instructions.
Loading alone records nothing; the instrumentation determines coverage.

Rust's [shared syscall helper](https://github.com/rust-lang/rust/blob/main/library/std/src/sys/pal/unix/weak/syscall.rs)
explicitly prefers libc symbols to support `LD_PRELOAD` interposition. Missing
symbols fall back to `libc::syscall`, bypassing hooks on the individual functions.

**Limits:** Symbol interposition misses direct syscalls and calls that bypass
replaced symbols. Preloading needs a compatible dynamic loader, excluding fully
static binaries. Children need compatible loading; inherited variables alone do
not guarantee coverage.

**Platforms and permissions:** Usually unprivileged for ordinary owned programs.
Linux secure execution restricts preloading. macOS SIP strips loader variables
from protected launches; Hardened Runtime and library validation can reject
injection. Wrappers add per-call work without a tracer round trip.

**Sources:** [Linux loader](https://man7.org/linux/man-pages/man8/ld.so.8.html),
[Frida loading modes](https://frida.re/docs/modes/),
[Apple runtime restrictions](https://developer.apple.com/library/archive/documentation/Security/Conceptual/System_Integrity_Protection_Guide/RuntimeProtections/RuntimeProtections.html),
[library validation](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation).

## ptrace

**Use and coverage:** Stop traced threads at syscall entry/exit, inspect registers
and memory, and follow fork/clone/exec events. Covers direct syscalls and static
binaries; a tracer can decode filesystem, network, and process operations.

`PTRACE_GET_SYSCALL_INFO` exposes syscall numbers, raw arguments, and results.
The tracer still decodes architecture-specific ABIs, reads strings/structures
from process memory, and tracks descriptors and relative paths. `strace` handles
this decoding, with filters such as `%file` (paths) and `%desc` (descriptors).

**Limits:** Syscall tracing does not observe ordinary memory reads, including
`getenv`, individual accesses to mapped files, or time reads served through vDSO.
Every relevant thread/child and syscall ABI needs coverage. Frequent stops cost time.

**Platforms and permissions:** Linux usually permits tracing one's own child
without root, subject to Yama and container policy. macOS has a different ptrace
API without Linux's syscall-stop interface.

**Optimization:** [seccomp-BPF filtering](#seccomp-bpf-filtering) selects which
syscalls stop for ptrace inspection; the rest execute without tracing stops.
`strace --seccomp-bpf -f` supports this. Savings depend on how many stops it avoids;
ptrace still reads path strings and decodes results for the selected calls.

**Sources:** [Linux API](https://man7.org/linux/man-pages/man2/ptrace.2.html),
[strace options](https://man7.org/linux/man-pages/man1/strace.1.html),
[Yama](https://www.kernel.org/doc/html/latest/admin-guide/LSM/Yama.html),
[vDSO](https://man7.org/linux/man-pages/man7/vdso.7.html),
[Apple API](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/ptrace.2.html).

## seccomp-BPF filtering

**Use and coverage:** Kernel filters select syscalls by number, architecture,
instruction pointer, and raw argument values. They can deny calls or send selected
ones to ptrace, reducing stops for irrelevant operations.

**Limits:** Filters cannot dereference argument pointers to inspect paths and do
not report completed results. Allowed calls still need another observation layer.
This classic-BPF filtering is distinct from privileged eBPF tracing.

**Platforms and permissions:** Linux-only. A process can install filters after
setting `no_new_privs`, without root; kernel/container support is required.
Filters persist across allowed fork/exec operations.

**Sources:** [seccomp API](https://man7.org/linux/man-pages/man2/seccomp.2.html).

## seccomp user notification

**Use and coverage:** A selected syscall blocks while a supervisor receives its
arguments. The supervisor can read pointed-to data, emulate an operation, return
a result, and inject file descriptors into the caller.

**Limits:** Continuing an original syscall after inspecting mutable memory can
race with other threads. Notification itself does not provide the eventual
result of a continued call. Emulation needs accurate syscall semantics and adds
supervisor round trips.

**Platforms and permissions:** Linux-only, with user notification available since
5.0 and features added later. Filter setup can be unprivileged; reading another
process's memory has separate access checks. No macOS equivalent API.

**Sources:** [Notification API and races](https://man7.org/linux/man-pages/man2/seccomp_unotify.2.html),
[Syd's implementation](https://docs.rs/syd/latest/syd/).

## eBPF tracing

**Use and coverage:** Kernel programs attached to tracepoints or function probes
record selected operations and process activity independently of libc. They can
filter and aggregate before sending events through buffers to a collector.

**Limits:** Coverage depends on probe selection, process attribution, and event
delivery. Buffer exhaustion can lose observations; a cache must detect incomplete
traces. Ordinary user-space memory reads are outside syscall/kernel probes.

**Platforms and permissions:** Linux; tracing requires root or suitable capabilities
and kernel support. No native macOS backend. Kernel-side filtering reduces
collector work but does not remove probe/event-processing cost.

**Sources:** [bpftrace capabilities](https://bpftrace.org/release-025-notes),
[kernel ring buffers](https://docs.kernel.org/bpf/ringbuf.html),
[BuildXL hooks](https://github.com/microsoft/BuildXL/blob/8782d1593b3c8fe54f2c7223641b903eadcef599/Public/Src/Sandbox/Linux/ebpf/sandbox.bpf.c).

## kdebug and ktrace

**Use and coverage:** XNU tracepoints write timestamped kernel events to buffers;
ktrace sessions collect them in userspace. `fs_usage` decodes syscall, filesystem,
and paging events. Collection avoids the per-syscall stop/inspect/resume exchange
with a userspace tracer used by Linux ptrace.

**Limits:** Only emitted events and their fields are visible; ordinary memory
reads, including `getenv`, are not. Buffer overruns lose events. Decoding paths
and correlating threads/processes still requires work.

**Platforms and permissions:** macOS; `fs_usage` requires root for live collection.
Its collector uses Apple's private libktrace interfaces. Buffering avoids tracer
round trips but still adds kernel recording and userspace processing costs.

**Sources:** [XNU event buffers and loss reporting](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kdebug.c),
[fs_usage collection and private interfaces](https://github.com/apple-oss-distributions/system_cmds/blob/main/fs_usage/fs_usage.c).

## DTrace

**Use and coverage:** Provider probes expose events such as syscall entry/return
and process creation; scripts filter by process and collect arguments/results.
Helpers such as `copyinstr` read strings from process memory. Suitable for
observing native programs without source changes.

**Limits:** Available providers and process visibility bound coverage. File-open
probes alone do not establish complete inputs; scripts must follow descendants
and correlate events. Tracing and event collection add execution cost.

**Platforms and permissions:** macOS syscall tracing normally needs root; sandbox
policy can also deny access. SIP blocks protected system processes even as root,
but its warning alone does not establish that an owned test binary is blocked.
`sudo dtruss ./test` also runs the test as root; attaching a privileged tracer by
PID to a separately launched test preserves the test's user permissions. Linux
availability depends on the installed DTrace port/kernel.

**Sources:** [Apple runtime protections](https://developer.apple.com/library/archive/documentation/Security/Conceptual/System_Integrity_Protection_Guide/RuntimeProtections/RuntimeProtections.html),
[Oracle Linux DTrace](https://docs.oracle.com/en/operating-systems/oracle-linux/dtrace-v2-guide/).

## Endpoint Security

**Use and coverage:** Observe macOS filesystem/process events and authorize
supported operations. `es_new_descendants_client` delivers notifications for the
caller and notification/authorization events for its descendants.

**Limits:** Coverage is limited to supported event types. Sequence-number gaps
reveal lost events; authorization handling adds latency and has response deadlines.

**Platforms and permissions:** macOS only. System-wide clients require root and
TCC Full Disk Access. The descendant-scoped client requires macOS 27+, without
root or privacy (TCC) approval. Both require a signed client with Apple's restricted
Endpoint Security entitlement, granted on request.

**Sources:** [Descendant client](https://developer.apple.com/documentation/endpointsecurity/es_new_descendants_client(_:_:)),
[entitlement requirements](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.endpoint-security.client),
[system-wide client permissions](https://developer.apple.com/videos/play/wwdc2022/110345/?time=407),
[event loss](https://developer.apple.com/documentation/endpointsecurity/es_message_t/seq_num),
[response deadlines](https://developer.apple.com/documentation/endpointsecurity/es_message_t/deadline).

## Filesystem interception: FUSE

**Use and coverage:** A userspace filesystem handles operations on a mounted view,
allowing observation of opens, metadata, missing paths, reads, and writes below libc.

**Limits:** Paths outside that view, network, environment, and clocks are not
covered. Kernel caching and process attribution need care; mounting and forwarding
I/O add costs. Containing all relevant accesses requires a separate access policy.
NFS-based workarounds (like FUSE-T) can avoid kernel extensions, but client caching
can delay visibility of other clients' changes, potentially causing races in
concurrent workloads.

**Platforms and permissions:** Linux supports unprivileged mounts through FUSE
infrastructure. On macOS, macFUSE requires installation: its VFS backend uses a
kernel extension; FSKit avoids that extension but has compatibility and caller-context
limitations. Backend support cannot be assumed interchangeable.

**Sources:** [Linux FUSE](https://docs.kernel.org/filesystems/fuse/fuse.html),
[macFUSE backends](https://github.com/macfuse/macfuse/wiki/FUSE-Backends),
[NFS name-cache consistency](https://www.rfc-editor.org/rfc/rfc7530.html#section-10.8).

## File-change notifications

**Use and coverage:** Linux inotify and macOS FSEvents can identify changes to
known inputs between executions, reducing repeated filesystem scans.

**Limits:** They do not provide a complete per-test dependency trace. inotify
also reports opens/accesses but lacks the originating process identity. Events
can be coalesced or dropped; overflow requires rescanning/revalidation.

**Platforms and permissions:** Native APIs on their respective OSes, generally
usable without root for accessible paths. Watch limits, event backlog, and
recovery scans affect cost.

**Sources:** [inotify](https://man7.org/linux/man-pages/man7/inotify.7.html),
[FSEvents](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html).

## Declarations and sandboxing

**Use and coverage:** Declarations select allowed inputs; sandbox rules restrict
access during execution and can expose missing declarations. Filesystem views,
write restrictions, and network isolation are separate policy choices.

**Limits:** A symlink tree alone does not block absolute host paths. Sandboxing
does not reveal which permitted inputs were used or prove they remain unchanged.
Broad declarations invalidate unused inputs; setup is paid per sandboxed unit.

**Platforms and permissions:** Linux commonly combines user/mount namespaces,
subject to host policy. macOS `sandbox-exec` applies profiles without requiring
root for ordinary use, but its interface is deprecated, although widely used
tools such as Bazel and Codex still rely on it.

**Sources:** [Bazel's strategies](https://bazel.build/docs/sandboxing),
[Linux user namespaces](https://man7.org/linux/man-pages/man7/user_namespaces.7.html),
[Apple on sandbox-exec](https://developer.apple.com/forums/thread/661939),
[Codex sandboxing](https://learn.chatgpt.com/docs/agent-approvals-security#os-level-sandbox).

## Binary rewriting and dynamic instrumentation

**Use and coverage:** Patch instruction sites or execute instrumented copies of
basic blocks. Hooks can observe direct syscalls or selected memory accesses
without changing source code; zpoline and Frida use different forms of this technique.

**Limits:** Only covered instructions/threads are observed; new code and children
need handling. Architecture-specific decoding and OS-specific syscall semantics
remain necessary. Rewriting, translation, and callbacks add work.

**Platforms and permissions:** Linux/macOS with suitable backends. A preloaded
engine instruments its own process without external attach authorization;
external injection may require permission to control another process.
Both remain subject to library-loading and executable-memory restrictions.
zpoline additionally requires address-zero mapping permission.

**Sources:** [Frida Stalker](https://frida.re/docs/stalker/),
[preloaded Gadget](https://frida.re/docs/gadget/#script),
[injection permissions](https://frida.re/docs/troubleshooting/),
[zpoline](https://github.com/yasukata/zpoline/blob/56aec879727af984ed7ebc28067f4f9c7cb60bdf/README.md).

## Static analysis and compiler instrumentation

**Use and coverage:** Symbols, relocations, or compiler IR expose possible code/API
dependencies. Inserted probes record functions or source regions actually executed.
These can classify effects or refine selection after code changes.

**Limits:** Possible API use does not identify runtime paths or values. Previous
coverage does not guarantee that a changed execution follows the same path.
External inputs still need observation or declarations.

**Platforms and permissions:** No inherent root requirement on Linux/macOS;
compiler/tool support determines portability. Requires build integration, with
analysis costs and possible compiler-version coupling.

**Sources:** [Cackle analysis](https://github.com/cackle-rs/cackle/blob/f9f09eeb1f016aaf50adb8e7503efeef3d28595d/HOW_IT_WORKS.md),
[RustyRTS modes](https://github.com/tum-i4/rustyrts/blob/5f09a1346a71bb8470ca5eadb5a14eec6d980fbd/README.md).

## Emulation and runtime interfaces

**Use and coverage:** QEMU user-mode translates machine instructions and syscalls;
WASI modules request external services through host imports. Both expose boundaries
where an implementation can record requests and results.

**Limits:** Events still need resource interpretation and validation. Emulation
adds translation costs; WASI changes the compilation target and available APIs.
Neither automatically supplies deterministic inputs.

**Platforms and permissions:** Ordinary user-mode execution needs no tracing
privileges. QEMU user-mode targets Linux/BSD binaries, not native macOS programs.
Wasmtime runs on Linux/macOS, subject to guest-target compatibility.

**Sources:** [QEMU user-mode](https://www.qemu.org/docs/master/user/main.html),
[TCG callbacks](https://www.qemu.org/docs/master/devel/tcg-plugins.html),
[Wasmtime security](https://docs.wasmtime.dev/security.html),
[Rust WASI limitations](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1.html).

## Input manifests and fingerprints

**Use and coverage:** Persist discovered/declared dependencies, then compare code,
arguments, environment, and resource state before reuse. Relevant state can include
contents, metadata, directory entries, and failed lookups, not just existing files.

**Limits:** Hashing cannot recover missing dependencies; mtime/size shortcuts can
hide changed contents. Inputs changing during observation/validation need a
consistency policy. A conservative cache must decline reuse for unverifiable
inputs. Restoring generated artifacts is separate from reusing a verdict when
those files are disposable.

**Platforms and permissions:** Linux/macOS with access to the resources being
validated. Content hashing reads data; metadata checks cost less but are weaker.

**Sources:** [Go's revalidation](https://github.com/golang/go/blob/go1.27.2/src/cmd/go/internal/test/test.go#L2018),
[Riker's dependency model](https://www.usenix.org/system/files/atc22-curtsinger.pdf),
[Gradle outputs](https://docs.gradle.org/9.8.1/userguide/build_cache.html).
