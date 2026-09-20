# Cluster LOD on core WebGPU

A standalone LLP 1041.011 O1 / §5 Q2 experiment: offline cluster-LOD DAGs,
GPU selection/culling and ordinary hardware rasterization on Apple M5 Max / Metal.
One statue filling the screen **loses** when selection is counted: without shadows,
Gaul takes 1.826 vs 1.318 ms and Washington 5.866 vs 2.286 ms (cluster vs naive).
With shadow LOD included the single-statue totals win 2.04× / 1.46×, so shadow
savings must not be mistaken for cheaper close-up geometry. Grid/field totals win
**67.9–280.4×** (Washington grid: 67.9×). Use total GPU ratios, not main-only ratios.
The ≤1.5 ms every-frame selection target remains unmet.

Native and streaming Chrome demos share Rust/WGSL, 4× MSAA, baked AO, lighting,
camera motion and debug colours. Hero: Smithsonian *George Washington*, Horatio
Greenough, 1840 (CC0); quick fixture: SMK *Dying Gaul* (Public Domain Mark).
No software rasterizer, 64-bit atomics, vendor edits or optional rendering features.

<!-- L3B_BENCHMARK_START -->
Apple M5 Max / Metal, format v5, 2560×1440, 4× MSAA, 4096² shadows, 1 px. Regenerate with `bun measure.mjs benchmark` (from this directory). Seven measured frames after one warmup per renderer, interleaved ABAB. Cluster shadows use 2× the threshold; naive shadows draw full geometry. Single/ring/grid/field use t=0; avenue uses t=.45. Both renderers use the same instance-frustum cull. Times are medians in milliseconds; total ratio is naive total / cluster total (selection + main + shadow). Main-only times and ratios are secondary. Totals are per-frame sums followed by medians; component medians need not sum.

| Asset · layout | Source triangles × instances | Drawn triangles | TOTAL GPU cluster / naive ms | Total ratio | Main-only cluster / naive ms (ratio) | Shadow cluster / naive ms | Select ms | CPU ms | Resident bytes cluster / naive | RGB mean / p99.9 | Coverage cracks / interior |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| gaul · single | 4,000,020 × 1 | 137,746 | 1.426 / 2.912 | 2.04× | 0.834 / 2.194 (2.63×) | 0.140 / 0.673 | 0.451 | 0.454 | 339,640,556 / 287,819,868 | 0.0005753 / 0.0627451 | 0 / 293,493 |
| gaul · ring:12 | 4,000,020 × 12 | 100,315 | 0.770 / 5.302 | 6.89× | 0.486 / 2.999 (6.17×) | 0.076 / 2.304 | 0.074 | 0.658 | 354,031,768 / 287,820,572 | 0.0003542 / 0.0823529 | 0 / 47,515 |
| gaul · avenue:25 | 4,000,020 × 25 | 156,824 | 1.034 / 2.665 | 2.58× | 0.517 / 1.642 (3.17×) | 0.063 / 0.908 | 0.450 | 0.527 | 371,039,564 / 287,821,404 | 0.0006941 / 0.0666667 | 0 / 404,734 |
| gaul · grid:400 | 4,000,020 × 400 | 430,640 | 1.483 / 162.129 | 109.35× | 0.739 / 86.444 (116.92×) | 0.213 / 75.685 | 0.531 | 0.777 | 422,289,064 / 287,845,404 | 0.0019742 / 0.2196078 | 0 / 58,354 |
| gaul · field:5000,1 | 4,000,020 × 5000 | 653,612 | 8.951 / 2510.244 | 280.44× | 0.892 / 1347.311 (1510.30×) | 0.335 / 1152.827 | 7.720 | 1.657 | 423,092,264 / 288,139,804 | 0.0017910 / 0.2196078 | 0 / 2,273 |
| washington · single | 16,860,930 × 1 | 447,833 | 2.926 / 4.258 | 1.46× | 0.750 / 2.696 (3.60×) | 0.185 / 1.541 | 1.994 | 0.746 | 833,201,768 / 584,434,288 | 0.0005229 / 0.0509804 | 0 / 314,134 |
| washington · ring:12 | 16,860,930 × 12 | 956,523 | 1.239 / 21.121 | 17.05× | 0.790 / 11.997 (15.18×) | 0.269 / 9.337 | 0.182 | 0.731 | 895,019,876 / 584,434,992 | 0.0004866 / 0.0941176 | 0 / 78,345 |
| washington · avenue:25 | 16,860,930 × 25 | 618,793 | 4.208 / 8.640 | 2.05× | 0.857 / 5.275 (6.15×) | 0.126 / 3.333 | 3.240 | 0.678 | 911,481,140 / 584,435,824 | 0.0008133 / 0.0549020 | 0 / 550,697 |
| washington · grid:400 | 16,860,930 × 400 | 15,031,084 | 13.507 / 917.493 | 67.93× | 7.283 / 474.526 (65.15×) | 5.253 / 449.373 | 1.059 | 2.380 | 911,673,140 / 584,459,824 | 0.0018550 / 0.1803922 | 0 / 99,979 |
| washington · field:5000,1 | 16,860,930 × 5000 | 135,558,443 | 89.402 / 10312.781 | 115.35× | 47.239 / 5314.063 (112.49×) | 32.875 / 4953.511 | 9.155 | 2.223 | 914,021,940 / 584,754,224 | 0.0021388 / 0.2235294 | 0 / 13,198 |

