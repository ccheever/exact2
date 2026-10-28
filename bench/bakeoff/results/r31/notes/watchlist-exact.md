# Watchlist implementation notes

Finished, built, installed, and launched on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.
Bundle ID: `dev.bakeoff.watchlist.b9bc6df`. App name: Watchlist.

## Delivered

- All 260 supplied tickers, with symbol, company, price, and signed percentage change from open.
- Every second, 20 distinct randomly selected stocks move by at most ±1%. Full precision is retained internally and prices are rounded only for display.
- Symbol ascending, price descending, and percentage-change descending sorting; case-insensitive symbol/company search that ignores surrounding whitespace.
- Native All / My List segmented control and favorite stars in the list and detail screen. Favorites, sort, and selected segment persist across launches.
- Native navigation, search, sort menu, and scrolling lists. Live detail includes open, volume, current price, change, and a chart retaining the latest 60 one-second samples.
- Accessible labels, tabular prices, semantic light/dark colors, and layouts that wrap at the largest accessibility text size.
- Custom app icon. The original dataset is incorporated into the data module and also shipped verbatim at `assets/data/tickers.json` in the installed bundle.

## Verification

- `bun exact.mjs verify`: clean; six tests passed. Checked light/dark, two viewport sizes, relaunch behavior, and largest text size.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; six tests passed. Eight screens checked at the largest iOS text size, with no spills. Light/dark and relaunch checks had no findings.
- Native scrolling verification used 32 seconds of real touches and reported nothing for the app to fix. The host was heavily loaded, so the verifier judged scrolling physics but declined to judge timing. It reported a toolkit-owned status-bar scroll-to-top difference.
- `bun .exact/check-market.ts`: 90 updates checked for exactly 20 distinct changed stocks, the ±1% limit, positive prices, correct percentage calculations, the 60-sample cap, finite chart coordinates, and no extra price movement when changing detail selection.
- Final `bun exact.mjs ios --run` built and installed the release of the source checked above, plus the icon and explicit dataset asset. Inspected the resulting bundle for the correct ID, name, icon metadata, and an exact 260-ticker dataset copy.

## What was hard / what I looked up

- Exact's shared Cargo build lock required sandbox escalation before compiler-backed commands could run.
- Native list rows and large text required attention to tap-target size, explicit horizontal link layout, wrapping numeric columns, and placement of the segmented control within the scrolling list.
- Read Exact's required start guide and focused recipes for native navigation, search, segmented controls, timers, numeric readouts, simulator driving, and asset packaging.
- Used semantic up/down theme colors because the default light green had insufficient contrast on white.

## Decisions made without questions

- Price and change sort highest first; symbols sort alphabetically.
- Each stock records one chart sample per second, including unchanged prices. The chart starts with the supplied price and fills over the first minute.
- Simulated prices and chart history reset on launch. Favorites and sort persist, as specified.
- Volume and open remain the supplied session values. Prices are formatted in dollars.
- Resuming does not invent a burst of prices for time spent suspended.

## Unfinished work

No requested feature is left unfinished. Scrolling timing under an idle host remains a verification limitation, not an observed app failure.

## What would make this faster

- Sandbox-ready shared compiler/cache access.
- An Exact example combining a live, keyed 260-row list with independently tappable stars and accessible numeric columns.
- A clearly surfaced distinction between a plan used only in agent drives and the installed app plan; the final `ios` install is essential.
- An idle simulator host for reliable scrolling performance measurements.
