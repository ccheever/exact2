# Podcasts build notes

Finished and installed as `dev.bakeoff.podcasts.b36448e` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. All requested features are complete. The original 420-episode JSON is bundled unchanged. Playback is intentionally simulated, with Play, Pause, Resume, and a shared mini-player.

The trickiest parts were preserving queue order when moving items downward, keeping every screen's counts and played indicators synchronized, and making the largest accessibility text size usable. At accessibility sizes, episode artwork gives way to text, actions move below each episode, the filter becomes a native switch, and headers become simpler. Dates are treated as calendar dates in UTC so the displayed day does not shift with the device's time zone.

Decisions where the specification was silent: episodes on the same date retain the dataset's order; a queue contains each episode at most once; playing an episode leaves it in the queue until explicitly removed. Played state and queue membership/order are saved atomically in Application Support. The transient playback session is not restored on launch.

No web research or third-party dependencies were needed. Local compiler diagnostics and simulator/XCTest tooling supplied the API feedback. The starter had no Xcode project, so a small test-only project was added under `Tests/` to exercise the installed interface.

Verification passed: model checks for dataset size, sorting, played changes, queue moves/removal, duplicate prevention, saved-state round trips, invalid saved IDs, and empty filters; simulator UI tests for list actions and swipes, filtering, search and keyboard interaction, episode details, Play/Pause/Resume, show navigation and counts, queue editing, and persistence across relaunch. Screenshots were inspected in light/dark mode and at the largest accessibility text size. The final app was rebuilt and installed using the provided `build.sh --run`; simulator appearance was restored to light with standard text size. Test changes were cleaned up.

Nothing from the specification remains unfinished. A starter UI-test target and preconfigured app-icon asset support would have made this faster. Test instructions are in `Tests/README.md`; screenshots and result bundles are in `build/`.
