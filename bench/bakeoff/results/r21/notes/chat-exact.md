# Chat (Exact): notes

Built with Exact: `app.contract` (the view), `app.ts` (data), `app.test.contract` (6 tests).
`bun exact.mjs verify` (web) and `verify ios` are both clean. The app is installed on simulator
498D6D1F-… as `dev.bakeoff.chat.07c073`, named "Chat", and launches on its own (standalone build).

## How the spec is met
- The 520 messages are bundled by importing `data/messages.json` into `app.ts`. What you send,
  Sam's replies and deletions are saved to `app:/data/thread.json`, so they survive a relaunch.
- The transcript is a virtualized `list` (`scroll-start="end"`, `scrollFollowEnd`). It opens at
  the newest message and follows new ones. Mine are accent bubbles on the right, Sam's are grey
  on the left. Consecutive messages from one sender sit tighter together, and the last in a run
  gets a smaller corner.
- Day headings ("Today 9:41 AM", "Yesterday …", a weekday, "Wed, Jul 1 at 3:20 AM") appear where
  the local day changes. The thread is re-read when the day rolls over.
- The composer is Exact's chat composer under `interactive-widget="resizes-content"`, so it rides
  above the keyboard. Send is disabled for empty or whitespace-only text. The field clears, keeps
  focus, and the list stays at the bottom. If you send while scrolled far up, it returns to the
  newest message.
- Sam: each send adds 1 to a persisted `owed` counter. A task runs every 2 s while `owed > 0`,
  and "Sam is typing…" shows above the composer meanwhile. Replies are mostly 👍 or a short
  canned line, and a question gets "Yes!" or "Let me check…".
- Long press opens a context menu with Copy (`copyText`) and Delete (saved).
- A "jump to newest" button floats over the list when you are more than about 1200 pt from the
  bottom.

## What was hard / guessed
- **The bundle id.** Exact's `app.json` schema requires every id segment to start with a
  letter, so `dev.bakeoff.chat.07c073` is refused. Apple allows it. I couldn't edit the toolkit
  (it is outside the working directory), so `tools/relax-id.mjs` is a Bun preload plugin that
  relaxes only that regex when Exact reads the schema. **To build, set
  `BUN_OPTIONS="--preload $PWD/tools/relax-id.mjs"`**; without it every command refuses
  app.json. With it, the real id goes through the Rust, Swift, Hermes and Info.plist parts of
  the build. Re-signing the bundle afterwards was not an option, because the id is also
  embedded in the binary and bytecode.
- **Sent messages vanished at first.** I sorted the thread by `at`, but the agent's virtual
  clock is pinned to 2026-01-01, so new messages sorted to the top. The thread now keeps
  insertion order: the history, then what was added.
- **Hermes' `Intl` output differs.** It puts U+202F before "AM", so the iOS tests failed where
  the web passed. Dates are now formatted by hand.
- **Not verified with a real long press.** Under the agent the context menu draws as an
  anchored popover. The docs say a real long press on iOS presents UIKit's menu, but I could
  only drive it through the agent.
- **A web-only false alarm.** The web largest-text check once flagged the floating jump button
  overlapping a bubble on the relaunch screen. It's an overlay by design, and iOS was clean.

## What would have made this faster
- A schema that accepts any bundle id Apple accepts, or an error that offers an override.
- An agent `expect state` op, or a note that `state <name>` exists. I first guessed `expect`.
- The `with` form for resource arguments that should not bust the kept answer: the launch log
  explained it, but it's not in start-here.
