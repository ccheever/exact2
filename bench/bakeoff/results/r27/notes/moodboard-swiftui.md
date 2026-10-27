# Moodboard build notes

Finished and installed as **Moodboard**, bundle ID `dev.bakeoff.moodboard.bba00a1`, on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. Only that simulator was used. The final installation starts with all 600 tiles and the 62 original likes. Test mutations were cleared, and simulator appearance/text size were returned to light/large.

## Implementation

- SwiftUI navigation stacks and native tabs, a lazy three-column grid, and an eleven-option board menu in each tab.
- Correct HSL-to-sRGB tile colours with the supplied emoji centred in each square. `Resources/tiles.json` is an exact copy of `data/tiles.json` and ships inside the app bundle.
- Swipeable detail pages with title, board, date, like/unlike, position, and accessible previous/next buttons.
- Multiple selection, select/deselect all, batch liking, batch unliking in Liked, confirmed batch deletion, and undo of the latest deletion during the current session.
- Likes and deleted IDs save atomically to the app's Documents directory immediately after every change. Failed saves retain the previous state and display an error.
- Adaptive light/dark colours, VoiceOver labels, scrollable detail content, and stacked filter/action layouts at accessibility text sizes.
- A custom app icon. `build.sh` now compiles the asset catalog as part of the existing build/install flow.

## Hard parts

The minimal starter had no UI automation target or asset catalog compilation. I added a small UI-test project that drives the installed app, and extended the build script for the icon.

The largest text size needed more than scalable fonts: selection mode now uses an inline navigation title, removes the decorative subtitle, and stacks batch controls so the grid remains usable. Visual review also caught insufficient contrast on prominent buttons in dark mode; the button foreground now adapts to the lighter accent.

Paging needs to preserve its context when unliking a tile in Liked, while reacting immediately to a deletion made from another tab. The detail view retains its opening collection but removes deleted IDs and moves to the nearest remaining tile.

## Lookups and decisions

I used the starter README and local Xcode command-line tooling. No web research was needed.

Unspecified behaviour was resolved as follows: equal dates sort by ascending ID; detail neighbours follow the filtered grid that opened the detail; unlike keeps the current detail page visible until returning to the grid; each tab retains its own board filter. Dates are parsed and displayed in UTC so a date-only value cannot shift to the previous day with the device's time zone.

## Verification

- Foundation model checks passed for the 600 records, initial likes, newest-first ordering, all ten board filters, HSL conversion, date formatting, liking/unliking, deletion, empty collections, undo, and Codable persistence.
- All seven final simulator UI tests passed with zero failures. They cover swipes and paging boundaries, individual and batch likes, relaunch persistence, every board, deletion confirmation/cancellation, whole-board and whole-collection deletion/undo, empty Liked states, and deletion while a detail is open in the other tab.
- The separate dark-mode run at `accessibility-extra-extra-extra-large` passed, including actual tile selection and enabled, reachable batch actions. Screenshots were visually reviewed.
- Final build/install/launch succeeded using the requested command. The app remains installed and running. `build/final.png` captures the final main screen.

Model checks can be run with:

```sh
xcrun swiftc Sources/Tile.swift Tests/ModelChecks.swift -o build/model-checks
./build/model-checks
```

The UI suite is in `Tests/MoodboardUITests.xcodeproj`. Run it against a fresh installation; its final test deliberately deletes two tiles to verify persistence. Test results are in `build/Final-results.xcresult`; the largest-text dark-mode results are in `build/Accessibility-final-results.xcresult`.

## Unfinished work

None of the requested features remain unfinished.

## What would make this faster

A starter with an existing UI-test target, a simulator interaction helper, and asset-catalog support would remove most setup work. A defined policy for equal-date ordering and detail paging after unliking would also eliminate a few behavioural decisions.
