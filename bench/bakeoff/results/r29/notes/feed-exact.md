# Feed — build notes

Finished and installed as **Feed** (`dev.bakeoff.feed.bd1dd6c`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` with `EXACT_SIM=… bun exact.mjs ios`.

## What ships

- All 400 supplied posts, all 120 JPEGs, and the original dimensions metadata are bundled in `assets/data/`. Bundled copies match the source files byte for byte.
- The feed sorts by actual age, rather than assuming JSON order. Images preserve their recorded aspect ratios; carousels support repeated image filenames without duplicate keys.
- Likes, composed posts, and locally refreshed posts persist in an atomic JSON file. Writes are serialized so rapid likes do not lose changes.
- Native navigation, composition with keyboard support, full-screen photos with swipe-down dismissal, pull-to-refresh, an accessible Refresh button, dark mode, and Dynamic Type are implemented.

## Validation

- `bun exact.mjs verify ios`: **clean**, 11 tests passed.
- Light/dark checks: no findings. Largest accessibility text size (53-point body): nothing spills across 13 screens.
- Real simulator touches verified pull-to-refresh adding exactly three posts and photo swipe-down dismissal. Portrait media and navigation to post 400 were also checked.
- Scrolling physics matched UIKit. The verifier disclosed high machine load, so frame timing was not treated as a reliable performance verdict.
- Web functional tests also passed all 11 cases. Its synthetic largest-text audit reports two partially visible carousel titles as clipped by the vertical feed; those titles remain reachable by horizontal scrolling. The iPhone audit has no such findings.

## What was hard / looked up

Exact's navigation projection, modal/full-screen routes, bundled assets, persistence grants, mutation queues, horizontal layout, gesture payloads, and clock APIs needed toolkit lookups. Explicit single-column navigation removed an unwanted inferred wide split. `overflow-wrap="break-word"` was needed for words wider than an entire line at the largest text size; it preserves normal word wrapping elsewhere.

A background native bake rejected an asset changed during compilation. A rebuild fixed that. The machine also ran out of disk space; removing this project's disposable build intermediates allowed compilation to continue. No source data was removed.

## Decisions and remaining work

Composed posts use “You” / `@you`. Refresh creates three text posts from a rotating local set of fictional updates. No network or account setup is involved. No requested iPhone feature is unfinished.

A feed starter combining bundled data, variable-aspect images, horizontal carousels, and a photo viewer would have sped this up. More direct large-word wrapping diagnostics, documentation of real pull gesture travel, and early disk-space checks would also help.
