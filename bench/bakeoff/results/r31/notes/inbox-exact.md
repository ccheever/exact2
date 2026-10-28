# Build notes

Inbox uses Exact’s native iPhone navigation, mail rows, swipe actions, segmented filters, search, context menus, pull to refresh, and system text and colors. The 320 supplied messages are bundled through the data module; the original JSON is also included in assets/mail.json.

The hardest part was arranging bidirectional swipe controls without shifting the row content. The native screenshot and largest-text checks caught an offset introduced while working on the web layout. Keeping the action controls after the content, as Exact’s swipe recipe specifies, corrected it.

I looked up Exact’s list-detail starter, native swipe wiring, file grants, refresh handlers, date formatting, accessibility text-size fact, icon tokens, and simulator-driving commands. The first compiler command needed expanded sandbox access because the prescribed toolkit keeps its build lock in its own checkout.

The supplied data has previews but no separate message body, so the reader displays each supplied preview as its complete available text. Refresh generates two deterministic local messages, adds unique IDs and increasing timestamps, and persists them. Inbox unread counts exclude archived messages. Archive supports restoring messages to Inbox. Saved updates are serialized to prevent refresh and message actions from overwriting each other.

Faster next time: a mail-list starter with bidirectional swipes, bundled seed data, and guidance for accessibility previews would remove several lookups. A compiler cache inside the app workspace would avoid the initial sandbox failure.

Validation completed:
- `bun exact.mjs verify` was run during development for behavior and layout feedback.
- Final `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: **clean**, eight tests passed, no light/dark layout findings, and no overflow on ten screens at the largest accessibility text size.
- Real-touch simulator checks confirmed pull to refresh adds exactly two messages and native swipe actions appear in both directions.
- `.exact/data-check.ts` checks preservation of all supplied content, serialized concurrent updates, repeated refreshes, unique IDs, sorting, and a device clock earlier than the seed timestamps.
- Final `bun exact.mjs ios --run` built, installed, and launched `dev.bakeoff.inbox.b4b4418` on the specified simulator.

No requested iPhone feature remains unfinished. Exact’s scrolling check found nothing for the app to fix; it reported a toolkit-level status-bar scroll-to-top timing difference and noted that machine load prevented judging frame timing.
