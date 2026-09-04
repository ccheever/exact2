# Level-zero hosts still link and publish the update store

**Status:** Closed
**Resolution:** Implemented 2026-09-04: native hosts no longer depend on the updater; the bake selects a core or adapter entry, and L=0 deploys classify as binary-only.
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
