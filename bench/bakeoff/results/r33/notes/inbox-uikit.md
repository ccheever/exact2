# Inbox build notes

Finished and installed as `dev.bakeoff.inbox.b0fcfa5` (Inbox) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left running on its Inbox screen, in light mode with standard text size. No other simulator was used.

## What shipped

Programmatic UIKit app and scene delegates, native navigation and Inbox/Archive tabs, newest-first mail, two-line previews, unread dots and count, swipe actions in both directions, flagging, unread/flagged filters, a reader that marks opened mail read, and pull-to-refresh that creates exactly two local messages. Archive supports restoring messages to Inbox. Search, context menus, VoiceOver actions, empty states, a custom icon, dark appearance, and Dynamic Type are included.

`Resources/mail.json` is an exact copy of `data/mail.json`, with all 320 messages. Changes and locally generated messages are saved atomically to the app's Documents directory. Save failures roll back the attempted change and present an error. A corrupt saved file falls back to the bundled inbox with a visible explanation.

## What was hard

- Making sender/date layouts adapt to the largest accessibility text size without truncating the sender or subject. At accessibility sizes the date moves below the sender, and rows grow automatically.
- Getting nested stack views to keep dates and flags together at the trailing edge at standard sizes.
- Making generated timestamps round-trip exactly through ISO-8601 persistence. A test caught fractional seconds being dropped; generated dates now use whole seconds.
- Keeping search results reachable above the keyboard while retaining native search presentation and dismissal.
- CoreSimulator access needed sandbox escalation; the supplied build script worked once that access was available.

## Lookups and decisions

I read the supplied README, build script, and mail data. No external documentation or network service was needed. The dataset supplies a preview but no separate body, so the reader displays the complete supplied preview as the message text. Filters and search apply to the current mailbox; the unread count describes that mailbox. The Inbox tab badge excludes archived mail. Refresh preserves the current filter and creates two unread, unflagged Inbox messages with unique IDs and dates newer than existing messages.

## Verification

Built with `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run` and tested repeatedly on that simulator.

Run `./Tests/check-simulator.sh` after a build to reproduce the checks. The final run passed 35 checks at standard text size and 34 at the largest accessibility size (the horizontal date-alignment assertion applies only to standard layouts). Checks cover all bundled records, sorting, filters, read/flag/archive persistence, restoring archived messages, repeated refreshes, unique generated IDs, saved timestamp round-trips, corrupt-file recovery, failed-save rollback, reader navigation/actions, and adaptive layout. Test mutations to the shared mailbox are restored.

Screenshots and reports are in `build/checks/`. I visually reviewed light/dark lists, flagged mail, the reader, largest-text layouts, the archive empty state, and search with its keyboard. The checks invoke controller actions programmatically; physical touch/swipe gestures were not automated. Swipe presentation uses UIKit's standard contextual actions.

## Unfinished work and improvements to the toolkit

No known unfinished specification features. A simulator touch/accessibility automation command would make gesture and keyboard interaction checks faster and more thorough without needing in-app scenario hooks. A defined message-body field and reference appearance would remove the main data and design assumptions.
