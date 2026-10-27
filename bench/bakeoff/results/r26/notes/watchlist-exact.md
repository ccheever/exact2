# Watchlist build notes

Implemented the six requested features using Exact. The original 260-ticker dataset is imported by the data module, and a raw copy ships at `assets/data/tickers.json`. The bundle ID is `dev.bakeoff.watchlist.bae2db3`.

## What was hard

Native search needed explicit keyboard dismissal when opening a result. Sort needed to remain reachable while search was active. At accessibility text sizes, stock rows needed a dedicated stacked layout, minimum 44-point navigation targets, and explicit width constraints. The stock count initially used a sticky section header; changing it into a normal scrolling row stopped it covering large stock labels while scrolling. Standard system green had insufficient contrast for small percentages, so the app uses paired light/dark green and red tokens.

## What I looked up and decided

I used Exact's tested list-detail pattern and its references for persistence, native search, segmented controls, menu radio items, timers, seeded randomness, SVG charts, numeric formatting, the reactive accessibility viewport source, and iOS driving/verification. Exact requires literal sort directions and a declared viewport resource.

The specification did not choose a currency or sort direction. I used USD, symbol ascending, and price/change descending, with a reverse-order menu action. Each second updates exactly 20 distinct randomly selected tickers. Prices retain full internal precision to respect the 1% movement bound and display as currency. Every stock records one sample each second, retains at most 60, and starts with its bundled price; the chart does not invent historical data. Favorites, sorting, direction, search, and list selection survive relaunch. Simulated prices and chart history restart from the bundled dataset.

## What would have made this faster

A stock-list starter with sibling star and navigation targets, an accessibility layout example, and a short native scrolled-screen check would have reduced iteration. Full iPhone verification takes several minutes; the machine was heavily loaded during scrolling measurements. Root-font-size emulation alone does not reproduce all native control metrics, so testing the simulator's actual largest Dynamic Type category was necessary.

## Completion and validation

All specified features are finished. `bun exact.mjs verify` passed seven tests, light/dark checks, relaunch checks, and largest-text checks. The final `bun exact.mjs verify ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96` was clean: seven tests passed, light/dark checks had no findings, and nine screens fit at the largest native accessibility size. Real scrolling touches found nothing for the app to fix. The verifier noted high machine load, so its timing numbers were not treated as a performance verdict; it separately identified the toolkit's status-bar scroll-to-top timing as framework-owned.

The finished standalone app was built and installed with `bun exact.mjs ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96`. Only that simulator was used. Its original Large text-size setting was restored. No requested feature remains unfinished.
