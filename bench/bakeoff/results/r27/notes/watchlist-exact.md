# Watchlist build notes

Finished, built, installed and launched as **Watchlist** (`dev.bakeoff.watchlist.b74f27e`) on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. No other simulator was used.

All requested features are complete. The app includes all 260 supplied tickers, live simulated quotes, search, sorting in either direction, persistent favorites and sort settings, and live detail charts. The original catalogue is imported from `data/tickers.json`; an unchanged raw copy also ships in `assets/data/tickers.json`.

## Validation

- `bun exact.mjs verify`: clean; six tests passed, light/dark and largest-text checks passed.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: clean; six tests passed, all screen and relaunch checks passed, and eight screens passed at accessibility-extra-extra-extra-large (53-point body).
- Native scrolling: nothing for the app to fix; 34.1 seconds of real touches, no dropped frames on the two measured lifts, no scrubbing delay. The machine was heavily loaded, so Exact did not judge absolute frame timing. Its reported status-bar scroll-to-top discrepancy belongs to the toolkit.
- A separate 90-tick data check verified 260 retained quotes, about 20 actual price changes per tick, the ±1% bound, unchanged open/volume values, valid chart paths and the 60-sample history limit.
- Final installation used `bun exact.mjs ios --run` with the required simulator selected.

## What was hard / looked up

Used Exact's start guide, list-detail pattern and focused documentation for timers, seeded randomness, persistence, SVG, menus, search, numeric text and adaptive colors.

Native layout needed more attention than web layout. Pinned controls and plain-list sticky headers overlapped scrolled content at the largest accessibility size. The final native grouped list avoids those overlaps, keeps full company names, and lets rows grow and wrap. Empty states retain the segmented control outside the list.

Web reload restored navigation while iOS reload returned to the root. Tests now navigate explicitly. Transient chart selection also needed source context rather than a durable resource identity. System green was too light for small text, so gain/loss colors use accessible light/dark variants.

Adding assets during the initial native prebuild invalidated its receipt; rebuilding after assets settled resolved that. Later native builds took seconds.

## Decisions and limitations

Prices display in USD because no currency was specified. Symbol sorting initially uses A–Z; price and change initially use descending order. Both direction and key persist.

History samples once per second, including unchanged prices, and retains the latest 60 samples from the current session. Prices and history restart on launch; favorites and sorting survive. Cent rounding is clamped so each simulated move stays within ±1%.

No requested feature remains unfinished. Absolute scrolling timing could not be conclusively benchmarked under the host's high load.

## What would make this faster

A tested streaming-catalogue pattern, clearer guidance on transient resource context and cross-platform relaunch behavior, and an early native accessibility scrolling check would have reduced iteration. Finalizing bundled assets before prebuilding would also save the first rebuild.