Single statue close-up, t=1, shadows off (isolates geometry + selection):

| Asset | TOTAL GPU cluster / naive ms | Total ratio | Main / select ms |
|---|---:|---:|---:|
| gaul | 1.826 / 1.318 | 0.72× | 0.477 / 1.321 |
| washington | 5.866 / 2.286 | 0.39× | 0.533 / 5.333 |

Hero quality dial, Washington avenue t=.75, same settings:

| Threshold px | Drawn triangles | GPU total / main / shadow / select ms | RGB mean / p99.9 | Coverage cracks / interior |
|---:|---:|---:|---:|---:|
| 0.5 | 1,251,764 | 6.009 / 1.461 / 0.280 / 4.279 | 0.0006175 / 0.0313725 | 0 / 939,636 |
| 1 | 737,305 | 4.487 / 1.006 / 0.168 / 3.314 | 0.0009652 / 0.0509804 | 0 / 939,636 |
| 2 | 415,617 | 3.870 / 0.733 / 0.107 / 3.030 | 0.0014353 / 0.0745098 | 0 / 939,636 |
| 4 | 228,140 | 3.520 / 0.524 / 0.076 / 2.909 | 0.0020722 / 0.1098039 | 0 / 939,636 |

RGB mean is the mean absolute sRGB channel difference, normalized to 0–1; p99.9 is the nearest-rank percentile of the maximum RGB-channel difference per pixel. Lit comparisons include each renderer’s own shadows. Coverage counts completely missing pixels inside the fully covered naive mask after one-pixel erosion; silhouettes and partial MSAA samples are excluded. These image metrics are measured errors, not a proof that quadric bake error bounds screen pixels. Resident bytes count each renderer separately, including its allocated geometry, selection buffers and attachments, excluding driver overhead and diagnostic readbacks. GPU select includes main and shadow selection. CPU time covers selection plus encoding/submission, excluding blocking diagnostic readback and PNG encoding. Raw measurements: [F3 benchmark records](results/f3-benchmark.json).

Measured rows: 10 + 4 sweep + 2 close-up; failures: 0.
<!-- L3B_BENCHMARK_END -->

## Run

Run from this directory with the launch environment (debug info and incremental
compilation off). This is an independent Cargo workspace, outside the repo's five
checks. Use only `target/`. `<cache>` means `~/Library/Caches/exact2-cluster-lod/`;
all source meshes, bakes, media and logs stay there. Never commit generated assets.

```sh
cargo build -p clod-bake -p clod-view
asset="$HOME/Library/Caches/exact2-cluster-lod/out/washington-5.clod"
target/debug/clod-view demo "$asset"
target/debug/clod-view demo "$asset" --stats
target/debug/clod-view demo "$asset" --frames 600 --exit
target/debug/clod-view demo "$asset" --frames 600 --exit --intervals-only
bun measure.mjs bake       # two release bakes per cached scan
bun measure.mjs verify     # all workspace tests, clippy, fmt, two wasm library builds
bun measure.mjs oracles    # real assets: set/image equality, grazing cameras, overflow
bun measure.mjs benchmark  # ABAB timings and image metrics; regenerates the lead table
bun measure.mjs pacing     # both assets × both modes × instrumented/interval-only
bun web/build.mjs
bun web/proof.mjs
bun web/serve.mjs          # http://127.0.0.1:8765/ ; ?asset=washington for the hero
```

