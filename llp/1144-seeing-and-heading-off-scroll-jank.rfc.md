# LLP 1144: Seeing and heading off scroll jank

**Type:** RFC
**Status:** Draft r3, 2026-10-26. r1 was reviewed blind by GPT-6 Astra (max), Claude Fable 5.1 and Grok 4.7 (xhigh) (`llp/reviews/rfc-2026-10-26-1144-r1.{astra,fable,grok}.md`), r2 by Astra and Fable (`…-r2.{astra,fable}.md`; Grok's run returned only its opening line, twice); Muse was not available. Changes are in §R.
**Systems:** the frame sampler (`host/apple/Sources/ExactKit/FrameSampler.swift`) and the scroll pump; `HostPhases`; the touch runner (`host/apple/touches.mjs`); `perf`'s rendering (`scripts/agent-inspect.mjs`); the runner's per-site counts and its journal; `apps/excord`
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-26
**Related:** LLP 1142 (late-frame phases, `perf` handler counts, alert lines); LLP 1079 (perf, traces); LLP 1080.000 (real touches); `apps/excord/llp/0001-excord.rfc.md` ("no stall and no jump"; no frame over budget during a fling)

## R. Review, and what r2 changed

| Change | Raised by | Where |
|---|---|---|
| Jank is not only a late frame. A fling's report also says where the list jumped or stalled and where rows showed blank, from the scroller's offset and its unfilled rows recorded with each frame. | Fable | D1 |
| The sampler stops 500 ms after the last batch, so a list already built glides unrecorded. A fling holds its segment open from before the first touch to the end of deceleration. | Grok | D1 |
| A plain launch cannot reach Excord's transcript. The benchmark drives the app to its screen over the agent's socket under platform timing, and never sends `clock`, so the frames stay the wall's. | Grok | D1 |
| A late frame is a miss or an overrun, not a miss alone. | Astra | D1 |
| The two `perf` reads give the window's work, not a frame's. The report gives the fling's totals beside its late frames. | Astra | D1, D3 |
| A simulator's timing is no gate. The report fails on nothing by default; what is stable on a simulator is the work per fling, which it reports for comparison between commits. | Fable | D1 |
| macOS, where the jank was found, gets the same report around a fling a person makes. | Astra, Fable | D1 |
| The compile-time growth test would not have caught Excord's DM list or member list. It is replaced by a notice the runner gives when an unvirtualized repeat in a scroller passes a row count. | Fable, Astra, Grok | D4 |
| The analyzer warning on scroll handlers is dropped. The runner already drops unchanged values, so an unconditional assignment is not an unconditional update. What a handler costs is measured instead (D3). | Astra, Fable | — |
| Whether `scroll=` should leave the paint path is a platform decision, not an authoring warning. | Fable | Open question 1 |
| `fling.mjs` is a new script, which RULES asks a human to approve. Charlie asked for this work (2026-10-26, "do both"); the RFC names it rather than claim it adds nothing. | Fable | D1 |

Second round (r2 → r3):

| Change | Raised by | Where |
|---|---|---|
| Every driver input ended in `clock land`, which took the clock under platform timing and stopped the sampler. Under platform timing, before any explicit `clock`, landing now lands the `then`s at the wall's now and leaves the clock the wall's, so `perf frames` works in every platform-timing drive. | Astra, Fable | D1 |
| On macOS, agent mode applied every wheel at once, so AppKit's momentum and rubber band never ran under the agent, whatever the timing. That is now keyed on frozen timing: under platform timing AppKit scrolls, as for a person. | Fable | D1 |
| An offset step is not always a visible jump: an anchor correction or a rubber band moves it legitimately. Motion is recorded, read where the turn committed, and stays diagnostic until checked against known jumps, stalls and bounces. | Astra, Fable | D1 |
| Work counts on a real fling depend on timing too: the scroll pump sizes its slices from measured cost. They are reported as measurements, with their spread across repeated runs, not as a ratchet. | Astra | D1 |
| The D3 line is a count with no advice. | Fable | D3 |
| D4 fires on any `for` of 200 rows or more outside a virtualized list, scroller or not: building every row is the cost either way. | Fable | D4 |

## 1. The problem

Excord scrolls with visible jank, and nothing an agent runs would have said so. What
was found was found by a person, mostly on macOS, and fixed one cause at a time:
hover hit-testing on every row a fling carried under a still pointer (`4d404187a7`),
a rubber band fighting AppKit's elastic animation (`068772f00a`), a DM list that
built 3,000 rows (`268396d49f`), and member rows keyed by position (`f3af880a77`).
Excord's own bar names two things: no stall and no jump, and no frame over budget
during a fling. Neither has been measured on any host.

An agent cannot measure them today:

- Under the agent's clock, `perf frames` answers `virtual`. Under `--touch
  platform`, every input takes the clock back.
- The frame sampler records only while batches arrive. It stops 500 ms after the
  last one, so a list that was built up front glides unrecorded.
- No frame record says where the scroller was, so a jump or a stall cannot be seen.

## 2. Decisions

### D1. A fling, measured

`bun scripts/fling.mjs <app> [--to "<op>" …] [--target <scroller>] [--flings N]`
on an iOS simulator:

1. It launches the app on the agent's socket with `--timing platform` and
   `--touch platform`, and runs the `--to` ops (`tap`, `type`, `wait`) to reach the
   screen. Under platform timing an input's landing no longer takes the clock, so
   the runner and the sampler stay on the wall's.
2. It holds a frame segment open (`perf frames hold`, a form of `perf`) and reads
   the runner's per-site counts.
3. It plays N real flings on the target through the touch runner, alternating
   direction. Each fling is one drag with no resting samples before the lift, so
   UIKit decelerates as it does after a finger. The flings run until the scroller
   is at rest.
4. It reads the frames and the counts again, releases the hold, and prints:
   - **Frames:** presented, late (missed or overrun), p50/p95/p99, and the late
     frames' phases summed, largest first.
   - **Motion:** each frame record carries the target scroller's offset as the
     turn committed it, the anchor correction applied that turn, whether UIKit was
     tracking, decelerating or bouncing, and how many of its rows in the port were
     not yet filled. The report lists offset steps an anchor or a bounce does not
     explain, stalls while the fling still had velocity, and frames with blank rows.
     These stay diagnostic until they are checked against known jumps, stalls and
     bounces.
   - **Work:** rows created, and handler dispatches and commits. They depend on
     timing too (the scroll pump sizes its slices from measured cost), so a report
     from repeated runs gives their spread.

It fails on nothing by default. `--fail-on late` turns late frames into a non-zero
exit for an app that wants it. The simulator's frames are
the Mac's, so a frame budget is the device run's (LLP 1080.000 stage 4).

The touch runner turns on the simulator's accessibility keys, which load UIKit's
accessibility runtime into an app launched after them. The script launches the app
first, and reports whether the runtime was loaded.

`fling.mjs macos <app> [--to …]` gives the same report around flings a person makes
on the trackpad. It holds the segment, waits for Enter, and reads. AppKit ignores
momentum events an app posts (LLP 1142 §12), so there is no synthetic fling on the
Mac.

### D2. An `image` phase

`HostPhases` gains `image`: the main thread's time delivering decoded pictures to
their layers (`RasterLoader.reconcile`, unparking) and drawing animated frames
(`AnimatedRasters`). Today that time sits in `unattributed`. Core Animation's upload
of new contents happens at its commit, outside every phase, and the reader says so.

### D3. `perf … during` says what a scroll cost

`perf <target> during "<op>" …` already reports per-site counts and LLP 1142's
handler counts for the window. When a continuous handler (`scroll`, `pointermove`,
`wheel`, `drag`) changed the tree on nearly every one of a fling's worth of events,
its line says so: each event changed the tree, and each was a runner turn the main
thread waited on. The counts are the window's, not any one frame's.

### D4. A long list built whole, said by the runner

In a development build, when a `for` outside a virtualized `list` repeats 200 rows
or more, the runner journals a line once per `for` and session. It reaches `logs`,
and the unified log (LLP 1143's open question 1):

```
long-list: a `for` under site 40 repeats 312 rows and builds every one; `list virtualized=true` builds the rows that show (`perf` names the site's source)
```

The runner already counts live instances per plan site, so the notice guesses
nothing about the data. It would have named the 3,000-row DM list on its first
launch.

### D5. Excord, measured and fixed

Excord is the first app measured:

- its transcript flung on the iOS simulator;
- its transcript, DM list and member list on macOS, flung by hand.

The reports go into `apps/excord/diary/`. What they find is fixed in Excord or in
the host.

## 3. Not proposed

- **A compile-time warning on scroll handlers.** The runner compares values before
  it marks what reads them changed, so an unconditional assignment updates the view
  only when its value changes. D3 measures what a handler costs.
- **`perf frames live` on Apple under the agent's clock.** D1 measures without that
  clock.
- **A stack sample on a late frame** (DEFERRED: no flame chart).

## 4. Plan

D2, D3 and D4 are built. D1's prerequisites are built: landing without taking the
clock, the frame segment's hold, and AppKit's own scrolling under platform timing.
Then the script, then its motion record, then D5. Each commit
runs the six checks; D4 touches `runner/`, so `contract-difftest quick` runs too.

## Open questions

1. **`scroll=` on the paint path.** Recommended (Fable): deliver `scroll` after the
   frame, coalesced, by default, and keep synchronous delivery for a handler that
   asks for it. On iOS, every scroll event with a `scroll=`
   handler runs that handler synchronously before the frame paints
   (`NodeViewIOS.swift`'s `sendScrollEvent`, through `Owner.shared.sync`). Main
   waits for the runner to dispatch, commit and lay out, up to 120 times a second.
   A hand-built UIKit app does not run app code on every frame's paint path, and
   DEFERRED routes scroll-linked presentation to engine-evaluated timelines.
   Delivering `scroll` after the frame (coalesced, applied on the next one) would
   take that cost off every fling. The price: authored geometry that follows the
   scroll would lag one frame. Charlie decides; D1 measures the cost either way.
2. If the touch runner's drag does not keep UIKit's velocity at 3,000+ pt/s, the
   fallback is an in-app synthetic fling through a development hatch (the Bluesky
   port's `PerfHatch`), which does not exercise UIKit's tracking.
