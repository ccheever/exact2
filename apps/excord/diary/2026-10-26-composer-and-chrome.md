surfaces: web, ios, windows
agent: Claude Code (Claude Opus 5.5)
task: the composer and the chat screen's chrome against Discord's clients on web, iPhone and Windows

### Rough

- A build refused with `[lower-css-shorthand]` on `transcript.contract`'s `time … font=face`, a
  line the change did not touch. The real errors were in the edit, an unquoted `-exact-label`
  and a `color` under `:hover` (which takes background, border, opacity and shadow), but the
  lint that runs after a type error read the unexpanded `face` as a bad literal. Fixed in the
  compiler; the emoji button dims with `opacity` and brightens on hover.
- `app.contract` reached the 1,500-line cap; the message menu moved to `controls.contract`.
- The contract tests named the fixture at port 4790, which this checkout's fixture no longer
  uses (LLP 1142 D13), and `fail fetch` and `request` with a `/api/…` path went to the
  build's own origin, not its backend. Ten web tests failed for it; `scripts/agent-test.mjs`
  now resolves both under the backend the build selected.
- On iPhone the image viewer's close button sits under the status bar: a box outside every
  route cannot read the safe-area insets (LLP 1123 D9, QUEUE.md). The viewer's status bar is
  now light over its black backdrop.
- On Windows the switcher, profile, member list and emoji pickers do nothing, because the
  Linux presenter (which draws Windows and Android) presents only alert dialogs and menu
  popovers; and at 1100 px the channel list does not stand beside the chat. Both are
  presenter work, not Excord's.
- An icon-only filled button was a pill on iOS and a rounded square on the web, where Messages
  and Discord's mobile app draw Send as a circle. Both hosts now draw a symbol alone as a circle
  (iOS keeps UIKit's height and narrows the sides; the web's radius is a capsule unless authored).

### Lean in

- `symbol:emoji` landed in the symbol table in one row; the composer's emoji button is now a
  face, as on Discord, on every host.
- A `RoundButton` component made Add, Cancel reply and Cancel edit the same filled circle,
  with a larger reach on touch than its drawn size.

### Needed

- navigation: partial; Windows has no list beside the chat when wide, and author-laid-out dialogs
  and popovers do not open on Windows or Android.
- safe areas: missing for a box outside every route on iPhone (LLP 1123 D9).

### Checkpoints

- A reply preview cut at 100 characters ends with `…`.
- Reply, Edit and the other actions that focus the composer put the caret at the draft's end
  (`focusDraft`); before, iOS left it at the start.
- web contract run: 75 of 75.
- iOS contract run: 72 of 75; the same three fail without these changes (QUEUE.md).
