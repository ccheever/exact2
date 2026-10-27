# Build notes

Finished and installed **Podcasts** (`dev.bakeoff.podcasts.bfd978f`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

## Delivered

- All 420 episodes from `data/episodes.json` are bundled through a JSON import, available offline, and displayed newest first. Original played values are respected.
- Episodes and individual show lists support the persistent Unplayed only filter. Search matches episode titles and show names.
- Shows lists all 12 shows with live episode and unplayed counts.
- Episode details include summary, date, duration, played status, and Play/Stop with a visible Playing state. Playing marks the episode played; no audio is required or included.
- Episode rows have direct played/unplayed and queue-add controls.
- Up Next prevents duplicates, preserves insertion/custom order, supports swipe removal, and offers accessible Up/Down/Remove controls in Edit mode. Boundary movement controls are disabled.
- Played changes and the full queue order survive relaunch. Edit mode and the simulated Playing state are intentionally temporary.

## Validation

- `bun exact.mjs verify`: clean; 10 tests passed. Light/dark, two web widths, persistence, and largest text checks passed.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; 10 tests passed. Light/dark and 12 screens at the largest accessibility text size had no findings. Relaunch persistence passed.
- Native real-touch scrolling physics matched UIKit. The verifier reported heavy machine load, so its performance timing was not judged.
- Inspected simulator screenshots of the episode list, Shows, Up Next, queue editing, and episode details.
- Final `bun exact.mjs ios --run`, with the assigned simulator set, built, installed, and launched the final embedded plan after verification.

## What was hard / looked up

Exact's tabs starter was the starting point. I consulted its guide and vocabulary for navigation, list sections, persisted state, switches, bundled JSON, swipe actions, symbols, and simulator driving. A few familiar TypeScript idioms differ: flatMap becomes map/flat, array access uses at/match, and action names must not shadow methods. Native switches use checkbox plus the switch attribute. Queue-only swipe handlers required distinct row branches.

I used persistent arrays of episode IDs for played overrides and queue order; even the full dataset remains well under Exact's documented persistence limit. Show count memberships are prepared in the data module so changes do not repeatedly scan the entire library for every show.

I chose explicit Up/Down controls for queue reordering because the toolkit did not expose a documented native list-reorder property. They work with large text and accessibility labels. Equal-date episodes use ascending ID as a deterministic tie-breaker. Dates use UTC parsing to avoid timezone shifts.

Large text needed flexible header and tab-label sizing and word wrapping. One early test incorrectly treated mounted content in another tab as absent; the corrected test checks the current show's count and opens the matching episode.

## What would make this faster

- A documented native drag-reorder list API and optional-handler syntax.
- Starter examples combining a seeded JSON catalog with persistent overrides.
- A clearer hint when sandbox restrictions block the toolkit's shared build-cache lock.
- A quick way to scope tree assertions to the visible tab or route.

No specification features remain unfinished. No external service or audio playback is needed.
