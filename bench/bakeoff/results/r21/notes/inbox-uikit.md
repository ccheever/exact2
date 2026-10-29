# Inbox — build notes

Finished and installed as **Inbox** (`dev.bakeoff.inbox.218842`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final build launches with the original 320 messages (109 unread), normal text size, and light appearance. QA mutations were removed from this app’s storage; the simulator was not erased.

## Implementation

- Programmatic UIKit app/scene delegates, navigation controllers, Inbox and Archive tabs, self-sizing mail cells, and a scrollable reader.
- Bundles `Resources/mail.json`, byte-for-byte identical to `data/mail.json`.
- Newest-first mail; sender, subject, two-line preview, localized dates, unread indicators and counts.
- Left swipe archives (or restores from Archive); right swipe changes read status. Flags are available on rows, through swipe/context actions, and in the reader. VoiceOver exposes matching custom actions.
- All/unread/flagged filters and local search work in both mailboxes. Opening mail marks it read. Pull to refresh adds exactly two unique, locally generated messages.
- Atomic JSON storage in Application Support retains all message states and refreshed mail across launches.
- Dynamic colors, Dynamic Type, accessibility labels, native search keyboard/cancel, haptics, and a custom app icon.

## Hard parts and decisions

The simulator’s iOS 26 navigation bar reserved large-title space but did not render its large title. A self-sizing title in the table header, with the native compact navigation title appearing on scroll, gives consistent results. Preventing stack views from stretching sender labels also required an explicit top-aligned row layout.

At accessibility text sizes, three filter pills consumed too much vertical space. They become a native filter menu instead. Rows switch to a vertical metadata layout, show explicit unread/flagged status text, and allow full sender/subject wrapping. Empty states live in a self-sizing table footer so they remain below the header even at the largest text size.

The supplied data has no body field, so the reader displays the complete `preview` as the message content. The main unread count excludes archived mail; Archive shows its own unread count. Refresh timestamps are guaranteed newer than existing mail, including future-dated seed messages.

I read the skeleton README and build script and inspected the dataset. No external reference lookup was needed. Simulator operations required running outside the filesystem sandbox, using only the authorized device.

## Verification

- Final `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run` succeeded without compiler warnings and installed/launched the production app.
- Foundation model checks passed: seed counts, ordering, saved read/flag/archive states, filter intersections, archived unread-count exclusion, repeated refresh with unique IDs, whitespace search, and reloading refreshed mail.
- Simulator UIKit harness passed: opening marks read, read/unread and flag/unflag actions, archive/restore, filters, search/no-results state, swipe action configurations, and refresh adding exactly two newest messages.
- A separate process-relaunch check passed for refreshed messages and read, flagged, and archived states.
- Visually inspected simulator screenshots for light/dark appearance, largest accessibility size, large-text reader and list, Archive empty state, search keyboard, and the final production launch. Screenshots are in `build/`.
- The provided CLI has no touch injection; gesture configuration and the actual controller action paths were checked by the local harness rather than automated physical swipes.

To rerun model checks: `swiftc Sources/MailStore.swift Tests/ModelChecks.swift -o build/model-checks && ./build/model-checks`.

`Tests/build-test.sh --suite` installs a separate test-enabled build. Use `--reset` before and after a suite for the original dataset, then rerun the normal build command to restore the production app. For a process persistence check, run `--suite`, `--prepare-relaunch`, then `--verify-persistence`.

## Unfinished work and improvements to the toolkit

No requested feature is unfinished. A simulator touch/gesture driver and a small starter UIKit navigation/theme template would have made implementation and end-to-end interaction checks faster. A body field would permit a richer reader without inventing mail content.
