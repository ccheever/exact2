surfaces: windows, web
agent: Claude Code (Claude Opus 5.5)
task: the composer's icons on Windows against the web and iPhone

### Rough

- The emoji button's face was the accent on Windows and grey on the web and iPhone. The
  Linux presenter, which draws Windows and Android, drew a symbol in its `tint-color`,
  whose initial value is the accent, where LLP 1081 stage 2 says a symbol takes its
  `color` on every host. Fixed in the presenter, keeping the tints a grouped list's sheet
  gives its leading symbol and chevron (reviews by Astra and Grok both caught that case).
- Eight Linux presenter tests that compile Shelf fail on main (QUEUE.md), unrelated to
  Excord; the gate does not run them.

### Lean in

- `agent.mjs windows … layout <testId>` lists every style row with where it came from
  (authored, inherited, initial), which pointed at the tint's initial value at once.

### Needed

- navigation: partial on Windows; the list beside the chat and author-laid-out dialogs are
  in progress.

### Checkpoints

- Windows: the emoji face is grey under the button's authored colour; the message menu opens
  on right-click with its symbols.
- web: hover tints a message and shows its action bar; a long draft grows the field with
  its buttons held at the bottom.
