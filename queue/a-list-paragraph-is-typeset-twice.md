**A list paragraph is typeset twice** (2026-09-19): in black to be measured, then
in its colours to be painted, because `TextShapeKey` carries the paint; only the
line breaks are handed over. 0.7 ms at the median and 3 ms at worst, since
2026-09-19 a fill unit of its own. A shape whose colour comes from the context at
draw time would serve both, for a paragraph of one colour at least.

*Filed under “Next, in order (2026-08-29)”.*
