# A bare input stretches to its container on native but keeps its intrinsic width on the web

**Status:** Open
**Systems:** kernel, Apple host, Linux host
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1012.001 F4, LLP 1001 §1

Caltrain's `station-search` (`apps/caltrain/app.contract:284`: an `input` in a `section`, padding 12, no width) is **201 px** wide on the web page and **352 px** on macOS and Linux, the full width of its container. Found 2026-09-30 driving all three hosts with `layout station-search`; the same numbers on fresh builds.

A browser keeps an input's intrinsic width (about 20 characters) even at `display: block`, which the page's base CSS sets (`button,input,textarea{all:unset;display:block}`). So by the house rule (the web is the standard; a bare node behaves as a bare `<div>`, a bare `input` as a bare `<input>`), the native hosts are the ones off. Neither the smoke nor the conformance run pins an input's width, so nothing flagged it.

To look at: the kernel's measured-leaf size for `TextInput` (it should report an intrinsic inline size and not be stretched by a block container), then each native presenter. Verify by `bun scripts/agent.mjs <web|macos|linux> "tap change-station" "layout station-search"` giving one width, and pin it in the conformance run.
