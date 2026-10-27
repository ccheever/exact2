# Inbox build notes

Inbox (`dev.bakeoff.inbox.b06dfbf`) is finished, built, and installed on iPhone simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using `bun exact.mjs ios`. No other simulator was used.

All requested features are implemented: 320 bundled messages sorted newest first, sender/subject/preview/date rows and unread markers, native leading Read/Unread and trailing Archive/Flag swipes, an Archive screen with restore-to-Inbox actions, flag toggles, All/Unread/Flagged filters, a reader that marks messages read, pull-to-refresh generating two persistent local messages, and an inbox unread count. Read, flagged, archived, and generated-message state are saved with atomic local file writes and queued mutations. There is no sending or network access.

The original `data/mail.json` is imported as seed data and shipped unchanged as `assets/mail.json`; the built app bundle was checked for that file and all 320 records.

## Validation

- Nine behavior tests pass on the web and on the designated iPhone simulator.
- Final `verify ios` result: **clean**, nine passed, zero failed.
- Light and dark mode: no layout findings across the initial screen, test end screens, and relaunch.
- Largest accessibility text size (53-point body): nothing spills across 11 checked screens. Rows expand to keep text readable.
- Real simulator touches exercised full Archive, restore, Read/Unread swipes, the native filter menu, and pull-to-refresh. Refresh added exactly two messages.
- Relaunch preserved message state and refreshed messages.
- Native scrolling verification found nothing for the app to fix. The host machine was heavily loaded, so Exact judged physics but did not judge timing. Exact also reported its own rubber-band and status-bar-scroll differences, outside the app's control.

## What was hard and what I looked up

I started from Exact's list-detail pattern and looked up its native swipe actions, routing, file grants, queued mutations, date formatting, menus, asset packaging, and simulator-driving APIs. The shared compiler cache needed sandbox escalation even for some documentation commands.

A fixed filter strip beside the native list overlapped scrolled rows at normal and accessibility sizes. Final filters use UIKit navigation menus instead. An optional search feature exposed a separate native search presentation that the test driver could not dismiss through ordinary Back; it was removed because search is outside the requested specification. The final required navigation flows pass.

Real swipe tests needed a start point near a row edge: a drag ending outside the simulator screen falls back to a recognized scroll. Naming the date and unread-marker nodes made full native swipes testable. Native plans update quickly during development, but a final `ios` build was necessary to install the latest plan for ordinary launches.

## Assumptions and completeness

The data provides a preview but no separate full body or recipient address. The reader therefore displays the supplied preview as the message body and identifies the recipient as “You.” Dates use the device's local formatting. Refresh messages are synthetic local content with unique IDs and timestamps that keep each refresh above existing mail.

No requested feature remains unfinished.

## What would make this faster

A mail starter with leading/trailing swipe actions and native filter menus, clearer guidance about fixed controls next to native lists, a complete icon-role list, direct support for dismissing the native search presentation in tests, and a documented drag-start-position option would reduce iteration. A writable shared compiler cache and a less-loaded simulator host would shorten build and verification waits.
