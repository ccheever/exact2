# Hermes icon-cache repair

The native final replay exposed `timelineReadsRefreshed: Cannot read property 'set' of undefined`. A reduced test compiled with the same pinned Hermes compiler and executed by the same engine reproduced it: an async closure inside a `for...of` loop loses iteration-scoped captures after the loop advances or skips a following row. Two nonempty iterations also wrote only one cache key. This was not reproduced by Bun tests.

The app's icon loader now passes each row's key, origin, cache and pending-request map as explicit arguments to a named async helper outside the loop. The reduced same-engine test passes with two distinct keys, including skipped rows. The independent reviewer rebuilt and executed the positive reproduction. The framework and Hermes revision are unchanged; this is an app workaround, not an upstream issue resolution.

Portable engine source, before/after JavaScript and exact reproduction commands are in `../../20261005-upstream-timeline-and-markdown/20261006-hermes-icon-capture/`. The full app rebuild passes; `checks-icon-capture/report.json` records 1,199 Bun tests, strict TypeScript, Contract compile, root build/clippy/format and boot checks passing with unchanged source. The actual-host keyboard regression also passes with the README command; its existing extra hidden hook Tab stop is explicitly logged.

Mounted replay on the rebuilt app no longer reports the `.set` error. Out-of-order response, keyboard/wheel, activity retry and silent-disconnect capture results are recorded separately and determine final acceptance.
