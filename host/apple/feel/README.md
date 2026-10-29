# Scroll feel probe

Plays the same real touches on any iOS simulator app's list and says, metric by metric, whether its scrolling feels
like UIKit's. It does not care what built the app: an Exact app, a SwiftUI `List` and a `UICollectionView` are
probed the same way, so the native arms are the baseline for Exact. `bun exact.mjs verify ios` runs it on an app's
first screen when that screen lays out content well below its bottom (`scripts/verify.mjs`, `scrolling`).

| File | What it is |
|---|---|
| `feel.m` | the recorder: a dylib inserted at launch that watches, samples the main thread's stack, and moves the list only for the blank check (`probe.mjs` builds it into `target/feel/recorder-sim.dylib`) |
| `probe.mjs` | relaunches the app with the recorder, plays the gestures through the touch runner (`host/apple/touches.mjs`), analyzes; `play` is the gestures alone, for a drive that launched the app itself (`verify`) |
| `analyze.mjs` | trace → `feel.json` and `feel.txt`; given several output directories, a comparison table; `findings`, each tell or bad with UIKit's value and its cause for an app author |

## Running it

```sh
bun host/apple/feel/probe.mjs --udid <simulator udid> --app <bundle id> [--out DIR] [--short]
# a list behind a tab or a push: steps first (a label a person reads, or window points; wait in seconds; drag ends
# with an optional speed)
bun host/apple/feel/probe.mjs --udid U --app B --steps 'tap "All";wait 1'
# several lists: one line of steps each, each probed from a fresh launch (DIR, then DIR/list-2, …)
bun host/apple/feel/probe.mjs --udid U --app B --steps $'tap "All"\ntap "Shows";tap "Daily Brief"'
# several runs side by side
bun host/apple/feel/analyze.mjs DIR1 DIR2 DIR3
```

The app must already be installed on the booted simulator. A probe takes about 80 s (8 s when there is no list),
plus about 5 s the first time the touch runner starts on a simulator. `--out` defaults to
`target/feel/<bundle id>-<time>/`; it gets `trace.jsonl` (every recorded frame, touch and scroll-view scan),
`gestures.json`, `feel.json` and `feel.txt`. `--at x,y` moves where the gestures land (default: the middle of the
list); `--dump DIR` saves the first sampled frames and any with a blank band as PNGs, each beside a JPEG of the
window as the screen showed it, and the blank check's frames.

`tap "label"` finds what it names as the bake-off's driver does (`host/apple/elements.mjs`: label, id or
placeholder, exactly, then ignoring case, then as part of a label), waiting up to 8 s for it. With several lines of
steps, `DIR/feel.txt` has every list under its steps and `DIR/feel.json` is the first list's with `others` naming the
rest. `bench/bakeoff/round.mjs` passes a spec's `feel-steps.txt` (lines starting with `#` are comments).

If the screen the steps reach has no shown scroll view whose content is at least 1.5 × its height + 100 pt, the
result is `no-scrollable-list` (exit 0), not a failure.

## What it does

The recorder hooks `-[UIApplication sendEvent:]` to log touches and runs a display link at the screen's maximum
rate. On every frame while a finger is down or a scroll view moves, it records the moving scroll view (the innermost
one tracking, dragging or decelerating, else the one whose offset changed), its offset, content size, insets and
flags, and the main run loop's busy time since the last frame.

A list that measures rows as they show (Exact's, a chat's) moves its content under the offset when a row above the
viewport changes height, and moves its offset by as much in the same frame to keep what is shown in place: the
offset jumps and nobody sees it. So each frame also records how far a layer holding the middle of the viewport (a
row) moved in the content since the last frame (`ag`), and every motion metric reads where the content shows, the
offset less those moves. A row above that grows while the offset stays is still a jump: in a plain `UIScrollView`
whose row two above the viewport grows 60 pt every 100 ms of motion, the probe reads 18 jumps and a tracking ratio
of 0.59 with the offset left, and 0 and 1.0 with it moved by as much.

While the driver asks for it, every 4th moving frame
is rendered after Core Animation's commit and searched for uniform bands (60 pt or more of uniform scanlines where
the list has content).

