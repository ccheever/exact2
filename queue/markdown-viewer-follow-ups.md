**Markdown viewer follow-ups** (LLP 1033, macOS milestone 2026-09-05):
iOS selection and link gestures; iOS/web file-opening adapters; heading anchors,
tables, and syntax highlighting. macOS drag selection across paragraphs and copy
now use CoreText's existing lines. Keyboard selection extension and bidi selection
geometry still need dedicated fixtures. Variable-height windowing and scroll
anchoring now run; startup against Legend is still a tie, and scrolling is
ahead on one 60 Hz machine (see `apps/markdown/README.md` and the 120 Hz item
above). At a 420-point
window width, the open folder pane leaves the text cramped and clips header
controls; the reader needs a compact layout at that width.
During verification the macOS timer-step smoke once read width 50 at t=1250
instead of 75, then passed on repeat; investigate the intermittent clock fixture.

*Filed under “Next, in order (2026-08-29)”, item 4.*
