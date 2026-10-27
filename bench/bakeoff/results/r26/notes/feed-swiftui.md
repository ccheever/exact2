# Feed build notes

Built and installed **Feed** (`dev.bakeoff.feed.b84d370`) on the assigned simulator, `498D6D1F-C21C-4B31-90F4-5D857216E256`. The final running app is the production build, without QA flags. No other simulator was booted, erased, or operated.

## Implemented

- All 400 posts, sorted by actual relative age rather than source order; complete multiline text, author, handle, live relative time, likes, and reply counts.
- All 120 JPEGs plus both JSON files bundled under `Resources/data/`; verified byte-for-byte against the supplied files.
- Images preserve their recorded aspect ratios. Carousels scroll horizontally and show every image and title.
- Post detail navigation, larger rounded photos, full-screen image presentation, downward-swipe dismissal, close control, pinch/double-tap zoom, and zoomed-image panning.
- Pull to refresh inserts exactly three locally generated posts. Compose supports multiline Unicode text, rejects whitespace-only submissions, and protects nonempty drafts from accidental dismissal.
- Likes and added posts saved atomically in the app's Documents directory and restored on relaunch. Updates remain consistent between the feed and detail screens.
- Adaptive light/dark colors, Dynamic Type through the largest accessibility size, VoiceOver labels, generous touch targets, reduced-motion handling, keyboard focus, and a custom app icon.

## What was hard

The input isn't ordered chronologically. I converted `minutesAgo` to dates on first launch and persisted those dates so sorting and time labels remain stable across relaunches. New posts use unique IDs and timestamps newer than the current first post.

The simulator's current SwiftUI toolbar initially hid the Feed wordmark. Giving it its natural width and removing its shared glass background resolved this. The largest text size also required vertical metadata arrangements and a simpler compose header, leaving room for the editor above the keyboard.

Images are decoded off the main thread and kept in a bounded cache. Their metadata reserves the correct space before decoding, avoiding jumping post heights.

## Verification

`QA/qa-report.json` records **27 passing XCTest-free checks run inside the assigned simulator**. They cover seed loading and ordering, both initial like states, count reversibility, invalid IDs, blank and long Unicode composition, exact refresh increments, concurrent refresh coalescing, unique IDs, minute/hour/day boundaries, all image dimensions, complete disk roundtrips, and preservation of posts and likes across actual process termination and relaunch. These checks used an isolated file and did not add test posts to the final feed.

Simulator screenshots in `QA/` cover the feed, carousel detail, full-screen images, dark mode, and the largest accessibility text size with the keyboard. `QA/feed-final.png` shows the final production app.

Interactive desktop access to Device Hub was not granted. Consequently, taps, horizontal flicks, pinch gestures, and swipe dismissal were implemented and reviewed but were not driven by an external gesture automation tool. Screens were reached using QA-only launch arguments; those routes are compiled out of the production build. No specified app feature remains intentionally unfinished.

## Lookups and assumptions

Read the supplied README, build script, schemas, and image metadata. No web lookup or external image assets were needed. The local identity is “You” / `@you`; refresh content is a rotating set of fictional local moments. Reply counts are displayed as supplied; reply bodies and writing replies were not part of the specification.

## What would have made this faster

An approved simulator tap/drag automation interface would have enabled end-to-end gesture tests. A starter with an app-icon pipeline and documented behavior for the current system's toolbar would also have reduced setup work.

## Rebuild

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```

`QA/run-checks.sh` installs a QA-flagged build and runs the isolated simulator checks. Re-run the production build command afterward to leave the normal app installed.
