# Inbox build notes

- Built with Exact for `dev.bakeoff.inbox.b8ab7da`; only simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` was used.
- All 320 original messages are imported from `data/mail.json` and the original JSON is also shipped as `assets/mail.json`.
- The dataset supplies a preview but no full body or email addresses. The reader uses that preview as its body and displays the sender name and “To: You.” Refresh creates two local messages, without sending or networking.
- Read, flagged, archived, and refreshed-message records are saved atomically to the app’s local data directory. A shared write queue prevents a refresh and a swipe from overwriting each other.
- The most time-consuming issue was authoring both native leading and trailing swipe actions. Exact expects the action buttons after the content; moving the leading action before it offsets the row on iOS. Native gesture screenshots were essential for diagnosing this.
- Looked up Exact’s swipe, persistence, dates, menu, icon token, routing, and iOS driver APIs. Used a toolbar filter menu because segmented labels did not fit at the largest Dynamic Type setting.
- Faster building would benefit from a complete mail-style swipe example, better web diagnostics for hidden native swipe controls, and a reliable direct tap on revealed UIKit swipe buttons in the real-touch driver.
- Back navigation uses Exact’s `go` operation for external locations; tests explicitly check returning from archived messages to Archive and then Inbox without duplicate screens.
- The separate web verification passes all ten functional tests but reports layout findings around hidden swipe actions and open popovers. This deliverable targets iPhone; its native verification checks the actual UIKit controls.

## Final validation

- `bun exact.mjs verify ios`: clean. Ten functional tests passed; light and dark appearances had no findings; twelve screens at the largest iPhone Dynamic Type size had no spills.
- Real simulator touches revealed native swipe actions in both directions. Direct native swipe-action activation confirmed the unread count changes and the message moves to Archive.
- A real pull-to-refresh gesture added two new messages at the top and changed the unread count from 109 to 111.
- The scrolling checker found nothing for the app to fix. It noted Exact-owned physics differences and a heavily loaded machine, so it did not judge timing on the final run.
- Final build installed and launched using `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`.
- No requested iPhone features remain unfinished. No data was sent to a server or to the Exact team.
