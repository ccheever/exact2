# Podcasts build notes

Finished and installed as **Podcasts** (`dev.bakeoff.podcasts.ba50283`) on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The app is left open on Episodes, in light mode at the standard text size, with the original played states and an empty Up Next queue.

All seven specified features are implemented. `Resources/episodes.json` is an unchanged copy of `data/episodes.json`, bundled with the app. There are 420 episodes, 12 shows, and 295 initially unplayed episodes. Search, custom show artwork, an app icon, swipe actions, and a compact playing indicator are also included.

## What was hard

- SwiftUI List added unwanted disclosure controls when a row contained multiple navigation links. Explicit navigation paths keep episode navigation separate from played buttons and action menus.
- The largest accessibility text size needed deliberate layouts: one-column show cards, stacked counts, simpler headers, and vertically arranged status controls. The optional compact player is hidden at accessibility sizes; the episode screen retains the full Play/Pause controls and playing state.
- Queue editing needs explicit state so removing the final item exits editing correctly. This was verified by adding another episode afterward.
- Persistent state uses episode IDs, atomic JSON writes, and cleanup of duplicate or stale queue IDs. UI tests use a separate saved library so repeated runs do not change the normal library.
- Compiler plugins and CoreSimulator required execution outside the default sandbox. All simulator builds and tests targeted only the specified device.

## Lookups and decisions

No external web research or dependencies were needed. I used the starter README, the supplied dataset, and compiler diagnostics. The compiler identified that the conditional native tab accessory API requires iOS 26.1; the app includes an iOS 26.0 fallback.

I treated dates as calendar dates in GMT to avoid shifting publication dates across time zones. Older-year dates include their year in lists. Equal-date episodes are ordered by ID for stable ordering.

Playing marks an episode played immediately. Played episodes remain in Up Next until removed, and adding the same episode twice does not create duplicates. The current playing state is transient; played state and queue order persist. Audio is intentionally absent, as specified.

## Verification

- The final standard-size UI suite passed all three tests with zero failures: episode/show navigation, playback, list marking, live filtering, counts, queue persistence, drag reordering, removal, and search.
- Dark-mode navigation and playback passed at the largest accessibility text size. Captured layouts were reviewed, including after the header refinements.
- An extended queue/search test passed after the final queue-editing change, including deleting the last item while editing and adding another afterward.
- All 16 in-app data/persistence checks passed, including both reorder directions, duplicate prevention, stale-ID cleanup, and saved unplayed overrides.
- The final app was rebuilt and installed with the provided `build.sh`. Final screenshots of Episodes, Shows, episode detail, and the empty queue were reviewed.

Evidence is in `build/FinalUITests.xcresult`, `build/QueueEdgeCases.xcresult`, `build/AccessibilityFinal.xcresult`, `build/model-verification.log`, and `build/final-screenshots/`. `Tests/create_project.py` regenerates the small Xcode project used solely for UI testing; the app still builds through the original script.

## Unfinished work and improvements to the toolkit

No required functionality remains unfinished. A starter UI-test target and a documented simulator interaction driver would have reduced setup time. An explicit minimum-runtime/API compatibility guide and initial visual direction would also have shortened iteration.
