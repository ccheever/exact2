**Markdown scrolling with real input** (2026-09-19): the separate upstream
`4d91f9e` build measured ahead of Legend with in-process input on an M4 Pro
at 60 Hz and an M5 Max Retina panel at 120 Hz (`apps/markdown/README.md`:
inputs later than 8.33 ms, of 2,400 — about 2 against 84 at 60 Hz; 1, 1, 1
against 1, 6, 4 at 120 Hz). The 120 Hz runs had a load average of 30–67;
an Instruments trace under that load made app updates about four times as
long. The later integrated build still has no demonstrated Hitches advantage.
Owed: a quiet-machine comparison with real wheel and trackpad input, which
requires Accessibility permission for the event sender; responsive scrolling
for a contained list, which the in-process probe cannot drive; and a repeat
of first-content and memory measurements on the integrated build.

*Filed under “Next, in order (2026-08-29)”.*
