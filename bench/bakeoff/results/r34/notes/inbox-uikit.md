# Inbox — build notes

Completed the UIKit app and left it built, installed, and running on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86` with bundle identifier `dev.bakeoff.inbox.b3e2387`. The final installation starts with the original 320 messages (109 unread, 22 flagged). The supplied JSON is shipped unchanged in the app bundle.

## Implemented

- Newest-first inbox, initials, unread dots, flags, localized dates, and previews of up to two lines.
- Leading swipe to toggle read status; trailing swipe to archive or flag. Long-press and VoiceOver actions offer the same operations.
- Archive tab with restoration to the inbox, search, and all/unread/flagged filters in both mailboxes.
- Reading screen that marks messages read when opened, with flag, read/unread, and archive controls.
- Pull-to-refresh that adds exactly two local unread messages above existing mail, with unique persistent IDs.
- Unread counts, native navigation, keyboard dismissal, empty states, dark mode, Dynamic Type, accessibility labels, and an app icon.
- Atomic local JSON persistence for all messages and read, flagged, and archived states. A bulk mark-all-read menu is also included.

## What was hard

Getting self-sizing rows, a scrolling header, sticky horizontally scrollable filters, and native search to cooperate at the largest accessibility text size required simulator iteration. Decorative heading text is hidden at accessibility sizes, sender/date layouts become vertical, and avatar initials stay bounded. Filtering must retain stable message IDs so opening, moving, or changing a message cannot accidentally act on the next row after the list changes.

## What I inspected or assumed

I read the skeleton README, build script, and dataset, and used compiler diagnostics and simulator tools. No web lookup was needed. The dataset contains no separate message body or email addresses, so the reading screen displays the complete supplied preview text and sender name. Refresh messages are locally invented, as requested. Unread counts describe the current mailbox, and the Inbox tab badge excludes archived mail. Refresh returns to all mail so the two added messages are visible even when a filter was selected.

## Verification

28 simulator checks passed, including original data counts/order, filters, search and empty search results, opening a message, flag/unflag, read/unread, archiving/restoration, refresh, repeated refresh IDs, bulk marking read, and persisted state after a full process restart. Results are saved in `build/verification.txt` and `build/verification-relaunch.txt`. I inspected screenshots in light/dark mode and at the largest Dynamic Type size; no Auto Layout conflict messages were found in the app's simulator logs.

The temporary test harness invoked real UI action handlers and checked swipe configurations; it did not synthesize finger swipes. It remains under `Tests/` for reference and is excluded from the final app. The test-modified installation was replaced with a clean final build, and the bundle identifier, display name, scene manifest, and bundled JSON were checked. `build/final-inbox.png` captures the final screen.

## Unfinished work and faster tooling

No known specification features are unfinished. A supplied UIKit/Xcode project and a simulator gesture/accessibility automation driver would have made visual regression and end-to-end swipe/keyboard testing faster. The provided compiler-and-simctl workflow was sufficient to build, install, and verify the app.
