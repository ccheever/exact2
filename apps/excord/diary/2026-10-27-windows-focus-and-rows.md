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
- A photo shrunk to its box stair-stepped on Windows where Chrome is smooth: the PNG decoder
  kept one source pixel per output pixel and the painter shrank with plain bilinear. Both
  now average the area they cover, and Windows's own decoders scale with WIC's high-quality
  cubic; the photo's rounded edge is unchanged.
- The line between the channel list and the chat was mid-grey (the control-edge colour);
  the web draws its separator. It now uses the separator, as LLP 1115.004 D16 says.
- Opening Excord on a channel ringed its "# general" heading, where the route had moved the
  focus. Chrome rings a script's focus before any input, so the web does too; WinUI draws
  no focus visual until a key, so Windows no longer does.
- On a desktop the composer sat 6 px above the window's edge, cramped beside Discord's
  desktop app, and its + sat 4 px left of the avatars. With a fine pointer it now has
  16 px below and beside it; touch keeps the tighter fit over the home indicator.
- Back was a chevron and More a circled ellipsis on Windows, where WinUI draws an arrow and
  three bare dots. A symbol role now takes a platform's shape where it has its own, on
  Windows and on Android (LLP 1115.001 D14); see `2026-10-27-platform-symbols.md`.
- The private channel's lock overlaps its hash at 1x; Discord cuts the hash under the lock.
  Left as is: it reads at 2x.
- On iPhone in dark mode the composer's emoji face is a filled disc, not the outline the web
  and Windows draw. That is UIKit's own: SF Symbols draws `face.smiling` filled in a dark
  image view (and AppKit does too), so a hand-built app shows the same face. A fix that
  pinned the outline was built and not landed, since the role's glyph is the platform's
  (LLP 1115).

### Lean in

- A drive pins DejaVu Sans for repeatable pixels; `EXACT_FONT="Segoe UI Variable"` in the
  drive's environment shows the window as Windows draws it.
- The driver's `layout <testId>` showed the row's padding and the chevron's box at once, and
  `screenshot` on the nucbox made the web and Windows side by side a matter of one drive each.

### Needed

- nothing new.

### Checkpoints

- Windows, 1100×760: clicking #help fills its row with no ring, and the switcher's field
  keeps its own. Every channel name shows whole with its badge at the row's end.
