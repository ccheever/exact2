# Podcasts — build notes

Finished and installed as `Podcasts` (`dev.bakeoff.podcasts.b4658da`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final app was built and launched with the supplied `build.sh`. No other simulator was used.

## Implemented

- All 420 supplied episodes, sorted newest first, with show, title, calendar date, duration, and played status.
- All/unplayed filtering, episode and show search, and useful empty states.
- All 12 shows with live episode/unplayed counts and individual show episode lists.
- Episode summaries, Play/Pause/Resume, and played/unplayed controls.
- List menus, context menus, and swipe actions for played status and Up Next.
- An ordered, duplicate-free Up Next queue with native drag reordering, deletion, and a Move to Top action.
- Atomic local persistence of played overrides and queue order.
- Custom show artwork and app icon, native navigation and tabs, dark mode, VoiceOver labels, and layouts adapted to accessibility text sizes.

The original `data/episodes.json` is copied without changes into `Resources/episodes.json` and shipped in the app bundle. The final installation starts with the supplied played values (295 unplayed) and an empty queue.

## Verification

Local Xcode UI tests live in `Tests/`; the normal app remains a SwiftUI app built directly by `build.sh`. Tests use separate saved-state storage in DEBUG builds. The finished build excludes that test hook. The test runner and temporary app installation were removed before the final build.

Passing checks covered played/unplayed changes from lists, filtering, search and empty results, keyboard dismissal, show counts and navigation, episode details, Play/Pause/Resume, queue addition/removal, native drag reordering, swipe actions, empty queue navigation, and played/queue persistence across termination and relaunch. A separate test exercised dark mode at `accessibility-extra-extra-extra-large`, including queue editing. Reordering and swipe actions were checked again after the final layout changes.

Successful result bundles are in `build/Interactions.xcresult`, `build/AccessibilityVerified.xcresult`, and `build/FinalReorder.xcresult`. Screenshots are in `screenshots/`. Final bundle checks verified the name, bundle ID, icon resources, and byte-for-byte inclusion of all episode data.

## What was hard

Date-only source values must stay on their calendar day: formatting UTC midnight in the local time zone initially displayed the previous day. Display formatting now explicitly uses UTC.

The largest text size needed deliberate layout changes. Decorative headers and large cover art are reduced or omitted there, the segmented filter becomes a native toggle, Shows becomes one column, and editing queue rows removes secondary controls so titles have room beside native delete/reorder handles.

The starter had no Xcode project or simulator tap driver. I created a small local UI-test project to exercise real interactions. The toolchain required builds outside the shell sandbox for Swift macro plugins and CoreSimulator. Disabling automatic test diagnostics avoided a test-reporting hang.

## Decisions and lookups

I used the supplied README, local compiler feedback, and simulator/Xcode tooling; no web lookup or external dependencies were needed. Episodes released on the same day use descending IDs as a deterministic tie-breaker. Queue additions append; playing an episode keeps it in the queue until removed. Playback is an in-memory playing/paused state with no audio, as requested. Original played values remain the defaults until explicitly changed.

No specified feature remains unfinished. A ready-made simulator interaction harness and supplied show artwork would have made building and visual verification faster.
