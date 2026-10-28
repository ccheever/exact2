# Podcasts — build notes

Finished and installed as `Podcasts` (`dev.bakeoff.podcasts.bbd9c85`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was booted, erased, or used.

## Delivered

- All 420 bundled episodes, newest first, with show, title, date, duration, and played status.
- All/unplayed filtering, plus search across titles, shows, and summaries.
- Twelve shows with live total/unplayed counts and show-specific episode lists.
- Episode details, summary, Play/Pause, played/unplayed controls, and a compact current-player bar.
- Direct list controls and swipe actions for played status and Up Next.
- Up Next with native Edit/reorder/delete controls, removal directly from rows, and accessibility reorder actions.
- Played overrides and queue order saved atomically in Application Support and restored on launch.
- Light/dark palettes, scalable text, full-width episode titles at accessibility sizes, compact accessible show layouts, custom show artwork, and an app icon.

The shipped `Resources/episodes.json` is byte-for-byte identical to `data/episodes.json`. The final installed build contains no simulator verification routing.

## Decisions

- Equal publication dates use ascending episode ID as a deterministic tie-breaker.
- Shows sort alphabetically; episodes within a show keep newest-first order.
- Queue entries are unique by episode ID. Already-played episodes can be queued. Playing an episode leaves its queue membership and order intact.
- Play immediately marks the episode played. Pause keeps that status; marking the current episode unplayed stops its simulated playback.
- Current playback is session-only. Played state and Up Next persist, as specified.
- Dates are parsed in UTC to avoid publication dates shifting across time zones.
- Show artwork was created locally with SwiftUI shapes and SF Symbols; no artwork, network service, or audio asset is required.

## Validation

Built and launched repeatedly with the supplied `build.sh` on the assigned simulator. Verified final bundle identity, signature, resource equality, installed container, and launched screen.

`Tests/ModelChecks.swift` passed checks for the full catalog, 12 show totals, chronological sorting, date parsing, initial played counts (125 played / 295 unplayed), toggling both ways, all/zero unplayed boundaries, queue deduplication, single/multiple reorder, removal, invalid-ID cleanup, and JSON persistence round trip.

A temporary simulator verification build exercised the actual observable store's play/pause, played toggles, stopping playback when marked unplayed, queue add/remove/reorder, and live counts. A separate termination/relaunch verified the played flag and exact queue ordering survived. Its fixtures were then reset to the original played values and an empty queue before installing the finished build.

Simulator screenshots were reviewed for Episodes, Shows, episode details, populated/empty Up Next, native queue edit mode, search with keyboard, dark appearance, and the largest accessibility text size. Screenshots remain under `build/`.

## What was hard / looked up

The minimal skeleton compiles Swift directly rather than using an Xcode project. SwiftUI macro expansion and CoreSimulator needed execution outside the shell sandbox. I consulted the supplied README and local `simctl help` for simulator launch, screenshot, appearance, and content-size controls; no external research was needed.

The first iOS 26 toolbar treatment cramped a custom text item; using standard navigation titles fixed it. The largest accessibility sizes needed different row geometry rather than simply enlarging the standard two-column layout. Decorative labels are capped independently; episode information continues to scale.

Desktop UI access to Device Hub was denied, so direct automated taps and drags were not available. Simulator validation used programmatic store checks and rendered screenshots, rather than a complete gesture-driven UI suite. Native editing controls were visually verified; their queue mutations were checked programmatically.

## Unfinished / faster next time

No known specification feature is unfinished. Audio is intentionally absent. A simulator touch/accessibility automation tool available from the start would have made end-to-end navigation and drag testing faster and more comprehensive. A supplied icon/cover-art direction would also have shortened visual design work.

## Rebuild

```sh
SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
```

Model checks:

```sh
mkdir -p build/ModuleCache
xcrun swiftc -module-cache-path build/ModuleCache Sources/Models.swift Tests/ModelChecks.swift -o build/ModelChecks
build/ModelChecks
```

`Tests/SimulatorChecks.swift` is only included in a temporary build compiled with `-D UI_TESTING`; normal `build.sh` does not include it. `Tools/GenerateIcon.swift` regenerates the already-shipped icon PNGs.
