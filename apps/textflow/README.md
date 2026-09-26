# Textflow

Seven studies in live typography, after Cheng Lou's Pretext demos. Original prose,
system serif, light and dark paper. One Contract runs on Linux, macOS, iOS and web.

| Tab / stable id | What to try |
| --- | --- |
| Drag — `scene-drag` (opens first) | Move any of `orb-1`, `orb-2`, `orb-3`. Text reflows while held; release leaves it in place. |
| Orbs — `scene-orbs` | Six different masses drift, collide elastically and bounce off the page edges. |
| Dancer — `scene-dancer` | Fourteen geometric pieces form an articulated figure. Each filled limb and its exclusion share exactly the same vertices. |
| Ball — `scene-ball` | The original single moving exclusion. |
| Editorial — `scene-editorial` | Drop cap, inset pull quote and circles; two columns become one stacked column on a phone. |
| Polygon — `scene-polygon` | A moving triangle with matching visible and exclusion edges. |
| Article — `scene-article` | A drop cap and a pull quote in a block column. No paragraph has a height: each is measured around the shapes it meets, and what follows moves (LLP 1043.000 §8). |

Pause freezes app time. Dark paper changes the palette. Type 115% exercises wider
font metrics: the first six scenes keep definite heights, with at least 20% spare
height at normal Linux type; Article's paragraphs are auto height. At 390×844 the main scenes fit a single
column; Editorial is a longer page and scrolls. Shapes use layout positions, never
transforms or transitions on `left`/`top`. No font or image binaries are required.

```sh
export EXACT_UPDATE_TRUST=development

# Web (open the URL printed by the dev server)
EXACT_APP=textflow bun host/web/dev.mjs

# macOS; append --ios for the simulator
bun host/apple/build.mjs --app textflow --run
bun host/apple/build.mjs --app textflow --ios --run

# Linux's CPU painter, also usable headlessly on macOS
cargo build --release -p textflow-linux --offline
bun scripts/agent.mjs linux --app textflow --size 960x900 tree
```

The agent owns the clock. The same command sequences work with `linux`, `macos`,
`web`, or `ios` in the host position (iOS uses its simulator's viewport). Desktop
hosts accept `--size 390x844` for the narrow layout. Linux contacts are explicitly
reported as presenter synthesis; the other carriers' replies state their delivery
method. Physical pointer/touch smoothness needs a live host check.
Held contacts on iOS use a desktop pointer into an on-screen, unobscured Simulator;
the terminal needs its existing Accessibility permission for that carrier.

```sh
mkdir -p target/textflow-scratch/m6

# Repeat with HOST=macos, HOST=web, HOST=ios after building that host.
HOST=linux
bun scripts/agent.mjs "$HOST" --app textflow --size 960x900 \
  "tap scene-drag" "tap orb-1 down" \
  "tap move by 120 40 over 160" "layout drag-prose" \
  "screenshot target/textflow-scratch/m6/held.png" "tap up"

bun scripts/agent.mjs "$HOST" --app textflow --size 960x900 \
  "tap scene-orbs" "clock +800" "layout orbs-prose" \
  "clock +800" "screenshot target/textflow-scratch/m6/orbits.png"

bun scripts/agent.mjs "$HOST" --app textflow --size 960x900 \
  "tap scene-dancer" "clock +800" "layout dancer-prose" \
  "clock +1600" "screenshot target/textflow-scratch/m6/dance.png"

bun scripts/agent.mjs "$HOST" --app textflow --size 960x900 \
  "tap scene-ball" "clock +800" "layout ball-prose" \
  "tap scene-editorial" "layout editorial-left" "layout editorial-right" \
  "tap scene-polygon" "clock +800" "layout polygon-prose" \
  "tap scene-article" "layout article-lede" "layout article-second" \
  "tap type-size" "tap appearance" logs

cargo test -p textflow-linux --release --offline -- --nocapture --test-threads=1
```

`pan=action` supplies incremental `(dx: number, dy: number)` in viewport CSS
pixels after any curried arguments. It is an ordinary action commit, so it can
change layout state, a scrubber or another application value. It acquires no
motion token. One primary contact belongs to the nearest eligible handler;
removal/disablement cancels delivery. Cancellation retains committed state.
Mouse/browser recognition uses a four-pixel threshold; UIKit recognizes its pan.
The browser coalesces samples into one action per animation frame. Native hosts
commit each delivered recognized sample. `touch-action="none"` on the orbs keeps
the page scroll gesture outside their hit areas.

The scene source is `sceneGeometry(scene, elapsedMs, width) -> list<Part>`.
Its input is app time advanced by `every(16, step)`; scene selection resets app
time, pause freezes it. Geometry is independent of query history. Six discs use
fixed 16 ms steps and elastic impulses. A smooth 60-second cycle traverses a
30-second physical trajectory forwards and backwards (at most 1,875 simulation
steps × 15 pair tests per query), without a loop reset; the dancer uses overlapping
smooth joint-angle cycles. Each capsule has twelve vertices, below the sixteen
vertex budget. The data source refuses nonfinite input and clamps width to
240…900 logical pixels. No timing HUD is shown because the app has no cheap,
portable frame-cost resource. Bubbles is deferred: tight-width line-count search
needs a layout capability beyond these scenes.

The existing runner limits one clock advance to 4,096 timer firings. A larger
seek returns `TimerFireLimit` at the last committed time; it does not replay an
unbounded number of frames. Nonfinite clocks and clocks above the runner's
maximum are refused. A pan sample commits one action, and the app clamps each
orb inside the prose box.
