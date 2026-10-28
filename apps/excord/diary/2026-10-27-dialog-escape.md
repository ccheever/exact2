surfaces: web
agent: Claude Code (Claude Opus 5.5)
task: Escape over a channel: the member sheet, a member's card and the other dialogs

### Rough

- On the web, Escape did not close the member sheet or a member's card while a channel was
  open. The window's `key` handler takes Escape on the channel (cancel an edit or a reply,
  else jump to the present) and calls `preventDefault()`, and a key pressed in a dialog
  bubbles to it, so the browser never made the dialog's close request. The emoji sheet, the
  quick switcher and the confirmations had the same gap. Each now hands its keys to
  `dialogKey`, which stops them there.

### Lean in

- `agent.mjs`'s `back` presses a real Escape when a dialog is open and says when the dialog
  stayed open, which is how this showed.

### Needed

- A `KeyboardEvent` does not say where the key was pressed, so a window's shortcut cannot
  leave a dialog's keys alone by itself; every dialog has to stop them.

### Checkpoints

- web: Back over the member sheet closes it, and over a member's card closes the card and
  leaves #general open (`app.test.contract`).
