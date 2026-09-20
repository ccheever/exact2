# F1 historical cost of requiring manifold interiors

These are the F1 v2 measurements. F2 supersedes the criterion: four-incident,
zero-winding edges are balanced pinches, not holes or a real bake defect.
The 21-fixture F2 evidence is in [f2-unfiltered.json](f2-unfiltered.json).

## Results

Measurements below are on Apple M5 Max / Metal, wgpu 30.0.1, opt-level=2,
debug=false, incremental=false. `<out>` means `<cache>/out/F1/`.
[Round-1 archive](results/round-1.md) preserves every prior README number for
provenance; its v1 timings and earlier validation claims are superseded.

### Topology and regression evidence

Command: `cargo test -p clod-bake --test topology -- --nocapture`.
Before: 21 fixtures, 315 uniform +10,500 oriented camera cuts, 378 failed cuts,
616,144,192 triangle occurrences, 62.419445333 s. The first topology-only fix
checked 622,100,574 occurrences in 26.939051209 s with zero failures; subsequent
normal normalization changes the deterministic fixture bake. Final counts are in
the verification table below. Detailed failures include every edge's incident
clusters, group IDs, depths and both projected errors in `<out>/topology-before.log`.
Smallest observed source: `<out>/subdivision-5-seed-0.ply` (8,192 triangles).

Uniform cuts fail too. All 21 original fixtures had zero ancestor overlaps and
zero sphere-containment violations at 1e-6 tolerance. Edge (594,2331), formerly
2 incidences in child group 4, had 4 in replacement clusters 21/22 at depth 2;
simplified/refined errors 4.940409/2.817221 px, threshold 3.167388 px. Thus this was
a balanced pinch rejected by the over-strict manifold criterion, not projection nonmonotonicity, early terminals,
FLT_MIN or a wrong refined link. Rejected transitions preserve the finer patch.

| Review item | Failing evidence before fix | Passing gate / disposition |
| --- | --- | --- |
| Root cause | `topology-before.log`: 378 bad cuts, including uniform | `multi_fixture_closed_cuts`: 21 fixtures ×515 cuts |
| 1: zero threshold | `projection-before.log`: 8/9 cases drew 24,382 instead of 32,768 | CPU, envelope GPU and brute GPU: 9/9 source cuts |
| 2: vacuity/coverage | `lod-disabled-old-oracle.log` passed forced-original; new gate failed it. Injected crack passed mean gate | Reduction/diversity/mixed cuts, 96 scan coverage cameras and two crack controls |
| 3: reader | `reader-before.log`: eight new corruptions accepted | 36 malformed files rejected, five accessor-overflow and four writer-arithmetic cases |
| 4–5: depth/scene | `render-before.log`: equal depths, nonuniform/zero extent accepted | 0.01-separated surfaces at distance 100, both render paths; scene errors |
| 6: fairness/timing | Unculled naive count; invalid timestamp counters; wrap failed | Same instance cull, eight on/off timing cases, five arithmetic cases, 52 timing commands |
| 7: FFI | Borrowed pointers remained; unchecked narrowing | Clear pointers, checked conversions, exercised by all fixture/scan bakes |
| 8: normals | `inputs-before.log`: six extreme-normal cases failed | Six finite cases preserve direction; nonfinite inputs rejected before FFI |
| 9: glTF | Mixed primitive lost authored normals | Preserve authored primitive; missing buffer panic **not reproduced**, index 99 already Err |
| 10–11: scene/limits | Zero extent accepted; naive cluster limit rejected; oversized baseline panicked | Error before GPU allocation; unused 134,217,856-byte cluster table accepted in naive |
| 12–13: tooling/docs | Python runner and superseded notebook | Bun verify/sweep/oracles; structured README and historical numeric archive |

Pop remains a report, not a quality test. No pop-bound claim is made. FFI structural
repairs were verified through bakes rather than fabricated runtime failing cases.

### Verification

Command: `bun measure.mjs verify`. 18 tests passed; zero GPU/asset skips.

