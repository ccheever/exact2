# Podcasts build notes

Completed and installed as `dev.bakeoff.podcasts.b6eb7f7` (Podcasts) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## Delivered

- All 420 episodes, sorted newest first with stable ordering for equal dates.
- Library search and an All/Unplayed navigation-bar filter.
- Twelve shows with live episode and unplayed counts, and individual show lists.
- Episode summaries, dates, lengths, Play/Stop, and a simple Playing state.
- Played/unplayed and Up Next actions directly on episode rows.
- Up Next removal, long-press drag reordering, and accessible move-up/down controls.
- Played changes and queue order persist across relaunches. Original played values are preserved until explicitly changed.
- Native navigation, independent tab stacks, semantic colors, Dynamic Type, and a custom icon.
- The original `data/episodes.json` remains unchanged. Its content is embedded through the data module and the complete file also ships as `assets/episodes.json`.

## What was hard and what I looked up

I used Exact's start guide, tabs pattern, and references for persistence, native lists, navigation, drag reordering, simulator driving, and responsive text. No web research was needed.

Exact's platform-owned grouped lists cannot also be virtualized. The long episode feeds and queue use virtualized lists; the short Shows screen uses a native grouped list. A fixed filter row initially overlapped the native scrolling content, especially at the largest text size. Moving that filter into the navigation bar resolved it.

Drag grips need a non-button view and an intentional long press on iOS. The automated gesture also needs to land well inside its destination; a drop at the first row's top boundary behaved differently between the web and iOS drivers. Both native drag reordering and accessible arrow controls were exercised.

Dates are formatted from a UTC date to prevent calendar-day shifts. I chose to leave played episodes in Up Next until explicitly removed. The temporary Playing state stops on relaunch; only played status and queue order were specified to persist. Playback intentionally has no audio.

## Validation

- `bun exact.mjs verify`: clean, 8 tests passed, no default or largest-text layout findings, including light/dark and relaunch checks.
- `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: clean, 8 tests passed, no light/dark or largest-text layout findings; real-touch scrolling checked.
- Native manual drives exercised the software keyboard, search, episode navigation, playing, tab returns, queue actions, and long-press reordering.
- Final `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs ios` completed successfully and installed the final bundled app.

The native scrolling report found nothing for this app to fix. It flagged an Exact-owned momentum hand-off measurement and declined timing judgments because the shared machine was heavily loaded. Details are in `.exact/verify/ios-feel/feel.json`.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A podcast-sized list pattern combining tabs, bundled JSON, row actions, and native reordering would have reduced setup. Earlier documentation of native filter placement and the grip's long-press gesture would have avoided iterations. The shared compiler caches and heavily loaded simulator host also added waiting time.
