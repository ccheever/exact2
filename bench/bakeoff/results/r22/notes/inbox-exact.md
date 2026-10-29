# Inbox build notes

Implemented every requested feature using Exact. The complete 320-message `data/mail.json` is imported into the bundled data module; the original file is retained in the project. Read, flag, archive, and locally refreshed messages are stored with an atomic JSON write in the app’s private data directory. No sending or network access is present.

## Decisions and assumptions

- The supplied data has a preview but no body or recipient. The reader shows that supplied text as the message content, without fabricating an email address or a longer conversation.
- The unread count describes the Inbox; archived unread mail does not contribute to it. Archive supports restoring mail to Inbox.
- Refresh creates two local, unread messages with unique increasing IDs and dates that sort above existing messages. Refreshed messages survive relaunch too.
- Previews naturally occupy up to two lines at the normal iPhone text size. At accessibility sizes they expand to preserve readability.
- Filters use a native checked menu, which keeps full labels readable at the largest text size. Search is an extra local convenience.

## What was hard / looked up

- Read Exact’s starter, swipe, navigation, file storage, date formatting, menu, icon, and iOS driving recipes. Its date formatter supports specific named styles rather than arbitrary Intl styles.
- The first compiler invocation needed access to Exact’s shared compiler cache outside the workspace. Used the toolkit with the appropriate sandbox escalation.
- Native and web relaunch behavior differs: native tests restart at Inbox, so the archive persistence test explicitly reopens Archive.
- A native active search hides navigation items. The search test now finishes using the keyboard’s Search/Return interaction rather than trying to tap an obscured navigation item.
- Real-touch automation cannot tap a hidden native swipe action by its source test ID. Tested with actual full swipes instead: leading toggles read state, trailing archives. A real downward pull added exactly two new messages.
- Reduced row content by removing the optional third swipe action; Flag/Unflag remains in the native long-press menu and message screen.
- Exact’s built-in EmptyState already owns a scroll view. Nesting it inside a refreshable scroll view hid its content and intercepted the gesture. Used the same empty-state layout inside one refreshable scroller instead. A real pull from empty Archive increased Inbox from 320 to 322 and unread from 109 to 111, while Archive stayed empty.
- Message accessibility labels include unread status, sender, date, subject, preview, and flag status. Native menus expose the message actions.

## Validation and limitations

- All nine functional tests pass on iOS and web.
- Final full iOS verification: nine tests passed; no layout findings in light or dark mode; nothing spilled across eleven screens at the largest Dynamic Type size (53 pt body). After the empty-state refinement, all nine iOS and web tests passed again, and focused empty-state checks also passed at normal and largest text sizes.
- Remaining limitation: Exact’s native scrolling stress probe still reports eight timing/momentum findings against UIKit. Simplifying rows reduced hitch ratio from 33.6 to 25.1 ms/s, longest frame from 114 to 78.5 ms, and main-thread p95 from 11.76 to 9.39 ms/frame. Rows contain text, a small status marker and a chevron, without shadows, blur, images loaded from disk, or a per-frame scroll handler. Frame-perfect UIKit scrolling was not achieved. Diagnostics are in `.exact/verify/ios-feel/feel.json` and `feel.txt`.
- Web’s normal-size layout checks were clean. An earlier extreme-text pass reported open-menu overlaps and hidden swipe/route geometry; the delivered target is iPhone, whose layout checks are clean.
- No requested feature was left out.

## What would have made this faster

A writable shared compiler cache, clearer documentation of native relaunch routing and active-search automation, and better exclusion of hidden swipe actions and intentional menu overlays from web accessibility checks. The native list renderer could also benefit from lower per-row layout cost.

## Final installation

Built, installed, and launched with `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs ios --run`. The installed app is **Inbox**, bundle ID `dev.bakeoff.inbox.bbade70`. Only that simulator was used. Restored its text size to its original `large` setting after testing. No requested feature was left unfinished; the scrolling timing limitation above remains.