| Command | Exit | Wall seconds |
| --- | --- | --- |
| `cargo test --workspace --no-fail-fast -- --nocapture` | 0 | 72.81098458299999 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 1.4698457909999998 |
| `cargo fmt --all -- --check` | 0 | 0.14374258299999929 |
| `cargo build -p clod-format --target wasm32-unknown-unknown` | 0 | 0.10872258400000283 |
| `cargo build -p clod-view --lib --target wasm32-unknown-unknown` | 0 | 0.9148390840000066 |

45 source files; maximum 705 lines (`view/src/gpu.rs`); two WGSL shaders validate with no capabilities. Rendering: zero required features, eight storage bindings/stage, 134,217,728-byte storage binding and 268,435,456-byte buffer limits.

| Gate | Measured result |
| --- | --- |
| 21-fixture closed topology | 10,815 cuts, 621,330,298 triangle occurrences, 0 failures, 33.439433416 s |
| L1 camera cuts | 240 cameras, 105 distinct sizes, 2,038–524,288 triangles, 28,936,928 occurrences including 15 uniform cuts; zero edges/overlaps |
| Procedural threshold 0 | 306,628 culled triangles from 524,288 source; RGB max/mean/fraction = 0 |
| Procedural 1 px | 27,580 triangles +2,372 padding, 17 draws; mean 0.003517335934663217, max 61/255, fraction 0.12141927083333333 |
| Planar threshold 0 | 9 cases ×32,768 triangles, scales 1 or 1e-20, distances 1 or 1e20; all source cuts |
| Depth at 100 / separation .01 | Before both 0.99998206; after 1.8000037e-5 / 1.7998036e-5; both paths return near red [213,2,1,255] |
| Reader / arithmetic | 36 corruptions, 5 accessor overflows, 4 writer cases; all rejected as expected |
| Loaders / normals | 8 formats, 3 malformed inputs, 3 CLI commands; 6 extreme normals; two glTF primitives retain 3 authored +3 generated normals |
| Timing | 8 shadows on/off render cases and 5 modular-arithmetic cases; all pass |
| Shadow depth | 4,532,800 clusters /499,906,400 triangles; configured and sphere-only cuts both match unculled pixels exactly |

Command: `cargo test -p clod-view --test coverage -- --nocapture`. 512², one-pixel silhouette exclusion, fully covered MSAA interior.

| Asset | Cameras / mixed | Interior pixels | Missing | Injected cracks detected |
| --- | --- | --- | --- | --- |
| gaul | 48 / 48 | 3,094,126 | 0 | 1 |
| washington | 48 / 48 | 3,985,199 | 0 | 1 |

Each injected one-pixel crack has mean 0.0000012715657552083333, fraction 0.000003814697265625, max 255/255: the old mean gate accepts it; localized coverage detects it.

Command: `bun measure.mjs oracles` (real scans) and `cargo test -p clod-view --test gpu_selection -- --nocapture` (procedural). All image comparisons include numerical-boundary cases.

| Asset | Cuts / exact images | Pixels | Cull pairs | Deterministic PNGs | Boundary pairs | One-slot drops | Failures |
| --- | --- | --- | --- | --- | --- | --- | --- |
| procedural | 192 / 192 | 7,077,888 | 96 | 192 | 0 | 15,114 | 0 |
| gaul | 192 / 192 | 12,582,912 | 96 | 192 | 1 | 2,019,560 | 0 |
| washington | 192 / 192 | 12,582,912 | 96 | 192 | 1 | 8,995,784 | 0 |

The procedural GPU oracle also checks 5,968,822 decoded triangles. Real scan closed-edge count is intentionally zero. Threshold-zero scan tests check 12 comparisons at 2560×1440, both triangle orders, t=0/.5/1: every max/mean/fraction is zero (18 GPU frames).

Command: `cargo test -p clod-bake --test oracles -- --nocapture`; 100,000 samples per row, one-sided cut→source.