`demo` and `time` are Metal-only native commands. `time`, `bench`, `reel` and
instrumented `demo` need `TIMESTAMP_QUERY`; ordinary `demo` and the browser need
no optional GPU features. `--exit` or `--stats` opts into GPU timing;
`--intervals-only` disables timestamps while retaining completeness counters.
Space pauses; arrows scrub; C/D/T toggle cluster/depth/triangle colours; N toggles
naive; brackets adjust threshold; 1–4 choose single/ring/avenue/grid; drag while
paused to orbit; Esc exits. `--layout field:5000,1` selects the far field.
Normal playback traverses the 40-second path in both directions. Resize replaces
attachments, preserving geometry. Interactive titles display main/shadow overflow.

`--exit` samples the full path uniformly for N frames after ten warmups. It copies
32 bytes of counters per measured frame into bounded staging and checks every
frame at the end; **any main/shadow overflow fails**. Ordinary playback polls HUD
counters asynchronously. Instrumented mode waits for Metal fragment timestamps;
interval-only mode has no per-frame GPU wait. Retained timing samples cap at 1,000.
Host present intervals include FIFO/vsync and the blit; they are not compositor
scanout telemetry. Missed refreshes sum `max(round(interval/period)-1,0)`; late
intervals exceed 1.5 periods. GPU totals exclude the presentation blit.

Other CLI commands: `render`, `compare`, `pop`, `oracle`, `reel`; `--size 2560x1440`,
`--threshold-px 1`, `--t 0.75`, `--view clusters`, `--layout avenue:25` and
`--out <cache>/out/...` select a still or report. Bake with
`target/release/clod-bake <source> <output.clod>`; `--inspect <output.clod>` reports
format statistics. `measure.mjs images` runs image/pop reports; `sweep` checks quotas.
`reel --seconds 40 --fps 60` exports 2,400 frames and two videos. The installed
ffmpeg needs `DYLD_LIBRARY_PATH=<cache>/out/reel/encoder-lib` (cached ABI-215 x265).
Reel rejects `--eye/--target`: clearance certifies only the authored path.

## Format v5 and selection

Little endian, magic `CLOD0005`, version 5; older files are rejected. Sections:
192-byte header, clusters, groups, page table, BVH, geometry, with 16-byte alignment
and zero padding. Records: cluster 128 bytes, bounds 20, group 32, node 32,
page 80, vertex 20. Vertex = float32 XYZ, octahedral snorm16×2 normal, RGB + AO8.
RGB is low-byte red; high-byte AO 255 means unoccluded. `HAS_COLOR=1` iff some
stored RGB differs from white; `HAS_AO=2` iff some stored AO differs from 255.
An all-neutral AO result is canonically unflagged; this flag describes data, not
bake provenance. Whole-file validation requires exact flag/data agreement;
streamed pages reject nonneutral unflagged channels before upload.

Clusters contain ≤256 vertices and ≤128 triangles with u8 local indices. Pages
are 32 MiB by default (4 KiB–128 MiB); all terminals must fit page zero. Positions
retain source bits. Reader checks SHA-256, ranges, padding, finite geometry,
group coverage, matching refinement bounds, original counts, error/depth ordering,
terminal references, sphere containment and BVH reachability/leaf agreement.
Sphere containment permits `8*EPSILON*max(abs(centres),radii)` world units in f64.
The BVH is **emitted and validated but not read by the current selector**. It is
a hook for the next selection-cost experiment: bounded group/page traversal and
stable prefix/scatter, while retaining current cut and quota semantics.

Select iff projected simplified error > threshold, and refinement is ORIGINAL
or projected refined error ≤ threshold. Terminal MAX is a sentinel; zero stays
zero and positive errors saturate at MIN_POSITIVE. CPU and WGSL use:

```text
perspective = error * scale / max(stable_length(center-eye)-radius*scale, near)
              * cot(fovy/2) * 0.5 * viewport_height
orthographic = error * scale * viewport_height / orthographic_span
```

Stable length scales by the maximum component. Candidate pruning uses descending
suffix-max/prefix-min envelopes with outward guards; every intermediate must be
finite, otherwise the full range is retained. CPU brute selection stays independent
of the range optimization; `CandidateIndex::range` mirrors and tests the GPU range.
Main depth is reversed-Z (clear 0, Greater); backdrop depth writes are disabled.
Shadow depth is forward-Z; shadow cone culling remains disabled.

