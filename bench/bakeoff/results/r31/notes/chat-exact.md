# Chat (Exact): notes

Built with Exact. `Chat` (`dev.bakeoff.chat.bc7fdae`) is installed on simulator
EF198926-1958-489D-A7A4-9196E454DD3A. The app is in three files: `app.contract` (the view),
`app.ts` (the data) and `app.test.contract` (7 tests, passing on web and iOS).

## How the spec maps
1. The 520 shipped messages are imported from `data/messages.json`. Mine are on the right in the accent
   colour, Sam's on the left in `-exact-fill`, all in a virtualized list. A day heading
   ("Jul 4, 2026 at 3:07 AM", "Today at 2:15 PM") comes before the first message of each
   local day. The data module passes each message's `prevAt`, and the view compares the day keys
   in the device's zone. The list opens at the end (`scroll-start="end"`).
2. Send is disabled while the text is blank or only spaces. A send clears the composer, keeps the keyboard
   up and scrolls to the new message.
3. `interactive-widget="resizes-content"` keeps the composer above the keyboard. The list
   follows its end, so the newest messages stay in view (checked on the simulator).
4. Each send queues a due time 2 s ahead. A task reads the queue every 100 ms and posts Sam's
   reply. A "Sam is typing…" bubble shows above the composer while any reply is due. The reply is
   "👍" or one of several short canned lines, picked by a hash of your text.
5. A long press opens a Copy / Delete menu (UIMenu). Copy uses `copyText`. Delete takes the
   message out and the deletion persists. A shipped message's deletion is stored as its id.
6. The list is virtualized. `verify ios` played real touches on it and judged the
   scrolling "like UIKit". Once you are more than about 1.5 screens up, a "Jump to Newest" button
   appears above the composer and scrolls smoothly back to the end.
7. Sent messages, Sam's replies and deletions go to `app:/data/chat.json`. Writes run one
   at a time in the data module, so a send and a reply can't overwrite each other.
   Tests cover a relaunch.

## What was hard / guessed
- The Contract syntax took a few build-error round trips. It has no `xs[i]` (use `xs.at(i)`) and
  no `last()`. A block `match` has no `=>`. A virtualized list allows only one `for` child, so the
  typing indicator sits outside the list, above the composer. `-exact-secondary-system-fill`
  isn't a colour role. The compiler's messages named each fix.
- Tests that scroll into history need `tap "transcript" into "<key>"` and a `clock settle`
  before the scroll-driven button shows.
- I could not see the native long-press UIMenu in a screenshot. The agent paints its own
  stand-in for the popover, and that happened with `--touch platform` as well. I relied on
  the documented mapping of `contextPopover` to a UIMenu.
- `verify ios` still lists "overlap" findings: bubbles under the Jump button and composer. The
  framework scrolls the list under a blurred bottom bar, as iOS 26 Messages does, so content
  sitting behind the composer counts as overlap. A direct `check` at the largest text size reports
  nothing, and the screenshots look right. I left the layout as it is rather than fight this.
- Dropped Linux (`update --without linux`). The web build is kept only for fast tests.

## Not finished / known gaps
- A reply still pending when the app quits is lost. The timer runs on `performanceNow()`, which
  restarts at each launch, and pending replies aren't persisted.
- Delete removes the message straight away, with no confirmation and no undo.

## What would have made it faster
- A chat pattern (`pattern chat`) that already has day headings, a typing indicator and the
  long-press menu. The `chat-thread` recipe was a good start, and `apps/messages` was too custom
  to copy from.
- A `verify` setting that knows about scroll-under bars, so content behind a translucent
  composer isn't reported as overlap.