A uniform band is not yet a blank: a grouped list's background, the space under its last section and a row's own
plain area are uniform at rest too. After each sampled gesture the probe asks for the blank check: the recorder
takes up to 10 of the frames with a band, spread over them, moves the list to each one's offset, and draws the
same region again 150 ms later (and 500 ms, when nothing new is drawn yet), both moved by the content's recorded
moves since the sample. Lists that measure rows as they show (Exact's, SwiftUI's) can still move their content
under a fixed offset in ways the anchor does not see, so the frame at rest is first aligned to the sampled one: the
shift whose scanlines differ least from the sample's inked ones, accepted only when they differ by under
1 (of 255) on average; a band the shift puts off screen, or a frame that aligns nowhere, is not judged. A band
counts as blank when the frame at rest has at least 12 pt of ink inside it: rows that will be drawn, not drawn yet.
The share of the judged bands that are blank stands for all of them. The list goes back to where it was, and the
check's frames are left out of every timing metric.

While it records, the recorder samples the main thread's stack about every 2 ms (a thread of its own suspends it,
reads its frame pointers and resumes it). Each sample is labelled by who (the innermost author frame: the app's
JS, which is Hermes in an Exact app; Exact's runtime, its Rust; Exact's host, its Swift and in an Exact app any
other native code; or the app's own code) and what (the innermost platform framework below it: SwiftUI, UIKit,
Core Animation, text & drawing, images, accessibility), and by the run loop's callout it runs in (timers, the main
queue, events, display links and ports, observers such as the commit). A main thread blocked on another thread is
labelled by that thread when it is running: Exact's runtime runs on `exact.owner`. Each late frame gets the labels'
share of its interval. A late frame whose timers took a quarter or more of its busy time (4 ms or more) counts as a
timer's tick; any other as the scrolling. `FEEL_STACKS=0` in the app's environment turns sampling off; with it on
and off, watchlist-exact read 141–174 and 155–162 ms/s on a loaded Mac, no difference beyond the noise.

The gestures, all real HID touches through XCTest's synthesizer:

1. a slow drag (250 pt/s) held 0.5 s before the lift;
2. flings down at 1,500, 4,000 (blanks sampled) and 6,000 pt/s, each left to stop;
3. flings up at 8,000 pt/s, 1.2 s apart, until the list is back at the top (blanks sampled); the last meets the
   top with momentum;
4. a pull 240 pt past the top, held, released;
5. a fling down at 3,000 pt/s, then a tap on the status bar (on its right: the left may hold the back-to-app
   breadcrumb, which leaves the app);