| Threshold | Cut triangles | Claimed refined error | Max distance | RMS | Max / claim | 4× +1e-6 gate |
| --- | --- | --- | --- | --- | --- | --- |
| 0.0010000000474974513 | 259,552 | 0.0009929724037647247 | 0.0007128301127924904 | 0.00008782397147560541 | 0.7178750487827139 | 0.003972889615058899 |
| 0.009999999776482582 | 16,332 | 0.009320216253399849 | 0.005378318276093772 | 0.0011488024152074676 | 0.577059386806809 | 0.0372818650135994 |
| 0.10000000149011612 | 2,038 | 0.04534897953271866 | 0.03280584918284454 | 0.005445760300585687 | 0.7234087629066841 | 0.18139691813087463 |
| 1 | 2,038 | 0.04534897953271866 | 0.030166941867430696 | 0.005430983040835787 | 0.6652176560150745 | 0.18139691813087463 |

### Repeated v2 bakes

Commands: `target/release/clod-bake <source> <cache>/out/<asset>-2.clod`, repeated to `<asset>-2-repeat.clod`. Sources are the cached SMK STL and Smithsonian OBJ named below. Full phase records: `<out>/<asset>-bake-{0,1}.log`.

| Asset | Source triangles / vertices | Clusters / groups / BVH | Depth / pages | File bytes | B/source triangle | Terminal triangles |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 4,000,020 / 1,999,991 | 62,852 / 3,921 / 4,494 | 6 / 4 | 132,955,424 | 33.238689806550965 | 296,378 |
| washington | 16,860,930 / 9,022,298 | 269,362 / 16,851 / 19,275 | 7 / 17 | 597,206,000 | 35.41951719151909 | 1,249,766 |

| Asset/run | Load s | Normals s | Build s | Encode s | Write s | Peak RSS bytes |
| --- | --- | --- | --- | --- | --- | --- |
| gaul/0 | 1.396051666 | 0.102508667 | 6.809361041 | 0.622238917 | 0.018604125 | 683,212,800 |
| gaul/1 | 1.615406375 | 0.0767055 | 6.542359542 | 0.574022583 | 0.023276416 | 681,607,168 |
| washington/0 | 10.680276375 | 0.1151795 | 26.470118792 | 3.143933583 | 0.189276417 | 2,654,011,392 |
| washington/1 | 10.524749 | 0.098171666 | 29.162845375 | 2.7213552500000002 | 0.225657916 | 2,652,061,696 |

gaul: source `smk-dying-gaul-kas1312/smk-190-inv-dying-gladiator.stl`, SHA-256 `4246ebd08faad0d3a83adf9d77e1117a509e98cfe93afe5e3ca36abc1ef7c2b2`. Both baked outputs: **`04614794a20f516ffaf79d835dd9476de38968a4a94ff73c08d95d4c61eddefc`**. Triangles per depth: 4,000,020, 1,998,656, 986,540, 453,509, 184,718, 62,586, 14,494; clusters per depth: 31,491, 16,767, 8,445, 3,899, 1,590, 535, 125.

washington: source `si-george-washington-greenough/george-washington-greenough-statue-(1840)-master-geometry.obj`, SHA-256 `ce558e481460d31678b1e31d4a18602cbede7a89fa83adddf109c3bd5be53cb9`. Both baked outputs: **`98d74ea42db9d4cf95fce58e2dba6c7e320f72bd3c93e4f75ae73f0e673a6f7a`**. Triangles per depth: 16,860,930, 8,408,464, 4,134,230, 1,924,118, 806,474, 267,311, 51,427, 3,224; clusters per depth: 133,228, 72,178, 36,448, 17,233, 7,303, 2,459, 483, 30.

### Timings

Command: `bun measure.mjs sweep`. Apple M5 Max / Metal; 2560×1440, t=0, 1 px, one warmup +7 measured frames, median milliseconds. Both modes use the same instance-frustum cull. GPU select includes main and shadow selection; CPU includes selection +encode/submit. Shadows-on clusters use 2 px shadows; naive uses full resolution. Off rows remove the shadow pass. Main-pass comparisons therefore lead. Individual stage medians need not sum to the median total.