Scene transforms require positive, approximately uniform orthogonal affine scale
with normalized squared-length/dot tolerance 2e-5. The admitted residual stretch
is covered for all instance/cluster/main/shadow spheres by max column length ×
1.00004. This follows a Gershgorin bound on MᵀM with outward rounding. Main cone
culling retains meshoptimizer's perspective apex test and 1e-5 angular guard.

Watertight means every replacement preserves its signed, position-welded boundary
chain, so every complete cut of a closed source remains closed. Balanced pinches
cancel; manifoldness and geometric self-intersection are not guaranteed. The
21-fixture evidence found 503 pinched-edge occurrences across 501 cuts, incidence 4,
winding zero. Open scans use image/coverage oracles instead of a closed-source test.

## Browser and residency

Core-feature WebGPU renders to an sRGB canvas view. Gaul is the default; Washington
is query-selected. Gaul's naive geometry loads lazily; Washington's naive toggle
is declined to bound tab memory. HTTP Range loads metadata then page zero, then
remaining pages in order. WebCrypto and Rust validate each page before upload.
Downloaded bytes are discarded after upload. No eviction or fallback renderer.

Readiness requires every page of a group **and every parent group**. CPU/WGSL use
`ready(g) && projected(g)>threshold`, preserving whole transitions and coarse
ancestors. Partial residency disables range pruning; full residency restores it.
Camera/scene companions are deterministic and bound to the baked metadata digest.
Native preparation measures source bounds/hero anchors before streaming; browser
camera and lighting math remain shared. Layout changes preserve uploaded geometry.
The browser reports device loss, validation/OOM and fetch/integrity failures.

Build uses wgpu 30, wasm-bindgen 0.2.127 and wasm-opt -O3. Generated companions,
Wasm, Chrome profiles, screenshots and proof JSON remain under `<cache>/out/web/`.
The proof owns and reaps only its recorded Chrome PID; four HTTP Range controls,
input/resize checks, native/browser image comparisons and 600 browser frames run.
Browser rAF/queue-completion timings are wall time, not GPU timestamps or scanout.

## Decisions

1–4. Preserve position bits; lock borders; regular simplification only; accumulate
error as `max(previous,current)+current`; terminals first then descending depth/ID;
lift zero errors to MIN_POSITIVE. Failed boundary transitions stop their ancestors.
5. Hierarchy construction is single-threaded; AO uses up to 16 disjoint workers.
Integer smoothing and fixed samples preserve worker-count-independent bytes.
6–8. PLY/OBJ/STL and first glTF mesh only; no material/scene/transparency import.
Stream and hash inputs; assemble bounded pages then one output Vec. No network buffers.
9–10. Retain 21 seeded closed fixtures. Error honesty is sampled cut→source with a
4× refined-error + 1e-6 gate, not a Hausdorff or strict pixel bound.
11–14. Naive uses cache/fetch-optimized ORIGINAL vertices and indexed instancing.
CPU caches group predicates. Page draws pad short clusters with clipped vertices.
Normalize longest extent to two world units; deterministic layouts, Washington Y-up.
15–16. Superseded camera/shadow decisions are archived in `results/pre-f3.md`.
17–23. Seven timing samples after warmup; sRGB errors; bounded report retention;
1 px quality/reduction and eroded coverage gates; invariant clip positions.
24–27. Keep stable per-instance scans and quota isolation; default quota is
min(cluster count, floor(4,194,304/instances)). Overflow is incomplete geometry.
Resolve opted-in timestamps after GPU completion; missing/stale samples are errors.
28–39. Keep oriented-chain topology checks, f64 normal normalization, numerical
projection regressions, reversed-Z and identical instance culling for naive/cluster.
No manifold filter, permissive simplification, HZB, geomorphing or software raster.
40. Bun writes measured tables from current **v5** runs; historical results are archived.
41–43. Equality oracles reserve the full 128 MiB visible binding divided per instance;
test one-slot overflow separately. Shadow cone rejection stays off; even near-boundary
set differences must preserve exact images. L4 residency preserves whole cuts.
44. F3 range arithmetic falls back on any nonfinite intermediate. Sphere bounds cover
the full admitted transform tolerance. Disable only backdrop depth writes.
45. F3 rejects page counts above dispatch limits and every oversized storage buffer
at load time; no tiled dispatch is needed for these assets.
46. F3 AO rejects degenerate rays relative to edge scale, preserving 1e-9 units.
Expose explicit worker counts for the proxy determinism oracle; use canonical HAS_AO
and bump to v5. Keep geometry records unchanged and rebake both real assets twice.
47. F3 benchmarks interleave ABAB and lead with total GPU, retaining main-only values.
Cluster shadows use 2× the main threshold; naive shadows draw full geometry.
48. F3 native timing is opt-in. Reuse offscreen views and present staging; mutate
camera-light fields in the owned scene without cloning instances. Acquired surface
textures still need a fresh view. Bound retained samples; asynchronously show HUD
overflow and check every measured frame's counter record at run completion.
49. F3 rejects reel camera overrides so preflight and rendering use one authored path.
Hierarchy BVH levels use max(depth)+1; an unsorted-depth compile-time fixture guards it.
50. F3 retains the perspective apex cone: 256 grazing cameras per real asset produced
zero changed pixels. Do not replace audited compaction, reversed-Z planes or topology.
51. F3 keeps the existing reel: rebaked page payloads match v4 and two 1440p endpoint
stills are byte-identical. Archive L3a, historical F1/F2 and pre-F3 numbers together.

