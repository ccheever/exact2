# Feed — build notes

Finished and installed as `dev.bakeoff.feed.b7ec690` on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`.

## Implementation

- SwiftUI navigation, lazy vertical feed, horizontal carousels, compose sheet, post detail, and full-screen photos with swipe-down dismissal.
- All 400 supplied posts and all 120 JPEGs, including their metadata, ship under `Resources/data/`.
- Posts are sorted by actual relative time; the supplied JSON is not already chronological.
- Atomic local JSON storage preserves likes, composed posts, refresh posts, and the initial date used to interpret `minutesAgo`.
- Adaptive light/dark colors, Dynamic Type, accessible control labels, rounded images at their original aspect ratios, and a custom app icon.

## What was hard

The largest accessibility text size needed real keyboard testing. Decorative compose copy crowded the editor, so that copy is omitted at accessibility sizes. The system confirmation sheet hid its cancel action with the keyboard showing at that size; an alert presented after dismissing the keyboard keeps both choices available. A disabled author header also dimmed the post-detail screen, so it now retains normal contrast without accepting taps.

The workspace initially reported no space even for an empty directory, and sandboxed simulator commands could not connect to CoreSimulator. Retrying the directory creation and running the supplied build/simulator toolkit with the required sandbox access resolved those environment issues.

## Lookups and decisions

Read the skeleton README and build script, inspected the actual JSON schema and image metadata, and used local `simctl`/`xcresulttool` help. No external research or network service is needed by the app.

The specification leaves the current user's identity and refreshed content open. Composed posts use “You” / `@you`; refresh generates three locally written posts with unique IDs. Initials provide avatars because the dataset supplies no profile photos. Carousel titles also label their full-screen photos.

## Verification

Automated simulator checks verified like counts and persistence after termination/relaunch, post navigation, full-screen image opening and swipe dismissal, pull-to-refresh, whitespace-only compose validation, multiline publishing, and composed-post persistence. Additional checks reached the final carousel card, opened its photo, exercised dark mode, and verified text entry plus both draft-cancellation choices at the largest accessibility text size. Successful functional results are in `checks/Results2.xcresult`; the final accessibility regression passed in `checks/AccessibilityFixed.xcresult`. Screenshots were reviewed during development.

Resource checks verified every referenced file and byte-for-byte equality between supplied data and bundled data. Persistence was also inspected directly after relaunch. QA-created posts were then removed; the installed app starts with the 400 supplied posts in newest-first order. The simulator is left in light mode at its normal text size, with Feed running.

## Unfinished work

None of the specified features remain unfinished.

## What would have made this faster

A ready-made simulator interaction-test target and reliable initial sandbox access to CoreSimulator would have shortened setup. Explicit sample copy and a current-user profile would have removed the few product assumptions.

## Rebuild

`SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 ./build.sh --run`
