# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.b3febdc`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used.

## Implementation and decisions

- All 260 supplied tickers are included. The original JSON is imported into the data module, and an unchanged copy ships at `assets/data/tickers.json` in the app bundle. The bundled file was checked byte for byte against the original.
- Every second the simulation selects 20 different tickers without replacement. Each selected price moves by at most ±1%, rounded to cents without exceeding that bound. Symbol keys keep rows stable during updates.
- Search matches symbol or company name, ignoring case and surrounding whitespace. Symbol sort is A–Z; price and percent-change sorts show the highest first, with symbol as the tie breaker.
- Stars toggle independently from navigation. My List and the chosen sort persist using native saved settings.
- Detail screens update live and show current price, percentage change from open, open, volume, and a rolling chart. History samples every ticker once per second, including unchanged prices, and retains 60 samples. A newly opened detail therefore has the same history as a ticker that was already open.
- Assumptions: prices are USD; the simulation and chart history restart from the supplied prices on relaunch. Only favorites and sort were specified to survive relaunch. The chart starts with real collected samples rather than invented history.

## What was hard / what I looked up

- Exact's supported symbol roles, platform text-style names, native navigation/search/menu syntax, timers, persistent state, SVG attributes, theme tokens, and resource request context needed targeted documentation lookups.
- Native ticker links initially measured 42 points high. They now have a 44-point minimum hit area.
- Default system green was too light for small text on white. Separate light/dark green and red tokens now pass contrast checks.
- Largest-text layouts needed wrapping with flex bases in rem. The filter and summary now scroll with the list, preventing a large sticky summary from covering ticker rows. Empty states retain their filter control.
- The market resource originally used tick and selected symbol as cache identity. Those are request context now, so returning from a detail and relaunching does not reject a previously kept answer.
- The native build was started early and reused while iterating on the baked app plan. Final installation used `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios`.

## Validation and limitations

- Five functional tests pass on iOS and web: all data/search, favorites and persistence, actual sort ordering and persistence, live detail/history capping, and unknown ticker navigation.
- Final platform verification reports **clean** on both hosts. iPhone light/dark checks and all seven screens checked at accessibility extra-extra-extra-large have no findings.
- Real simulator touches exercised sorting, segmented controls, keyboard search, scrolling, and native navigation.
- The final iPhone scrolling probe reports zero hitches, no row jumps, a 16.7 ms longest frame, and 3.96 ms main-thread p95. Exact separately reports its own status-bar scroll-to-top timing difference; the report explicitly says there is nothing for the app to fix.
- `.exact/check-market.ts` checks 120 simulated seconds across all 260 tickers, move bounds, history length, percentage calculations, stable opening data, navigation preserving history, and restart behavior.
- Nothing in the requested app specification is unfinished. Exact's supplementary formal semantics pass cannot complete: its oracle skips seeded scripts and its semantics roster does not support `toLowerCase`. Consequently the combined `verify` commands exit 1 after their platform reports say clean. This is distinct from the five passing platform tests and clean layout/accessibility checks; no formal proof is claimed.

## What would have made this faster

A tested live-list/chart pattern, one consolidated reference for native symbol and text-style names, clearer guidance about cache identity versus request context, and a verifier that distinguishes unsupported formal semantics from platform failures would reduce iteration time. The platform-driven checks and screenshots were especially useful for catching native-only target and maximum-text issues.
