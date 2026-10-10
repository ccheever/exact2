# Apple: `Intl.NumberFormat` rounds half to even, so USD 0.825 prints $0.82 where the web prints $0.83

**Status:** Open
**Systems:** js runtime, Apple hosts
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x72-apple-intl-half-even.md

## Summary

In a TypeScript data module on macOS, `Intl.NumberFormat` rounds a half to even. A browser rounds it half away
from zero (ECMA-402's default `roundingMode`, `"halfExpand"`). The browser rounds from the number's shortest decimal
digits, so 0.825 is a half there even though its binary value is 0.82499…. So
`new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(0.825)` is `$0.82` on macOS and
`$0.83` in Chrome, Node and Bun. 0.125 is `$0.12` against `$0.13`. `toLocaleString` takes the same path.
`resolvedOptions().roundingMode` is `undefined`, and an explicit `roundingMode: 'halfExpand'` is ignored. An app
therefore cannot ask the engine for the web's rounding.

The cause is in Hermes's Apple `Intl`, which Exact links unmodified on Apple hosts (upstream `d412d3bd`,
`vendor/ibex/crates/hermes-lean-sys/receipt_schema.rs:5`). `lib/Platform/Intl/PlatformIntlApple.mm`
`NumberFormatApple::initializeNSFormatters` (lines 2565-2625) builds an `NSNumberFormatter` and never sets its
`roundingMode`, and Foundation's default is `.halfEven`. The file notes "roundingType is not supported" (line 2568)
and reads no `roundingMode` option. `format` (line 2627) hands the double to `stringFromNumber:`. Linux and Windows
link ICU (`hermes-lean-sys`'s `icu-en-data`), which rounds `halfExpand`. They were not run here.

## Why this arose

T3 Code's Usage page formats every amount with its `usageFormat.ts` `CURRENCY`, an `Intl.NumberFormat` for en-US USD
with two digits. The 2026-10-09 desktop audit (PG-2) found that the clone printed `$0.82` for a Codex row whose cost
was 0.46 + 0.365 = 0.825 USD. The reference (Electron, ICU) printed `$0.83`. Likewise a model's Input cost was `$1.27`
against `$1.28`. The clone's formatter used `toFixed(2)`, which rounds the binary value. Moving it to `Intl` would not
have matched on macOS, because of this gap. So the clone formats USD itself: `formatUsd` (`examples/t3-code/pages-usage.ts`
on the T3 branch, merged in T3 PR #375 as `7c3708cf7`) rounds the shortest digits half away from zero. Its test uses
Bun's ICU `Intl.NumberFormat` as the oracle. Any app that formats money or percentages with `Intl` on Apple prints a
different last digit from its web build at every half.

## Reproduction

The app is a one-file TypeScript-module app made with `bun scripts/exact.mjs new <dir>` on main
`a88a90cd60ef0bd6b1d1504f09c43e375ce89a9f`.

`app.contract`:

```text
shape Amounts
  a0825: string
  a1005: string
  a2675: string
  a0125: string
  mode: string
  explicit: string
  locale: string

component X72app
  resource amounts = amounts() as shape Amounts
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 display="flex" flex-direction="column" gap=8 background-color="#ffffff" color="#111111"
      text `0.825 -> ${amounts.a0825}` testId="a0825"
      text `1.005 -> ${amounts.a1005}` testId="a1005"
      text `2.675 -> ${amounts.a2675}` testId="a2675"
      text `0.125 -> ${amounts.a0125}` testId="a0125"
      text `resolvedOptions().roundingMode -> ${amounts.mode}` testId="mode"
      text `0.825 with roundingMode halfExpand -> ${amounts.explicit}` testId="explicit"
      text `(0.825).toLocaleString -> ${amounts.locale}` testId="locale"
```

`app.ts`:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.x72app';
export const grants = '';

const usd = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' });
const sources: Sources = {
  amounts: (): Result<'amounts'> => ({
    a0825: usd.format(0.825),
    a1005: usd.format(1.005),
    a2675: usd.format(2.675),
    a0125: usd.format(0.125),
    mode: String((usd.resolvedOptions() as { roundingMode?: string }).roundingMode),
    explicit: new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', roundingMode: 'halfExpand' } as Intl.NumberFormatOptions).format(0.825),
    locale: (0.825).toLocaleString('en-US', { style: 'currency', currency: 'USD' }),
  }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent macos --size 560x260 "clock settle" tree logs`, and
`bun exact.mjs agent web --size 560x260 "clock settle" tree`. On macOS, the `logs` show that the app's Hermes
answered at launch (`query amounts: amounts`, `amounts answered`). The reference rows run the same four
calls as a plain script: a `file://` page in headless Chrome 155.0.8059.39 (`--dump-dom`), `node -e` (Node 26.7.0,
ICU 78.3) and `bun -e` (Bun 1.4.2).

| Call | Actual: macOS 26.6.2 (25G83), Apple Silicon, Hermes | Expected: Exact web (the agent's Chrome 155) | Chrome 155 page, Node, Bun |
|---|---|---|---|
| `format(0.825)` | `$0.82` | `$0.83` | `$0.83` |
| `format(1.005)` | `$1.00` | `$1.01` | `$1.01` |
| `format(2.675)` | `$2.68` | `$2.68` | `$2.68` |
| `format(0.125)` | `$0.12` | `$0.13` | `$0.13` |
| `resolvedOptions().roundingMode` | `undefined` | `halfExpand` | `halfExpand` |
| `format(0.825)` with `roundingMode: 'halfExpand'` | `$0.82` | `$0.83` | `$0.83` |
| `(0.825).toLocaleString('en-US', USD)` | `$0.82` | `$0.83` | `$0.83` |

The macOS host rounds the decimal digits, not the binary value: 2.675 gives `$2.68`, where `toFixed(2)` gives
`2.67`. It goes to the even digit at a half, so 0.825 and 0.125 go down and 2.675 goes up.

As a check of the cause, a Swift `NumberFormatter` was configured as `initializeNSFormatters` configures one for
currency (`.currency`, two fraction digits, grouping, locale `en-US@currency=USD`). Its default rounding mode is
`.halfEven`, and it prints `$0.82, $1.00, $2.68, $0.12, -$0.82` for 0.825, 1.005, 2.675, 0.125 and -0.825. With
`roundingMode = .halfUp` it prints `$0.83, $1.01, $2.68, $0.13, -$0.83`, which is ICU's `halfExpand` output for all
five values.

iOS and tvOS link the same Apple `Intl` source and were not run.

## Constraints

- The web is the standard (`CLAUDE.md`). ECMA-402's default `roundingMode` is `"halfExpand"`, and
  `resolvedOptions()` reports the mode in use.
- Exact links Hermes unmodified from the pin above. A fix inside `PlatformIntlApple.mm` is a Hermes patch or an
  upstream change. Exact already corrects Hermes's Apple `Intl` in its prelude, as `js/src/intl.js` does for
  `notation: "compact"` (x2apps stocks #3).
- `docs/reference.md:388` lists `Intl.NumberFormat` and `toLocaleString` as available, with no note on rounding.
- `js/tests/fixtures/pure.ts` (source `intl`) compares Hermes's output with Chrome's
  (`js/tests/it/pure.rs` `pure_utilities_match_the_web_executor`). Its currency values (1234.5, -0.41, 166.07)
  contain no half, so this gap passes that comparison.
- Linux, Windows (ICU) and the web already round `halfExpand` and must not change.

## Acceptance criteria

- In the repro on macOS, the four amounts print `$0.83`, `$1.01`, `$2.68` and `$0.13`, as Chrome does. The
  `toLocaleString` row prints `$0.83`, and -0.825 prints `-$0.83`.
- `resolvedOptions().roundingMode` is `"halfExpand"` on Apple hosts. An explicit `roundingMode: 'halfExpand'`
  prints the same as the default.
- The `intl` source in `js/tests/fixtures/pure.ts` gains currency half cases (0.825, 1.005, 2.675, 0.125 and
  -0.825). `pure_utilities_match_the_web_executor` passes on macOS with them.
- Linux and the web print what they print today.