| Scene | Path / shadows / quota | Main ms | Shadow ms | GPU select ms | CPU ms | Main triangles | Drops main / shadow |
| --- | --- | --- | --- | --- | --- | --- | --- |
| gaul/single | gpu / on / 62852 | 0.733167 | 0.143584 | 0.436291 | 0.423792 | 289,677 | 0 / 0 |
| gaul/single | brute / on / 62852 | 0.591791 | 0.116583 | 1.567333 | 0.533875 | 289,677 | 0 / 0 |
| gaul/single | cpu / on / — | 1.719000 | 0.336625 | 0.000000 | 1.996583 | 289,677 | 0 / 0 |
| gaul/single | naive / on / — | 1.060458 | 0.434834 | 0.000000 | 0.232500 | 4,000,020 | 0 / 0 |
| gaul/single | gpu / off / 62852 | 0.540708 | 0.000000 | 0.317542 | 0.291916 | 289,677 | 0 / 0 |
| gaul/single | naive / off / — | 0.788125 | 0.000000 | 0.000000 | 0.133625 | 4,000,020 | 0 / 0 |
| gaul/ring:12 | gpu / on / 62852 | 0.915375 | 0.535000 | 0.151167 | 0.470792 | 3,401,146 | 0 / 0 |
| gaul/ring:12 | brute / on / 62852 | 1.285166 | 0.732208 | 1.452499 | 0.425750 | 3,401,146 | 0 / 0 |
| gaul/ring:12 | cpu / on / — | 1.412083 | 0.868625 | 0.000000 | 4.070584 | 3,401,146 | 0 / 0 |
| gaul/ring:12 | naive / on / — | 2.505209 | 2.191792 | 0.000000 | 0.216709 | 48,000,240 | 0 / 0 |
| gaul/ring:12 | gpu / off / 62852 | 1.470541 | 0.000000 | 0.131750 | 0.295583 | 3,401,146 | 0 / 0 |
| gaul/ring:12 | naive / off / — | 2.408292 | 0.000000 | 0.000000 | 0.194250 | 48,000,240 | 0 / 0 |
| gaul/grid:400 | gpu / on / 10485 | 22.331875 | 15.619375 | 0.774291 | 0.435541 | 111,876,590 | 0 / 0 |
| gaul/grid:400 | brute / on / 10485 | 22.307000 | 15.630833 | 4.246292 | 0.476750 | 111,876,590 | 0 / 0 |
| gaul/grid:400 | cpu / on / — | 22.616708 | 15.743959 | 0.000000 | 76.129291 | 111,876,590 | 0 / 0 |
| gaul/grid:400 | naive / on / — | 74.608667 | 68.396459 | 0.000000 | 0.257084 | 1,600,008,000 | 0 / 0 |
| gaul/grid:400 | gpu / off / 10485 | 22.193583 | 0.000000 | 0.380250 | 0.361458 | 111,876,590 | 0 / 0 |
| gaul/grid:400 | naive / off / — | 75.965667 | 0.000000 | 0.000000 | 0.262084 | 1,600,008,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / on / 838 | 98.858916 | 64.904292 | 8.284375 | 0.509792 | 492,021,329 | 7,786,187 / 8,435,000 |
| gaul/field:5000,1 | brute / on / 838 | 98.824083 | 65.643875 | 46.562334 | 0.533792 | 492,021,329 | 7,786,187 / 8,435,000 |
| gaul/field:5000,1 | cpu / on / — | 289.722625 | 204.712250 | 0.000000 | 1421.765291 | 1,416,304,398 | 0 / 0 |
| gaul/field:5000,1 | naive / on / — | 1286.603292 | 1196.325291 | 0.000000 | 0.700500 | 20,000,100,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / off / 838 | 105.177875 | 0.000000 | 4.475292 | 0.539626 | 492,021,329 | 7,786,187 / 0 |
| gaul/field:5000,1 | naive / off / — | 1267.483875 | 0.000000 | 0.000000 | 0.494292 | 20,000,100,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / on / 3000 | 357.430375 | 248.213292 | 11.890125 | 0.746125 | 1,416,304,398 | 0 / 0 |
| gaul/field:5000,1 | gpu / off / 3000 | 369.544750 | 0.000000 | 6.315834 | 0.516084 | 1,416,304,398 | 0 / 0 |
| washington/single | gpu / on / 269362 | 1.017375 | 0.360917 | 1.391334 | 0.629666 | 1,186,453 | 0 / 0 |
| washington/single | brute / on / 269362 | 0.652834 | 0.221125 | 4.246875 | 0.521334 | 1,186,453 | 0 / 0 |
| washington/single | cpu / on / — | 2.854000 | 1.050292 | 0.000000 | 10.340625 | 1,186,453 | 0 / 0 |
| washington/single | naive / on / — | 1.808333 | 1.150167 | 0.000000 | 0.227833 | 16,860,930 | 0 / 0 |
| washington/single | gpu / off / 269362 | 0.810375 | 0.000000 | 0.926709 | 0.391583 | 1,186,453 | 0 / 0 |
| washington/single | naive / off / — | 1.728084 | 0.000000 | 0.000000 | 0.215168 | 16,860,930 | 0 / 0 |
| washington/ring:12 | gpu / on / 269362 | 3.714708 | 2.145250 | 0.473167 | 0.537625 | 13,611,402 | 0 / 0 |
| washington/ring:12 | brute / on / 269362 | 3.703208 | 2.180667 | 4.262751 | 0.490667 | 13,611,402 | 0 / 0 |
| washington/ring:12 | cpu / on / — | 3.718750 | 2.182500 | 0.000000 | 30.492167 | 13,611,402 | 0 / 0 |
| washington/ring:12 | naive / on / — | 10.421709 | 8.951791 | 0.000000 | 0.262541 | 202,331,160 | 0 / 0 |
| washington/ring:12 | gpu / off / 269362 | 3.637333 | 0.000000 | 0.245500 | 0.353625 | 13,611,402 | 0 / 0 |
| washington/ring:12 | naive / off / — | 9.855667 | 0.000000 | 0.000000 | 0.188709 | 202,331,160 | 0 / 0 |
| washington/grid:400 | gpu / on / 10485 | 117.542375 | 65.565916 | 2.160792 | 0.571750 | 464,852,668 | 14,051 / 338,800 |
| washington/grid:400 | brute / on / 10485 | 116.210042 | 65.570917 | 20.132667 | 0.592584 | 464,852,668 | 14,051 / 338,800 |
| washington/grid:400 | cpu / on / — | 114.643583 | 70.762250 | 0.000000 | 658.895209 | 466,600,422 | 0 / 0 |
| washington/grid:400 | naive / on / — | 428.439500 | 357.158209 | 0.000000 | 0.345000 | 6,744,372,000 | 0 / 0 |
| washington/grid:400 | gpu / off / 10485 | 126.044541 | 0.000000 | 1.207792 | 0.675499 | 464,852,668 | 14,051 / 0 |
| washington/grid:400 | naive / off / — | 523.388625 | 0.000000 | 0.000000 | 0.376168 | 6,744,372,000 | 0 / 0 |
| washington/field:5000,1 | gpu / on / 838 | 106.213625 | 71.173667 | 23.835417 | 0.789417 | 459,013,110 | 46,525,494 / 52,470,000 |
| washington/field:5000,1 | brute / on / 838 | 148.147584 | 101.576375 | 308.533709 | 0.878042 | 459,013,110 | 46,525,494 / 52,470,000 |
| washington-field-5000-1-cpu-shadows-on | **ERROR** | — | — | — | — | — | page 0 visible list exceeds 128 MiB core storage binding |
| washington/field:5000,1 | naive / on / — | 5036.812875 | 4586.307042 | 0.000000 | 0.807042 | 84,304,650,000 | 0 / 0 |
| washington/field:5000,1 | gpu / off / 838 | 97.746292 | 0.000000 | 11.691416 | 0.639416 | 459,013,110 | 46,525,494 / 0 |
| washington/field:5000,1 | naive / off / — | 5189.691791 | 0.000000 | 0.000000 | 0.516583 | 84,304,650,000 | 0 / 0 |
| washington/grid:400 | gpu / on / 12000 | 115.171750 | 70.338875 | 2.119624 | 0.758917 | 466,600,422 | 0 / 0 |
| washington/grid:400 | gpu / off / 12000 | 114.340458 | 0.000000 | 1.128125 | 0.408125 | 466,600,422 | 0 / 0 |