6. a fling down at 4,000 pt/s and, 200 ms after the lift, a tap in the middle of the list;
7. on a horizontal list in the list (a carousel: a shown scroll view whose content is at least 1.5 × its width + 100
   pt and no taller than it, whole in the list's viewport; the widest, met within eight slow drags down the list),
   along its middle: a slow drag (250 pt/s, held 0.5 s), flings at 1,500 and 4,000 pt/s, flings back at 6,000 pt/s
   until it is at its start, then two drags held before the lift, one mostly along it (4:1) and one mostly down the
   list (1:4), both starting on it;
8. the scroll indicator dragged to scrub (UIKit's interactive indicator): shown by a short fling, held 0.5 s, dragged
   to the far end of its track in 1 s, held 0.3 s, back in 1 s, held 0.3 s; then again with blanks sampled.

The horizontal gestures and the scrub run only while the list still shows (a tap that opened a row skips them).

`--short` is verify's script, about 15 s: the slow drag; flings down at 6,000 and 4,000 pt/s (blanks sampled in the
second); the status-bar tap from where they left the list; the pull past the top; and on a list whose content is
12,000 pt or more, one scrub, timed (on a shorter one, dragging the indicator moves the list less than twice as fast as the
6,000 pt/s fling). It skips the 1,500 pt/s fling and the flings back up, so the edge bounce is measured only when a fling
reaches the list's end, and blanks are sampled in one fling (about 45 samples, not 130). It skips the tap while
decelerating and the carousel: in bake-off rounds r24 to r26 (103 verify runs) the findings an app could fix came
from the slow drag, the flings, the status-bar tap and the pull, and those two found only Exact's own. `verify ios`
measures once per build: until the build, the plan it runs, the app's server or the probe changes, it reuses the
last measurement (`--rescroll` takes another).

## Reading `feel.txt`

Each line is a verdict (`ok`, `tell` — someone who knows iOS would notice — or `bad`), the metric and its value.

| Metric | What it is | ok |
|---|---|---|
| hitch ratio | ms by which frames were late, per s of motion (Apple's metric) | < 5 (tell to 10) |
| longest frame | the longest frame interval during motion | ≤ 34 ms (one dropped frame at 60 Hz) |
| main thread p95 | the app's main run-loop time per moving frame, the recorder's own time removed | < half a frame |
| tracking ratio | content travel ÷ finger travel during the slow drag, after touch slop | 0.97–1.03 |
| touch-to-content lag | frames by which content trails the finger (the best-fitting shift) | 0 |
| glide after a still lift | content travel after lifting a finger held still | < 5 pt |
| deceleration τ | the time constant of v(t) = v₀·e^(−t/τ) fitted to each fling's momentum (median) | 440–560 ms (UIKit `.normal`: 0.998/ms, 500 ms) |
| momentum hand-off | the fitted velocity at the lift ÷ the asked speed (median of the three flings down and the 3,000 pt/s one) | 0.8–1.25 |
| flings that drop a frame at the lift | of the flings without blank sampling, those with a frame over 1.5 periods in the three after the lift | none |
| offset jumps while decelerating | frames off the fitted curve by more than 15 % (or 150 pt/s): a stall, a frozen frame, a catch-up or the content moving back; a still frame in the two after the lift is the hand-off's; a fling that reaches the list's end is judged until it does (the bounce is the edge's) | none |
| edge bounce settle | from momentum crossing the top to resting there | 0.25–1.0 s |
| rubber band | overscroll ÷ finger travel past the top; and the time from the lift to resting | 0.3–0.65; 0.25–1.0 s |
| status-bar scroll to top | from the tap to the top | ≤ 1.0 s |
| tap while decelerating | content travel after the touch lands; `activated` if the list left the screen | < 5 pt |
| blank samples | the share of sampled moving frames with a band blank where the same frame at rest has rows (the blank check) | 0 |
| scrub: hitch ratio, longest frame | the same two over the frames of the timed scrub, kept out of the list's own | as above |
| scrub: blank samples | the blank check over the sampled scrub's frames, the holds at each end included | 0 |
| carousel: hitch ratio … edge bounce settle | the metrics above along x, on the carousel's frames (its own τ, hand-off and bounce) | as above |
| carousel: a mostly-horizontal drag | carousel travel ÷ finger travel along it, and how far the list moved | ≥ 0.8 and < 5 pt |
| carousel: a mostly-vertical drag | list travel ÷ finger travel down it, and how far the carousel moved | ≥ 0.8 and < 5 pt |

The scrub and carousel lines appear only when the probe played them. A scrub that crossed under 0.3 of the list
(an indicator UIKit does not make interactive, or none shown) reads `took: false` in `feel.json` and is not judged.

Under the verdicts, `where the late frames' main-thread time went` gives each gesture with late frames: how many
(how many on a timer's tick), how late in all, the three labels that took most of their intervals (each with the
framework it was in) and the callouts that time ran in; and, on its first line, the hitch ratio split into timer
ticks and scrolling. A finding on the hitch ratio or the longest frame names the two labels that took most and
whose each is: the app's data module (a live list updating on a tick), Exact's own, the app's own code, or the
platform's work for the rows.

```
  where the late frames' main-thread time went (hitch ms/s from timer ticks 116.4, from scrolling 75.6):
    fling-6000: 30 late frames (5 on a timer's tick), 871 ms late; Exact host 664 ms (in UIKit 223) · app JS (main waits on exact.owner) 278 ms · …
    pull-top: 2 late frames (2 on a timer's tick), 176 ms late; app JS (main waits on exact.owner) 113 ms · Exact host 54 ms (in UIKit 17) · …
```

`feel.json` has every number behind these, per fling (`flings`: τ, R², content speed, lift frame and main-thread
time, content-size changes, `contentShifts`: frames whose content moved under the offset, `jumpAt`) and where each hitch was (`smooth.hitchAt`), plus the list's declared
`decelerationRate`, `bounces`, `scrollsToTop` and `delaysContentTouches`.

## Calibration

On `bake-feel` (iPhone 18 Pro simulator, iOS 27, 60 Hz: the simulator reports `maximumFramesPerSecond` 60), three
runs each of the crypto list benchmark's native apps and five of its Exact app (`bench/crypto-list`: 5,000 rows with
a chart each, the same data):

| | UIKit `UICollectionView` | SwiftUI `List` | Exact (`exact-crypto-svg`) |
|---|---|---|---|
| hitch ratio (ms/s) | 0, 0, 0 | 0, 0, 0 | 2.2, 2.5, 1.1, 0, 5.5 |
| longest frame (ms) | 16.7 | 16.7 | 33.3, 36.3, 33.3, 16.7, 54.5 |
| main thread p95 (ms/frame) | 1.5, 1.5, 2.1 | 5.3, 6.2, 5.3 | 4.1, 4.2, 3.8, 3.9, 4.9 |
| busiest frame at a fling's lift (ms) | 3.2, 3.8, 3.3 | 6.9, 8.7, 7.3 | 29.5, 26.7, 25.0, 16.7, 36.5 |
| flings that drop a frame at the lift | 0/4 each run | 0/4 each run | 2/4, 1/4, 1/4, 0/4, 3/4 |
| offset jumps while decelerating | 0 | 0 | 2, 2, 1, 0, 5 |
| tracking ratio, lag | 1.000, 0 frames | 1.000–1.005, 0 | 1.000–1.004, 0 |
| deceleration τ (ms) | 502 | 502–503 | 497–503 |
| momentum hand-off | 0.95–1.05 | 1.06–1.08 | 1.06–1.15 |
| edge bounce settle (s) | 0.63–0.85 | 0.73–0.82 | 0.70–0.82 |
| rubber band, settle | 0.47, 0.54–0.55 s | 0.47–0.49, 0.54–0.55 s | 0.47–0.49, 0.54–0.55 s |
| status-bar scroll to top (s) | 0.93–0.94 | 0.94–0.96 | 0.94–0.96 |
| tap while decelerating | stops in 1 frame, 0 pt | the same | the same |
| blank samples | 0 of ~130 | 0 of ~135 | 0 of ~135 |
| content-size changes during flings | 0 | ~90 a run (estimated row heights; no offset jump) | 0 |

The physics metrics agree to the millisecond between runs and between the apps: Exact's list is a `UIScrollView`,
so UIKit supplies its deceleration, bounce and scroll to top. The edge settle varies with the speed at which the
last fling meets the top, and the hand-off with how many moves the synthesizer sends in a short fling (±10 %).

What sets Exact apart is main-thread time at the edges of a gesture. At a fling's lift Exact's main thread is busy
4–8 ms (1,500 pt/s), 10–15 ms (3,000), about 15 ms (4,000) and 17–37 ms (6,000), so the fast flings drop a frame at
release in 7 of 20 (UIKit and SwiftUI: 0 of 24); `UIScrollView` then shows no movement for a frame and catches up in
the next (6,500 → 0 → 12,300 pt/s in one run), a hitch at the moment of release. A touch landing on Exact's list costs 4–7 ms (12–17 ms when it
stops a decelerating list), against 1–2 ms for UIKit and 2–7 ms for SwiftUI. During steady motion Exact's p95 sits
between UIKit's and SwiftUI's.

Scrubbing and carousels, on `bake-s1` (iOS 27, 60 Hz), bake-off round s1's apps, load 10 to 70:

| | UIKit photo grid (`UICollectionView`) | SwiftUI feed (`LazyVStack`) | Exact photo grid | Exact feed |
|---|---|---|---|---|
| scrub: hitch ratio (ms/s) | 0 | 495 | 0, 7.3, 0 (52–107 before glimpses) | 69, 72, 73 (420–474 before) |
| scrub: longest frame (ms) | 16.7 | 146 | 17, 47, 17 (57–63 before) | 72, 57, 78 (154–211 before) |
| scrub: blank samples, while held | 0, 0 | 0, 0 | 0–0.03, 0 | 0–0.5, 0 |
| carousel: hitch, τ, hand-off, edge settle | – | 0, 500 ms, 0.85, 0.67 s | – | 0, 498–501 ms, 1.0–1.07, 0.55–0.68 s |
| carousel: drag mostly along it, mostly down the list | – | 0.94 and 0 pt, each | – | 0.95 and 0 pt, each |

Dragged end to end in a second, the indicator moves a long list 2,000 to 10,000 pt a frame, so every frame shows rows
no frame before showed. UIKit's collection view configures a screen of cells in 3 to 5 ms a frame; SwiftUI's lazy
stack takes 50 to 150. Exact builds a glimpse a frame (`CollectionFill::glimpse`: the rows nearest the middle that
three fifths of a frame fits), so the rest of the port shows blank until the finger holds, and a feed post that
costs more than a frame still makes that frame late.

## Limits

- 60 Hz only: the simulator runs at 60 Hz, so a 120 Hz phone's budgets (8.3 ms a frame) are not exercised, and the
  Mac's CPU stands in for the phone's. Treat main-thread numbers as relative between arms.
- Hitches are seen from the app's side (display-link intervals); a frame the render server drops while the main
  thread is idle is not counted.
- The synthesizer sends moves at 30–60 Hz and eases a slow drag in and out; the hand-off compares against the asked
  speed, not the finger's speed at the lift.
- Blank sampling renders the list's layers (`renderInContext`), which omits Metal content and costs 3–7 ms, so it
  runs only during the 4,000 pt/s fling and the flings back up, and those frames are left out of the timing
  metrics. A list that draws through Metal would read as blank (and blank at rest too, so not counted).
- The blank check, validated on the round r21 apps: before it, uniform bands read as blank on SwiftUI's Settings
  (0.36), UIKit's inbox (0.22), SwiftUI's moodboard (0.10) and four Exact apps (expenses 1.0, settings 0.49,
  contacts 0.24); with it, 0 on all twelve native arms and on those Exact apps. A UIKit list whose cells fill
  their content 200 ms after they are dequeued reads 0.43 (full script; 20 of 20 judged bands blank), the same
  list filling at once 0. Where an Exact list's content moved too far under the offset to align, its bands are
  unjudged and the share is unknown (`–`), not 0.
- The stack sampler reads frame pointers and names frames with `dladdr`: code built without frame pointers loses
  its callers, and a stripped app's own code is `app code`. The platform's run-loop callouts are named by their
  symbols; where `dladdr` cannot name one, the time has no callout and counts as scrolling.
- A list that opens at its end (a chat) has its slow drag and first flings go up it instead; the pull past the top
  is skipped when the flings did not bring the list back to its top.
- Horizontal scrolling is probed only on a carousel inside the vertical list; a screen whose only scroller is a
  horizontal pager reads `no-scrollable-list`. The vertical gestures land in the middle of the list and assume a
  row there tolerates a tap (the tap during deceleration). The detector picks the largest qualifying scroll view;
  use `--steps` and `--at` for another.
- The touch runner turns on the simulator's accessibility runtime for its session (`touches.mjs`), so every arm
  runs with it loaded; the sampler shows its cost as `accessibility`.

## On a phone

Not yet. A phone at 120 Hz is what would exercise 8.3 ms budgets, and the recorder itself would run there, but:

- the touch runner is simulator-only (LLP 1080.000 stage 4 is not built): it needs the runner built for
  `iphoneos`, signed with the team profile, installed with `devicectl` and started with `UseDestinationArtifacts`;
  its session's accessibility keys are set with `simctl spawn … defaults`, which a phone has no equivalent of;
- the recorder writes its trace to a host path and reads its two switches (`.sample`, `.check`) from host paths,
  which an app on a phone cannot reach. The probe follows the trace live, so a phone needs a live channel (the
  phone carrier's token bridge, which the touch runner already connects out over), not `devicectl device copy`
  after the run;
- the dylib must be built for `iphoneos` and embedded and signed into the app, as `bench/heavy-list/probe/resign.sh`
  does for its own probe, and inserted at launch through `devicectl device process launch --environment-variables`
  (`devrun.sh`).
