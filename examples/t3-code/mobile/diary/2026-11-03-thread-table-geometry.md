surfaces: ios
agent: Codex gpt-6.1-sol
task: keep transcript Markdown tables stable while they scroll

### Rough

Transcript tables used content-sized grid columns with a 60-point minimum and
320-point maximum. Long filenames and wide values could therefore resize one
column and move the neighboring columns while the outer transcript scrolled.

### Lean in

The thread table now follows the pinned mobile table geometry used by file
previews: each column is a 160-point border-box, the table width is derived from
the column count, and the table owns horizontal overflow. Cell text breaks long
words inside its fixed column. This keeps wide tables scrollable without
changing surrounding prose or the stable block identities used by streaming.

### Needed

A fresh iPhone Duo table drive is still needed to compare table pixels and
repeated horizontal pans with the reference. The current normal build installs
and opens on the Duo, but no paired server transcript was available to reach a
table. This source unit does not claim native frame pacing or selection parity.

### Checkpoints

- Focused Markdown and thread tests pass: 87 tests, 490 assertions.
- Full mobile suite passes: 4,291 tests, 2 intentional native skips,
  37,928 assertions.
- Contract generation and strict mobile TypeScript pass. TypeScript uses the
  repository's ES2024 library target because the existing app uses
  `Promise.withResolvers`.
- Normal iOS Simulator no-install build passes and the signed app installs and
  opens on the local iPhone Duo. A clean Home screenshot is retained under the
  ignored `target/t3-table-geometry/` evidence directory; table scrolling was
  not reachable without a paired server transcript.
