surfaces: windows, linux
agent: Claude Code (Claude Opus 5.5)
task: the channel list beside the chat on Windows, against the web

### Rough

- Clicking a channel drew a blue rectangle around its row. WinUI and Chrome draw the focus
  visual only after keyboard input (LLP 1115.004 D9.2); the painted presenter ringed a
  mouse's focus. Fixed in the presenter to Chrome's `:focus-visible` rules. Three review
  rounds left edge cases that come from tracking the last input per node rather than per
  document (QUEUE.md).
- Beside the chat, "#announcements" read "announceme…" with its badge 24 px short of the
  row's end: the split left the row's chevron unpainted but kept its box. It now takes no
  room, as the web's `display: none` has it.
- A photo shrunk to its box stair-steps on Windows where Chrome is smooth: the painter
  samples with plain bilinear filtering. In progress.
- The private channel's lock overlaps its hash at 1x; Discord cuts the hash under the lock.
  Left as is: it reads at 2x.

### Lean in

- The driver's `layout <testId>` showed the row's padding and the chevron's box at once, and
  `screenshot` on the nucbox made the web and Windows side by side a matter of one drive each.

### Needed

- nothing new.

### Checkpoints

- Windows, 1100×760: clicking #help fills its row with no ring, and the switcher's field
  keeps its own. Every channel name shows whole with its badge at the row's end.
