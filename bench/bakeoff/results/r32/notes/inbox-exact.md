# Inbox build notes

Finished and installed as **Inbox**, bundle ID `dev.bakeoff.inbox.b2b9110`, on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## Delivered

- All 320 supplied messages ship through the imported `data/mail.json` module, sorted newest first.
- Native mail rows show sender, subject, preview, date, unread indicator and flag indicator.
- Native leading swipe toggles read/unread; trailing actions archive and flag/unflag. Full swipes work.
- Archive is a separate navigation screen, with message reading and restoration to Inbox.
- Inbox and Archive support unread and flagged filters. Inbox shows its unread count.
- Opening a message marks it read. The reader also offers flag, read/unread and archive/restore controls.
- Pull to refresh adds exactly two locally composed unread messages. Repeated refreshes generate unique IDs and keep new messages above older batches, even if the clock moves backward.
- Atomic file writes preserve messages and read, flagged and archived states across launches. Queued mutations prevent rapid actions from overwriting one another.

## Verification

`bun exact.mjs verify ios` passed all **6 tests**, with no layout findings in light or dark mode. All eight checked screens fit at the largest accessibility text size (53-point body text). The native scrolling check reported nothing for the app to fix; it noted a busy host machine and two toolkit-level physics differences.

Additional real simulator touches verified both full-swipe directions and pull to refresh. The pull changed the collection from 320 to 322 messages and the unread count from 109 to 111. The final `bun exact.mjs ios` completed and installed the app on the requested simulator.

## What was hard / looked up

- Looked up Exact's long-form swipe wiring, native navigation bars, refresh handlers, file grants, icon tokens, date formatting and simulator gesture syntax.
- Exact documentation commands need access to the toolkit's build cache outside this directory. The initial sandboxed command failed on the Cargo build lock; approved Exact commands resolved that.
- Empty states must replace the list rather than appear as a list row, and a section footer must stay at the section's end. Compiler feedback made those corrections straightforward.
- The initial web verification could not directly activate a hidden leading swipe action and reported web-specific menu/layout findings. Final acceptance used the requested iPhone platform, whose complete verification passed. Web parity was not a delivery target.
- Native swipe actions can be activated directly by the agent for functional tests, but actual touch verification needs a full row swipe. Pull gestures must start below the native navigation bar.

## Decisions / assumptions

The supplied dataset contains previews rather than separate message bodies, so the reader displays the complete supplied preview as the message text. Refresh messages are fictional local messages from Maya Chen and Alex Morgan. Previews naturally occupy up to two lines at standard size and expand at accessibility sizes so text remains readable.

Nothing required for the iPhone specification remains unfinished.

## What would have made this faster

A mail-oriented starter with both swipe directions, bundled initial data and persistent per-message state would save setup time. A web driver that reliably reveals hidden leading actions would make cross-platform verification more useful. The native verifier's combined functional, dark-mode, large-text and scrolling report was particularly helpful.
