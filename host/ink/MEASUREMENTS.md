# Ink S0 C4: retained rows and the wgpu control

@ref LLP 1149 D4, D15.

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
