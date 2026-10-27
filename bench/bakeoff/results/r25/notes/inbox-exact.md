# Inbox

Finished, built, installed, and launched as `dev.bakeoff.inbox.b8c7755` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

All requested iPhone features are implemented. The 320-message `data/mail.json` fixture is imported into the bundled data module. Read, flagged, archived, and newly received messages are saved atomically in local app storage. Writes are serialized to protect changes made during a refresh.

## Validation

- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: **clean**; seven tests passed.
- First screen, all seven test endings, and relaunch: no light/dark findings.
- Largest accessibility text size (53-point body), nine screens: nothing spills.
- Real simulator touches verified leading full swipe to mark unread, trailing full swipe to archive, and pull-to-refresh adding exactly two messages.
- List scrolling: nothing for the app to fix. Exact identified its own rubber-band and status-bar scroll timing differences; performance timing was not judged because the machine was heavily loaded.
- Final installation and normal launch: `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`.

## Hard parts and lookups

The largest issue was placing a fixed filter bar beneath a large-title navigation bar: UIKit's collapsing bar allowed the list to overlap it. A native toolbar filter menu solved this and works at every text size. Previews use two lines at normal sizes and expand at accessibility sizes.

I looked up Exact's list sections, native swipe declarations, refresh/refreshing attributes, icon tokens, persistence grants, accessibility viewport fact, and simulator touch commands. Native tests can activate swipe actions directly; checking real swipes requires `--touch platform` and bringing the row into view first.

A background prebuild briefly captured the starter app identity while it was being changed. Rebuilding after the identity was consistent resolved it. Native reloads and browser reloads also retain different navigation state; tests account for that. Search uses UIKit's search mode and cancellation behavior.

## Assumptions and limitations

The fixture contains previews but no separate message bodies or email addresses. The reading screen displays the complete supplied preview and the sender name. Locally generated messages use unique IDs and monotonically newer timestamps, including when the device clock predates the fixture.

No requested iPhone feature remains unfinished. The optional web preview passes functional tests and ordinary layout checks, but its largest-text diagnostics still report clipped offscreen swipe labels and overlaps beneath open popovers. The iPhone release target passes its separate verification cleanly.

## What would have made this faster

An Exact inbox starter with bundled fixtures, leading/trailing swipe actions, filtering, and refresh would help. Clearer guidance about fixed controls beneath large navigation titles, native search-mode testing, and the difference between assistive action activation and real touch gestures would also shorten iteration.
