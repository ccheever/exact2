# Inbox — build notes

Finished and installed as `dev.bakeoff.inbox.b0d7e78` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using `bun exact.mjs ios`.

## Delivered

All specified features work: newest-first inbox, unread indicators and count, two-line previews, native swipes to archive and mark read/unread, flagging, All/Unread/Flagged filters, message reader, Archive with restoration, and pull-to-refresh adding two local messages. Read, flagged, archived and newly added messages persist in an atomically written local file. Search is also included.

All 320 supplied messages ship with the app. `assets/mail.json` is byte-for-byte identical to `data/mail.json`; `seed.ts` embeds the same records for the data module.

## Validation

- `bun exact.mjs verify`: clean; six behavior tests pass, including relaunch checks, light/dark layouts and largest-text layouts.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: clean; six tests pass, light/dark layouts pass, and eight screens fit at the largest accessibility text size.
- Physical simulator touches verified full-swipe archive, the opposite swipe marking unread, and opening marking read again.
- A physical pull-to-refresh produced exactly 322 messages and 111 unread from the original 320 messages and 109 unread.
- The native scrolling check found nothing for the app to fix. Machine load prevented meaningful timing judgment; the toolkit separately reported its own rubber-band and status-bar scrolling differences.

## What was hard / looked up

- Exact's guide and vocabulary supplied navigation, grants, native swipe actions, refresh handling, clock access and icon token syntax.
- Creating a date formatter for each message exceeded the native JavaScript turn budget on a cold load. Reusing formatters fixed it.
- A fixed filter strip could overlap the list after navigation. The final native toolbar filter menu avoids this and keeps the unread count readable.
- Physical gestures must start inside visible content. The list's coordinate origin includes navigation insets; dragging a visible row was reliable.
- Starting the native prebuild before assets were finalized caused a stale asset receipt and one rebuild.

## Assumptions and remaining work

The supplied records contain previews but no separate message body, so the reader shows all supplied message text. Previews expand at accessibility text sizes. Unread counts describe the current mailbox; archived unread messages do not inflate the inbox count. No requested feature remains unfinished.

## What would make this faster

A complete mail starter combining both swipe directions, secondary swipe actions, local seed data and pull-to-refresh would help. Examples showing filter placement with native collapsing navigation bars, and a note to reuse expensive formatters under the native turn budget, would also save time.
