# Inbox — build notes

Finished, built, installed, and launched as `dev.bakeoff.inbox.b06c115` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used.

## Implemented

- Programmatic UIKit app delegate, scene delegate, navigation controllers, Inbox and Archive tabs.
- All 320 supplied messages are bundled unchanged in `Resources/mail.json`, listed newest first.
- Sender, subject, two-line preview, localized dates/times, unread dots, flags, unread counts and a tab badge.
- Leading swipe toggles read/unread; trailing swipe archives or restores, with a flag action. Flags also have visible buttons, a context menu, detail controls, and VoiceOver custom actions.
- Reading marks a message read. Archive supports reading, filtering, flagging and restoring messages.
- All, Unread and Flagged filters, plus local search with keyboard avoidance and empty states.
- Pull-to-refresh adds exactly two locally generated, unread messages. IDs remain unique and new messages sort above previous refreshes even when refreshed rapidly.
- Atomic local persistence includes read, flagged and archived state and newly generated messages.
- Adaptive colors, Dynamic Type, accessible labels/actions, native navigation, and a custom app icon.

## Hard parts and choices

The largest accessibility text size required different layouts: sender/date stack vertically, subjects wrap, the count summary becomes simpler, and filter pills become one native menu. The decorative sender avatar disappears at accessibility sizes so the sender name retains the full width.

The input has preview text but no separate message body or recipient. The reader displays the complete supplied preview as the message text and labels the local recipient “You.” Archive is reversible through Move to Inbox. Filters apply to the current mailbox; unread summaries count the entire current mailbox.

No web research was needed. I used the supplied README and `simctl` help for screenshots, appearance, content size and launching. Simulator access needed execution outside the shell sandbox because CoreSimulator was inaccessible inside it.

## Verification

`Tests/run-tests.sh` runs the checks on the assigned simulator and restores the final non-test build afterward. All **33 checks passed**: 29 behavior checks and four checks in a separate app process verifying persistence. Logs are in `Tests/behavior-results.txt` and `Tests/persistence-results.txt`.

The checks exercise UIKit control targets, table selection and swipe-action handlers, filtering, empty search results, repeated refreshes, unique IDs, live counts and save/reload behavior. They use a separate test data file and do not alter the normal inbox. Swipe callbacks were invoked programmatically; these are not physical gesture automation tests.

I inspected simulator screenshots in light mode, dark mode, the largest Dynamic Type size, message detail, Archive, and with the search keyboard visible. Screenshots are in `screenshots/`. The final bundle's plist and all 320 bundled messages were also verified.

## Remaining work and tooling

No specification features remain unfinished. A ready-made simulator gesture driver and screenshot matrix would have made testing faster. The lightweight build script worked well; a built-in test target would have reduced the setup for isolated simulator checks.

Build and launch:

```sh
SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run
```
