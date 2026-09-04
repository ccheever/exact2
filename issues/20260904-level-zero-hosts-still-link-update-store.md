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
(`update/src/binary.rs:54-59`, `update/src/client.rs:81-93`). The code and
network-capable path are still in the binary.

The publisher has the matching semantic error. `streamPlatforms()` treats the
presence of a key under `deploy.store` or `deploy.binaries` as a stream,
regardless of whether `store.<platform>` is `0`
(`scripts/deploy.mjs:238-245`). It can therefore offer a bundle stream to a
client whose only legal carrier is the binary.

Done when `L = 0` selects a build/link composition with no updater symbols,
keys, persistent store, or check path, without adding a cargo-feature matrix
to a core crate; and when deploy emits only the platform's binary row. Hold it
with an artifact/symbol or size assertion and a deploy-table fixture for
`store.linux = "0"`.
