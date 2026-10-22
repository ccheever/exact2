# Narrow re-check after round 3: package icon tokens (cc3beef2b6), Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — A missing SVG still hides an app-vendored package from watching.** [sources.rs:604](/tmp/rv-pkgicons-astra/contract/cli/src/sources.rs:604) drops its origin when moving the missing asset into `consulted`; manifest collection and [watch roots](/tmp/rv-pkgicons-astra/contract/cli/src/tamber_packages.rs:120) depend on surviving SVG sources. Repro: use the round-3 `vendor/reading` fixture, leave `assets/` present but `pin.svg` absent, start the standalone Tamber server, then create `pin.svg`. The initial hello lacks that package’s `watch.roots` and `watch.assets`; its manifest is unrecorded, and creating the SVG triggers no rescan. Register ownership and the manifest independently of SVG existence. Previously established watches survive deletion; this affects startup with the asset missing.

2. **NIT — Explicit colour inheritance is unresolved.** [icon_svg.rs:242](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:242) lets `color="inherit"` replace the inherited colour with the literal keyword. Repro: under `<svg color="red" fill="currentColor">`, use one path with `color="inherit"` and another with `fill="red"`; supply a valid viewBox and path data. This one-colour image is rejected because the comparison sees `currentcolor` versus `red`.

3. **NIT — Confinement retains a contrived multiple-swap race.** [confined.rs:48](/tmp/rv-pkgicons-astra/contract/lower/src/icons/confined.rs:48) canonicalizes separately from pathname metadata. Repro interleaving: open an outside-pointing symlink; replace it with an inside file before canonicalization; restore the outside-pointing link before `metadata(&real)`. Device/inode then match the outside descriptor, whose bytes are accepted. The original single-swap repro is fixed.

The stated red/currentColor, red/blue, mixed explicit-red, and case/whitespace `!important` cases pass inspection. Existing vendored SVGs get the correct origin, manifest dependency, and watcher entries. All requested readers use the shared helper; absent files avoid a second open, and dangling final links return `Leaves`.

Static review only: no builds, tests, or file changes.