51 rows completed; every one has seven valid samples for each reported GPU stage.
One row fails explicitly, so `sweep` exits 1 after reporting all 52 configurations.
Rows with drops are incomplete images and support no speedup claim. Washington
field:5000 CPU errors because page zero exceeds its 128 MiB visible-list limit.
Washington field cannot fit its complete terminal cut in the GPU's 128 MiB binding
either. The 12,000-slot Washington grid and 3,000-slot Gaul field rows have zero
drops. Raw exact values, every command and PID remain in `<out>/*shadows-*.log`,
`processes.jsonl` and `runs.jsonl`.

### Image and camera-path reports

Commands: `clod-view compare <asset>-2.clod --out <out>/<asset>-compare` and `clod-view pop <asset>-2.clod --out <out>/<asset>-pop`. Compare: five hero positions ×five thresholds ×two assets, 2560×1440. Each row below is the largest mean error among five positions at that threshold; maxima/fractions belong to that same pair.

| Asset | Threshold px | t | Mean | Max /255 | Fraction >2/255 | Cluster main triangles |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 0.5 | 1 | 0.0010314499931917212 | 27 | 0.04269911024305555 | 301,154 |
| gaul | 1 | 0.75 | 0.0014355862353622004 | 64 | 0.04125217013888889 | 370,736 |
| gaul | 2 | 0.75 | 0.002080289465323166 | 86 | 0.06336073133680556 | 240,773 |
| gaul | 4 | 0.75 | 0.002420610504039579 | 86 | 0.07476372612847222 | 201,104 |
| gaul | 8 | 1 | 0.003186876616966231 | 85 | 0.1133932834201389 | 114,772 |
| washington | 0.5 | 1 | 0.0009235270714188454 | 27 | 0.03732638888888889 | 53,483 |
| washington | 1 | 1 | 0.0019767486638752724 | 119 | 0.07094780815972222 | 50,347 |
| washington | 2 | 1 | 0.001992602379493464 | 120 | 0.07128228081597222 | 46,501 |
| washington | 4 | 1 | 0.002000243254130356 | 120 | 0.0713916015625 | 41,595 |
| washington | 8 | 1 | 0.002643800069217502 | 120 | 0.09010281032986112 | 35,788 |

Pop is a report, **not a test or a no-popping guarantee**. Two 240-frame paths, 478 temporal pairs.

| Asset | Pairs | Max spatial temporal residual | Worst step | Max excess MAD | Excess step | Failures |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 239 | 0.003003001316267248 | 194 | -0.000014553759872004343 | 239 | 0 |
| washington | 239 | 0.0018133520986519608 | 174 | 0.00018020308528503975 | 232 | 0 |

Final lit/cluster PNGs: `<out>/<asset>-close-{lit,clusters}.png`; winning compare/pop frames are in their output directories. No mesh, baked output, PNG or log is committed. [Detailed measurement tables](results/f1-details.md) retain the per-fixture, camera, image, memory, padding and temporal numbers without lengthening this README. Preliminary F1 numbers before the shadow-cone repair remain in `<out>/pre-shadow-fix/`; those timings are superseded.
