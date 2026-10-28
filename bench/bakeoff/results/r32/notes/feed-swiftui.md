# Feed — build notes

Finished and installed as `dev.bakeoff.feed.bc8d993` on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The final app was built and launched with the supplied `build.sh`. All 400 source posts, the image metadata, and all 120 JPEGs are bundled unchanged under `data/`.

## Implementation

- SwiftUI feed, native navigation, a focused text composer, draft-discard protection, full-screen images with downward-swipe dismissal, and horizontal image/title carousels.
- Posts are sorted by `minutesAgo`, with ID breaking ties. The JSON itself is not chronological. Initial ages become local dates so time labels continue advancing.
- Likes, authored posts, refreshed posts, and refresh sequence are saved atomically in the app's Documents directory. Feed and detail use the same observable store.
- Images retain their source aspect ratios. Post detail uses wider images; the viewer fits the full image and also supports pinch/double-tap zoom.
- Adaptive colors, Dynamic Type, VoiceOver labels, accessible tap targets, lazy vertical loading, and a bounded image cache. The decorative subtitle is omitted at accessibility text sizes to preserve reading space.

## Validation

All testing used only the assigned simulator. Passing automated UI runs are retained in `build/`:

- `FlowTests2.xcresult`: likes and shared detail state, image opening and swipe dismissal, whitespace rejection, multiline composition, relaunch persistence, three-post refresh, and the last card in a nine-card carousel.
- `AccessibilityFinal.xcresult`: dark mode at accessibility-extra-extra-extra-large text size, composer/keyboard use, keeping a draft, navigation, and zero-to-one-to-zero like counts.
- `MediaTests.xcresult`: measured portrait and landscape image ratios, full-screen portrait viewing, and repeated scrolling through mixed feed content.

The final app bundle's identity and every bundled data file were checked against the originals. Screenshots include `build/final.png` and the passing accessibility attachments under `build/accessibility-reviewed/`. Simulator text size and appearance were restored to Large and Light before the final install. Test content was reset, leaving the original 400-post feed.

## What was hard / what I looked up or decided

The skeleton had no Xcode project or UI-test target, so I created a small test-only project and shared scheme around the existing sources. Simulator access needed the sandbox's elevated execution path. One failed test driver stalled while collecting diagnostics and was stopped before continuing.

Simulator behavior showed that a toolbar-anchored confirmation dialog could omit its cancel action. A native alert gives explicit Discard post and Keep writing actions instead. Visual review also caught SwiftUI dimming detail text inside a disabled navigation button; detail now renders normal selectable text. The largest text size needed a stacked feed heading and a compact header.

No web research or third-party dependencies were needed. I inspected the starter README, the data shape, image dimensions, simulator test output, and screenshots. Since the specification supplies no profile photos or current-user identity, I chose deterministic initial avatars and `You` / `@you`. Refresh uses a rotating set of locally written posts and bundled images. Reply counts are displayed; there is no reply-writing feature in the specification.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A ready-to-run UI-test target and simulator interaction harness would have saved the most time. Explicit guidance about whether seed ages should advance, the current user's identity, and the desired visual style would remove small product assumptions.
