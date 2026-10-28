# Inbox build notes

Finished and installed as **Inbox** (`dev.bakeoff.inbox.b1899a3`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

## Delivered

- All 320 supplied records from `data/mail.json` are included through the bundled data module, ordered newest first.
- Native navigation, search, unread indicators and counts, unread/flagged filter menu, reading view, Inbox and Archive.
- Native leading swipe for read/unread; trailing actions for Archive and Flag/Unflag. Archived messages can also be restored to Inbox.
- Pull to refresh creates two local unread messages. A navigation-bar refresh action provides an accessible alternative.
- Atomic local file storage retains read, flagged, archived, and newly generated messages across relaunches. No network or sending functionality.
- Custom app icon, system appearance, and scalable text.

## Decisions and lookups

The input provides a preview but no separate message body or recipient. The reading view displays the entire supplied preview and uses “You” as the recipient. Refresh messages are locally authored examples with unique IDs and dates newer than existing mail, including when the test clock predates the supplied records.

I started from Exact’s list-detail pattern and looked up swipe actions, file grants, pull to refresh, date formatting, icon tokens, and native simulator gestures. Persisted records are read from the file for each operation, and writes use a queued mutation and atomic replacement.

## What was hard

- A segmented filter did not fit at the largest text size. A native filter menu avoids squeezing its labels.
- A fixed filter row could overlap the native scrolling list after search. Moving it into the navigation bar removed the overlap.
- Hidden routes remain in the view tree. One test incorrectly treated a hidden Inbox count as evidence that Archive was nonempty; the corrected test asserts stored message state.
- The web driver could not consistently reveal a hidden swipe action after a message changed lists. The iPhone driver and actual native swipes worked. Web verification was run, but its swipe/menu findings are not a clean web release result; this deliverable targets iPhone.
- Initial shared compiler-cache access needed sandbox escalation. No toolkit files were edited.

## Validation

Final `verify ios`: **clean**, **6 passed, 0 failed**. Light and dark screens have no findings. At accessibility-extra-extra-extra-large (53-point body), nothing spills. Relaunch checks retained state and refreshed mail.

Actual simulator touches verified a full leading swipe, a full trailing Archive swipe, and pull to refresh increasing the collection from 320 to 322. The final scrolling report says “like UIKit”; the machine was busy, so it judged physics rather than timing. An earlier probe reported input lag, retained in the local verification history.

Final build/install command: `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`. It completed successfully and launched the installed app.

No required iPhone feature remains unfinished. Verification artifacts are in `.exact/verify/`.

## What would make this faster

A mail-style starter with bidirectional native swipe actions, a documented accessible filter-menu example, and more consistent web-driver handling of hidden swipe actions would have reduced iteration. Clearer separation between a native test’s visible screen and hidden navigation views would also help. A supplied body field would remove the need to interpret preview text as the readable message content.
