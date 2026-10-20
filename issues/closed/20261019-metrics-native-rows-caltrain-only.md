# `metrics --app` measures the native pipeline for Caltrain only

**Status:** Closed
**Resolution:** plan-metrics (exact-web-capabilities bin) measures any app from the plan its web build baked, with nothing answering after boot; metrics.mjs uses it for an app without its own metrics bin, --press <testId>[,<testId>] names the update; the bake is not measured. Verified on Shelf: every native row but the bake.
**Systems:** scripts/metrics.mjs, `exact new` (scripts/exact.mjs), apps outside the repo
**Severity:** P3
**Author:** Claude (Opus 5.5), from LLP 1054 O4 (the Bluesky port)
**Date:** 2026-10-19
**Related:** LLP 1054 O4; LLP 1054.000 "Not proposed" ("O4 waits for the scaffold"); LLP 1086 (what `exact new` writes)

## What happens

`bun scripts/metrics.mjs --app <app>` gets its native numbers (`compile_ms`,
`bake_ms`, `boot_ms`, `layout_ms`, `update_ms`, `web_update_ms`, …) by running
the app's own `web/src/bin/metrics.rs` (`scripts/metrics.mjs`, step 1). An app
without that file gets `NaN` in every native row and the note
"<app> has no web/src/bin/metrics.rs". The browser rows still measure it.

Only `apps/caltrain/web/src/bin/metrics.rs` exists. It is 1,226 lines written
against Caltrain: `caltrain::compile()`, `caltrain_data::Caltrain`, and the
`change-station` and `stations-back` test ids for its update timings. Neither
`exact new` nor any recipe writes one, and nothing says how to write one. So
no other app in the repository, and no app outside it, gets native numbers.

LLP 1054.000 deferred this until there was a scaffold. `exact new` now exists,
so that reason no longer holds.

## What would fix it

A native measurement that takes any app's plan and data source, not one bin
per app. Compile, bake, decode, boot, layout and the plan sizes need nothing
app-specific. The update timings need a target to press, which could come from
a flag (`--press <testId>`) or the app's first `testId`, or be left out when
none is named.
