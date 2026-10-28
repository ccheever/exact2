# Moodboard build notes

Finished and installed as `dev.bakeoff.moodboard.b57fd02` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final production build is running with the original 600 tiles and 62 seeded likes. No other simulator was used.

## Implementation

- Bundled the supplied JSON unchanged in `Resources/tiles.json`.
- Three-column square tile grids, exact HSL-to-RGB colours, centered emoji, board menu with All and ten boards, and newest-first ordering.
- Native navigation and horizontal detail paging through the current filtered collection, with explicit previous/next controls and first/last boundaries.
- Individual like/unlike, Liked grid, selection, select all/deselect all, bulk like, and confirmed bulk delete.
- Likes and deleted IDs are written atomically to the app's Documents directory before the displayed state changes. Failed saves leave the existing state intact and show an error.
- Dark appearance, Dynamic Type, VoiceOver labels, scrollable detail content, empty states, and a matching app icon.

## What was hard

Calendar dates in the source have no time zone. Initial rendering converted UTC midnight to the previous calendar day locally; explicit UTC date-only formatting fixes that. iOS 26's floating tab bar overlapped a standard bottom toolbar, so paging controls use a separate safe-area inset. At the largest accessibility size, the board filter stacks above the tile count and action icons retain practical touch targets.

## Decisions and lookups

Read the skeleton README and consulted local `simctl` help for screenshot, appearance, content-size, launch, and app inspection commands. No web research or external packages were needed. Equal added dates sort by ascending ID. Deletion is permanent within the app and requires confirmation. The detail pager retains its opening collection so unliking a tile in Liked does not unexpectedly jump to another page; returning to the grid reflects the new likes immediately.

## Validation

The separate QA build in `Tests/SimulatorChecks.swift` ran 21 passing checks covering shipped data, boards, initial likes, sorting, dates, HSL conversion, individual and bulk persistence, deletion, an empty collection, malformed saved data, and failed writes. A further test terminated and relaunched the simulator app and verified persisted likes and deletions. Results are in `build/qa-results.txt` and `build/qa-relaunch-result.txt`.

Simulator screenshots were inspected for the main grid, an Alpine filter, Liked, first/last detail pages, empty collection, and dark appearance at the largest accessibility text size. Screenshots are in `build/`. Direct computer control of the simulator was unavailable, so physical tap/swipe automation was not performed; the QA build instantiated the actual SwiftUI screens in the states being checked. Native SwiftUI handles navigation and page gestures.

`Tests/run-simulator-checks.sh [collection|detail|last-detail|filter|liked|selection|empty|persistence-write|persistence-read]` builds the isolated QA app. After using it, restore the production build with `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`. QA uses separate state files and its entry point is excluded from production.

No specification features were omitted. A simulator touch/accessibility automation tool would have made interactive verification faster and more complete.
