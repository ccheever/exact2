surfaces: web, ios, windows
agent: Claude Code (Claude Opus 5.5)
task: the composer and the chat screen's chrome against Discord's clients on web, iPhone and Windows

### Rough

- Adding a `:hover` colour or background rule anywhere in `app.contract` made the compiler
  refuse `transcript.contract`'s `time … font=face` with `[lower-css-shorthand]`, a file
  the change did not touch. The emoji button dims with `opacity` on hover instead until
  the compiler is fixed.
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

### Lean in

- `symbol:emoji` landed in the symbol table in one row; the composer's emoji button is now a
  face, as on Discord, on every host.
- A `RoundButton` component made Add, Cancel reply and Cancel edit the same filled circle,
  with a larger reach on touch than its drawn size.

### Needed

- An icon-only filled button on iOS is a pill, not a circle (Send).

### Checkpoints

- A reply preview cut at 100 characters ends with `…`.
- Reply, Edit and the other actions that focus the composer put the caret at the draft's end
  (`focusDraft`); before, iOS left it at the start.
- web contract run: 75 of 75.
