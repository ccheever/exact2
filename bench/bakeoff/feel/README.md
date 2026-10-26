# Scroll feel probe

Plays the same real touches on any iOS simulator app's list and says, metric by metric, whether its scrolling feels
like UIKit's. It does not care what built the app: an Exact app, a SwiftUI `List` and a `UICollectionView` are
probed the same way, so the native arms are the baseline for Exact.

| File | What it is |
|---|---|
| `feel.m`, `build.sh` | the recorder: a dylib inserted at launch that only watches (writes `target/feel/feel-sim.dylib`) |
| `probe.mjs` | relaunches the app with the recorder, plays the gestures through the touch runner (`host/apple/touches.mjs`), analyzes |
| `analyze.mjs` | trace → `feel.json` and `feel.txt`; given several output directories, a comparison table |

## Running it

```sh
bun bench/bakeoff/feel/probe.mjs --udid <simulator udid> --app <bundle id> [--out DIR]
# a list behind a tab or a push: steps first (window points; wait in seconds; drag ends with an optional speed)
bun bench/bakeoff/feel/probe.mjs --udid U --app B --steps "tap 300,820;wait 1"
# several runs side by side
bun bench/bakeoff/feel/analyze.mjs DIR1 DIR2 DIR3
```

The app must already be installed on the booted simulator. A probe takes about 40 s (8 s when there is no list),
plus about 5 s the first time the touch runner starts on a simulator. `--out` defaults to
`target/feel/<bundle id>-<time>/`; it gets `trace.jsonl` (every recorded frame, touch and scroll-view scan),
`gestures.json`, `feel.json` and `feel.txt`. `--at x,y` moves where the gestures land (default: the middle of the
list); `--dump DIR` saves the first sampled frames and any with a blank band as PNGs.

If the screen the steps reach has no shown scroll view whose content is at least 1.5 × its height + 100 pt, the
result is `no-scrollable-list` (exit 0), not a failure.

## What it does

The recorder hooks `-[UIApplication sendEvent:]` to log touches and runs a display link at the screen's maximum
rate. On every frame while a finger is down or a scroll view moves, it records the moving scroll view (the innermost
one tracking, dragging or decelerating, else the one whose offset changed), its offset, content size, insets and
flags, and the main run loop's busy time since the last frame. While the driver asks for it, every 4th moving frame
is rendered after Core Animation's commit and searched for blank bands (60 pt or more of uniform scanlines where
the list has content).

The gestures, all real HID touches through XCTest's synthesizer:

1. a slow drag (250 pt/s) held 0.5 s before the lift;
2. flings down at 1,500, 4,000 (blanks sampled) and 6,000 pt/s, each left to stop;
3. flings up at 8,000 pt/s, 1.2 s apart, until the list is back at the top (blanks sampled); the last meets the
   top with momentum;
4. a pull 240 pt past the top, held, released;
5. a fling down at 3,000 pt/s, then a tap on the status bar;
6. a fling down at 4,000 pt/s and, 200 ms after the lift, a tap in the middle of the list.

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
| offset jumps while decelerating | frames off the fitted curve by more than 15 % (or 150 pt/s): a stall, a frozen frame or a catch-up | none |
| edge bounce settle | from momentum crossing the top to resting there | 0.25–1.0 s |
| rubber band | overscroll ÷ finger travel past the top; and the time from the lift to resting | 0.3–0.65; 0.25–1.0 s |
| status-bar scroll to top | from the tap to the top | ≤ 1.0 s |
| tap while decelerating | content travel after the touch lands; `activated` if the list left the screen | < 5 pt |
| blank samples | the share of sampled moving frames with a blank band | 0 |

`feel.json` has every number behind these, per fling (`flings`: τ, R², content speed, lift frame and main-thread
time, content-size changes) and where each hitch was (`smooth.hitchAt`), plus the list's declared
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

## Limits

- 60 Hz only: the simulator runs at 60 Hz, so a 120 Hz phone's budgets (8.3 ms a frame) are not exercised, and the
  Mac's CPU stands in for the phone's. Treat main-thread numbers as relative between arms.
- Hitches are seen from the app's side (display-link intervals); a frame the render server drops while the main
  thread is idle is not counted.
- The synthesizer sends moves at 30–60 Hz and eases a slow drag in and out; the hand-off compares against the asked
  speed, not the finger's speed at the lift.
- Blank sampling renders the list's layers (`renderInContext`), which omits Metal content and costs 3–7 ms, so it
  runs only during the 4,000 pt/s fling and the flings back up, and those frames are left out of the timing
  metrics. A list that draws through Metal would read as blank.
- Vertical lists only; the gestures land in the middle of the list and assume a row there tolerates a tap (the
  tap during deceleration). The detector picks the largest qualifying scroll view; use `--steps` and `--at` for
  another.
- The touch runner turns on the simulator's accessibility runtime for its session (`touches.mjs`), so every arm
  runs with it loaded.
