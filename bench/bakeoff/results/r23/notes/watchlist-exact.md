Watchlist is complete, built, installed, and launched on simulator CEDFB9C9-FF04-4617-B156-94C56B83CA86. The app is named Watchlist and uses bundle ID dev.bakeoff.watchlist.ba17786. All 260 records from data/tickers.json are embedded through the data-module import, so the app works offline.

Implemented every specified feature: live prices and signed, colored percentage changes; symbol/name search; symbol, price, and change sorting; reversible sort direction; All/My List segmentation; stars in both the list and detail; live detail statistics and a rolling chart. Stars, sort, direction, and the selected segment survive relaunch.

Decisions made without clarification:
- Prices use USD formatting. Symbol sorting defaults to A–Z; price and change default to highest first.
- Each second selects 20 distinct tickers and makes bounded, nonzero cent changes. A price too small to move one cent within 1% stays unchanged.
- Every ticker records one sample per second, including unchanged prices. Charts begin with one real sample and retain at most 60. Simulation and history restart on relaunch.

What was hard:
- Exact requires literal sort directions and primitive/empty-list test assertions; its compiler messages supplied the fixes.
- System green needed a darker light-mode alternative for readable small text. Adaptive gain/loss tokens fixed contrast while preserving green/red meaning.
- Native inner links needed their own 44-point minimum despite the surrounding star button already meeting it.
- At accessibility sizes, fixed metadata could overlap native collection-view content during scrolling. Moving metadata into the list resolved this.
- Restored chart answers needed symbol validation and contextual live arguments to avoid restoring another symbol’s chart.
- Editing assets during the first background build invalidated its packaging receipt. Rebuilding reused the compiled dependencies.

I looked up Exact’s timers, seeded randomness, persistence, resource identity/context, numeric formatting, SVG, menus, assets, and simulator gesture APIs in its local guide.

Validation completed:
- bun exact.mjs verify: clean; five functional tests, both themes, largest text, and relaunch checks.
- EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios: clean; five tests, both themes, AX5 text, persistence, and real-touch scrolling.
- Pure-data checks: 3,000 ticks / 60,000 bounded movements, plus 1,000 sub-dollar boundary ticks; chart values and rolling limits stayed consistent.
- A real held scroll advanced the market revision from 0 to 2. Native touches also exercised sorting and detail navigation.
- Final installation: bun exact.mjs ios --run with the specified EXACT_SIM.

Nothing from the specification remains unfinished. The scrolling verifier found nothing for the app to fix; machine load prevented a formal timing judgment, and it separately identified Exact’s own status-bar scroll-to-top overhead.

This would have gone faster with an explicit live-resource restoration recipe, a native list example with fixed controls at AX5, and a warning about nested row-link targets below 44 points. Keeping asset changes outside an active native build also avoids packaging retries.
