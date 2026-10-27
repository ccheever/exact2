# Podcasts

Finished, built, installed, and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`.
Bundle ID: `dev.bakeoff.podcasts.b56fdd1`.

## Implemented

- All 420 supplied episodes, newest first, with show, title, calendar date, duration, and played status.
- Live episode/show search and a persistent Unplayed Only switch.
- All 12 shows, accurate episode/unplayed counts, and each show's episode list.
- Episode detail with summary, date, duration, Play/Pause playing state, played toggle, and queue action.
- Played/unplayed and Up Next controls directly on episode rows.
- Duplicate-free Up Next queue with drag reordering and Edit controls for Earlier, Later, and Remove. Boundary moves are disabled.
- Played overrides and queue order survive relaunch. The supplied initial played flags are preserved until changed.
- Original JSON bundled unchanged as `assets/episodes.json`; the data module also imports the catalogue directly for offline use.
- App icon, native navigation stacks and tabs, empty states, accessible control labels, dark mode, and scalable text layouts.

## Validation

- `bun exact.mjs verify`: clean; 6 tests passed, light/dark and both widths checked, and largest-text checks passed.
- `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: clean; 6 tests passed, no light/dark layout findings, and nothing spilled at accessibility-extra-extra-extra-large.
- Native scrolling physics matched UIKit. The machine was busy, so Exact did not judge overall timing; measured main-thread p95 was 6.11 ms and neither tested lift dropped a frame.
- Real UIKit touches independently verified queue drag reordering, episode navigation, and Play marking an episode played.
- Final installation: `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs ios --run`.
- Verification screenshots are in `.exact/verify-ios/` and `.exact/verify-web/`.

## What was hard / looked up

- Started from Exact's tabs pattern and consulted its list, persistence, route, reorder, packaging, and iOS driving documentation.
- Reordering requires a virtual list with `appearance="none"`; grouped lists use a different rendering path. The queue uses the supported virtual drag API, with Edit buttons as an accessible alternative.
- Native relaunch returns to the root screen, while web reload retains the URL. Persistence tests explicitly reopen the relevant detail or queue after relaunch.
- Large Dynamic Type needed wrapping around row accessories and word-based layout for the large episode title. Tab captions follow the platform's compact navigation treatment.
- A standalone symbol initially expanded to the available width; explicit rem bounds fixed its presentation while retaining text-size scaling.
- An action named `filter` shadows the built-in array method in Contract. Renamed the action.

## Decisions / unfinished work

No specification features remain unfinished. Playback is a simple visible state, with no audio, as requested. Playing state resets on relaunch; played status and the queue persist. Queueing a played episode is allowed, and playing an episode does not automatically remove it from Up Next.

## What would have made this faster

A combined tabs/episode-list/reorder starter, a prominent cross-platform reload note, and a small example of appropriately constrained SF Symbols would have reduced lookup and iteration time.
