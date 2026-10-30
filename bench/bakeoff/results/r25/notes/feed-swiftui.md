# Feed — build notes

Finished and installed as **Feed**, bundle ID `dev.bakeoff.feed.bb96ee6`, on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. Only that simulator was used. The final app is launched with the original 400 posts, original likes, and no generated test posts or drafts. The simulator is back in light mode with standard text size.

## What was hard

- The JSON is not ordered by age. Posts are sorted by their actual relative timestamps, with stable ordering for ties. The initial time anchor is persisted so the feed keeps aging naturally across launches.
- SwiftUI canceled a refresh task when a published loading flag changed during an artificial delay. Refresh now commits its three local posts immediately, without a delay or observable loading flag. The native refresh gesture was verified with a deliberate pull and release.
- Post navigation, like buttons, photo buttons, and horizontal carousels need independent interaction. Simulator tests checked that liking and scrolling a carousel do not accidentally navigate.
- The largest accessibility size needed wrapping author metadata, full-height text, growing carousel titles, and a composer that leaves enough room above the keyboard. Screenshots caught a standalone separator and unnecessary composer helper text; both were refined.
- An iOS toolbar initially collapsed the leading brand to its icon. A principal toolbar title keeps the full Feed name visible.

## Lookups and decisions

Read the starter README and inspected local `simctl` and `xcresulttool` help. No web research or external packages were needed. A small XCTest UI-test project was added to drive real simulator taps, drags, keyboard entry, navigation, and relaunches.

The local composing identity is **You / @you**. Refresh rotates through nine locally written sample posts in batches of three, with fresh IDs and timestamps. Single-photo posts preserve the supplied aspect ratios; carousel thumbnails use a consistent crop, and their full-screen images preserve their original proportions. Likes and added posts are saved atomically in the app's Documents directory. Compose drafts also persist.

## Validation

- `artifacts/FunctionalVerified.xcresult`: four functional tests passed with zero failures. Covers likes and relaunch persistence, feed/detail synchronization, larger detail photos, full-screen swipe dismissal, all nine carousel cards, empty/whitespace compose rejection, multiline composing, and pull-to-refresh persistence.
- `artifacts/AppearanceFinal.xcresult`: two tests passed with zero failures in dark mode at `accessibility-extra-extra-extra-large`. Covers feed, detail, carousel titles, photo captions, keyboard, like controls, and saving/discarding drafts. Exported screenshots were visually reviewed.
- `Scripts/check-store.sh`: verified all 400 like round trips, chronological order, whitespace rejection, a long multiline post, composed-post likes, reload persistence, unique IDs, and exactly three additions on each of two refreshes.
- `Scripts/CheckAssets.swift`: verified all 400 shipped posts and all 120 JPEGs against the originals, every referenced filename, and every declared image dimension.
- Final build/install/launch succeeded using the requested `build.sh` command. Bundle name, ID, resources, and the final screen were checked. See `artifacts/final-build.log` and `artifacts/feed-final.png`.

## Unfinished work

None of the requested features remain unfinished. Everything runs locally without network access.

## What would have made this faster

A starter UI-test target or a simulator interaction driver would have saved setup time. A documented refresh-cancellation quirk for this simulator runtime and an initial large-text screenshot pass would also have shortened the iteration cycle.
