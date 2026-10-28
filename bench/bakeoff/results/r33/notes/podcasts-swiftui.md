# Podcasts — build notes

Finished and installed as **Podcasts**, bundle ID `dev.bakeoff.podcasts.b3bfd97`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The app is left running on the normal Episodes tab. No other simulator was used.

## Delivered

- All 420 supplied episodes, newest first, with show, title, publication date, duration, and played state. The original JSON is bundled unchanged; SHA-256 checks match.
- All/unplayed filtering, episode and topic search, and 12 show libraries with live total and unplayed counts.
- Episode summaries, simulated play/pause, and a persistent session mini-player. Playback marks an episode played; there is intentionally no audio.
- Played/unplayed and Up Next actions through row menus, context menus, and feed/show swipe actions.
- Up Next with native Edit/reorder handles, swipe removal, menu removal, and Move to top.
- Atomic on-device persistence of played flags and queue order, including recovery from invalid stored IDs or a damaged save.
- Original show artwork and app icon, adaptive light/dark colors, accessible control labels, and layouts that adapt at the largest Dynamic Type setting.

## Decisions and guesses

Episodes sharing a publication date are ordered by ID for a stable feed. A played episode stays in Up Next until explicitly removed. Queue additions append to the end. Playing another episode switches the preview; marking the current episode unplayed stops its preview. Playing/paused state is session-only; played flags and queue order survive relaunch. Dates use their source calendar date regardless of the device timezone.

## What was hard

Large accessibility fonts need different layouts: decorative artwork is omitted from episode rows, featured actions stack, detail buttons stack, and empty queue content scrolls. The iOS 26 floating tab bar and native list row accessories also affect available width. Artwork needed explicit scaling to keep longer show names inside each cover.

The compiler's macro plugin and CoreSimulator require execution outside the filesystem sandbox. The required build and simulator commands succeeded with that execution permission.

## What I looked up

Read the starter README and build script, inspected the JSON schema and counts, and used the installed `simctl` help for screenshot/input capabilities. No external services, downloads, or network data are required.

## Validation

Built and launched repeatedly using the provided `build.sh`. The final build passes 25 in-process checks on the designated simulator: decoding and sorting, all show counts, played/unplayed transitions, play/pause/switching, queue insertion, forward/backward reorder, moving to top, removal including the last item, save restoration, unknown/duplicate ID cleanup, and corrupt-save recovery.

A separate QA library was populated, terminated, and relaunched to verify that queued episodes and played status survived a real process restart. QA runs never modified the normal user library. Screenshots were reviewed for the feed, show list, show episodes, episode details, queue editing, playing state, restored queue, empty queue, unplayed filter, and empty search results. Dark mode and accessibility-extra-extra-extra-large were included. Simulator appearance/text size were returned to light/large afterward.

Test results: `build/verification-results.txt`. Screenshots: `build/screenshots/`.

The toolkit does not supply tap/swipe injection, so automated behavioral checks exercise the same shared model methods used by the controls; screen coverage uses simulator launch routes and screenshots rather than end-to-end touch automation. No specified app feature remains unfinished.

## Repeating checks

Build/install/launch:

```sh
SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 ./build.sh --run
```

Run the isolated library checks:

```sh
xcrun simctl launch --terminate-running-process C2188CC6-FBE9-409A-92A8-ABD89AC2C432 dev.bakeoff.podcasts.b3bfd97 --verify-library
```

The report is written inside the app container at `Documents/Verification/results.txt`. Optional screenshot routes are `--screen-shows`, `--screen-show`, `--screen-detail`, `--detail-bottom`, `--screen-queue`, `--queue-edit`, `--unplayed`, and `--search-empty`. `--qa-state --seed-qa` selects and populates a separate QA save file.

## What would make this faster

A supported simulator accessibility/touch automation driver would allow end-to-end gesture and keyboard tests. A scaffold with app-icon asset handling and snapshot automation would also reduce setup time.
