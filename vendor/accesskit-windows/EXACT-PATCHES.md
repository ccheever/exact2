# AccessKit Windows 0.35.2 — Exact patches

- Upstream: https://github.com/AccessKit/accesskit
- Release: https://static.crates.io/crates/accesskit_windows/accesskit_windows-0.35.2.crate
- Archive SHA-256: `498668b90da9995302a0055aaecad4756932333da2ddef6baee1c00844c430fc`
- VCS revision: `f3026222559c7f5f46d8f25cb7404de68eb149f0`,
  `adapters/windows`, from the archive's `.cargo_vcs_info.json`.
- Licenses: Apache-2.0 or MIT; the upstream licenses remain in this copy.
- Selection: the root `[patch.crates-io]`; no feature or dependency change.
  Neither `game/` nor `snapback4/` depends on this crate.
- Packaging omissions: `.cargo_vcs_info.json`, `CHANGELOG.md`, `Cargo.lock`
  and `Cargo.toml.orig`. The provenance above preserves the source pin.

LLP 1115.004 D7 requires RichEdit's native fragment under the field's
AccessKit node. Private node navigation and context ownership require a
patch inside the adapter. The candidate remains disabled and unqualified;
Shelf's UIA discovery result is recorded in `host/ink/MEASUREMENTS.md`.

The numbered inventory is relative to the unmodified crates.io source.
Tests below are in `src/tests/embedded.rs`, compiled under
`subclass::embedded_tests` so they can inspect the adapter's own context.
They use the public HWND registration API and call the COM providers.

1. **`adapter.rs`, `Adapter::embedding_context`.** Expose the active context
   to the subclass bridge, refusing inactive and placeholder trees.
   `embedded_navigation_and_roots` activates the tree and registers a native
   fragment; `embedded_removal` checks registration after node removal.
2. **`context.rs`, `Context`.** Own agile references in an embedded-provider
   map and remove the registration with its platform node.
   `embedded_removal` removes a node through an adapter update and asserts
   that the map is empty and its retained site refuses navigation.
3. **`lib.rs`.** Declare `embedded` and re-export `windowless_site` and
   `embed_windowless`. All four embedded tests exercise those entry points
   through `subclass`; the Shelf discovery fixture uses the public exports.
4. **`node.rs`, fragment navigation and roots.** First/last child navigation
   returns the registered native fragment; `GetEmbeddedFragmentRoots`
   returns its provider in a COM SAFEARRAY. `embedded_navigation_and_roots`
   checks both directions, parent navigation, runtime prefix and array identity.
5. **`node.rs`, hit testing and focus.** Delegate a hit to an embedded root;
   fall back to the AccessKit node if the root cast or delegated hit fails.
   Focus returns the native fragment for the focused field. COM calls happen
   after releasing the tree lock. `embedded_hit_test_and_fallback` covers
   success, no registration, missing root interface and delegate refusal;
   `embedded_focus` covers registration and explicit removal.
6. **`subclass.rs`.** Resolve the window's installed adapter on its owning
   thread; expose site creation and registration outside `WM_GETOBJECT`.
   The four embedded tests register through this bridge on a hidden native
   window. `embedded_removal` covers explicit unregister and removed-node
   refusal. The file also includes their test module.
7. **New `embedded.rs`.** Resolve agile references outside the map lock and
   implement the weakly held windowless site, parent fragment and runtime
   prefix. `embedded_navigation_and_roots` and `embedded_removal` cover its
   live and removed-node behavior.

Run on Windows, serially:

```sh
cargo test --manifest-path vendor/accesskit-windows/Cargo.toml \
  --target-dir target --lib -- --test-threads=1
```

Upstream status: **to send**, as “embedded native UIA fragments under a node”.
Remove this vendor override when a pinned published release supplies that
behavior and these four tests, the upstream suite and Shelf's native UIA
journey pass. On a version bump, reapply each numbered hunk by behavior and
rerun those tests; the upstream suite alone does not cover the patch.
