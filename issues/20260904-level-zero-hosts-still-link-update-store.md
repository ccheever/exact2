# Level-zero hosts still link and publish the update store

**Status:** Open
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
