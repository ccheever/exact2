# Inbox build notes

Finished and installed as **Inbox**, bundle ID `dev.bakeoff.inbox.cdd432`, on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final command was `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`.

## Implementation and decisions

- All 320 original messages ship through the data module's direct import of `data/mail.json`; the source file is unchanged.
- Messages are sorted newest first. Inbox and Archive each have search and All, Unread, and Flagged filters.
- Native leading swipe toggles read state. Native trailing swipe archives; its additional action flags or unflags. Long-press menus expose the same operations. Archived messages can be restored.
- Opening a message marks it read. The data has no separate body field, so the reading screen displays the complete supplied preview as its message text.
- Each refresh adds two locally composed, unread messages with unique IDs. Their timestamps remain above the existing messages, including when a test clock predates the seed data.
- Read, flagged, archived, and added-message state is saved atomically in `app:/data/mail-state.json`. A shared queued mutation serializes writes. No network access or sending is present.
- Semantic system fonts and colors, UIKit navigation, native menus, native refresh controls, and a custom inbox icon supply the iPhone presentation. Previews wrap further at accessibility text sizes to preserve their content.

## What was hard and what I looked up

I used Exact's local start guide, tabs pattern, and references for swipe actions, file grants, menus, dates, icon tokens, navigation, and simulator driving. No external web research was needed.

Native swipe rows required particular care: an authored scroll offset briefly shifted their content because UIKit already projects the swipe controls. Simulator screenshots caught the problem, and restoring the documented content-first arrangement fixed it. A native filter menu also avoids squeezing segmented labels at very large text sizes.

The real-touch simulator runner failed to register vertical list pulls. Exact's documented native scroll-drag path successfully triggered the actual refresh control, and the automated pull-to-refresh test passed. Full horizontal swipes were also checked using real simulator touches.

## Validation and remaining limitations

- `bun exact.mjs verify ios`: **clean**, 11 tests passed.
- Native light and dark checks: no findings.
- Largest accessibility text size, body 53 pt: nothing spilled across 13 checked screens.
- Relaunch verification confirmed persisted state. Tests cover read/unread, flags, archive and restore, combined archived/unread/flagged state, filtering, search, repeated refresh, and pull-to-refresh.
- The companion web build passed its functional suite and normal-size layout checks. Its largest-font popover checks still reported clipping and overlap. The requested iPhone app uses UIKit menus and passed its native checks.
- No requested iPhone features remain unfinished.

## What would have made this faster

A complete two-sided mail swipe example, a warning about scroll offsets on UIKit-projected swipe rows, and more reliable vertical-touch detection in the simulator driver would have reduced iteration. The existing native screenshots and largest-text verification were especially useful.