52. Add a shadows-off single-statue close-up to distinguish selection cost from
the shadow-LOD savings seen in the new total-GPU results. Preserve both outcomes.

## Results

<!-- F3_RESULTS_START -->
`bun measure.mjs bake`: two identical v5 bakes per asset; only three header bytes
differ from v4, with identical metadata/page payloads after the 192-byte header.
[All bake numbers and regression records](results/f3-validation.json).

| Asset | Bytes | Repeat wall seconds | SHA-256 |
|---|---:|---:|---|
| Gaul | 138,247,344 | 26.525 / 28.736 | `65dce583862e07481f20ab2cc3df23fe6bccfce2052ba2610e52478e67890dbe` |
| Washington | 626,622,224 | 59.230 / 55.632 | `3d4f20c19307bc72c39fa5c3ef460842966594fe7df60f3e006e06c718ac0af6` |

`cargo test -p clod-view --test projection --test backdrop -- --nocapture`: the
scale 1e20/distance 1e21/threshold .5 cut went from GPU 0 vs CPU 2 clusters to 2/2.
The eye 1e19·(1,1,1)/threshold 1e-20 case did **not** reproduce on Metal (65/65
before and after), and remains tested. Far geometry at 3,000, far=3,030 had
0/1,221 visible/covered pixels in both paths before the fix; now 1,221/1,221.
The accepted stretched sphere changed from rejected to visible (tip Y=0.000035286).

`cargo test -p clod-bake --test ao --test validation -- --nocapture`: the 1e-9
crease went from AO 255 to 248, matching unscaled 248; convex mean 255. Proxy
fixture: 262,144→250,000 triangles, crease 174; convex 524,288→250,000, mean 255.
Three runs (1/1/4 workers) preserve AO and 7,060,608 baked bytes. Reader rejects
38/38 corruptions. Load-limit tests reject 240,160,000-byte counts and 65,536
page workgroups before allocation; the valid control passes. Unsorted depths
[0,4,1] now produce five BVH levels instead of two. Reel override test: 2/2 cases.

`demo <gaul-5.clod> --capacity 1 --frames 3 --exit --size 320x180`, with and
without `--intervals-only`: both now fail, reporting 667 main / 3,564 shadow drops.
The original command returned success and omitted completeness counters.

`bun measure.mjs oracles`: Gaul 15.628 s, Washington 40.820 s, zero failures.
384 CPU/GPU exact image pairs, 384 deterministic repeats, 192 original culling
pairs, plus **512 grazing camera pairs, zero changed pixels and zero overflow**.
510 grazing views were nonempty. One numerical-boundary set difference retained
exact pixels. The perspective cone concern was not reproduced; apex form stays.

`bun measure.mjs pacing`: 8 runs × 600 frames after ten warmups, 2560×1440,
avenue:25, 1 px, shadows on; 120 Hz monitor. **4,800/4,800 counter records checked,
zero main/shadow overflow**, zero pixel readbacks or PNG encodes. All spikes remain.
Timing off means no timestamp feature or per-frame completion wait, with counter
copies still enabled. [Full pacing distributions](results/f3-pacing.json).

