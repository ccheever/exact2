# Level-zero hosts still link and publish the update store

**Status:** Closed
**Resolution:** Completed the upstream Rust split with a separate ExactUpdates Swift target and two bake-selected app compositions; Level 0 skips stream discovery and emits binary rows only. Paired linked artifacts, all native runtime/network probes, and sample-host smokes prove updater omission while ordinary networking remains available.
**Systems:** Build, Delivery, Apple host, Linux host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030 D4; LLP 1030.000 D4

LLP 1030 D4 defines `L = 0` as a link-time property: the updater crate, keys,
store, check, and network path are absent, and the only carrier is a binary.
The implementation makes it a runtime flag instead.

`exact-update` is an unconditional dependency of both native hosts
(`host/apple/Cargo.toml`, `host/linux/Cargo.toml`), and both hosts reference
its client and exports unconditionally. `Baked::from_compat()` merely sets
`store_linked = false`; `Client::open()` then returns a refusal at runtime
(`update/src/binary.rs::Baked::from_compat`, `update/src/client.rs::Client::open`).
Apple's `ExactApp` initializer also calls `Updates.open` unconditionally
(`host/apple/Sources/ExactKit/Session.swift`). The flag is consulted by the
updater at runtime; it does not choose a host composition without it.

The publisher has the matching semantic error. `streamPlatforms()` treats the
presence of a key under `deploy.store` or `deploy.binaries` as a stream,
regardless of whether `store.<platform>` is `0`
(`scripts/deploy.mjs::streamPlatforms`). It can therefore offer a bundle stream to a
client whose only legal carrier is the binary.

Done when `L = 0` selects a build/link composition with no updater symbols,
keys, persistent store, or check path, without adding a cargo-feature matrix
to a core crate; and when deploy emits only the platform's binary row. Hold it
with link-map/artifact inspection, a running-app check, and a deploy-table
fixture for `store.linux = "0"`; paired size measurements quantify the benefit
but do not prove the machinery absent by themselves.

## Implementation direction and evidence boundary (2026-09-04)

Charlie requested a follow-up during the project-principles review. Think
of the updater as equipment in a box: marking it disabled does not remove
the equipment. Level zero should build the box without the updater.

Keep generic boot/apply/session behavior in the host. Compose update-store
ownership, exports, launch selection, and the after-paint check only into
an app artifact that chose updates. Use the existing separate-artifact /
composition rule, not a cargo feature on a core crate. Preserve a useful
`state.delivery` answer for a binary-only app without linking the client.
The debug dev connection remains available on its own terms (LLP 1031 D11).

`cargo tree -p exact-apple --depth 1 --edges normal` and the corresponding
Linux command confirm the unconditional dependency; the initialization and
runtime-refusal paths were inspected. This review has not built and measured
a paired `L = 0` / `L = A` release artifact. Do not infer an exact byte or
millisecond saving from the dependency graph. Closure requires actual
artifact inspection and a running level-zero app: no updater implementation,
no store creation/check, correct delivery state, and binary-only deploy rows.

## Verification (2026-09-04)

The native host, adapter, compiler and updater tests pass, including selected
asset tombstones and corruption fallback, stalled downloads, dev-plan precedence,
compatibility metadata without keys, and a Node execution of the real publisher
that produces one binary row with zero stream reads for `store.linux = "0"`.
Core entries refuse metadata that claims a delivery adapter they did not link.

A disposable Caltrain workspace built paired L=0/L=A release artifacts with the
same dependencies and optimization settings, retaining symbols for inspection:

| Artifact | L=0 bytes | L=A bytes | Difference |
|---|---:|---:|---:|
| Linux executable, arm64 macOS headless target | 8,699,280 | 8,952,752 | 253,472 |
| Apple Rust archive (not the final app size) | 28,419,632 | 30,574,864 | 2,155,232 |

