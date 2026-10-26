# Podcasts build notes

Finished and installed as `dev.bakeoff.podcasts.bc1aa03` on simulator
`C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final app is open on Episodes,
in light mode with standard text size. No other simulator was used.

## What shipped

- All 420 episodes and 12 shows, with the supplied JSON copied unchanged into the app bundle.
- Newest-first episode browsing, unplayed filtering, search, show counts, and show episode lists.
- Episode summaries, date and duration, Play/Pause state, and a compact playing strip.
- Played/unplayed and queue actions in row menus and swipe actions.
- Up Next with native drag reordering, edit-mode removal, swipe removal, and accessible Move up/down actions.
- Persistent played IDs and queue order, including duplicate prevention and cleanup of invalid saved IDs.
- Custom show artwork and app icon, adaptive dark colors, native navigation, and Dynamic Type layouts.

## What was hard

Date-only strings must be formatted in UTC: displaying midnight UTC using the device's Pacific timezone initially shifted dates back a day. This is fixed.

The largest accessibility text size needed separate layout decisions: shorter headers, hidden decorative artwork in rows, vertically arranged filters, buttons that grow with wrapped text, and a smaller playing strip.

The starter had no test project. I built a small XCUITest runner and in-memory state checks. Native iOS controls required inspecting their actual accessibility representations: search closes with “Close,” and edit-mode minus controls appear as images rather than Delete buttons. Tests now isolate their queue state between runs.

Compiler macros and CoreSimulator required execution outside the default process sandbox. The supplied build script itself remains unchanged.

## Decisions and lookups

I used the starter README, local SDK/compiler feedback, simulator output, and accessibility trees; no online research was needed. Same-day releases use ascending IDs as a deterministic tie-breaker. Playing an episode leaves it in Up Next until explicitly removed. Playing state is transient; played state and queue order persist.

## Validation

Passed state checks cover seed counts, date display, ordering, played transitions, duplicate queue additions, reorder/removal, persistence, stale IDs, and corrupt saved-state recovery.

Passed simulator checks cover the full browsing and relaunch flow, real drag handles, swipe and minus-control removal, search and keyboard dismissal, plus dark mode at the largest accessibility size. Test results and screenshots are under `build/`; the final screen is `build/finished.png`. Reproduction commands are in README.md.

No required features remain unfinished. A ready-made UI test target and preconfigured compiler/simulator sandbox access would have made this faster.