| Asset / mode | Timing | Interval p50 / p95 / p99 / max ms | Missed refreshes / late intervals | GPU p50 / max ms |
|---|---|---:|---:|---:|
| gaul-5 / cluster | on | 8.335 / 8.943 / 20.383 / 68.816 | 39 / 25 | 4.384 / 7.340 |
| gaul-5 / cluster | off | 8.337 / 10.886 / 21.130 / 74.231 | 46 / 29 | — |
| gaul-5 / naive | on | 8.402 / 16.920 / 33.589 / 100.537 | 160 / 121 | 5.924 / 15.315 |
| gaul-5 / naive | off | 8.365 / 16.837 / 34.635 / 155.794 | 151 / 96 | — |
| washington-5 / cluster | on | 8.334 / 9.413 / 19.238 / 67.038 | 38 / 22 | 5.184 / 11.052 |
| washington-5 / cluster | off | 8.338 / 8.984 / 19.519 / 58.269 | 36 / 22 | — |
| washington-5 / naive | on | 15.470 / 59.818 / 89.077 / 114.582 | 1229 / 487 | 13.579 / 88.421 |
| washington-5 / naive | off | 16.600 / 57.911 / 91.188 / 372.577 | 1111 / 390 | — |

`bun measure.mjs verify`: **29 tests / 17 nonempty suites, zero failures**;
tests 90.260 s, clippy 0.415 s, fmt 0.166 s, wasm-format 0.113 s, wasm-view 0.129 s.
67 source files; maximum 733 lines. `cargo clippy -p clod-web --target
wasm32-unknown-unknown -- -D warnings` also passes. Residency: 96 CPU cuts,
492,732 triangle occurrences, 84 GPU cuts, zero winding/overlap/pixel errors,
476,941 covered interior pixels and two exact shadow comparisons.

`bun web/build.mjs && bun web/proof.mjs`: exits 0, failures 0. Wasm bytes:
638,595 raw / 364,296 optimized / 153,561 gzip. Gaul first/full load:
1,111.600 / 1,489.300 ms; Washington: 3,106.600 / 5,122.400 ms (lazy preparation
included). Five parity frames / 4,608,000 pixels, maximum RGB error 3/255,
maximum normalized mean 1.914828431e-7. Four HTTP Range checks, 17 input controls,
naive toggle, 18/18 hero pages and DPR/resize checks pass. Browser 600-frame
interval p50/p95/p99/max: 16.700/17.300/17.500/17.700 ms; reported overflow 0.
The proof's Chrome PID 87830 was killed and reaped. Full measurements and every
page's upload time: [F3 browser proof](results/f3-web.json). I inspected the new
Gaul cluster-colour still; its hair, torso and ground shadow retain their framing.

F3 disposition: **1–9 and 11 fixed; 10 not reproduced** (512 real + 256 synthetic
grazing cameras). Nothing deferred. Selection ≤1.5 ms and flawless native refresh
pacing remain unachieved goals, not claims of this fix. Two 1440p reel stills cover
7,372,800 pixels with zero changed bytes; the existing reel is retained.
<!-- F3_RESULTS_END -->

The existing reel is `<cache>/out/reel-l3b/cluster-lod-reel.mp4` (2560×1440,
2,400 frames, 40 s, 27,202,779 bytes) and `cluster-lod-reel-1080p.mp4`
(1920×1080, 15,423,430 bytes). Both were completely decoded with zero errors.
Endpoint comparisons for this revision: [F3 still digests](results/f3-stills.json).
Historical L3a, F1/F2, L3b and L4 tables and superseded decisions are preserved
in [the pre-F3 archive](results/pre-f3.md); they are not current performance claims.

## Known limits

The native player and browser both work; demo/time are Metal-only. Browser parity
checks Gaul's five frames; Washington loads and is inspected, not in that parity
set. One GPU, seven timing samples, no machine-wide load isolation. No promise of
zero popping, cross-ISA bake identity, subpixel coverage or global geometric error.
No eviction or occlusion hierarchy; partial residency scans full metadata. Selection
still serializes each instance's candidate range. Washington retains seven total
stopped transitions (six direct boundary rejects); the bake does not repair input.
The full baked BVH is unused by selection. GPU residency excludes driver overhead;
the native player keeps both renderer modes resident for instant toggling.
