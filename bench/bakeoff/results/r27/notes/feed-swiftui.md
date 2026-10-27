# Feed — build notes

Finished and installed as **Feed**, bundle ID `dev.bakeoff.feed.bc5d36b`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final app was built and launched with the supplied `build.sh --run`. It is left running with the original 400 posts and original like states; posts created during testing were cleared before the final install.

## Implementation

- Native SwiftUI navigation, a lazy feed, full post text, original-aspect-ratio photos, horizontal titled image collections, and a larger post detail view.
- Full-screen photos support swipe-down dismissal, a close button, pinch zoom, and double-tap zoom.
- Likes update consistently across the feed and post detail. Atomic local JSON saves preserve counts, liked states, composed posts, and refresh additions across relaunches.
- Pull to refresh adds exactly three locally invented posts. Compose rejects whitespace-only posts, brings published posts to the top, supports keyboard dismissal, and confirms discarding a draft.
- Adaptive light/dark colors, Dynamic Type, accessibility labels, and vertically arranged actions at accessibility text sizes.
- The supplied JSON and all 120 JPEGs are shipped unchanged under `Resources/data/`. Images use a bounded in-memory cache.

## Hard parts and decisions

The input array is not chronological: some later IDs are newer than earlier IDs. Posts are sorted using `minutesAgo`, with stable ordering for ties. A first-launch time anchor turns the offsets into persisted dates, so relative times continue advancing across launches.

Keeping native navigation branding visible required opting that toolbar item out of the shared glass background and fixing its intrinsic width. On this simulator’s iOS version, a confirmation dialog hid its cancel action; a native two-button alert makes “Keep writing” and “Discard post” explicit.

Largest-size typography needed wrapping author metadata and vertical action layout. Image dimensions come from the supplied metadata, independently checked against every actual JPEG. Collections intentionally use uniform cropped thumbnails; opening a thumbnail shows the entire original image.

I chose initials for avatars, “You” / `@you` for the local author, and a small rotating set of invented refresh posts. Reply counts are displayed; reply writing and reply bodies are outside the specification.

## Verification

Only the assigned simulator was used. No network or third-party app dependencies are required.

- `python3 Tests/validate_data.py` validated 400 unique, unchanged posts, 120 unchanged JPEGs with matching dimensions, and 659 valid image references.
- Simulator UI checks passed for liking/unliking, exact count persistence after terminate/relaunch, shared detail state, opening a post, full-screen image viewing, and swipe-down dismissal.
- Checks passed for horizontal collection scrolling and opening a different card after scrolling.
- Checks passed for empty/whitespace compose rejection, publishing at the top, post persistence, refresh adding exactly three posts, refresh persistence, preserving a draft, and discarding it.
- Dark-mode checks at `accessibility-extra-extra-extra-large` passed for feed scrolling, action bounds, liking, composing with the keyboard visible, and draft dismissal. Saved screenshots were visually reviewed.
- Final evidence is in `build/FinalStandard.xcresult` (navigation/likes, compose/refresh, draft checks), `build/FinalCarousel.xcresult`, and `build/FinalAccessibility.xcresult`. The carousel check was corrected to scroll its collection fully into view before testing card hit targets; its final targeted run passes.
- `build/final.png` shows the final clean installation. Accessibility screenshots are in `build/accessibility-attachments/`.

Nothing in the requested specification remains unfinished.

## What would have made this faster

A ready-to-run simulator UI test target would have saved setup time. A documented viewport-aware gesture helper would also have avoided false test failures from partially visible carousel cards. Local `simctl` help and Xcode’s installed test tooling were sufficient; no web lookup was needed.
