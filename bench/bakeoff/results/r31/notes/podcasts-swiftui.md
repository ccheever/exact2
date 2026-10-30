# Podcasts — build notes

Finished and installed as **Podcasts** (`dev.bakeoff.podcasts.b26589c`) on simulator `EF30C0A9-C815-44B2-AB5D-7B7ADFA8A3DC`. The final build was launched on the Episodes tab. No other simulator was booted or used.

## What ships

- All 420 episodes from `data/episodes.json`, copied unchanged to `Resources/episodes.json` and bundled for offline use.
- Newest-first browsing, unplayed filtering, search, show counts and show-specific episode lists.
- Episode details, simulated play/pause/stop, and played/unplayed actions directly in lists.
- A unique Up Next queue with native drag reordering, accessible Move Up/Down actions, and removal.
- Atomic local persistence of played overrides and queue order in the app’s Application Support directory.
- Custom show artwork, app icon, light/dark colors, accessibility labels, and layouts for the largest accessibility text size.

## What was hard

The starter compiles directly with `swiftc`, so I created a small test-only Xcode project to exercise the installed app with XCTest. Simulator access and Swift macro compilation needed execution outside the shell sandbox.

The native tab accessory has a fixed height. An ordinary bottom inset hid the tabs, and a fully scaled two-line mini player overflowed at accessibility sizes. The final accessory keeps navigation visible and uses a compact single-line state at larger sizes. Episode rows stack their controls, wrap show names, and omit decorative thumbnails at accessibility sizes; Shows switches to one column. The current-episode sheet retains its selected episode if marking it unplayed stops playback.

The simulator’s newer search UI uses “Close” instead of “Cancel.” Automation also exposed underlying and presented episode controls with identical labels, so the sheet test needed an explicit scope. Automatic failure diagnostics stalled one test run; subsequent runs disabled verbose diagnostic collection.

## Lookups and decisions

I consulted local `simctl` and `xcodebuild` help and compiler availability diagnostics. No web research was needed. I chose ascending episode ID as a deterministic tie-breaker for equal dates. Dates are treated as calendar dates in UTC to avoid shifting a day in local time zones. Adding an episode appends it once; playing does not remove it from Up Next. Playback itself is transient, while played state and queue contents survive relaunches.

## Verification

- Model checks passed for all episode/show counts, ordering, state overrides, playback state, duplicate queue prevention, multi-item moves, deletion, persistence, unknown saved IDs, all-played/all-unplayed states, and invalid data.
- Simulator UI checks passed for filtering, row actions, search and keyboard behavior, show navigation, simulated playback, native drag reordering, queue removal, and played/queue persistence after termination and relaunch.
- The playback-sheet regression passed in `build/SheetVerified.xcresult`.
- Dark mode at `accessibility-extra-extra-extra-large`, including playback and navigation, passed in `build/AccessibilityVerified.xcresult`; screenshots were visually reviewed.
- The final app rebuilt and installed successfully. Simulator appearance and content size were restored to light/large. The temporary UI-test runner was removed.

The model checks are in `Tests/LibraryChecks.swift`; UI scenarios and the test-only project are in `Tests/`. Logs, screenshots, and result bundles are under `build/`.

## Unfinished work / what would help

No requested features remain unfinished. Playback intentionally produces no audio, as specified. A ready-made simulator interaction harness and UI-test target in the starter would have made this substantially faster.
