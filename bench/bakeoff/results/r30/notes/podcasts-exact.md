# Build notes

Podcasts is complete and installed on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` with bundle identifier `dev.bakeoff.podcasts.be96240`. No other simulator was used.

## Delivered

- A newest-first, searchable library of all 420 episodes with show, title, date, duration, and played status.
- A native filter menu for all episodes or unplayed only.
- All 12 shows, alphabetically ordered, with episode and unplayed counts and their own episode lists.
- Episode details with summary, metadata, Play/Pause, played-state actions, and queue actions.
- Direct played/unplayed and Up Next actions on episode rows.
- An ordered Up Next queue with real drag handles, accessible Move Earlier/Later buttons, and removal controls.
- Atomic local storage for played-state overrides and queue order, preserved across relaunches. Writes are serialized.
- Native navigation, search, menus, tabs, dark-mode colors, Dynamic Type, and a custom app icon.

The original `data/episodes.json` remains unchanged. It is imported into the bundled data module, and an identical raw copy is shipped as `assets/episodes.json`; the latter was confirmed inside the built app bundle.

## What was hard / looked up

Exact's tested tabs pattern was the starting point. Local toolkit documentation supplied routing, data storage, native menus, asset bundling, virtualized lists, and reordering syntax. No web research or external services were needed.

Grouped lists cannot be virtualized, so the long episode feeds use virtualized lists while episode details use native grouped sections. Looking up `reorder` did not find a vocabulary entry; searching the guide for `reorderdrop` found the necessary contract.

The initial segmented filter and queue controls beside text were too cramped at the largest accessibility size. The final filter is a native menu, episode text gets the full row width, and editing controls sit below it. Long words may wrap when necessary without reducing text size.

Native drag testing requires `--touch platform`, moving past the neighboring row's midpoint, and allowing the drop animation to settle. Real touches confirmed the reordered queue and the Play state. The initial compiler calls also needed approved access to Exact's shared build cache outside this app directory.

## Decisions where the specification was silent

- Equal dates are ordered by ascending numeric episode ID.
- Queue additions append once; duplicate entries are prevented.
- Playing an episode marks it played but leaves it in Up Next until explicitly removed.
- Pause clears the playing indicator without undoing played status. The active playing indicator is session-only; played state and queue order persist.
- Dates are formatted with an explicit UTC time zone to prevent calendar-day shifts.

## Validation and remaining limitations

`bun exact.mjs verify` and `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios` both finished cleanly: six tests passed on each, light/dark checks passed, relaunch persistence passed, and nothing spilled at the largest text size. Tests cover direct status changes, counts, show filtering, Play/Pause, queue ordering/removal, search, and empty-queue navigation.

The native scrolling check reported nothing for the app to fix. It separately flagged Exact-owned frame timing during scrubbing; its report is `.exact/verify/ios-feel/feel.json`. An earlier run could assess physics only because the machine was busy. No requested app feature is unfinished; audio is intentionally absent.

The final build was installed with `bun exact.mjs ios`. Development state was then cleared from the default store: the app starts with 295 unplayed episodes, the supplied 125 played episodes, and an empty queue. The final light/dark screen check passed.

## What would have made this faster

A tested episode/feed pattern combining tabs, persisted flags, queue reordering, and seed JSON; a `reorder` vocabulary alias; and clearer upfront guidance distinguishing grouped lists from virtualized feeds would reduce iteration. A direct Dynamic Type preference alias would also help: the driver uses `prefer root-font-size 53` for the largest iOS category.