`nm` found no `exact_update`, adapter, `ed25519_dalek` or `curve25519_dalek`
symbols in either L=0 artifact; their L=A counterparts contained 116 and 359
matching symbol lines. Neither L=0 artifact contained the manifest's verification
key id or public key. The linked Swift L=0 executable also had no such symbols.
These are artifact measurements, not inferred savings from Cargo declarations;
they are not compressed distribution sizes or startup savings.

The actual Linux L=0 executable booted and answered the agent's state with
`delivery.L = "0"`, `stream = "embedded"`, no pending work and no update directory,
even with an update origin and store-directory override supplied. The Swift L=0
app also reached first paint and exited its smoke with no store directory. Its
scratch bundle omitted the separately loaded GPU module (reported as missing),
so this probe proves updater omission and native first frame, not GPU packaging.
The scratch direct link also reported the host transport objects' newer macOS
minimum; the normal Apple build script remains the authority for distributable
artifacts and rejects mixed deployment targets.

Reproduce the structural proof by baking the same app with `deploy.store` set to
`"0"` then `"A"`, retaining release symbols, inspecting the executable/archive,
and driving the former with `EXACT_AGENT=1`/`EXACT_SMOKE=1` and a fresh
`EXACT_UPDATE_DIR`. Both normal app entry generators consume the compatibility
record; neither relies on a Cargo feature. Higher adapter dependencies may be
compiled by Cargo even when unused, but are absent from the L=0 linked artifact.

## Completion of the Swift and classifier boundaries (2026-09-04)

The Rust composition is retained. `ExactUpdates` is now a separate Swift target;
`ExactKit` owns only generic lifecycle and complete asset-provider seams. The
shared Apple script reads `compat.json` from Cargo's actual app bake output and
selects the embedded or updating composition, with separate Swift build graphs.
The classifier skips retired-stream discovery for all-L=0 runs and omits retired
records identified as an L=0 platform in mixed runs. The regression covers no
`deploy.streams`, an empty list, a declared retired stream, and a mixed L=0/L=A run.

A disposable external Caltrain workspace, using `EXACT_APP_DIR` and the normal
shared build/agent/smoke scripts, produced these paired release artifacts with
identical dependencies and retained symbols:

| Artifact | L=0 bytes | L=A bytes |
|---|---:|---:|
| Apple Rust archive | 32,270,448 | 34,574,000 |
| Linux executable | 9,077,296 | 9,324,496 |
| macOS standalone | 2,657,520 | 2,930,400 |
| macOS sample host | 2,663,968 | 2,953,232 |

Rust's matching `llvm-nm` inspected both the LLVM-bitcode archive members and
final executables: the updater, higher Rust adapters, verification crypto and
Swift `ExactUpdates` owner are absent at L=0 and present at L=A. L=0 artifacts
also contain neither the fixture's verification key nor its update origin.
The final iOS app, sample host and Rust archive likewise omit those symbols;
the same manifest selected macOS/Linux=A and iOS=0, proving target-specific
composition selection. Normal builds include GPU/web-arm packaging and pass the
Apple deployment-target warning gate.

macOS, Linux, the macOS sample host, and iOS each booted with delivery.L=0,
stream=embedded, an override update origin and a fresh update directory. Each
successfully performed an ordinary data request against an owned loopback server;
after the scheduled-check interval, none contacted the update path or created
that directory. First frames were 69.4, 553.0, 45.2 and 367.4 ms respectively
(single probes, not latency distributions). The paired macOS A app opened its
store and contacted the update origin after paint. The two-session shared host
smoke passed at L=0 (11.0 s), L=A (25.5 s), and iOS L=0 (3.6 s).

`caps.test` passes 40/40; the Apple core, both update adapters and updater tests
pass (82 tests), including the expanded publisher regression. Caps, boot, format
and issue-format checks pass. Broader native testing also found an unchanged
`exact-linux` library test, `fetch::tests::a_page_that_never_ends_is_refused`,
crashing with SIGSEGV on this Mac, reproduced once with one test thread. It is
recorded in QUEUE; the full workspace suite is not claimed green by this evidence.
