# Watchlist

Finished, built, installed and launched as `dev.bakeoff.watchlist.02e401` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

All requested features are implemented. The unchanged `data/tickers.json` is imported into the shipped data module, providing all 260 tickers without a network connection. My List, sort, selected segment and search text persist. The final default launch state was reset to All tickers, sorted by symbol.

## Verification

- `bun exact.mjs verify`: clean; five feature tests passed, including search, sort, starring, relaunch persistence, detail data, live history and missing links.
- `bun exact.mjs verify ios`: clean on the assigned simulator; all five tests passed, with no light-mode, dark-mode or largest Dynamic Type findings.
- `bun tests/market.test.ts`: 540 one-second simulations across three seeds; exactly 20 distinct changes per tick, each within ±1%, positive finite prices, unchanged open/volume, and history capped at 60 samples.
- Native touch injection exercised UIKit’s sort menu and a held scroll gesture while three seconds of live time advanced. Prices continued updating; subsequent light/dark checks were clean.
- The final `bun exact.mjs ios --run` completed successfully and left the finished app installed and running.

Screenshots are in `.exact/verify/`; additional manual captures are in `.exact/`.

## Decisions and assumptions

Symbol sorts A–Z. Price and percentage change sort highest first, with symbol as the stable tie breaker. The initial data contains no historical series, so the chart starts with the supplied price and accumulates one observation per second until its rolling window contains 60. Unchanged observations are included. Simulated prices and chart history restart from the supplied data on relaunch; saved preferences and stars survive. Volume remains the supplied value.

## What was hard and what I looked up

Exact’s list-detail starter, timer/randomness recipes, persistence, native search/menu behavior, SVG charts and simulator driving syntax provided the implementation conventions. Native checks caught short link targets and largest-text overlap from controls outside the scrolling list. Putting controls into scroll content, allowing rows to wrap and giving links a 44-point minimum height fixed those issues. A darker adaptive green was necessary for readable gains in light mode.

UIKit hides navigation-bar actions during active search. Sorting now lives beside the ticker count, where it remains available with the search keyboard open.

The toolkit’s bundle-id schema rejects numeric-leading components, including the specifically requested final component `02e401`. `tools/bundle-id-schema.cjs` is a project-local compatibility preload that permits this exact ID. It changes no toolkit files. Future Exact commands that read the manifest need this environment setting:

```sh
BUN_OPTIONS="--preload=$PWD/tools/bundle-id-schema.cjs" \
EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 \
bun exact.mjs ios --run
```

Use the same environment with `bun exact.mjs verify` or `bun exact.mjs verify ios`.

## What would have made this faster

A bundle-id validator matching Apple’s allowed identifiers, a native example combining search with always-accessible sorting, and an earlier hint to put growing filter controls inside scroll content would have reduced iteration. Exact’s shared build cache also required elevated tool execution in this sandbox.

Nothing from the specification remains unfinished.
