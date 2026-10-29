# Ink S0: retained rows, wgpu, Metal and Direct3D 11

@ref LLP 1149 D4, D15.

[C5: direct Metal against wgpu](#c5-direct-metal-measured-against-wgpu) reports the second decision gate. [C6: NucBox](#nucbox) reports the Windows spike.

[G1: GPUI on the Mac](../../bench/heavy-list/gpui/RESULTS.md) records the heavy-list arm, parity captures and pending whole-app measurement.

## First D15 verdict: frame time passes; the full gate is not established

**C beats B's frame-time threshold; D15 is not an unconditional pass.**
At 1920×1080, Ink cuts paint time by
29.59% to 41.66% across all measured modes. Messages Stress scroll paint is
1.434 ms [1.273–1.505] versus B's 2.152 ms [2.141–2.455]: **33.37% faster**.
Desktop scroll CPU is equal at 1.300 ms/frame; phone scroll CPU falls from
1.233 to 0.667 ms/frame (45.94%). The frame-time alternative earns the margin
without claiming a desktop CPU win.

No compared median regresses by more than 10%. The largest regressions are
Messages Stress's desktop lifetime peak RSS (5.301%) and text-update CPU
(5.263%). Device creation is within 0.2% for the empty-directory boot samples;
Ink's shader setup is substantially faster. Comparisons use unrounded data.
Displayed measurements are rounded consistently, not used to decide the gate.

Comparing each arm's highest RSS exceeds the 10% regression bound: Ink's
highest desktop resident-set peaks are 14.53% above B for Messages Stress,
15.40% for Shelf and 22.53% for Excord. The five-run median comparison passes,
but the wider RSS spread prevents an unconditional no-regression verdict.
macOS's separate physical-footprint peaks are lower for Ink in every workload;
that does not remove the observed RSS regression.

B and C each have zero of 21,000 redraws above 25 ms, with maxima of 6.148 ms
and 3.960 ms respectively. These are unpaced headless service times: **presented
frame intervals, input latency and actual late frames remain unmeasured**.
These results establish the headless frame-time win, not the complete D15 gate
or a qualification of a window carrier. S0's direct-device comparisons remain
separate.

| Desktop workload | Mode | B paint ms | C paint ms | C improvement |
|---|---|---:|---:|---:|
| Messages Stress | still | 2.129 | 1.306 | 38.69% |
| Messages Stress | scroll | 2.152 | 1.434 | 33.37% |
| Messages Stress | text | 2.148 | 1.512 | 29.59% |
| Shelf | still | 1.938 | 1.136 | 41.40% |
| Shelf | text | 1.985 | 1.164 | 41.39% |
| Excord | still | 1.885 | 1.102 | 41.55% |
| Excord | text | 2.023 | 1.180 | 41.66% |

Retention matters in both implementations. On desktop Messages Stress scrolling,
B reduces A's paint time from 2.716 to 2.152 ms. C reduces C−rows from 2.155 to
1.434 ms. Ink without rows is approximately tied with B on that workload.
With rows, Ink's lower submission and readback times account for most of the
remaining frame-time advantage; readback includes GPU completion, so this does
not isolate shader execution time. The atlas stays resident and the walk avoids
preparing unchanged row glyphs again.

All measured Ink frames use **zero layer-pool targets** and **zero overflow
atlas bytes**. These first screens therefore do not exercise the O(σ) backdrop
blur or the full-viewport layers and copies for transformed/extra clips flagged
by the C1–C3 reviews. Those costs are not the source of these timings, and these
results do not qualify workloads dominated by either operation.

## Method

Measured on 2026-10-28 on an Apple M5 Max: 18 CPU cores, 40 GPU cores,
128 GiB memory, macOS 27.0.1 (26A434), Metal 4. Rust 1.97.0, Bun 1.4.2.
The renderer is commit `b7184a2433` on `lane/ink`. The measurement module's
Git blob is `2b5432743f6101ee3c3f54bf41a4d2a3a380d010`; the remaining changes
add its test dependencies and this report, without changing the renderers.

The profile is **host-dev**: release optimization level 3, no LTO, 16 codegen
units, incremental compilation, debug assertions disabled. These are not C2's
debug-profile numbers. All arms use `Presenter::frame()` to render to a texture,
wait, read pixels back and return a CPU pixmap. None presents to a window.
The viewport and pixel dimensions are 390×844 and 1920×1080 at **1×**.
The screenshots below are separate captures at **2×**. The font is the fixture's
DejaVu Sans, loaded from `scripts/fixtures/fonts/assets`.

| Arm | Backend | Retained rows |
|---|---|---|
| A | Vello, wgpu 29 | `EXACT_ROWS=0` |
| B | Vello, wgpu 29 | on |
| C | Ink, wgpu 30 | on |
| C−rows | Ink, wgpu 30 | `EXACT_ROWS=0` |

Five rounds interleave A, B, C, C−rows for each workload and size. Each sample
runs in a fresh child process with isolated application storage. App activation,
image loading and the first frame precede measurement. No lane builds or test
suites run during the series. Other applications and macOS services remain
running on this shared Mac; the ranges retain their effects.

Messages Stress uses its 1,000-message history with viewport virtualization
enabled and its automatic updates stopped. It is the heavy-list workload allowed
by the C4 brief. Shelf uses its three seeded books. Excord uses its loaded first
channel directory against the local transcript fixture on port 4797, reset on
each identify. Shelf and Excord have no vertical scroll range on these first
screens; their scroll samples are unavailable, not zero-cost scrolls.

Each available mode measures 300 frames. Still forces a repaint of unchanged
content; an idle application would normally submit no frames. Scroll drives the
agent's `Presenter::wheel` operation by 20 points per frame and asserts the actual
offset change. Text alternates an A/B suffix on the first visible alphanumeric
text in a list row, directly through the kernel commit and row invalidation path;
it includes text layout but excludes application data-query work. Modes start
at scroll offset zero. The virtual clock stays fixed during each sample.

Paint is wall time inside `Presenter::frame()`. Encode and readback are the two
values of `last_frame_ms()`: encode covers backend render submission, not the
tree walk or all glyph preparation; readback includes waiting for the GPU and
copying the mapped pixels. These are not GPU timestamp measurements. The whole
step additionally includes the scroll operation or text commit before painting.

CPU is the change in the process's `ps -o time=` over a 300-frame sample,
divided by 300. It includes the operations, instrumentation and parent-process
cost of RSS sampling; it excludes the `ps` children's CPU. Its 10 ms reporting
quantum is about 0.033 ms/frame. RSS is sampled every 20 frames; `/usr/bin/time -l`
also records each worker's lifetime peak, including CPU boot and all three modes.
RSS is resident memory, not separately charged GPU memory. The memory table also
reports `/usr/bin/time -l`’s separate `peak memory footprint` field.

Tables use the median of the five per-run medians, with their minimum–maximum
range. CPU and sampled RSS use the median and range of the five per-run values.
The raw frame samples and boot records are in `target/ink-c4-results.jsonl`.
The session log is `target/ink-c4-series.log`.

## Reproduce

Start `bun apps/excord/fixture/server.ts --port 4797 --world transcript
--reset-on-identify`. Bake Shelf with `cargo check -p shelf-linux`, and Excord
with `EXACT_BACKEND_ORIGIN=http://127.0.0.1:4797 cargo check -p excord-windows`.
The harness finds their latest `target/debug/build/*/out` directories, then
passes those same paths to every worker. `INK_BAKED_SHELF` and
`INK_BAKED_EXCORD` can select explicit directories.

```sh
cargo test --profile host-dev -p exact-linux --lib \
  presenter::kept_rows_tests::measurement::series -- \
  --ignored --exact --nocapture --test-threads=1
```

The three measurement tests are ignored by name in the ordinary suite and refuse to run
with debug assertions enabled. `INK_PREVIEW=1 INK_ARM=C INK_APP=<app>` runs the
`worker` test as a 390×844, 2× screenshot capture instead of a timing sample.

## Frame results

All timing cells are milliseconds, median [minimum–maximum] across five runs.
RSS cells are MiB. Each available row represents 1,500 frames. Shelf and Excord
scroll rows are unavailable at both sizes for every arm.

### Messages Stress, 390×844

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 2.137 [2.076–2.149] | 0.315 [0.297–0.323] | 1.556 [1.547–1.557] | 1.200 [1.167–1.267] | 66.5 [63.7–67.6] |
| still | B | 1.934 [1.920–1.996] | 0.267 [0.260–0.297] | 1.547 [1.545–1.559] | 1.033 [1.000–1.133] | 66.3 [63.2–68.6] |
| still | C | 0.675 [0.626–0.717] | 0.118 [0.110–0.123] | 0.367 [0.350–0.418] | 0.667 [0.600–0.733] | 56.8 [55.5–57.0] |
| still | C−rows | 0.877 [0.854–0.909] | 0.124 [0.120–0.130] | 0.395 [0.388–0.434] | 0.833 [0.800–0.867] | 56.7 [54.4–56.8] |
| scroll | A | 2.202 [2.115–2.235] | 0.365 [0.325–0.382] | 1.551 [1.544–1.559] | 1.333 [1.200–1.367] | 68.8 [67.2–69.2] |
| scroll | B | 2.036 [1.977–2.166] | 0.348 [0.302–0.432] | 1.557 [1.549–1.563] | 1.233 [1.100–1.400] | 69.5 [68.1–69.7] |
| scroll | C | 0.670 [0.638–0.706] | 0.115 [0.108–0.119] | 0.365 [0.341–0.389] | 0.667 [0.600–0.767] | 58.6 [57.2–59.2] |
| scroll | C−rows | 0.964 [0.906–1.078] | 0.132 [0.124–0.168] | 0.419 [0.394–0.452] | 0.933 [0.867–1.133] | 57.2 [54.7–57.2] |
| text | A | 2.139 [2.047–2.201] | 0.336 [0.275–0.348] | 1.553 [1.544–1.561] | 1.300 [1.100–1.367] | 68.9 [68.2–69.2] |
| text | B | 1.983 [1.975–2.083] | 0.286 [0.279–0.348] | 1.549 [1.547–1.559] | 1.100 [1.100–1.267] | 69.6 [68.3–69.8] |
| text | C | 0.698 [0.658–0.739] | 0.113 [0.110–0.117] | 0.374 [0.337–0.423] | 0.700 [0.667–0.767] | 58.8 [58.7–59.3] |
| text | C−rows | 0.885 [0.869–0.916] | 0.122 [0.118–0.134] | 0.406 [0.390–0.430] | 0.833 [0.800–0.900] | 57.0 [56.0–57.0] |

### Messages Stress, 1920×1080

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 2.477 [2.445–2.539] | 0.458 [0.412–0.490] | 1.637 [1.632–1.644] | 1.567 [1.500–1.700] | 80.0 [79.2–80.4] |
| still | B | 2.129 [2.117–2.237] | 0.354 [0.339–0.432] | 1.642 [1.632–1.651] | 1.200 [1.167–1.400] | 79.4 [78.0–79.9] |
| still | C | 1.306 [1.245–1.577] | 0.148 [0.132–0.190] | 0.961 [0.904–1.156] | 1.200 [1.100–1.433] | 68.6 [68.3–69.0] |
| still | C−rows | 1.810 [1.730–1.879] | 0.172 [0.161–0.180] | 1.009 [0.985–1.089] | 1.633 [1.533–1.667] | 68.8 [67.8–68.9] |
| scroll | A | 2.716 [2.666–2.813] | 0.576 [0.531–0.627] | 1.634 [1.633–1.644] | 1.867 [1.767–2.067] | 83.6 [83.5–85.4] |
| scroll | B | 2.152 [2.141–2.455] | 0.370 [0.363–0.559] | 1.637 [1.630–1.674] | 1.300 [1.167–1.767] | 81.8 [81.7–82.8] |
| scroll | C | 1.434 [1.273–1.505] | 0.163 [0.142–0.182] | 1.033 [0.931–1.113] | 1.300 [1.233–1.433] | 70.2 [69.9–70.9] |
| scroll | C−rows | 2.155 [2.137–2.451] | 0.206 [0.204–0.249] | 1.145 [1.123–1.302] | 1.933 [1.933–2.267] | 70.5 [69.6–71.0] |
| text | A | 2.453 [2.405–2.528] | 0.441 [0.395–0.493] | 1.634 [1.632–1.641] | 1.533 [1.467–1.733] | 83.2 [83.2–85.1] |
| text | B | 2.148 [2.130–2.388] | 0.354 [0.332–0.502] | 1.638 [1.633–1.658] | 1.267 [1.200–1.700] | 81.9 [81.8–82.9] |
| text | C | 1.512 [1.435–1.596] | 0.177 [0.149–0.186] | 1.092 [1.005–1.138] | 1.333 [1.267–1.533] | 70.4 [70.2–78.2] |
| text | C−rows | 1.872 [1.817–2.059] | 0.178 [0.163–0.219] | 1.116 [1.065–1.178] | 1.700 [1.600–1.900] | 70.2 [69.3–70.7] |

### Shelf, 390×844

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 2.016 [1.966–2.038] | 0.230 [0.213–0.246] | 1.555 [1.549–1.561] | 1.133 [1.067–1.200] | 69.4 [68.9–69.7] |
| still | B | 1.911 [1.897–1.936] | 0.245 [0.238–0.261] | 1.564 [1.555–1.569] | 1.100 [1.000–1.133] | 69.9 [68.9–70.6] |
| still | C | 0.520 [0.508–0.544] | 0.088 [0.084–0.096] | 0.346 [0.342–0.355] | 0.533 [0.500–0.533] | 58.4 [57.7–59.7] |
| still | C−rows | 0.669 [0.648–0.689] | 0.092 [0.085–0.098] | 0.355 [0.344–0.358] | 0.700 [0.633–0.700] | 59.2 [56.4–59.6] |
| text | A | 1.990 [1.959–2.039] | 0.216 [0.203–0.222] | 1.552 [1.547–1.560] | 1.100 [1.033–1.167] | 69.8 [69.1–70.1] |
| text | B | 1.925 [1.904–1.964] | 0.228 [0.215–0.239] | 1.554 [1.551–1.564] | 1.100 [1.033–1.167] | 70.1 [69.4–71.0] |
| text | C | 0.573 [0.553–0.607] | 0.091 [0.083–0.104] | 0.350 [0.347–0.356] | 0.600 [0.567–0.700] | 59.6 [57.9–59.9] |
| text | C−rows | 0.663 [0.648–0.689] | 0.090 [0.083–0.096] | 0.351 [0.337–0.358] | 0.700 [0.633–0.767] | 59.8 [57.8–63.9] |

### Shelf, 1920×1080

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 2.077 [2.021–2.083] | 0.222 [0.199–0.237] | 1.635 [1.629–1.640] | 1.167 [1.100–1.167] | 93.0 [90.4–93.7] |
| still | B | 1.938 [1.904–1.964] | 0.213 [0.199–0.239] | 1.634 [1.629–1.636] | 1.033 [0.900–1.200] | 93.1 [89.0–93.8] |
| still | C | 1.136 [1.113–1.163] | 0.096 [0.092–0.105] | 0.947 [0.937–0.967] | 1.000 [1.000–1.067] | 84.6 [84.5–86.0] |
| still | C−rows | 1.310 [1.227–1.370] | 0.106 [0.093–0.125] | 0.976 [0.903–0.997] | 1.233 [1.100–1.267] | 84.4 [84.4–86.6] |
| text | A | 2.080 [2.012–2.103] | 0.210 [0.190–0.247] | 1.635 [1.627–1.658] | 1.200 [1.133–1.267] | 93.5 [91.0–93.9] |
| text | B | 1.985 [1.981–2.037] | 0.215 [0.208–0.227] | 1.635 [1.633–1.660] | 1.100 [1.033–1.200] | 93.5 [89.2–94.0] |
| text | C | 1.164 [1.130–1.179] | 0.093 [0.093–0.097] | 0.947 [0.880–0.955] | 1.067 [1.033–1.133] | 84.9 [84.8–92.7] |
| text | C−rows | 1.278 [1.222–1.361] | 0.097 [0.090–0.114] | 0.956 [0.928–1.002] | 1.200 [1.100–1.467] | 84.8 [84.6–100.8] |

### Excord, 390×844

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 1.939 [1.887–2.015] | 0.196 [0.163–0.212] | 1.549 [1.544–1.563] | 1.033 [0.933–1.167] | 72.8 [70.7–74.0] |
| still | B | 1.792 [1.748–1.853] | 0.196 [0.169–0.228] | 1.549 [1.544–1.567] | 0.867 [0.800–1.067] | 73.4 [70.8–74.2] |
| still | C | 0.479 [0.446–0.515] | 0.086 [0.081–0.090] | 0.344 [0.320–0.381] | 0.500 [0.433–0.533] | 63.1 [61.3–64.0] |
| still | C−rows | 0.638 [0.613–0.659] | 0.086 [0.085–0.088] | 0.352 [0.337–0.374] | 0.600 [0.600–0.667] | 63.7 [60.4–64.4] |
| text | A | 1.956 [1.895–2.033] | 0.203 [0.165–0.216] | 1.548 [1.544–1.561] | 1.133 [0.967–1.200] | 73.4 [72.2–74.2] |
| text | B | 1.877 [1.874–1.978] | 0.172 [0.170–0.219] | 1.545 [1.544–1.567] | 1.000 [0.933–1.167] | 74.2 [72.0–74.4] |
| text | C | 0.594 [0.578–0.637] | 0.083 [0.082–0.092] | 0.347 [0.328–0.370] | 0.633 [0.567–0.700] | 63.6 [61.5–64.1] |
| text | C−rows | 0.625 [0.617–0.659] | 0.081 [0.079–0.083] | 0.351 [0.331–0.382] | 0.633 [0.633–0.733] | 63.9 [63.2–66.0] |

### Excord, 1920×1080

| Mode | Arm | Paint | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---|---:|---:|---:|---:|---:|
| still | A | 2.025 [1.984–2.090] | 0.196 [0.173–0.225] | 1.635 [1.628–1.672] | 1.133 [1.033–1.200] | 91.9 [91.2–94.3] |
| still | B | 1.885 [1.842–1.901] | 0.194 [0.170–0.208] | 1.633 [1.627–1.641] | 1.000 [0.867–1.033] | 93.6 [91.6–94.3] |
| still | C | 1.102 [1.032–1.112] | 0.090 [0.087–0.095] | 0.966 [0.875–0.972] | 0.967 [0.900–1.000] | 84.0 [83.5–87.7] |
| still | C−rows | 1.256 [1.239–1.297] | 0.092 [0.089–0.103] | 0.978 [0.967–0.994] | 1.100 [1.067–1.167] | 86.7 [83.5–87.5] |
| text | A | 2.087 [2.062–2.219] | 0.212 [0.195–0.301] | 1.660 [1.636–1.661] | 1.233 [1.233–1.500] | 92.8 [92.1–94.6] |
| text | B | 2.023 [1.979–2.075] | 0.199 [0.186–0.213] | 1.636 [1.629–1.668] | 1.167 [1.100–1.233] | 93.8 [92.2–94.5] |
| text | C | 1.180 [1.127–1.217] | 0.088 [0.084–0.095] | 0.917 [0.865–0.962] | 1.100 [1.067–1.100] | 84.2 [83.7–87.9] |
| text | C−rows | 1.216 [1.165–1.272] | 0.087 [0.082–0.098] | 0.950 [0.868–0.972] | 1.100 [1.067–1.167] | 86.9 [83.8–87.8] |

## Lifetime memory peaks

Each cell is median [minimum–maximum], MiB, across five fresh workers. These
include boot and all available modes, unlike the samples taken during each mode
above. Footprint is the field reported by macOS `time -l`, not an estimate from RSS.

### Resident set

| Workload | Size | A | B | C | C−rows |
|---|---|---:|---:|---:|---:|
| Messages Stress | 390×844 | 69.6 [68.6–70.0] | 70.2 [69.0–70.5] | 60.6 [59.9–69.5] | 59.1 [57.7–66.6] |
| Messages Stress | 1920×1080 | 85.0 [84.9–87.9] | 82.5 [82.4–83.6] | 86.9 [71.3–95.7] | 87.0 [71.5–101.4] |
| Shelf | 390×844 | 70.0 [69.2–70.3] | 70.3 [69.5–71.1] | 59.9 [58.0–62.7] | 59.8 [59.4–67.1] |
| Shelf | 1920×1080 | 93.6 [91.2–94.0] | 93.6 [89.3–94.1] | 92.6 [84.9–108.6] | 92.6 [86.5–113.4] |
| Excord | 390×844 | 73.5 [72.3–74.2] | 74.3 [72.0–74.5] | 65.2 [64.3–75.1] | 67.9 [63.7–74.2] |
| Excord | 1920×1080 | 92.8 [92.1–94.7] | 93.8 [92.3–94.5] | 91.4 [87.9–115.8] | 94.9 [89.3–127.3] |

### Footprint

| Workload | Size | A | B | C | C−rows |
|---|---|---:|---:|---:|---:|
| Messages Stress | 390×844 | 532.8 [532.2–533.8] | 533.6 [532.1–533.8] | 380.1 [379.7–389.5] | 378.6 [376.7–387.4] |
| Messages Stress | 1920×1080 | 557.5 [556.5–560.1] | 553.7 [553.6–554.0] | 414.7 [405.9–429.0] | 421.8 [408.1–430.1] |
| Shelf | 390×844 | 529.3 [528.4–529.5] | 529.4 [528.5–530.2] | 380.6 [378.5–382.2] | 380.5 [380.1–388.3] |
| Shelf | 1920×1080 | 560.1 [557.7–560.8] | 560.2 [555.9–560.8] | 421.0 [421.0–437.0] | 423.1 [420.7–445.1] |
| Excord | 390×844 | 532.8 [531.8–533.7] | 533.8 [531.7–534.1] | 385.9 [383.8–396.5] | 387.6 [384.4–394.2] |
| Excord | 1920×1080 | 559.8 [559.3–561.6] | 560.8 [559.5–561.7] | 423.8 [419.6–444.2] | 423.5 [419.8–463.9] |

## Atlas and layer counters

Ink counters cover the timed frames, after the initial frame. Misses, hits and
uploads are per-frame averages from each 300-frame run, shown as median
[minimum–maximum]. Atlas KiB and pins are maxima over all five runs. The totals
combine grayscale glyph, colour glyph, path and gradient-ramp atlases. Replay
keeps a texture live without counting each retained glyph as an atlas lookup;
hit counts therefore differ substantially between C and C−rows.

Every row below has zero overflow bytes, zero used and retained layer targets,
and zero layer-pool bytes. Image textures, the final target and the readback
buffer are outside those atlas/layer counters. **A/B counters are unavailable**:
the retained-scene control exposes neither Vello's internal atlas counters nor
its GPU layer allocation counters; the raw records use null, not zero.

| Workload | Width | Mode | Arm | Misses/frame | Hits/frame | Upload B/frame | Atlas KiB max | Pinned textures max |
|---|---:|---|---|---:|---:|---:|---:|---:|
| Messages Stress | 390 | still | C | 0.000 [0.000–0.000] | 550.00 [550.00–550.00] | 0.00 [0.00–0.00] | 256 | 1 |
| Messages Stress | 390 | still | C−rows | 0.000 [0.000–0.000] | 1383.00 [1383.00–1383.00] | 0.00 [0.00–0.00] | 256 | 0 |
| Messages Stress | 390 | scroll | C | 0.133 [0.133–0.133] | 574.69 [574.69–574.69] | 10.53 [10.53–10.53] | 256 | 1 |
| Messages Stress | 390 | scroll | C−rows | 0.133 [0.133–0.133] | 1925.95 [1925.95–1925.95] | 10.53 [10.53–10.53] | 256 | 0 |
| Messages Stress | 390 | text | C | 0.007 [0.007–0.007] | 594.99 [594.99–594.99] | 0.60 [0.60–0.60] | 256 | 1 |
| Messages Stress | 390 | text | C−rows | 0.007 [0.007–0.007] | 1383.99 [1383.99–1383.99] | 0.60 [0.60–0.60] | 256 | 0 |
| Messages Stress | 1920 | still | C | 0.000 [0.000–0.000] | 550.00 [550.00–550.00] | 0.00 [0.00–0.00] | 256 | 1 |
| Messages Stress | 1920 | still | C−rows | 0.000 [0.000–0.000] | 2758.00 [2758.00–2758.00] | 0.00 [0.00–0.00] | 256 | 0 |
| Messages Stress | 1920 | scroll | C | 0.030 [0.030–0.030] | 585.89 [585.89–585.89] | 2.16 [2.16–2.16] | 256 | 1 |
| Messages Stress | 1920 | scroll | C−rows | 0.030 [0.030–0.030] | 4349.21 [4349.21–4349.21] | 2.16 [2.16–2.16] | 256 | 0 |
| Messages Stress | 1920 | text | C | 0.007 [0.007–0.007] | 594.99 [594.99–594.99] | 0.60 [0.60–0.60] | 256 | 1 |
| Messages Stress | 1920 | text | C−rows | 0.007 [0.007–0.007] | 2758.99 [2758.99–2758.99] | 0.60 [0.60–0.60] | 256 | 0 |
| Shelf | 390 | still | C | 0.000 [0.000–0.000] | 59.00 [59.00–59.00] | 0.00 [0.00–0.00] | 512 | 2 |
| Shelf | 390 | still | C−rows | 0.000 [0.000–0.000] | 213.00 [213.00–213.00] | 0.00 [0.00–0.00] | 512 | 0 |
| Shelf | 390 | text | C | 0.007 [0.007–0.007] | 105.99 [105.99–105.99] | 1.31 [1.31–1.31] | 512 | 2 |
| Shelf | 390 | text | C−rows | 0.007 [0.007–0.007] | 213.99 [213.99–213.99] | 1.31 [1.31–1.31] | 512 | 0 |
| Shelf | 1920 | still | C | 0.000 [0.000–0.000] | 70.00 [70.00–70.00] | 0.00 [0.00–0.00] | 512 | 2 |
| Shelf | 1920 | still | C−rows | 0.000 [0.000–0.000] | 221.00 [221.00–221.00] | 0.00 [0.00–0.00] | 512 | 0 |
| Shelf | 1920 | text | C | 0.007 [0.007–0.007] | 115.99 [115.99–115.99] | 1.31 [1.31–1.31] | 512 | 2 |
| Shelf | 1920 | text | C−rows | 0.007 [0.007–0.007] | 221.99 [221.99–221.99] | 1.31 [1.31–1.31] | 512 | 0 |
| Excord | 390 | still | C | 0.000 [0.000–0.000] | 11.00 [11.00–11.00] | 0.00 [0.00–0.00] | 512 | 2 |
| Excord | 390 | still | C−rows | 0.000 [0.000–0.000] | 70.00 [70.00–70.00] | 0.00 [0.00–0.00] | 512 | 0 |
| Excord | 390 | text | C | 0.007 [0.007–0.007] | 66.99 [66.99–66.99] | 1.31 [1.31–1.31] | 512 | 2 |
| Excord | 390 | text | C−rows | 0.007 [0.007–0.007] | 70.99 [70.99–70.99] | 1.31 [1.31–1.31] | 512 | 0 |
| Excord | 1920 | still | C | 0.000 [0.000–0.000] | 26.00 [26.00–26.00] | 0.00 [0.00–0.00] | 512 | 2 |
| Excord | 1920 | still | C−rows | 0.000 [0.000–0.000] | 80.00 [80.00–80.00] | 0.00 [0.00–0.00] | 512 | 0 |
| Excord | 1920 | text | C | 0.007 [0.007–0.007] | 76.99 [76.99–76.99] | 1.31 [1.31–1.31] | 512 | 2 |
| Excord | 1920 | text | C−rows | 0.007 [0.007–0.007] | 80.99 [80.99–80.99] | 1.31 [1.31–1.31] | 512 | 0 |

## Device and shader setup

Ten runs per arm and requested cache state, interleaved A/B/C, after the frame
series. Each arm has its own cache directory under `target/ink-c4-cache`.
The empty case removes that directory before every run; the retained case leaves
it in place. **Every run returned `cached=false` and no application pipeline
cache was created.** These wgpu Metal backends do not supply the requested
application-cache distinction. Both sets have warm system driver caches; deleting
an application directory does not make a first-ever Metal launch. Truly cold
shader compilation and an actual application-cache hit could not be measured
on this backend. These phases also exclude fonts, app activation and first paint.

Cells are median [minimum–maximum] milliseconds across ten runs.

| Requested cache state | Arm | Device | Shaders | Actual cached runs |
|---|---|---:|---:|---:|
| empty directory | A | 13.057 [11.408–19.294] | 9.031 [8.902–9.982] | 0/10 |
| empty directory | B | 12.534 [12.052–14.671] | 9.076 [8.852–11.143] | 0/10 |
| empty directory | C | 12.554 [11.764–17.517] | 1.951 [1.863–2.221] | 0/10 |
| retained directory | A | 12.652 [11.613–18.907] | 8.990 [8.842–9.303] | 0/10 |
| retained directory | B | 12.690 [11.759–15.907] | 8.992 [8.856–9.263] | 0/10 |
| retained directory | C | 12.408 [11.824–15.597] | 1.978 [1.849–2.177] | 0/10 |

## Fixture identity and screenshots

The baked inputs used by every worker, SHA-256:

```text
shelf/app.plan  65b682425c6c28bca40968917f7f9c3dc1885cf80b19fee4f949ca0bdf1b2847
shelf/app.hbc  4931b6ca5f3ca72c4f8426687ae40608c399d515a085bc614f977669f3396775
excord/app.plan  179036d9fb16dc2fca1ece8d571edeca2366274a0e458a3d605eae20eb04a2b6
excord/app.hbc  8370918824a4376654f2d74627ead3213a06b83b625c13dd378c394ace6376ba
```

The text-update samples use “You (synthetic)”, “The Dispossessed” and “general”
respectively, the same text in every arm, size and round.

The saved Ink screenshots were inspected at 2× (780×1688 pixels):

- [Caltrain](../../target/ink-caltrain.png)
- [Shelf](../../target/ink-shelf.png)
- [Messages Stress, the heavy-list workload](../../target/ink-messages-stress.png)

## Validation

The final source passes all six checks: `cargo build --all-targets --keep-going`,
`cargo test --tests --no-fail-fast`, `cargo clippy --all-targets --keep-going -- -D
warnings`, `cargo fmt --all -- --check`, `bun scripts/caps.mjs`, `bun
scripts/boot.mjs`, and `cargo check --workspace --all-targets --keep-going`.
The default-member test run reports 7,016 passed and 60 ignored across 111 test
binaries. `cargo test -p exact-linux --lib --tests --no-fail-fast` reports 1,875
passed and 32 ignored across 14 test binaries. The three measurement tests are
among the ignored tests. The additional `cargo clippy -p exact-linux --lib --tests
--keep-going -- -D warnings` passes.

The requested filtered suites were run with `EXACT_REQUIRED=1`, which refuses a
missing GPU or Ink backend. The test-result lines are copied verbatim:

paint (`--test pinned -- paint`):

```text
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 1.96s
```

borders (`--test pinned -- borders`):

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.38s
```

tint (`--test pinned -- tint`):

```text
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.35s
```

backdrop (`--test pinned -- backdrop`):

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.76s
```

gradients (`--test pinned -- gradients`):

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 0.36s
```

visual (`--test pinned -- visual`):

```text
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 87 filtered out; finished in 0.88s
```

materials (`--test pinned -- materials`):

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 1.08s
```

image (`--test it -- image`):

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.62s
```

Optimized series:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1499 filtered out; finished in 605.35s
```

## C5: direct Metal measured against wgpu

### D15 verdict: the frame-time alternative passes; startup regresses

**D meets the 10% frame-time margin on Shelf and Excord. It does not meet it on desktop Messages Stress, and it loses on uncached startup.** At 1920×1080, Shelf improves by 21.32–27.11% and Excord by 24.43–29.59%. Every one of those five-run ranges is separated from C’s. Messages Stress improves by only 3.53–6.59%; its scroll ranges overlap. This is a workload-specific frame-time result, not a claim that direct Metal is faster everywhere.

An empty application cache makes D’s constructor **162.048 ms [159.215–172.556]**, versus C’s **15.334 ms [14.556–15.993]**. Metal’s pipeline/archive phase takes 126.872 ms; populating a new binary archive is included, never moved outside the timer. With a retained archive that phase falls to 0.519 ms, but device creation still makes the whole constructor 31.699 ms versus C’s 15.564 ms. Direct Metal therefore does **not** earn a cold-start win. The startup regression must accompany any decision to keep the device layer.

D’s lifetime peak RSS and physical footprint are lower in every measured workload and size. C’s RSS spikes remain; the readback investigation below localizes them and names the remaining fix. This comparison does not reopen C4’s C-over-B verdict: B was not rerun here.

Both arms have zero of 21,000 redraws over 25 ms. Their maximum service times are 5.529 ms (C) and 3.715 ms (D). These are unpaced, synchronous headless redraws including readback, not presented frame intervals or input latency.

### Implementation and method

The renderer and harness are commit `62e64568e2` on `lane/ink`, after the C3f fixes that followed the C4 measurement. The final report also removes a disproved allocation comment; it changes no measured executable code. The machine, toolchains, `host-dev` profile, fonts, first screens, virtual clock, 300-frame modes, viewport sizes and five-round interleaving are those of C4. Arms C and D use the same Ink core, shader, glyph rasterizer and kept rows. C uses wgpu 30’s Metal backend; D uses objc2-metal 0.3.2 directly. The adapter field distinguishes `wgpu Metal` from `direct Metal`; wgpu remains the default.

`host/ink/build.rs` compiles naga’s MSL into an embedded `.metallib` for the Apple SDK/minimum-version pairs used by the Canvas module. `device/metal.rs`, `metal/setup.rs` and `metal/mips.rs` implement the device trait, premultiplied pipelines, uploads, a three-buffer instance ring, scissored instanced draws, copies, readback and completion-based retirement. Power-of-two mip chains use Metal’s blit encoder. Odd dimensions use the shared area-filter shader: a pixel test caught the blit’s 3→1 reduction selecting only the middle pixel. `device/selected.rs` and the Linux adapter select the layer at boot. No application shader source is compiled at launch.

The existing `presenter/kept_rows_tests/measurement.rs` harness adds arm D. Each sample is a fresh process. The series ran with no lane builds or other lane tests alongside it. Shelf and Excord used the same baked plan and bytecode SHA-256 hashes listed in C4; their scroll modes remain unavailable because the first screens have no 20-point scroll range.

For C5, macOS CPU samples use `getrusage` user-plus-system time and RSS samples use `proc_pid_rusage`, without spawning `ps`. Sampling still occurs every 20 frames. `INK_SAMPLE_PS=1` preserves the C4 sampler for the diagnostic comparison. Lifetime RSS and footprint remain `/usr/bin/time -l`’s fields. Both C and D use this same instrumentation; its CPU values are not quantized to C4’s 10 ms `ps` resolution.

Run the C4 fixture and bake commands above, then:

```sh
INK_ARM=D cargo test --profile host-dev -p exact-linux --lib \
  presenter::kept_rows_tests::measurement::series -- \
  --ignored --exact --nocapture --test-threads=1
```

The raw 220 records are `target/ink-c5-results.jsonl`; the session log is `target/ink-c5-series.log`. Raw-record SHA-256: `0875efc49af0b3357f20886b095c838bbcc342c2f0659ff9207120b9d13a5bb8`. There are 60 workload workers, 42,000 measured frames and 40 boot workers.

### Boot phases

Each cell is milliseconds, median [minimum–maximum] over ten fresh processes. “Empty” removes this arm’s application cache directory before the run; “retained” uses the archive that the preceding run wrote. The OS driver cache is not cleared. These are not first-ever launches on a new machine. Constructor time includes sampler/buffer setup and archive serialization; the pipeline phase includes library loading, pipeline creation and archive population, but excludes serialization.

wgpu’s Metal backend exposes no wgpu pipeline-cache feature here, so C reports `cached=false` for both requests. D reports `false` for empty and `true` for retained directories. Its flag means a persisted archive was successfully loaded and supplied to Metal; it is not a GPU-driver cache-hit counter.

| Arm | Application cache | Reported cached | Device | Pipelines/archive | Constructor |
|---|---|---|---:|---:|---:|
| C | empty | false | 12.931 [12.300–13.573] | 2.135 [2.103–2.270] | 15.334 [14.556–15.993] |
| D | empty | false | 29.573 [28.678–30.239] | 126.872 [124.375–137.271] | 162.048 [159.215–172.556] |
| C | retained | false | 13.188 [12.873–17.130] | 2.205 [2.123–2.728] | 15.564 [15.159–20.053] |
| D | retained | true | 31.085 [28.189–39.048] | 0.519 [0.477–0.547] | 31.699 [28.736–39.650] |

### Frame times

Milliseconds, median [minimum–maximum] of five per-run medians. Each arm in each row represents 1,500 frames. Percentage improvements use unrounded values.

| Workload | Size | Mode | C paint | D paint | D improvement | Ranges separated |
|---|---|---|---:|---:|---:|---|
| Messages Stress | 390×844 | still | 0.585 [0.573–0.617] | 0.529 [0.521–0.579] | 9.65% | no |
| Messages Stress | 390×844 | scroll | 0.623 [0.593–0.738] | 0.566 [0.554–0.576] | 9.15% | yes |
| Messages Stress | 390×844 | text | 0.672 [0.648–0.701] | 0.570 [0.560–0.598] | 15.13% | yes |
| Messages Stress | 1920×1080 | still | 1.227 [1.205–1.331] | 1.159 [1.152–1.201] | 5.50% | yes |
| Messages Stress | 1920×1080 | scroll | 1.298 [1.258–1.354] | 1.252 [1.238–1.280] | 3.53% | no |
| Messages Stress | 1920×1080 | text | 1.288 [1.270–1.358] | 1.203 [1.190–1.210] | 6.59% | yes |
| Shelf | 390×844 | still | 0.502 [0.481–0.532] | 0.369 [0.367–0.474] | 26.41% | yes |
| Shelf | 390×844 | text | 0.561 [0.545–0.576] | 0.416 [0.411–0.450] | 25.75% | yes |
| Shelf | 1920×1080 | still | 1.124 [1.045–1.147] | 0.820 [0.811–1.032] | 27.11% | yes |
| Shelf | 1920×1080 | text | 1.100 [1.085–1.213] | 0.866 [0.860–0.872] | 21.32% | yes |
| Excord | 390×844 | still | 0.492 [0.472–0.534] | 0.444 [0.326–0.468] | 9.90% | yes |
| Excord | 390×844 | text | 0.609 [0.582–0.610] | 0.455 [0.447–0.519] | 25.25% | yes |
| Excord | 1920×1080 | still | 1.091 [1.027–1.165] | 0.768 [0.758–0.914] | 29.59% | yes |
| Excord | 1920×1080 | text | 1.162 [1.117–1.200] | 0.878 [0.861–0.902] | 24.43% | yes |

### Encode, readback, CPU and sampled RSS

Timing cells are milliseconds, median [minimum–maximum] across five runs; CPU is process time per frame. RSS is MiB. Encode excludes the tree walk and glyph preparation. Readback includes GPU completion and the CPU pixel copy, so it does not isolate shader execution.

| Workload | Width | Mode | Arm | Encode | Readback | CPU/frame | Sampled peak RSS |
|---|---:|---|---|---:|---:|---:|---:|
| Messages Stress | 390 | still | C | 0.103 [0.098–0.123] | 0.313 [0.306–0.320] | 0.523 [0.504–0.554] | 54.1 [54.0–56.7] |
| Messages Stress | 390 | still | D | 0.018 [0.018–0.024] | 0.351 [0.347–0.374] | 0.405 [0.398–0.451] | 49.6 [49.4–52.0] |
| Messages Stress | 390 | scroll | C | 0.104 [0.100–0.132] | 0.346 [0.325–0.399] | 0.578 [0.557–0.713] | 58.4 [55.8–58.9] |
| Messages Stress | 390 | scroll | D | 0.019 [0.018–0.020] | 0.379 [0.373–0.384] | 0.453 [0.441–0.466] | 50.5 [50.1–52.8] |
| Messages Stress | 390 | text | C | 0.105 [0.099–0.114] | 0.366 [0.357–0.401] | 0.614 [0.589–0.627] | 58.5 [57.2–59.0] |
| Messages Stress | 390 | text | D | 0.019 [0.018–0.021] | 0.359 [0.353–0.371] | 0.468 [0.436–0.503] | 53.0 [50.5–53.2] |
| Messages Stress | 1920 | still | C | 0.136 [0.126–0.151] | 0.902 [0.880–0.979] | 1.080 [1.037–1.152] | 68.4 [67.8–68.7] |
| Messages Stress | 1920 | still | D | 0.023 [0.023–0.027] | 0.951 [0.948–0.977] | 0.913 [0.904–0.964] | 64.3 [64.0–65.0] |
| Messages Stress | 1920 | scroll | C | 0.139 [0.132–0.146] | 0.942 [0.915–0.988] | 1.173 [1.124–1.202] | 70.8 [69.8–71.0] |
| Messages Stress | 1920 | scroll | D | 0.027 [0.025–0.028] | 1.018 [1.011–1.039] | 1.042 [1.012–1.057] | 66.5 [66.1–66.8] |
| Messages Stress | 1920 | text | C | 0.129 [0.126–0.135] | 0.940 [0.920–1.011] | 1.162 [1.136–1.201] | 70.9 [70.4–71.1] |
| Messages Stress | 1920 | text | D | 0.023 [0.023–0.024] | 0.964 [0.955–0.967] | 0.960 [0.942–0.965] | 66.7 [66.2–66.9] |
| Shelf | 390 | still | C | 0.085 [0.080–0.086] | 0.329 [0.319–0.364] | 0.464 [0.444–0.486] | 58.1 [57.7–59.8] |
| Shelf | 390 | still | D | 0.015 [0.014–0.017] | 0.276 [0.276–0.364] | 0.274 [0.269–0.299] | 54.6 [53.5–55.8] |
| Shelf | 390 | text | C | 0.086 [0.081–0.087] | 0.353 [0.337–0.360] | 0.533 [0.514–0.586] | 59.6 [57.9–60.0] |
| Shelf | 390 | text | D | 0.015 [0.014–0.015] | 0.280 [0.277–0.313] | 0.332 [0.329–0.339] | 54.7 [53.6–56.0] |
| Shelf | 1920 | still | C | 0.092 [0.087–0.098] | 0.921 [0.872–0.948] | 0.989 [0.926–1.023] | 80.5 [80.2–85.4] |
| Shelf | 1920 | still | D | 0.017 [0.017–0.020] | 0.714 [0.708–0.890] | 0.656 [0.634–0.696] | 80.4 [75.8–80.7] |
| Shelf | 1920 | text | C | 0.085 [0.084–0.098] | 0.888 [0.873–0.949] | 1.014 [0.986–1.088] | 85.0 [80.8–104.7] |
| Shelf | 1920 | text | D | 0.017 [0.016–0.017] | 0.723 [0.719–0.727] | 0.697 [0.688–0.705] | 80.5 [76.0–80.8] |
| Excord | 390 | still | C | 0.084 [0.079–0.088] | 0.368 [0.341–0.406] | 0.444 [0.421–0.482] | 63.2 [60.5–63.3] |
| Excord | 390 | still | D | 0.015 [0.014–0.018] | 0.389 [0.276–0.410] | 0.234 [0.227–0.278] | 58.6 [58.0–59.0] |
| Excord | 390 | text | C | 0.082 [0.078–0.087] | 0.352 [0.343–0.369] | 0.581 [0.546–0.603] | 63.5 [62.0–64.8] |
| Excord | 390 | text | D | 0.014 [0.014–0.015] | 0.284 [0.279–0.326] | 0.385 [0.349–0.392] | 59.1 [58.5–59.4] |
| Excord | 1920 | still | C | 0.092 [0.081–0.099] | 0.938 [0.896–1.000] | 0.972 [0.889–1.020] | 84.2 [83.8–89.9] |
| Excord | 1920 | still | D | 0.017 [0.015–0.019] | 0.701 [0.691–0.836] | 0.590 [0.580–0.627] | 83.6 [79.9–83.8] |
| Excord | 1920 | text | C | 0.084 [0.081–0.087] | 0.917 [0.860–0.934] | 1.056 [1.011–1.070] | 84.4 [84.0–90.1] |
| Excord | 1920 | text | D | 0.015 [0.015–0.016] | 0.708 [0.690–0.723] | 0.715 [0.693–0.727] | 83.7 [80.0–83.9] |

### Lifetime memory

MiB, median [minimum–maximum] over five workers; includes CPU boot and every available mode. Physical footprint includes memory that is not charged as the process’s resident set.

| Workload | Width | Arm | Peak RSS | Peak footprint |
|---|---:|---|---:|---:|
| Messages Stress | 390 | C | 60.4 [57.5–60.9] | 377.8 [375.7–381.8] |
| Messages Stress | 390 | D | 53.1 [50.8–53.5] | 224.8 [222.4–225.1] |
| Messages Stress | 1920 | C | 78.7 [70.7–98.6] | 410.2 [403.2–429.3] |
| Messages Stress | 1920 | D | 66.8 [66.4–67.0] | 239.8 [239.3–241.4] |
| Shelf | 390 | C | 61.1 [58.6–61.8] | 375.0 [372.5–375.5] |
| Shelf | 390 | D | 54.7 [53.6–56.0] | 222.9 [221.8–224.1] |
| Shelf | 1920 | C | 89.6 [85.1–117.4] | 416.8 [408.8–438.0] |
| Shelf | 1920 | D | 80.6 [76.0–80.8] | 249.9 [245.3–250.2] |
| Excord | 390 | C | 64.9 [63.5–73.8] | 379.1 [376.3–387.0] |
| Excord | 390 | D | 59.2 [58.5–59.5] | 227.3 [226.6–227.6] |
| Excord | 1920 | C | 93.8 [91.0–123.4] | 419.8 [412.0–448.3] |
| Excord | 1920 | D | 83.8 [80.1–84.0] | 253.0 [249.3–253.2] |

All measured frames still have zero overflow atlas bytes and zero layer-pool targets. They do not qualify backdrop-heavy or deeply clipped workloads.

### Where the RSS spikes occur

Temporary phase probes sampled current and lifetime peak RSS before and after device setup, readback allocation, mapping, the CPU pixel copy and the harness’s `ps` calls. Three fresh 1920×1080 Messages Stress workers were interleaved for each sampler. The probes were removed before the C/D series. Their logs are `target/ink-c5-memory-{0,1,2}-{0,1}.log`; these diagnostic runs are not timing samples.

With no `ps` children, lifetime RSS reached **100.3–103.1 MiB**. With the original `ps` sampler it reached **75.9–83.3 MiB**. Thus children are not required for the spike; changing the sampler also changes the pauses between frames and the observed peak. Device setup ended at about 55 MiB and the first completed frame at about 66 MiB. Later peak increases occur inside the CPU pixel copy, often by one 1920×1080 RGBA image (7.91 MiB) at a time. They do not occur at pipeline creation or during first glyph rasterization. The separate physical-footprint rise during first-frame atlas allocation is already present before these later RSS spikes.

Zero-initializing the tightly packed output instead of allocating capacity and extending each row **did not remove the peaks**: three further runs reached 101.1–104.6 MiB without `ps`, and 76.1–79.3 MiB with it (`target/ink-c5-memory-fixed-*.log`). The C/D series uses this allocation in C and records its remaining peaks, including 123.4 MiB for Excord. The diagnostic establishes the readback-copy phase, not which allocator or driver retention policy keeps every page resident.

**Remaining fix:** reuse the presenter’s CPU pixmap and the device’s staging allocation instead of allocating a new full-frame output and staging buffer for each readback, then rerun the memory protocol. This requires the readback/Backend ownership seam to return the old output for reuse; it is recorded in `QUEUE.md`. No claim is made that C4’s memory concern is resolved.

### Validation

All six checks pass: root build, tests, Clippy and fmt; caps; boot; workspace/all-targets check. The root tests report 7,016 passed and 60 ignored across 111 binaries. Caps includes the wasm32 check. `cargo test -p exact-ink --lib --no-fail-fast` reports 19 passed. Extra Clippy for Ink and Linux passes.

`cargo test -p exact-linux --lib --tests --no-fail-fast` passes with both the default wgpu device and `EXACT_INK_DEVICE=metal`: 1,887 passed and 32 ignored across 14 binaries in each run. Filtered pinned runs use `EXACT_REQUIRED=1`, which refuses to silently omit Ink. Result lines below are verbatim.

| Device | Suite | Result |
|---|---|---|
| metal | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 3.30s` |
| metal | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.60s` |
| metal | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.56s` |
| metal | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 1.16s` |
| wgpu | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 2.02s` |
| wgpu | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.80s` |
| wgpu | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.76s` |
| wgpu | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 1.25s` |
| wgpu | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.78s` |
| wgpu | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 1.27s` |
| wgpu | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 1.57s` |
| wgpu | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.72s` |

Caltrain was built in `host-dev` and driven with `EXACT_PAINTER=ink EXACT_INK_DEVICE=metal bun scripts/smoke.mjs linux --app-only --shot target/ink-c5-caltrain.png`. Its three contract tests passed, the smoke completed, and the screenshot was inspected. The app drive exercises the headless Linux presenter on macOS; it is not a native window-carrier qualification.

Optimized series:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1502 filtered out; finished in 271.87s
```

Only macOS on this Apple GPU was measured at runtime. iOS/tvOS and discrete-GPU managed storage were not exercised. Presented frame intervals, input latency, energy and a machine’s first-ever driver-cache-cold launch remain unmeasured.

## NucBox

**The composition window and RichEdit spike work on the NucBox.** Direct3D 11
opens its hardware device in 48.101 ms and builds its DXBC pipeline in
2.703 ms, versus 138.590 ms and 457.330 ms for wgpu DirectX 12. Shelf's
1024×720 first complete screen reaches DXGI display at 406.236 ms. Its
composition setup costs 1.812 ms. These are ten-run medians.

The window path avoids GPU readback and CPU upload. It is not an across-the-board
device win: at 1920×1080, C4's headless scrolling frame costs 3.920 ms on
Direct3D 11 versus 2.718 ms on wgpu DirectX 12, a **44.26% regression**.
The Direct3D readback phase accounts for most of that difference. D5.2 requires
this Windows device regardless; these results establish the spike, not a
complete S2 replacement qualification.

C6 runs on the physical NucBox: AMD Ryzen AI Max+ 395, Radeon 8060S,
Windows 11 Pro 10.0.26200, display driver 32.0.23033.1002. The attached
5120×1440 display runs at 60 Hz and 1× scaling. C: had 305,256,144,896
bytes free before building. Rust is the repository's pinned 1.97.0 MSVC
toolchain; Bun is 1.4.2. DXBC was compiled by Windows SDK 10.0.26100.0
`fxc` during the build. No Direct3D shader source is compiled by the app.

### What ran in the window

Shelf's 1024×720 first screen ran through the Ink adapter and Direct3D 11,
with a two-buffer, premultiplied BGRA `FLIP_DISCARD` composition swap chain,
a frame-latency waitable object and the system Mica backdrop. The screenshot
shows the backdrop through the sidebar and frame.

[The captured Shelf window and typed RichEdit field](../../target/ink-c6-shelf-window.png)
show a windowless RichEdit texture in the same window. Windows keyboard input
entered “typed on the NucBox”. `CreateTextServices` and `TxDrawD2D` draw into
a BGRA render-target texture on the same D3D11 device; Ink samples that texture
as an image. The opt-in spike field is enabled by `EXACT_INK_RICHEDIT`; it is
not yet the implementation of Shelf's search field. The desktop test also
checks the returned text, visible glyph pixels and transparent background.
Selection UI, IME placement, accessibility and binding the editor to Contract
fields remain S2 work.

The NucBox accepts the requested composition `FLIP_DISCARD` swap effect.
Microsoft's [CreateSwapChainForComposition documentation](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition)
still specifies `FLIP_SEQUENTIAL`; the result here qualifies this Windows
11 driver combination, not older Windows releases. An SSH session alone
cannot open its composition display. Window tests and captures ran as the
logged-in user's interactive desktop task.

### Timing definitions

Device and pipeline measurements use fresh processes. Direct3D device time
includes `D3D11CreateDevice` and the adapter description; hardware falls back
to WARP only on failure, while the WARP arm forces the software driver.
Pipeline time includes creating shaders and state objects from embedded DXBC.
The control explicitly selects wgpu 30 DirectX 12 and asserts that API in its
boot worker. Its device time includes instance, adapter and device creation,
and its shader time includes wgpu's translation and pipeline creation.

Window startup identifies Shelf's complete first screen by its painted
`library-count` marker, using `EXACT_AGENT_WINDOW_READY=library-count`.
Unrelated background requests do not postpone that marker. Process creation
comes from `GetProcessTimes`. The Direct3D displayed timestamp uses
[DXGI frame statistics](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/ns-dxgi1_3-dxgi_frame_statistics_media)
and QPC. The common comparison with softbuffer uses process age after
[DwmFlush](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush):
a synchronization proxy, not an exact per-frame scanout timestamp.
A return from `Present` is reported separately. These are platform timestamps,
not photodiode measurements.

Ten rounds interleave the arms, rotating their order each round. No compiler
runs during the series. Application storage is isolated per process; ordinary
desktop applications and services remain running. These are fresh-process
samples after development and diagnostic runs, not the machine's first-ever
driver-cache-cold launch. All boot samples report `cached=false`; that describes
the renderer's explicit cache, not the driver's internal caches.

Device startup and C4 use `host-dev` (optimization 3, 16 codegen units, no
LTO, debug assertions disabled). Shelf uses `host/windows/build.mjs`'s normal
`gpu-dev` build (workspace optimization 1, dependencies 3, debug assertions
enabled). The tables keep these profiles separate. The measurement worker
and device boot binaries are the final optimized build of commit `910ca295e4`;
the Shelf executable is built from that commit too.

Each available mode measures 300 redraws. Shelf's carrier forces still redraws
and paces them at the monitor's 60 Hz. Its first screen has no scroll range.
C4 uses Messages Stress's paused, virtualized 1,000-message history, pinned
DejaVu Sans, fixed virtual time and the same asserted 20-point wheel steps
and row-text updates as the Mac protocol. `INK_WIDTH` and `INK_HEIGHT` select
1024×720 or 1920×1080 at 1×; `INK_WINDOW=1 INK_ARM=D` attaches a real composition
window. C and D without `INK_WINDOW` retain C4's synchronous readback protocol.

C4 window paint is wall time in `window_frame` minus the device's present
time. The latter includes the frame-latency wait and `Present(1)`; the unpaced
worker spends most of each refresh interval there. Shelf's carrier has already
paced the draw, so its present wait is much shorter. Neither is GPU execution
time. The headless table keeps the complete frame cost, including readback.
CPU uses `GetProcessTimes`; working-set memory uses `GetProcessMemoryInfo`.

Tables report the median of ten per-run medians, with minimum–maximum in
brackets. Percentages use unrounded values. Startup cells use ten individual
process measurements. Shelf's
carrier reports p50/p95 rounded to 0.01 ms; C4 retains each raw frame sample.
The raw logs and derived values are in `target/ink-c6-nucbox/`.

### Device startup, host-dev

Milliseconds, ten fresh processes per arm. No application-owned pipeline cache
was loaded in any arm. The hardware arm reports Radeon 8060S; forced WARP
reports Microsoft Basic Render Driver.

| Device | Device creation | Pipeline creation |
|---|---:|---:|
| D3D11 hardware | 48.101 [44.994–67.088] | 2.703 [2.439–2.852] |
| D3D11 WARP | 14.169 [12.122–22.590] | 3.142 [2.795–3.616] |
| wgpu DirectX 12 | 138.590 [132.968–240.469] | 457.330 [442.482–728.803] |

### Shelf startup, gpu-dev

The composition setup includes the swap chain, composition device/target/visual
and backdrop attributes. First-screen timings start at process creation.
A dash means that the softbuffer path has no app-owned DXGI displayed-frame
timestamp; it is not a zero-time result.

| Pixels | Painter and carrier | Composition setup | DWM synchronization proxy | DXGI displayed first screen |
|---|---|---:|---:|---:|
| 1024×720 | CPU / softbuffer | — | 342.250 [321.624–510.227] | — |
| 1024×720 | Ink DX12 / readback / softbuffer | — | 1156.401 [1027.982–1390.895] | — |
| 1024×720 | Ink D3D11 / DirectComposition | 1.812 [1.723–2.476] | 393.297 [381.811–493.200] | 406.236 [394.713–506.196] |
| 1920×1080 | CPU / softbuffer | — | 334.646 [325.226–392.451] | — |
| 1920×1080 | Ink DX12 / readback / softbuffer | — | 1113.303 [1056.257–1280.340] | — |
| 1920×1080 | Ink D3D11 / DirectComposition | 2.123 [1.761–2.796] | 393.834 [376.437–434.162] | 411.086 [390.044–446.967] |

Paired differences use each round's DWM proxy, not the difference between two
independent medians. They compare complete paths, including device creation,
not the swap chain in isolation. Positive values are added startup time.

| Pixels | D3D11 minus today's CPU/softbuffer | D3D11 minus wgpu/readback/softbuffer |
|---|---:|---:|
| 1024×720 | 45.822 [-113.059–171.576] | -753.673 [-1002.612 to -555.174] |
| 1920×1080 | 48.646 [33.454–101.688] | -701.178 [-876.693 to -661.847] |

All 20 Direct3D first-screen timestamps were confirmed against the swap chain's
last present ID. Statistics sometimes lag the first DWM synchronization; the
measurement waits for that ID. The DWM proxy can precede the corresponding
DirectComposition display by a refresh interval. Exact softbuffer scanout time
and its exact on-glass difference from D3D11 remain unmeasured.

### Shelf still redraws, gpu-dev

Each cell is the median and range of ten per-run p50 values, 299 post-first-frame
samples per run. Paint for the wgpu path includes readback; present for the two
softbuffer paths includes CPU conversion and upload.

| Pixels | Path | Paint ms | Present ms | Frame interval ms |
|---|---|---:|---:|---:|
| 1024×720 | CPU / softbuffer | 3.320 [3.260–3.410] | 1.085 [1.080–1.100] | 16.620 [16.600–16.660] |
| 1024×720 | wgpu DX12 / softbuffer | 3.735 [3.170–4.520] | 1.100 [1.080–1.120] | 16.625 [16.600–16.640] |
| 1024×720 | D3D11 / DirectComposition | 0.420 [0.380–0.460] | 0.350 [0.310–0.400] | 16.610 [16.580–16.630] |
| 1920×1080 | CPU / softbuffer | 6.570 [6.460–6.620] | 2.970 [2.950–2.990] | 16.600 [16.580–16.640] |
| 1920×1080 | wgpu DX12 / softbuffer | 5.105 [4.290–5.570] | 2.965 [2.950–2.990] | 16.620 [16.610–16.650] |
| 1920×1080 | D3D11 / DirectComposition | 0.405 [0.390–0.490] | 0.345 [0.310–0.360] | 16.625 [16.600–16.640] |

### C4 in the composition window, host-dev

Messages Stress, 3,000 frames per cell across ten runs. Present includes waiting
for the next refresh, so its roughly 16 ms duration is not rendering work.

| Pixels | Mode | Paint excluding present ms | Present including wait ms | Process CPU ms/frame |
|---|---|---:|---:|---:|
| 1024×720 | still | 0.627 [0.603–0.648] | 16.028 [16.009–16.066] | 1.458 [1.198–1.771] |
| 1024×720 | scroll | 0.583 [0.499–0.623] | 15.912 [15.861–15.988] | 1.745 [1.458–2.188] |
| 1024×720 | text | 0.615 [0.544–0.651] | 15.950 [15.921–16.072] | 1.641 [1.302–1.979] |
| 1920×1080 | still | 0.625 [0.557–0.682] | 16.033 [16.000–16.105] | 1.562 [1.094–1.719] |
| 1920×1080 | scroll | 0.673 [0.636–0.785] | 15.812 [15.723–15.847] | 1.927 [1.667–2.240] |
| 1920×1080 | text | 0.642 [0.590–0.705] | 15.952 [15.879–16.010] | 1.823 [1.562–2.188] |

### C4 with synchronous readback, host-dev

The same workload and protocol through each device, with no window. This
is the device comparison; it does not credit Direct3D for removing readback.
Readback includes GPU completion and CPU copying, not just transfer time.

| Pixels | Mode | Device | Whole frame ms | Encode ms | Readback ms |
|---|---|---|---:|---:|---:|
| 1024×720 | still | wgpu DX12 | 1.408 [1.390–1.420] | 0.328 [0.321–0.336] | 0.702 [0.692–0.714] |
| 1024×720 | still | D3D11 | 1.475 [1.399–1.565] | 0.046 [0.044–0.050] | 1.057 [1.003–1.099] |
| 1024×720 | scroll | wgpu DX12 | 1.426 [1.400–1.463] | 0.339 [0.331–0.351] | 0.706 [0.692–0.728] |
| 1024×720 | scroll | D3D11 | 1.595 [1.573–1.657] | 0.049 [0.047–0.051] | 1.128 [1.113–1.159] |
| 1024×720 | text | wgpu DX12 | 1.449 [1.431–1.467] | 0.333 [0.327–0.358] | 0.709 [0.694–0.716] |
| 1024×720 | text | D3D11 | 1.570 [1.520–1.621] | 0.046 [0.045–0.047] | 1.085 [1.060–1.119] |
| 1920×1080 | still | wgpu DX12 | 2.725 [2.668–2.823] | 0.414 [0.400–0.435] | 1.649 [1.624–1.673] |
| 1920×1080 | still | D3D11 | 3.311 [3.118–3.868] | 0.054 [0.053–0.058] | 2.646 [2.465–3.213] |
| 1920×1080 | scroll | wgpu DX12 | 2.718 [2.666–2.796] | 0.410 [0.399–0.421] | 1.632 [1.620–1.656] |
| 1920×1080 | scroll | D3D11 | 3.920 [3.312–4.115] | 0.059 [0.057–0.067] | 3.238 [2.570–3.290] |
| 1920×1080 | text | wgpu DX12 | 2.731 [2.620–2.796] | 0.399 [0.375–0.421] | 1.626 [1.607–1.637] |
| 1920×1080 | text | D3D11 | 3.922 [3.253–3.988] | 0.053 [0.053–0.055] | 3.250 [2.558–3.269] |

The series completed **150 successful processes**: 30 device boots, 60 Shelf
windows and 60 C4 workers. C4 contains 54,000 frame samples, including 18,000
composition-window frames. Shelf contributes 18,000 presented redraws. The raw
logs, executable hashes, PIDs, exit statuses and derived medians are preserved
in `target/ink-c6-nucbox/`; `summary.json` contains the table values.

The first-ever machine/driver-cold launch, exact softbuffer scanout timestamps,
energy, other Windows drivers and DPI scales were not qualified. A shared
desktop remains noisy: the startup ranges include every run, including the
slow CPU sample. This spike does not qualify RichEdit IME, selection or
accessibility, nor layer-heavy workloads absent from these first screens.

### Reproduce on Windows

Use the logged-in desktop, with the Windows SDK's `fxc` on `PATH`. Build the
workers before timing; run each arm in a fresh process and rotate their order
over ten rounds. The C4 window requires `INK_ARM=D`; omit `INK_WINDOW` for
headless readback, and select `INK_ARM=C` for wgpu DirectX 12.

```powershell
cargo test --profile host-dev -p exact-linux -p exact-ink --lib --no-run
$env:INK_BOOT_DEVICE='d3d11' # also warp and wgpu
cargo test --profile host-dev -p exact-ink --lib device::d3d11::tests::boot_worker -- --ignored --exact --nocapture --test-threads=1

$env:WGPU_BACKEND='dx12'
$env:INK_APP='messages-stress'; $env:INK_ARM='D'; $env:INK_WINDOW='1'
$env:INK_WIDTH='1024'; $env:INK_HEIGHT='720' # also 1920 and 1080
cargo test --profile host-dev -p exact-linux --lib presenter::kept_rows_tests::measurement::worker -- --ignored --exact --nocapture --test-threads=1

bun host/windows/build.mjs shelf
$env:EXACT_PAINTER='ink'; $env:EXACT_INK_DEVICE='d3d11'
$env:EXACT_SIZE='1024x720'; $env:EXACT_AGENT_WINDOW_FRAMES='300'
$env:EXACT_AGENT_WINDOW_READY='library-count'
$env:EXACT_AGENT_STORAGE='ink-c6-example'; $env:EXACT_AGENT_STORAGE_FRESH='1'
& .\apps\shelf\dist-windows\Shelf.exe
```

For the field demonstration, set `EXACT_INK_RICHEDIT` to its initial text before
launching Shelf and type into the window. The measurements leave this unset.
For the two softbuffer controls, select `EXACT_PAINTER=cpu` or
`EXACT_PAINTER=ink EXACT_INK_DEVICE=wgpu WGPU_BACKEND=dx12`.

### C6 validation

The Mac's six checks pass, including 7,016 root tests (60 ignored), the
workspace/all-targets compilation and the caps check's 29 wasm32 crates and
one binary. The separate Linux-host suite passes 1,887 tests (32 ignored);
Ink passes 19. The named Mac suites report:

| Suite | Result |
|---|---|
| paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 1.61s` |
| borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.53s` |
| tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.47s` |
| backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.90s` |
| gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.49s` |
| visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 1.02s` |
| materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 1.22s` |
| image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.56s` |

On the NucBox, `cargo build -p exact-windows` succeeds. The native suites
report the following. The pinned run sets `EXACT_REQUIRED=1` and
`EXACT_INK_DEVICE=d3d11`, so it cannot silently omit the device.

```text
exact-windows: test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
exact-ink: test result: ok. 19 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.64s
pinned: test result: ok. 102 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 12.32s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.83s
desktop RichEdit: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.34s
```

The native pinned tests include CPU/Ink parity, borders, tint, backdrop,
gradients, visual and materials. The desktop RichEdit test is explicitly
ignored in ordinary runs because it needs a logged-in composition display.
MSVC cross-compilation of Ink's tests and GNU cross-compilation of the
Windows carrier also pass on the Mac; the missing-`fxc` build stub names the
missing DXBC if opened at runtime.

## S2.1

**Windows Ink still defaults to wgpu.** The overlapped composition path exceeds
the 10 ms first-screen allowance at both sizes. Lock/unlock is also unqualified
because no unlock operator was confirmed; that journey was not attempted.

The S2.1 window qualification uses the same NucBox, display, Windows SDK and
profiles as C6. Shelf's manifest selects Ink; the desktop journeys select
`EXACT_INK_DEVICE=d3d11`. The window remains on the attached 5120×1440 display;
the DPI journey changes Settings from 100% to 150% and back through UI Automation.
The executed journeys run as native ignored tests in the signed-in desktop session.

### Implementation and measurement boundary

`device/d3d11.rs` prepares the D3D11 device, immediate context and DXBC pipeline
on a worker while the presenter loads fonts and performs the first layout.
The paint thread joins before installing the backend. Only COM objects cross
that boundary; the renderer's `Rc` resources are created on the receiving
thread. No two threads use the immediate context concurrently. This follows
[Microsoft's D3D11 threading model](https://learn.microsoft.com/en-us/windows/win32/direct3d11/overviews-direct3d-11-render-multi-thread-intro).

The composition timer now separates DXGI factory/swap-chain setup from the
DirectComposition device, target, visual and commit. Device and pipeline timers
still measure their full work; the join timer measures the remaining wait and
paint-thread resource initialization. First-present duration comes from the
window carrier. Shader objects use build-time DXBC; the application compiles no
shader source at startup.

A forced TDR also interrupted DWM. Recovery now establishes a CPU frame through
WARP composition before retrying hardware, and marks presentation successful
only after the CPU frame has been presented. The native test waits for that
success, then for hardware recovery and its backoff. It uses `dxcap -forcetdr`,
not an injected backend error.

The serial baseline is `8de5b7c264` with only the SDK input-watcher correction:
`build.rs` no longer registers every directory on `PATH` as source input. That
watcher pulled Rust's standard libraries into the app's captured-source check
and prevented the Windows bake. The correction changes no renderer behavior.
The measured candidate is `766fb4f015`; subsequent code commits change only
the desktop fixtures' activation after Settings and display-wake handling. Executable SHA-256:

| Shelf executable | SHA-256 |
|---|---|
| Serial baseline | `63b078e24c0e188932d25a27704b7a822c9763086dd7eb5f1085cafdc6dd4ce4` |
| Overlapped candidate | `546c492e21c01122882bfce10c50b6b699c610b6dd47551b700b676bed37a331` |

### Desktop journeys

Captures read the actual desktop client pixels with GDI after compositor
synchronization and stable captures. They wait through opening, restore and
DPI animations. Settings can retain Windows' foreground permission; in that
case the test activates Shelf with a title-bar click. Comparisons use a fresh
Shelf process at the same position and client size. The band is mean maximum
RGB error below 5/255 and fewer than 6% of pixels with error above 32/255.

| Journey | Result | Mean error /255 | Pixels above 32/255 |
|---|---|---:|---:|
| Drag-resize 1024×720 → 800×600 → 1280×900 → 1920×1080 → 1024×720 | Passed, real mouse drags | 0 | 0% |
| Minimize and restore | Passed | 0 | 0% |
| Focused field left idle | Passed; three transitions, 20 changed caret pixels | 0 | 0% |
| Settings 100% → 150% → 100% | Passed; HWND DPI 96 → 144 → 96 | 0.000071 | 0% |
| Forced device loss | Passed; CPU composition followed by hardware after 1,020.010 ms | 0 | 0% |
| Sleep and wake | Passed after the physical 5120×1440 desktop returned | 3.049929 | 0% |
| Lock and unlock | Not attempted; no unlock operator confirmed | — | — |

The resize log contains 77 frames: its 49.84 ms median interval follows the
test's 50 ms input polling, with paint p50/p95 of 1.84/2.36 ms. The idle caret's
last two transitions are 530.399 and 530.321 ms apart. Minimize/restore includes
an intentional 367.07 ms p95 interval. TDR includes a 5,279.21 ms frame gap and
a 4,820.49 ms failed paint while the compositor is unavailable; those are
recorded outages, not steady-state frame-time claims. After recovery the window
matches its fresh-launch reference. No ordinary journey reported device loss
or a resize error.

The sleep test enables its own `SeShutdownPrivilege` and calls
`SetSuspendState` with a 15-second resume timer. Windows records sleep and
resume events; the successful call returned after 74,742 ms, not 15 seconds.
The Modern Standby exit event names `Power Button`; the wake trigger was not
independently verified, so this does not qualify timer-only unattended wake.
A timer can resume Windows while the physical display remains asleep. The test
wakes the display with mouse input and `SetThreadExecutionState`, then requires
the original 5120×1440 desktop before comparing frames. An earlier run compared
against Windows' temporary 640×480 desktop and failed; it is not counted as a
successful journey. The final test takes 84.49 seconds. No sleep, wake or sign-in
policy is changed. This qualifies this machine's Modern Standby path, not S3.

### Startup, before and after

Ten rounds at each size rotate CPU/softbuffer, serial D3D11/composition and
overlapped D3D11/composition. There are 60 successful fresh processes, each
running 120 frames, with isolated application storage. All builds and other
GPU tests finish before sampling. The desktop's ordinary applications remain
running. These are warm-machine, fresh-process measurements; no driver-cache-
cold claim is made. The first complete screen is Shelf's `library-count` marker.

Milliseconds, median [minimum–maximum]. DWM is the same process-age
synchronization proxy used in C6. DXGI confirms the corresponding present ID
for all 40 Direct3D first screens. The CPU path has no DXGI display timestamp.

| Pixels | Path | DWM first-screen proxy | DXGI displayed first screen | Total composition setup |
|---|---|---:|---:|---:|
| 1024×720 | CPU / softbuffer | 174.082 [161.085–199.465] | — | — |
| 1024×720 | Serial D3D11 / composition | 230.032 [224.686–324.484] | 242.659 [237.674–337.140] | 2.064 [1.825–2.571] |
| 1024×720 | Overlapped D3D11 / composition | 201.091 [197.286–213.908] | 214.413 [209.673–227.023] | 2.018 [1.955–2.656] |
| 1920×1080 | CPU / softbuffer | 182.991 [176.463–190.043] | — | — |
| 1920×1080 | Serial D3D11 / composition | 230.890 [226.176–240.632] | 243.673 [239.014–253.361] | 2.050 [1.868–2.795] |
| 1920×1080 | Overlapped D3D11 / composition | 200.101 [196.439–228.877] | 213.012 [209.012–240.566] | 1.905 [1.790–2.134] |

Paired differences subtract the CPU run from the same round. They include the
whole path, not just composition creation. The new series replaces C6's older
46–49 ms comparison for this decision; C6's absolute startup times are not
mixed into it.

| Pixels | Before minus CPU | After minus CPU | Gate |
|---|---:|---:|---|
| 1024×720 | 59.237 [49.757–125.019] | 26.751 [-1.737–45.183] | Fails ≤10 ms |
| 1920×1080 | 48.904 [38.371–54.691] | 17.072 [6.396–49.256] | Fails ≤10 ms |

The candidate's phases are below. Device and pipeline work overlap the fonts
and layout; do not add their full times to the join time. Total composition
also includes backdrop attributes and the initial statistics query. First
present is the carrier's duration for frame one, which can precede the complete
first screen. Its first paint includes initial text/atlas work. The preserved
serial executable reports total composition and first-present return, but not
the newly split phase timers; those missing measurements are not inferred.

| Candidate phase, ms | 1024×720 | 1920×1080 |
|---|---:|---:|
| Device creation | 46.523 [44.409–58.295] | 46.508 [43.635–51.372] |
| DXBC pipeline objects | 2.670 [2.589–2.863] | 2.759 [2.612–2.815] |
| Remaining join and resource setup | 16.059 [14.016–26.620] | 16.089 [13.605–21.970] |
| DXGI factory and swap chain | 1.114 [1.005–1.534] | 1.032 [0.937–1.218] |
| Composition device, target, visual and commit | 0.720 [0.675–1.057] | 0.693 [0.655–0.769] |
| First paint | 11.927 [0.454–14.352] | 12.686 [0.453–15.504] |
| First present duration | 1.267 [1.150–1.426] | 1.312 [1.098–1.687] |
| First present return, elapsed from host start | 157.317 [149.493–178.156] | 155.644 [150.068–189.984] |

The remaining join is about 16 ms at both sizes. Overlap improves startup, but
neither size meets the promotion threshold. The new path's steady redraw
intervals are 16.635/17.100 ms p50/p95 at 1024×720 and 16.625/17.080 ms at
1920×1080 (medians of ten runs; 119 post-first-frame intervals per run).

### Native desktop test results

| Filter | Verbatim result |
|---|---|
| shelf_resize | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 6.63s` |
| shelf_minimize_restore | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 2.32s` |
| shelf_idle_caret | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 3.02s` |
| shelf_dpi_roundtrip | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 5.15s` |
| shelf_device_loss | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 8.80s` |
| shelf_sleep_wake | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 84.49s` |
| shelf_startup_series | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 138.63s` |
| direct_window_blinks_without_a_softbuffer_surface | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out; finished in 0.35s` |

`shelf_lock_unlock` was not run.

### Pinned and carrier suites

The NucBox runs set `EXACT_REQUIRED=1` and `WGPU_BACKEND=dx12`; no missing
GPU is silently skipped. Each filter uses its normal CPU, Vello and Ink arms.
The device column selects Ink's device; it does not change the CPU oracle.

| Device | Suite | Verbatim result |
|---|---|---|
| d3d11 | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 13.60s` |
| d3d11 | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 2.06s` |
| d3d11 | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 0.55s` |
| d3d11 | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 1.00s` |
| d3d11 | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.55s` |
| d3d11 | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 3.96s` |
| d3d11 | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.53s` |
| d3d11 | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.65s` |
| wgpu | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 72.27s` |
| wgpu | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 7.88s` |
| wgpu | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 2.29s` |
| wgpu | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 6.55s` |
| wgpu | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.65s` |
| wgpu | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 12.80s` |
| wgpu | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.68s` |
| wgpu | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 2.16s` |
| d3d11 | exact-windows | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.11s` |
| wgpu | exact-windows | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.11s` |

The native `exact-ink` unit tests, including transferring the prepared device
to its receiving thread, report `test result: ok. 25 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 8.46s`

The Mac named filters report:

| Filter | Verbatim result |
|---|---|
| paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 2.00s` |
| borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.94s` |
| tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.98s` |
| backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 1.44s` |
| gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.98s` |
| visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 1.49s` |
| materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 1.58s` |
| image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.72s` |

### Checks and reproduction

The Mac passes the six required checks, including both Clippy and formatting,
and `cargo test -p exact-linux --lib --tests --no-fail-fast`. The Linux library
reports 1,553 passed and 20 ignored; its complete integration run includes
130 `it` tests and 103 pinned tests, all passing. The Mac named filters above
ran with Ink's wgpu device. MSVC compilation of the Windows-only Ink tests and
GNU cross-compilation of the Windows carrier also pass. The shared Unix listener test module
is now correctly excluded from Windows, where neither Unix sockets nor its
linked listener implementation exists.

| Mac command | Exit | Seconds |
|---|---:|---:|
| `cargo build --all-targets --keep-going` | 0 | 0.79 |
| `cargo test --tests --no-fail-fast` | 0 | 105.63 |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | 0.87 |
| `cargo fmt --all -- --check` | 0 | 5.36 |
| `bun scripts/caps.mjs` | 0 | 2.15 |
| `bun scripts/boot.mjs` | 0 | 0.02 |
| `cargo check --workspace --all-targets --keep-going` | 0 | 25.26 |
| `cargo test -p exact-linux --lib --tests --no-fail-fast` | 0 | 94.06 |

An additional optimized Windows `exact-linux --lib` test build was stopped
while compiling that large test binary; it was not run or counted as passing.
Windows' requested pinned, image and carrier suites were built and run as
separate targets. The full Linux library/integration requirement ran on the Mac.

Build the executables before measurement. Run `device::d3d11::desktop_tests`
filters from `exact-ink --lib` in the interactive desktop, using `--ignored
--nocapture --test-threads=1`. `INK_SHELF` names the candidate and
`INK_SHELF_BEFORE` the preserved serial executable. Do not run the complete
module unattended: lock/unlock requires an operator. The DPI test writes
`target/ink-s2/dpi-waiting` and `dpi-at-150`; select 150%, then 100%, in Settings.
Its Scale control's automation ID is
`SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox`.

Raw desktop images, per-frame/startup logs and parsed startup samples are in
`target/ink-s2-nucbox/`; Mac check logs are in `target/ink-s2-mac-bcbda9/`.
The remote fixtures record every child PID and close or terminate only their
own children. Temporary desktop tasks are removed after the runs.


## S2.2

**Windows Ink still defaults to wgpu.** The paired median first-screen overhead
is 6.316 ms at 1024×720 and 9.710 ms at 1920×1080, within the 10 ms startup bar.
The complete promotion gate does not pass: the GDI-to-composition fixture fails
its window-style assertion, the measured switch includes a late frame interval,
and lock/unlock has no confirmed operator. `device/selected.rs` and LLP 1149
D12 remain unchanged. This chunk is not qualified for the default.

### Implementation

- `device/d3d11.rs` publishes the prepared COM objects through a channel and
  wakes the window. `try_finish` never waits for the worker; the existing
  blocking `finish` remains available to headless callers. Tests cover pending,
  failed and disconnected workers, and rendering after the thread transfer.
- `host/windows/src/window.rs` and `host/linux/src/app.rs` boot the real window
  on the CPU. At least one softbuffer frame precedes polling the prepared
  device. Preparation failure leaves CPU presentation available.
- `host/linux/src/presenter/painter.rs` uploads the CPU picture for the first
  composition frame, then installs Ink. The carrier releases softbuffer and
  its caret patches before attachment. After that point, recovery remains on
  composition, including WARP; there is no return to GDI.
- `device/d3d11/composition.rs` draws the first swap-chain buffer before
  committing the visual. An unattached visual can report `OCCLUDED` even from
  a real present; activation retries that buffer after the commit. The desktop
  fixture checks sampled pixels, window styles and the transparent backdrop.
  Its style check fails on this NucBox, as recorded below.
- `device/d3d11/desktop_tests.rs` compares CPU startup, wgpu readback and the
  migrating path using the same executable. The window logs the migration
  frame and reads its DXGI timestamp before submitting the next frame.

The first-picture copy preserves the CPU painter's output during the handoff.
This follows the one-way presentation boundary in
[Microsoft's flip-model guidance](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model).
The fixture failure leaves the backdrop and absence of flash unqualified.

### Startup measurement

The machine, display, SDK and profiles are those of S2.1. Executed renderer
commit: `1febad510c`; subsequent edits change comments and this report only.
Shelf's SHA-256 is
`BD2F2C6C11606FFD75591DD0C0CFA20ACA3373C6C0D58575F25C630E49AC03D5`.
The optimized desktop test runs ten rotating rounds per size, three arms per
round, 120 frames per process: 60 launches and 7,200 frames. Each launch has
fresh isolated app storage. No builds or other GPU tests overlap this series.
All arms use the same baked Shelf. `WGPU_BACKEND=dx12` selects the portable
arm's backend. These are warm-machine, fresh-process results.

Milliseconds, median [minimum–maximum] across ten launches. DWM is the
process-age synchronization proxy for Shelf's complete `library-count` screen,
not a display timestamp. The DXGI column is the first composition frame's
displayed timestamp relative to process creation; its present ID was available
in all 20 migrating launches. All 20 start with CPU frame one and migrate on
frame two. In this series, frame two also contains the complete first screen.

| Pixels | Path | DWM first-screen proxy | DXGI first composition |
|---|---|---:|---:|
| 1024×720 | CPU / softbuffer | 167.933 [161.612–203.972] | — |
| 1024×720 | Ink wgpu / readback / softbuffer | 861.309 [837.997–1072.975] | — |
| 1024×720 | CPU → D3D11 composition | 174.683 [166.352–184.741] | 186.714 [178.211–197.352] |
| 1920×1080 | CPU / softbuffer | 175.614 [171.317–229.214] | — |
| 1920×1080 | Ink wgpu / readback / softbuffer | 872.208 [863.187–1427.343] | — |
| 1920×1080 | CPU → D3D11 composition | 185.482 [179.422–196.336] | 197.245 [192.044–207.803] |

Paired differences subtract the CPU launch in the same round. CPU is the
cheaper first-pixel path. The startup medians pass, but individual migrating
launches exceed 10 ms; the ranges are retained rather than discarded.

| Pixels | wgpu readback minus CPU | Migrating path minus CPU |
|---|---:|---:|
| 1024×720 | 696.194 [669.421–869.003] | 6.316 [-37.620–23.129] |
| 1920×1080 | 692.593 [636.525–1252.405] | 9.710 [-48.427–25.019] |

The first-present-return clock below starts at host entry, unlike the
process-age columns above. The full device and pipeline times occur on the
worker; they must not be added to the main-thread paint times.

| Phase, ms | 1024×720 | 1920×1080 |
|---|---:|---:|
| CPU-only first present returned | 126.578 [124.557–151.758] | 132.185 [129.755–177.842] |
| Migrating path's CPU first present returned | 128.880 [122.166–132.528] | 134.452 [130.509–145.302] |
| wgpu readback first present returned | 818.314 [805.491–1033.339] | 828.006 [819.350–1390.827] |
| Asynchronous device creation | 44.726 [41.376–49.129] | 44.763 [41.298–48.021] |
| DXBC pipeline objects | 2.755 [2.247–3.101] | 2.739 [2.490–3.098] |
| Composition creation, before activation | 1.816 [1.697–2.166] | 2.002 [1.790–2.488] |
| Migration frame's paint and handoff | 11.136 [10.544–12.074] | 16.532 [16.015–17.313] |

### Migration intervals and backdrop

The per-frame log spans the migration, but it does **not** establish a
switch without late frames. Each migrating run has an interval above 25 ms from
frame two to frame three. That interval includes the complete-first-screen
DWM/DXGI synchronization probe. It is not a clean measurement of an
uninstrumented migration, nor evidence that the observed gap is harmless.
No production pacing claim is made from these samples.

| Interval, ms | 1024×720 | 1920×1080 |
|---|---:|---:|
| CPU frame 1 → migration frame 2 | 19.217 [18.533–21.023] | 25.034 [24.419–25.486] |
| Migration frame 2 → Ink frame 3 | 40.338 [34.378–50.383] | 44.639 [39.689–48.526] |
| Ink frame 3 → frame 4 | 16.910 [16.742–17.283] | 16.930 [16.737–17.071] |
| Per-run interval p50 | 16.639 [16.618–16.672] | 16.642 [16.603–16.661] |
| Per-run interval p95 | 17.097 [17.038–17.305] | 17.096 [17.018–17.140] |

The GDI handoff fixture samples desktop pixels after DWM synchronization while
activating a buffer with the same opaque color. The final run recorded eight
samples from 13.357 to 295.620 ms, all `COLORREF 0x336699`. The sample spacing
is 33–50 ms; this is not a per-refresh proof of no flash. `WS_EX_LAYERED` was
absent, but `WS_EX_NOREDIRECTIONBITMAP` was also absent after activation,
failing the assertion at `composition.rs:513`. The subsequent Mica attribute
and transparent-pixel assertions were not reached. Constant pixels alone
cannot establish that the composition handoff succeeded.

The same style assertion failed through three fix rounds. Work on that issue
stopped under `rules/RULES.md`; the code builds, the failing ignored native test
remains, and the default is unchanged. This report does not infer why Windows
did not retain the expected style.

### Desktop journeys

These comparisons use actual desktop client pixels and a fresh process at the
same position and size. The normal band is mean maximum RGB error below 5/255
and fewer than 6% of pixels above 32/255. A passing comparison does not replace
the separate backdrop and intermediate-frame checks.

| Journey | Result | Mean error /255 | Pixels above 32/255 |
|---|---|---:|---:|
| Drag-resize through 800×600, 1280×900, 1920×1080 and 1024×720 | Comparison passed | 0.399863 | 0% |
| Minimize and restore | Passed | 0 | 0% |
| Idle focused field | Passed; three transitions, 20 changed pixels | 0 | 0% |
| Settings 100% → 150% → 100% | Return comparison passed; intermediate image has residual text | 0.001006 | 0.001221% |
| Forced GPU TDR | Passed; CPU composition, then hardware after 1,017.028 ms | 0 | 0% |
| Sleep and wake | Comparison passed after physical desktop returned | 0.031217 | 0% |
| Lock and unlock | Not attempted; no unlock operator confirmed | — | — |

The native DPI test observed HWND DPI 96 → 144 → 96. Settings was restored to
100% and closed. Inspection of
[`dpi-150.png`](../../target/ink-s22-nucbox/desktop/dpi-150.png) shows faint
residual text at the previous 100% positions. Its automated comparison only
checks the return to 100%; it does not qualify that intermediate image.
This is another reason not to promote the window path.

The sleep test requested a 15-second resume timer. `SetSuspendState` returned
after 74,360 ms, and the complete test took 79.82 seconds. Windows recorded
sleep/resume and a Modern Standby exit whose reason is
`SetThreadExecutionState`. The fixture wakes the physical display after resume
and waits for its original 5120×1440 extent. These results do not isolate the
wake source or qualify a timer-only wake. No sleep or sign-in policy changed.
Unlock cannot be automated without credentials; no operator was confirmed, so
no lock was issued and no manual lock/unlock result is claimed.

### Native test results

Tests below ran in the signed-in NucBox desktop. The failing GDI fixture is the
third handoff fix round; no further fix was attempted. The remaining rows are
independent test runs, not retries of that failure.

| Filter | Verbatim result |
|---|---|
| gdi_picture_migrates_without_layering_and_alpha_reaches_mica | `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.74s` |
| shelf_startup_series | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 152.49s` |
| shelf_resize | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 6.12s` |
| shelf_minimize_restore | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 2.00s` |
| shelf_idle_caret | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 2.89s` |
| shelf_dpi_roundtrip | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 4.19s` |
| shelf_device_loss | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 8.51s` |
| shelf_sleep_wake | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 79.82s` |
| direct_window_blinks_without_a_softbuffer_surface | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out; finished in 0.19s` |
| exact-ink nonignored unit tests | `test result: ok. 26 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 7.73s` |

The pinned filters set `EXACT_REQUIRED=1` and `WGPU_BACKEND=dx12`, using their
normal CPU, Vello and Ink arms. Missing devices are not silently skipped.
`EXACT_INK_DEVICE` selects Ink's device in each row.

| Device | Suite | Verbatim result |
|---|---|---|
| d3d11 | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 14.48s` |
| d3d11 | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 2.60s` |
| d3d11 | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 1.09s` |
| d3d11 | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 1.23s` |
| d3d11 | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.90s` |
| d3d11 | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 4.57s` |
| d3d11 | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.83s` |
| d3d11 | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 0.82s` |
| d3d11 | exact-windows | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.13s` |
| wgpu | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 70.77s` |
| wgpu | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 7.03s` |
| wgpu | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 2.23s` |
| wgpu | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 6.87s` |
| wgpu | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 2.04s` |
| wgpu | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 13.35s` |
| wgpu | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.98s` |
| wgpu | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.90s` |
| wgpu | exact-windows | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.12s` |

### Mac checks

All six checks pass, including both Clippy and formatting, as does the explicit
`exact-linux` library/integration run. These ran after the final executable
change; subsequent changes are comments and this report.

| Command | Exit | Seconds |
|---|---:|---:|
| `cargo build --all-targets --keep-going` | 0 | 0.74 |
| `cargo test --tests --no-fail-fast` | 0 | 107.67 |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | 0.95 |
| `cargo fmt --all -- --check` | 0 | 5.32 |
| `bun scripts/caps.mjs` | 0 | 1.99 |
| `bun scripts/boot.mjs` | 0 | 0.02 |
| `cargo check --workspace --all-targets --keep-going` | 0 | 27.34 |
| `cargo test -p exact-linux --lib --tests --no-fail-fast` | 0 | 65.67 |

Mac named filters use Ink's wgpu device:

| Filter | Verbatim result |
|---|---|
| paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 2.07s` |
| borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.96s` |
| tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.92s` |
| backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 1.36s` |
| gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.96s` |
| visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 1.44s` |
| materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 1.65s` |
| image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.69s` |

The final executable source also passes the Mac's Windows cross-compilation
checks for `exact-windows --all-targets` (`x86_64-pc-windows-gnu`) and
`exact-ink --tests` (`x86_64-pc-windows-msvc`). Native Shelf, `exact-ink`,
`exact-windows`, pinned and image test builds completed on the NucBox before
execution. Windows builds retain the three existing `exact-linux` warnings.

Raw results and desktop captures are in `target/ink-s22-nucbox/`: `startup/`
holds the per-launch logs, `startup-samples.json` their parsed frames,
`regressions/` the native stdout/stderr, and `desktop/` the screenshots and
journey logs. Mac gate logs are in `target/ink-s22-mac-final/`; named filters
are in `target/ink-s22-mac/`. The two temporary desktop tasks were removed.

To reproduce, build Shelf with `bun host/windows/build.mjs shelf` and the
native test executables with `cargo test -p exact-ink -p exact-windows
--profile host-dev --lib --no-run`. Put the Windows SDK's `fxc.exe` on `PATH`
for both builds. Set `INK_SHELF` to that executable and run the individual
ignored desktop filters with `--nocapture --test-threads=1`. The startup test
now needs only this one executable; it no longer reads `INK_SHELF_BEFORE`.
Do not run `shelf_lock_unlock` unattended.

## S2.3

**Windows Ink still defaults to wgpu.** The composition window now exists from
creation, and the CPU painter presents through WARP until the hardware device
is ready. The paired median overhead over softbuffer is **14.153 ms at
1024×720 and 29.516 ms at 1920×1080**, above the 10 ms bar. Probe-free
handoff intervals also exceed 25 ms, and the final 100% DPI return remains
unqualified. `device/selected.rs` and D12 are unchanged.

### Implementation and measurement

- `host/windows/src/window.rs` creates direct Ink HWNDs with
  `WS_EX_NOREDIRECTIONBITMAP`; neither startup nor migration creates a GDI
  surface. CPU and wgpu selections retain their existing softbuffer path.
- `host/ink/src/device/d3d11.rs` prepares WARP, then hardware, on the worker
  while app boot continues. Starting both at once made WARP creation about
  45 ms; ordering them gives medians of 15.095/13.692 ms at the two sizes.
  Hardware creation is 33.733/34.810 ms; its pipeline objects take 2.700/2.680 ms.
- `device/d3d11/composition.rs` shares one visual across both swap chains.
  A replacement is populated before `SetContent` and `Commit`. WARP retires
  after its GPU completion query and the replacement's displayed present.
  Its final query needs a nonblocking flush because no later WARP frame will
  submit it. The composition device has no rendering-device dependency.
- `host/linux/src/surfaces/window.rs` uploads CPU startup frames to WARP;
  `presenter/painter.rs` transfers that same picture to hardware and retains
  WARP through retirement. `device/d3d11/desktop_tests.rs` adds the WARP Ink
  control, a probe-free launch series and a second 150% comparison.

Measured on the NucBox's signed-in 5120×1440, 60 Hz desktop at 100%, on
2026-10-28. Shelf is the `gpu-dev` build of `719969db26`; native fixtures are
`host-dev` at `7b4a4ec390`. Ten rounds rotate CPU/softbuffer, wgpu DX12,
Ink/WARP, and CPU/WARP→hardware at each size, with fresh storage and 120
frames per process. `WGPU_BACKEND=dx12` and `EXACT_REQUIRED=1` are set. No
build or other test suite runs during the series. These are warm-machine
fresh-process launches, not driver-cache-cold measurements.

Milliseconds, median [minimum–maximum]. The DWM proxy is the same
first-complete-screen measurement used in S2.1 and S2.2. It is distinct from
the DXGI displayed timestamp and is the paired-overhead basis.

| Pixels | Path | DWM first-complete-screen proxy |
|---|---|---:|
| 1024×720 | CPU / softbuffer | 175.590 [161.615–195.066] |
| 1024×720 | Ink / wgpu DX12 / readback | 885.946 [861.282–1110.168] |
| 1024×720 | Ink / WARP / composition | 213.603 [201.460–220.066] |
| 1024×720 | CPU / WARP → hardware | 191.618 [177.967–216.641] |
| 1920×1080 | CPU / softbuffer | 185.173 [171.379–211.597] |
| 1920×1080 | Ink / wgpu DX12 / readback | 892.448 [859.299–1344.256] |
| 1920×1080 | Ink / WARP / composition | 231.649 [219.635–294.953] |
| 1920×1080 | CPU / WARP → hardware | 212.909 [189.690–224.935] |

CPU painting on WARP is the cheaper first-complete-screen path. Paired
candidate-minus-CPU differences are 14.153 [−0.496–46.084] ms and
29.516 [−1.826–41.537] ms. All 20 candidates switch on frame two.
Their first WARP presents have DXGI timestamps in all 20 runs:
193.123 [180.795–200.564] ms and 208.509 [201.220–221.199] ms after process
creation. The hardware first-complete-screen timestamps are
199.792 [190.061–212.857] ms and 210.691 [201.456–228.990] ms.

### Intervals without the synchronization probe

A second 80-launch series sets `EXACT_AGENT_WINDOW_PROBE=off`. No DWM wait
or DXGI synchronization retry runs in its timed frame path. In the startup
series, the probe itself costs candidate medians of 26.477 and 15.848 ms;
those durations are not subtracted from individual intervals. The separate
probe-free series measures the actual frame-start intervals below.

| Candidate interval, ms | 1024×720 | 1920×1080 |
|---|---:|---:|
| CPU frame 1 → migration frame 2 | 23.571 [22.486–28.144] | 26.449 [24.950–30.235] |
| Migration frame 2 → Ink frame 3 | 20.968 [13.448–29.692] | 35.945 [21.937–41.387] |
| Ink frame 3 → frame 4 | 16.872 [2.070–17.248] | 17.081 [16.917–18.000] |
| Per-run interval p50 | 16.641 [16.607–16.696] | 16.639 [16.586–16.714] |
| Per-run interval p95 | 17.142 [17.086–17.819] | 17.129 [17.079–17.883] |

At 60 Hz, using S2.2's 25 ms late-interval threshold, frame 1→2 is late in
3/10 and 9/10 runs; frame 2→3 in 5/10 and 8/10. The no-late-frame condition
is refuted without the old synchronization probe. Across all 1,190 intervals
per size, candidates have 8 and 20 late intervals, versus CPU's 0 and 5.
The 1920×1080 candidate also has a later 154.451 ms maximum;
that outlier is retained, not attributed to migration. These are frame-start
intervals, not display-scanout timestamps for every frame.

### Handoff and DPI

The composition fixture's seven displayed-pixel samples all equal `0x336699`.
Mica is `0xf0f3f4` before and after; `WS_EX_LAYERED` is absent and
`WS_EX_NOREDIRECTIONBITMAP` is present. Completion-based WARP retirement
passes after flushing its final query. Sampling gaps reach 49.87 ms, so this
establishes no observed flash, not the absence of a one-refresh flash.

The 150% capture no longer contains the faint old-scale text. The first and
post-round-trip 150% captures each differ from a fresh 150% launch by a mean
of 0.001454/255, with 0.001899% of pixels over 32. No glyph-atlas or layer
invalidation change was needed. This supports the stale GDI redirection
surface as the residue's cause; the new window never creates that surface.

The full `shelf_dpi_roundtrip` journey did not pass. Attempt one lost the
Settings scale control during restore. Attempt two timed out resizing after
returning to 100%. Waiting for a painted frame at the new scale allowed
attempt three's second 150% comparison, but the last 100% return still
produced a 1030×737 client after requesting 1024×720. The external driver
stopped that attempt after its marker deadline. There was no fourth attempt.
The display was restored to 100%; the remaining resize qualification is in
`QUEUE.md`. Lock/unlock was not attempted: this is Charlie's desktop session,
with no person available to unlock it by hand.

### Native and Mac checks

The pinned filters use their CPU, Vello and Ink arms with `EXACT_REQUIRED=1`.
The device column selects Ink; wgpu uses DX12. Results are verbatim.

| Device | Suite | Result |
|---|---|---|
| d3d11 | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 4.92s` |
| d3d11 | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 1.29s` |
| d3d11 | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 0.57s` |
| d3d11 | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.63s` |
| d3d11 | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.54s` |
| d3d11 | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 1.56s` |
| d3d11 | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.57s` |
| d3d11 | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 0.54s` |
| d3d11 | exact-windows --lib --tests | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.06s` |
| wgpu | paint | `test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 47 filtered out; finished in 9.44s` |
| wgpu | borders | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 2.15s` |
| wgpu | tint | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 1.05s` |
| wgpu | backdrop | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 1.07s` |
| wgpu | gradients | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.08s` |
| wgpu | visual | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 2.19s` |
| wgpu | materials | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.18s` |
| wgpu | image | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 1.30s` |
| wgpu | exact-windows --lib --tests | `test result: ok. 30 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.15s` |
| d3d11 | exact-ink --lib | `test result: ok. 26 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 8.12s` |
| wgpu | exact-ink --lib | `test result: ok. 26 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 7.49s` |

All six Mac checks pass, including both Clippy and `cargo fmt --all -- --check`,
`cargo check --workspace --all-targets --keep-going`, caps and boot. The full
`cargo test -p exact-linux --lib --tests --no-fail-fast` also passes. Its library
reports `test result: ok. 1553 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out; finished in 75.74s`.
The complete pinned suite with `EXACT_REQUIRED=1` reports
`test result: ok. 103 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 2.65s`.
The image filter reports
`test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 2.11s`.
Windows GNU cross-checks of `exact-windows --all-targets` and MSVC cross-checks
of `exact-ink --tests` pass; native builds supply the SDK-compiled DXBC.

| Native desktop filter | Verbatim result |
|---|---|
| WARP handoff / Mica / retirement | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.58s` |
| shelf_startup_series | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 214.80s` |
| shelf_handoff_intervals | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 215.04s` |
| shelf_resize | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 6.19s` |
| shelf_minimize_restore | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 2.50s` |
| shelf_idle_caret | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 3.23s` |
| direct_window_blinks_without_a_softbuffer_surface | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out; finished in 0.30s` |
| shelf_device_loss | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 8.28s` |
| shelf_sleep_wake | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 81.50s` |

Forced device loss recovers from CPU/WARP to hardware after 1,015.979 ms;
the recovered capture has zero error against a fresh launch. Sleep returns
after 75,483 ms; its fresh-launch comparison has mean error 0.031193/255
and no pixels over 32. Resize, minimize/restore and the caret comparisons
have zero mean error. The unsuccessful DPI runs are described above;
the second attempt reports
`test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 34.07s`.
The third attempt was stopped by the driver and has no Rust test verdict.

Raw launch logs, parsed samples, native stdout/stderr and desktop captures
are in `target/ink-s23-nucbox/`. Mac logs are `target/ink-s23-*.log`.
The temporary desktop tasks were removed. Reproduction uses the S2.2 build
commands above, then `shelf_startup_series` and `shelf_handoff_intervals`
with `INK_SHELF` set. Both series rotate all four arms ten times per size.
The DPI fixture needs Settings changes 100% → 150% → 100% → 150% → 100%;
its final resize remains unresolved. No lock/unlock result is claimed.
