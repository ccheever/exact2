# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.b34c5ab`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final command was `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios --run`.

## What works

- All 260 supplied tickers are bundled. `app.ts` imports `data/tickers.json`; an identical raw copy ships in `assets/tickers.json`.
- Twenty distinct tickers are selected each second for simulated price changes. Cent-rounded prices stay positive and within ±1% of their preceding values.
- Symbol, company, price, and percentage change update in stable list rows, including during scrolling.
- Case-insensitive symbol/company search, All/My List switching, three numeric/alphabetic sorts, and ascending/descending order.
- Independent star controls in the list and detail, saved across relaunches with sort and direction. Search and list scope are also preserved.
- A native pushed detail with live price, change, open, volume, and a rolling 60-sample SVG line chart.
- Light/dark colors, accessible change colors, 44-point star targets, and wrapping at the largest accessibility text size.

## What was hard

The native largest-text checks caught problems that the web host did not. An inner navigation link squeezed names and complicated row measurement. Moving navigation to the row and keeping its star as an independent control improved this. The remaining overlaps came from large scrolling rows beneath pinned controls and a sticky count header. Putting the segmented control and caption in scrolling list rows resolved them. The switch remains visible above empty states.

System green was too light for small text on white. Dedicated light/dark market colors fixed contrast without losing the requested green/red meaning.

Exact builds use shared toolkit caches outside the app directory, which required sandbox escalation. Editing a contract while iOS verification was running invalidated that run; subsequent checks used a frozen set of source files. Shared machine load also made native iteration variable.

## Lookups and decisions

I used Exact's start guide and list-detail pattern, then looked up timers, seeded randomness, persistence, numeric formatting, SVG, menus, semantic theme colors, event handling, and simulator driving. Contract sort directions must be literals, and array reversal uses an indexed descending sort rather than `.reverse()`.

Decisions where the specification left room:

- Quotes display in USD.
- Each ticker begins with its supplied price and collects one price sample per foreground second, including unchanged samples, retaining the latest 60. A new launch resets simulated prices/history; favorites and sorting persist as requested.
- Sorts default to ascending symbol order; the menu can reverse any sort.
- Controls at the top of populated lists scroll with the content to keep large text unobstructed.

## Validation

- `bun exact.mjs verify`: clean; 7 tests passed, light/dark checks clean, largest text size clean, persistence/reload checks passed.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: clean; 7 tests passed, light/dark checks clean, all 9 largest-text screen checks clean. Thirty seconds of real-touch scrolling reported nothing for the app to fix.
- A separate data check exercised 120 ticks across all 260 tickers, checked roughly 20 changed prices per tick, cent rounding, positivity, the ±1% bound, unchanged static fields, the 60-sample cap, valid chart geometry, and unknown symbols.
- A final physical-touch drive confirmed that the star does not navigate, the row does navigate, the detail history advances on real time, and the timer advances during a held multi-part scroll. Both appearance checks remained clean.
- The raw bundled JSON matches the supplied data byte for byte.

No specified feature remains unfinished. The scrolling verifier did not judge timing under the shared machine's high CPU load; it did validate scrolling physics and reported no app-owned correction. It separately noted a toolkit-owned status-bar scroll-to-top timing difference.

## What would make this faster

A tested compound live-list pattern with independent accessory buttons, variable-height text, search, and sort would have avoided several iterations. Native overlap diagnostics that identify pinned section headers and fixed controls would help distinguish them from intrinsic row-sizing problems. Explicit examples of sort-direction syntax and an early warning about changing sources during verification would also save time.
