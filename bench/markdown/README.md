# Markdown reader benchmarks (macOS)

Black-box measurements of `apps/markdown` on macOS, the ones behind LLP 1044 §3:
scrolling heavy documents through the window server, and launch to first content.
They see only what reaches the screen, so any other reader can run the same trials
alongside (LLP 1044 compares Legend Markdown).

```sh
bun bench/markdown/run.mjs scroll --quick                 # 6 scenarios, a few minutes
bun bench/markdown/run.mjs scroll                         # 20 scenarios × 3, ~15 min per app
bun bench/markdown/run.mjs launch --reps 5                # launch to first content
bun bench/markdown/run.mjs scroll --base ../exact2-main   # this checkout against another
bun bench/markdown/run.mjs scroll --legend "/path/Legend Markdown.app" --record
```

Every run builds this checkout's Markdown app first (`host/apple/build.mjs
markdown-apple`; `--no-exact` skips it). To check a change, give the checkout without it
as `--base` (`git worktree add ../exact2-main origin/main`); it is built there and
labelled `base`. `--app label=<executable or .app>` adds any other app that opens a
Markdown file named on its command line; `--legend <path>` (or `EXACT_BENCH_LEGEND`)
is `--app legend=<path>`. Apps alternate trial by trial, so slow drift in the machine
lands on all of them.

The terminal needs **Accessibility** and **Screen Recording** (System Settings →
Privacy & Security). A scroll trial raises the app, puts the pointer over its window,
posts input at the HID tap and then puts the pointer and the frontmost app back; keep
your hands off the keyboard and mouse while it runs. A trial waits for 3 s without
input (`--idle <s>`), and one a person touched is discarded. One trial runs at a time
per machine (`/tmp/exact-bench-screen.lock`).

## What is measured

`scrollbench.swift` launches the app with a document, places its window at
900×700 through Accessibility, and records it with ScreenCaptureKit. It counts only
frames whose content changed, estimates each frame's scroll shift from its ink
profile, and finds the longest band with no ink (a blank the reader showed while
scrolling).

| Column | Meaning |
|---|---|
| fps | changed frames presented per second while input ran |
| hitch/s | ms per second by which frame gaps exceeded 1.5 refresh intervals |
| drop, p99gap, maxgap | frames missed; 99th-percentile and longest gap between changed frames |
| blank | frames with a band over 250 px without ink |
| travel | total content shift in px (input that went nowhere shows here) |
| jumps | the scroller set through Accessibility; ms to the first change and to content at the destination; frames that were blank on the way |

Scenarios: `slow`, `trackpad`, `fast` are trackpad gestures (phased, with momentum)
at 900, 3,600 and 12,000 pt/s over the corpus; `wheel` is unphased; `reverse` changes
direction every 2 s; `resize` sweeps the width 900↔640 at 60 steps a second; `jump`
sets the scroller five times. For a generated document `<doc>`: `p-<doc>` scrolls at
3,600 pt/s, `f-<doc>` flings at 12,000, `j-<doc>` jumps, `r-<doc>` resizes; `o-<doc>`
opens a small pathological one. `--scenarios a,b` picks; a wrong name lists them all.

`launchbench.swift` captures the screen region where the window opens (`--region
x,y,w,h`, default 120,80,900,700) with every window that existed before launch
excluded. It reports the first frame that differs from the empty desktop (a window)
and the first frame matching the app's own settled content (content). The first
launch per app after a build is discarded as cold.

## The documents

`docs.mjs` writes them into `target/bench/markdown/docs` once. The corpus is the 40
largest `llp/*.md` at 9dbaded0, the commit LLP 1044 measured (2,556,440 bytes), so it
does not move as the LLPs do. The generated documents (1 MiB paragraph, code block,
pipe table, dense inline markup, mixed scripts, emoji, combining marks, wide tables,
malformed brackets and autolinks) reproduce Python's seeded generator, and are
byte-identical to LLP 1044's.

## Over time

`--record` appends one line to `history.jsonl`: commit (and whether the tree was
dirty), machine, macOS, refresh interval, load average, and each app's medians per
scenario. Commit the line with the change it measures. Compare lines only from the
same machine. A loaded machine (`load`) makes numbers worse and noisier: record only
on an otherwise idle one.

All numbers are presented frames at the display's rate (60 Hz in LLP 1044); input
comes from posted events, not a finger.
