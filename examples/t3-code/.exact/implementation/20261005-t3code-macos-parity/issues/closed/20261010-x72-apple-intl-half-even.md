---
name: 20261010-x72-apple-intl-half-even
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X72: `Intl.NumberFormat` on Apple rounds half to even, so the Usage page's USD 0.825 would print $0.82

Moved to main `issues/20261010-apple-intl-number-format-half-even.md` (2026-10-10), where it is tracked. It was filed
on main by PR [#394](https://github.com/ccheever/exact2/pull/394). It was `EXACT2-GAPS.md`'s local draft X71 until
2026-10-10, when it was renumbered X72: the plan's X71 is the pointer-events gap
(`20261010-x71-pointermove-without-press-capture.md`).

## Summary

The reference formats every Usage amount with `usageFormat.ts` `CURRENCY`, an `Intl.NumberFormat` for en-US USD with
two fraction digits. In Electron that is ICU. ICU rounds the number's shortest decimal digits half away from zero
(`halfExpand`), so 0.825 is `$0.83`. In the clone's runtime, Hermes's Apple `Intl` (`lib/Platform/Intl/PlatformIntlApple.mm`,
`NumberFormatApple::initializeNSFormatters`) formats through an `NSNumberFormatter` and never sets its `roundingMode`.
Foundation's default is `.halfEven`, so 0.825 is `$0.82` and 0.125 is `$0.12`. `resolvedOptions().roundingMode` is
`undefined`, and an explicit `roundingMode: 'halfExpand'` is ignored, so the app cannot ask Hermes for ICU's rounding.

## Why it arose

The 2026-10-09 desktop audit (`../../reviews/20261009-desktop-audit.md`, PG-2 "USD half cents round down") found
`$0.82` in the clone where the reference showed `$0.83`. The case was the Past 24h Codex row (0.46 + 0.365 = 0.825 USD),
which appears in that row and in the Day table. The model dialog's Cost by type › Input was `$1.27` against `$1.28`. The
clone's `formatUsd` used `toFixed(2)`, which rounds the binary value (0.825 is 0.82499…). Moving to `Intl` would not
have helped on macOS, because of this gap. Task `20261009-usage-and-pr-pages` read it in the pinned Hermes source and
checked it with an `NSNumberFormatter` configured as Hermes configures one. It kept the draft local, as the user's rule
for new framework drafts then was.

## Clone workaround (usage-and-pr-pages)

`formatUsd` (`examples/t3-code/pages-usage.ts`) formats USD itself. It rounds the number's shortest digits half away
from zero, groups the whole part, and keeps ICU's sign for a negative or negative-zero amount. Every amount on the Usage
page goes through it: the rows, the Day table, the model dialog, Totals and the chart's ticks. `pages-usage.test.ts`
compares it with Bun's ICU `Intl.NumberFormat` (the oracle, since Chrome's is ICU too). It uses 27 edge values (0.825,
1.005, 2.675, 999.995, -0.825, -0, NaN, ±Infinity…) and 16,000 generated ones. The declared difference is `EXACT2-GAPS.md`
"Usage and Pull Requests pages: declared differences" › "USD rounding (X72)". No clone code waits on the main fix.
Once Apple's `Intl` rounds `halfExpand`, `formatUsd` could go back to `Intl.NumberFormat`, but nothing requires it.

## Evidence and history

- 2026-10-10, T3 PR [#375](https://github.com/ccheever/exact2/pull/375) (merged `7c3708cf7`): PG-2 passes. The Past
  24h Codex row reads "85.4% of tokens · $0.83", the Day table's Oct 9 Codex reads `$0.83` and gpt-5.5 Input reads
  `$1.28`, where the base read `$0.82`, `$0.82` and `$1.27`. Image:
  [pg2-rounding.png](https://raw.githubusercontent.com/ccheever/exact2/82126c6ce55240af5dba7a1480c313f27781910d/usage-and-pr-pages/pg2-rounding.png).
- 2026-10-10, main PR #394 reproduced the gap on main `a88a90cd6` in a one-file TypeScript-module app, run in the
  app's Hermes. On macOS 26.6.2, `tree` read `$0.82`, `$1.00`, `$2.68` and `$0.12` for 0.825, 1.005, 2.675 and 0.125,
  `undefined` for `resolvedOptions().roundingMode`, and `$0.82` with an explicit `roundingMode: 'halfExpand'`. The
  Exact web build in Chrome 155 read `$0.83`, `$1.01`, `$2.68` and `$0.13`, and `halfExpand`. A plain Chrome page,
  Node 26.7.0 (ICU 78.3) and Bun 1.4.2 gave the same as the web build. A Swift `NumberFormatter` configured the same
  way prints the macOS row, and with `roundingMode = .halfUp` it prints ICU's. The main file has the app's sources and
  the table.
