# Cluster LOD on core WebGPU

Standalone experiment for LLP 1041.011 O1 / §5 Q2: bake a cluster-LOD DAG, select
and cull on the GPU, then draw through ordinary hardware rasterization. No software
rasterizer, 64-bit atomics, optional rendering features, or vendor modifications.
The native CLI renders the two scanned statues and a continuous far-to-detail
camera path, including cluster-colour views. It is an offscreen demo, not a player.

F1 found a real bake defect: regular vendor simplification introduced four-incident
edges, including in uniform cuts. The shim now rejects those transitions and keeps
finer geometry. The permanent 21-fixture oracle checks 10,815 cuts with zero bad
edges or ancestor overlaps. This is boundary/edge preservation, not a proof against
geometric self-intersection. The new scans pass 96 mixed-LOD coverage cameras with
zero missing interior pixels. Threshold zero is pixel-exact in the tested images.

At 400 instances, 2560×1440 and 1 px, the main-pass medians are **22.332 vs 74.609 ms
(Gaul)** and **115.172 vs 428.440 ms (Washington)**, GPU cluster vs indexed naive.
The corresponding shadow medians are 15.619 vs 68.396 ms and 70.339 vs 357.158 ms.
These runs have zero drops; Washington uses a 12,000-cluster per-instance quota.
Shadows use twice the main error threshold, so the main-pass comparison leads.
The topology fix raises the minimum geometry substantially; the previous performance
headline is withdrawn. Quota-overflow rows draw incomplete images and support no speedup claim.

## How to run

Use this directory and the launch environment (debug info off, incremental off).
There is one target directory, `target/`; this is an independent Cargo workspace.
All assets, bakes, exported meshes, PNGs and logs belong in
`~/Library/Caches/exact2-cluster-lod/`, abbreviated `<cache>` below.

```sh
cargo build -p clod-bake -p clod-view
cargo run -p clod-bake -- <input.ply> <cache>/out/mesh.clod --max-triangles 128 --page-mib 32
cargo run -p clod-bake -- --inspect <cache>/out/mesh.clod
cargo run -p clod-bake -- --cut <cache>/out/mesh.clod --threshold 1e30 --obj <cache>/out/terminal.obj
cargo run -p clod-bake -- --generate 8 <cache>/out/sphere.ply
bun measure.mjs verify
bun measure.mjs sweep
bun measure.mjs oracles
```

`verify` runs workspace tests with `--no-fail-fast --nocapture`, clippy with
`-D warnings`, fmt, and both format/view-library wasm32 builds; it reports all
command failures and source-file lengths. `sweep` runs 48 default-quota timing
configurations plus four larger-quota runs for complete Gaul field/Washington grid cuts.
`oracles` checks 64 cameras × three layouts per real asset. The scripts record
commands, PIDs, exit codes and elapsed times in `<cache>/out/F1/`.

```sh
asset="$HOME/Library/Caches/exact2-cluster-lod/out/washington-2.clod"
out="$HOME/Library/Caches/exact2-cluster-lod/out/F1/demo"
target/debug/clod-view render "$asset" --out "$out/lit.png" --path hero --t 1
target/debug/clod-view render "$asset" --out "$out/clusters.png" --view clusters --t 1
target/debug/clod-view time "$asset" --out "$out/timing.png" --layout grid:400 --capacity 12000 --frames 7
target/debug/clod-view time "$asset" --out "$out/naive.png" --layout grid:400 --mode naive --shadows off
target/debug/clod-view compare "$asset" --out "$out/compare" --threshold-px 0.5,1,2,4,8 --t 0,0.25,0.5,0.75,1
target/debug/clod-view pop "$asset" --out "$out/pop" --threshold-px 1 --steps 240
cargo test -p clod-bake --test topology -- --nocapture
cargo test -p clod-view --test coverage -- --nocapture
```

Defaults: `--mode cluster --select gpu --cull on --shadows on --view lit`,
1 px, 2560×1440, vertical FOV 45°, hero t=0. Selectors: `gpu|cpu|brute`;
`brute` scans all clusters of surviving instances. Layouts: `single|ring:N|grid:N|field:N,seed`,
N=1..10,000. Views: `lit|clusters|depth|triangles|instances|overdraw|coverage`.
`depth` is DAG depth. `coverage` has white geometry, magenta background, no ground
or shadows. Cameras accept `--eye x,y,z --target x,y,z --fov degrees`; size caps
at 8192². `--capacity N` is a per-instance GPU quota, not a total budget.

The library accepts byte slices and caller-owned wgpu devices. Native I/O, PNG
encoding, blocking readback and meshoptimizer baseline optimization stay in the
binary. A browser supplies its own optimized baseline buffers. Only `time`
requires `TIMESTAMP_QUERY`; it errors if unavailable or invalid. Rendering uses
`Limits::default()` and no required features. Bake emits one JSON record; view
commands emit JSON lines, including errors with exit 1. `compare` and `pop` are
measurement reports; the permanent quality gates are the cargo tests.

## Format v2

Little endian, magic `CLOD0002`, version 2. Version 1 is rejected because the
accepted DAG changed. `format/src/lib.rs` defines the `repr(C)`/`Pod` records.
Sections, in order: header, clusters, groups, page table, optional BVH, geometry.
Section starts and page arrays align to 16 bytes; all padding is zero.

| Record | Bytes | Contents |
| --- | ---: | --- |
| Cluster | 128 | Culling sphere/cone, simplified/refined bounds, group IDs, page/ranges, depth, reserved zeros |
| Selection bounds | 20 | Float32 centre/radius/error |
| Group | 32 | Simplified bounds, depth, cluster range |
| BVH node | 32 | Bounds, group or children; one root per depth |
| Page | 80 | u64 extent, SHA-256, cluster/vertex/index ranges, reserved zeros |
| Vertex | 20 | Float32 xyz, octahedral snorm16×2 normal, RGBA8 |

A cluster has at most 256 vertices and 128 triangles with three u8 local indices
per triangle. Vertices then indices occupy each page; no cross-page pointers.
Pages default to 32 MiB, configurable from 4 KiB to 128 MiB. All terminal groups
must fit page zero. Positions retain source float32 bits. RGBA is low-byte red;
`HAS_COLOR` is set iff at least one stored colour differs from opaque white.
Unpack normals by signed division by 32767, hemisphere unfold and normalization.

The aligned, borrowing reader validates canonical section/page layout, all ranges,
indices, reserved words and zero padding (header gap, section tails, page arrays),
page SHA-256, finite geometry/culling records, group coverage, original triangle
count, depth/order/error monotonicity, matching refinement bounds, and terminal iff
unreferenced. Each ancestor group sphere must contain its directly referenced
child sphere; each cluster culling sphere must contain its decoded vertices.
Containment allows `8 * f32::EPSILON * max(abs(centres), radii)` world units,
evaluated in f64. Transitive containment inherits this per-edge rounding tolerance.
BVH checks cover reachability, unique leaves and exact leaf/group agreement;
internal-node geometric containment and normal-cone correctness are not validated.
The reader does not recompute topology: topology preservation is a bake check.

The writer validates its output with the reader. Offset/size arithmetic is checked,
including public geometry/page accessors and Vec address-space limits; a >4 GiB
output on wasm32 is an error, not a wrap. The source SHA-256 identifies the primary
input file; external glTF buffers are additional inputs. The format crate needs
neither filesystem access nor native code.

## Selection rule

Select a cluster iff `projected(simplified) > threshold` and either its refinement
ID is `ORIGINAL` (`u32::MAX`) or `projected(refined) <= threshold`.
Terminal error `f32::MAX` is a sentinel. Uniform cuts use stored world-space errors.
One CPU function, `format::projection::Projection::projected`, and its WGSL mirror use:

```text
perspective = error * scale / max(length(transformed_center - eye) - radius * scale, near)
              * cot(fovy / 2) * 0.5 * viewport_height
orthographic = error * scale * viewport_height / orthographic_span
```

The sentinel stays MAX. Zero stays zero; positive projected errors saturate at
MIN_POSITIVE, including underflow. Threshold zero bypasses candidate pruning and
therefore selects original geometry. Length uses max-component scaling. The scene
contract is positive, uniform, orthogonal affine scale (relative tolerance 2e-5);
non-uniform scale, shear, reflection, nonfinite transforms and zero asset extent
return errors. Thresholds are finite, nonnegative and below MAX. Camera orientation
does not enter the distance formula; the multi-fixture test applies random rigid
orientations to both bounds and camera. Culling remains separate from selection.

## Decisions

1. Preserve position bits; pack normals only. Identical border coordinates are
   necessary but insufficient for a crack-free cut: decision 32 also checks topology.
2. Disable permissive/sloppy simplification and lock borders. Accumulate error as
   `max(previous, current) + current`. Regular simplification still needs decision 32.
3. Pack terminals first, then descending depth and original ID. Page zero must hold
   the complete terminal cut; fail when its configured budget cannot.
4. Lift zero group errors to MIN_POSITIVE. Projection must preserve positivity too.
5. Build single-threaded; hash maps are lookup-only and never determine output order.
   Repeat digests are same-host; cross-ISA identity is not claimed for meshoptimizer.
6. glTF uses only `utils`: first mesh, triangle primitives, no scene/materials/images.
   GLB, base64 and plain local buffers work; network/percent-escaped URIs do not.
7. Stream hashed PLY/OBJ/STL. PLY supports ASCII and both binary endiannesses, normals
   and uchar RGB(A), triangular faces only. OBJ fans polygons. STL welds exact positions.
8. Assemble bounded pages then one contiguous output Vec; measure peak process RSS.
9. Keep the displaced octasphere plus 21 seeded fixtures, subdivisions 3..9.
   Each gets 15 uniform cuts and 500 oriented cameras; culling is off for closed edges.
10. Error honesty is **one-sided cut→source**, sampled at 100,000 points per threshold,
    against the maximum selected **refined** error with a **4× + 1e-6** gate.
    Accumulated quadric error is not a Hausdorff bound or a pixel guarantee.
11. Naive geometry comes from ORIGINAL clusters, welded on complete vertex records,
    then cache/fetch optimized. It uses indexed buffers and instancing.
12. CPU reference evaluates each group once per instance, then selects clusters.
    Scale affects error and radius. Projection has one CPU implementation.
13. Draw once per page per pass, including empty pages. Short clusters pad to the
    configured triangle count with clipped vertices; report padding separately.
14. Normalize the longest dimension to 2 world units, bottom at Z=0. Washington's
    source digest identifies its Y-up basis. Layout randomness uses a fixed 32-bit LCG.
15. Hero anchors hit actual hair triangles. A smoothstep path ends 0.055 world units
    from the hit, near=0.002. There is no geomorphing or guarantee of zero popping.
16. Use a 2048² orthographic shadow map at twice the main threshold, nine samples,
    bias, Lambert/GGX (.32 roughness, .04 F0), ambient, tonemap, sRGB and 4× MSAA.
17. `time` takes seven frames after one warmup. Report main/shadow/select GPU medians
    separately; CPU encode includes uploads/submission. Completion includes readback.
18. Cluster-only depth/triangle/cluster colours avoid adding primitive-ID features
    or duplicating naive vertices. Overdraw uses Always depth and no culling.
19. RGB errors are sRGB bytes /255, alpha excluded; a differing pixel exceeds 2/255.
    Pop reports signed-frame-difference excess and spatial temporal residual.
20. Gate procedural 1 px mean <.008 and fraction <.20, plus triangle reduction <50%.
    Those averages do not detect cracks; decision 38 adds localized coverage.
21. CPU page lists cap at 128 MiB; baseline chunks at 128 MiB indices/120 MiB vertices.
    GPU residency counts allocated buffers/textures, excluding driver overhead.
22. Compare/pop retain current and worst pairs; `time` retains timing records and
    only its last pixel frame. PNGs are written once per retained result.
23. Both paths evaluate `light * (model * position)` with invariant clip positions.
    Threshold-zero tests now require exact pixels for both baseline triangle orders.
24. GPU candidates use suffix-max/prefix-min error envelopes in baked order; radius
    padding 1.00002 and range guard 1e-5 only widen candidates. Final predicate is exact.
25. Instance/cluster spheres and perspective cone culls use a 1e-5 guard.
    Main and shadow frusta are separate; overdraw disables culling. Shadows use
    sphere/frustum culling and hardware backfaces, as required by decision 42.
26. Default per-instance quota is min(cluster count, floor(4,194,304 / instances)).
    Stable scans drop later IDs on overflow and count every drop. Quotas do not share
    spare space; overflow means an incomplete image, not a valid cut.
27. Resolve timestamp queries after frame GPU completion, before another frame,
    then read them in a second submission. Resolve only active counters. Modular
    subtraction handles wrap; missing/stale/ambiguous counters return errors.
28. The subdivision-6 bad edges are real. The permanent oracle includes smaller
    fixtures and uniform cuts; subdivision 5/seed 0 is the smallest observed failure.
29. No permissive/sloppy fallback, HZB, streaming or optional rendering feature is added.
    Retained finer terminals establish a measured geometry floor.
30. GPU frame work uploads fixed uniforms, runs fixed scans and page loops; instances
    and metadata upload at load. Diagnostic readbacks never decide a draw.
31. Sum every terminal group, including early depths. The deepest level alone is
    not the terminal floor. F1 remeasures the new v2 floors below.
32. Compare oriented boundary/nonmanifold-edge signatures of every replacement patch.
    Stop invalid transitions and dependent ancestors, remove their replacements and
    retain remaining finer clusters as terminals. Vendor unchanged; format bumped to v2.
33. Stable CPU/WGSL length and positive projection saturation cover tiny scale/far eye.
    The planar regression must draw all 32,768 source triangles in nine selector cases.
34. Reader sphere tolerance and canonical colour/padding rules are explicit above.
    Validate checked sizes before allocation; naive never binds a cluster table.
35. Normalize authored/generated normals in f64, validate before FFI, and compute
    missing glTF normals per primitive. Clear borrowed FFI pointers and check narrowing.
    Bad-buffer panic did not reproduce: glTF already rejected index 99; lookup is checked.
36. Main perspective uses reversed-Z, clear 0, Greater. Shadows remain forward-Z.
    At distance 100, 0.01-separated surfaces resolve in both paths.
37. Naive applies the same instance-sphere frustum cull per pass as clusters; render
    surviving contiguous instance ranges in order. Measure both with shadows off too.
38. Keep pop a CLI report, not a test. A forced-original mutation fails the reduction
    gate. A one-pixel injected crack fails coverage even when the old mean gate accepts.
39. Coverage erodes the fully covered naive MSAA mask by a one-pixel Chebyshev band.
    Partial MSAA pixels belong to the silhouette; test every remaining background hole.
    Use 16 hero, 16 ring-interior and 16 grid-interior cameras per asset, requiring ≥32
    nonempty mixed-depth cuts. Do not run a closed-manifold oracle on open scans.
40. Use Bun for measurement orchestration. Preserve superseded measurements in an
    explicitly historical archive; current tables must come from v2 and reversed-Z.
41. Equality oracles reserve the full 128 MiB core visible-list binding budget,
    divided per instance, to check complete cuts of the larger v2 terminal patches.
    Default-quota timings remain separate, and the one-slot overflow oracle remains.
42. Disable shadow cone rejection in CPU/WGSL: on Washington's grid it removed a
    rasterized texel (depth 0.48806113 → 0.48824522), while sphere-only culling was
    exact. Keep the exact-depth oracle; retain hardware backface culling. Re-measure
    timings after this change. The shader no longer carries an unused light direction.
43. Check CPU/GPU images even when a set difference is within the existing 1e-5
    numerical boundary band. Closed-edge diagnostics apply only to closed fixtures;
    the real scans use image equality and localized coverage.

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
invalid replacement topology, not projection nonmonotonicity, early terminals,
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

## Known limits

All pages are resident. The hero required about 775 MiB in the earlier run;
current exact byte counts are in the timing tables. A browser tab will not hold
this whole hero reliably: page residency/streaming is future work, and a web demo
must use a smaller asset or a page budget. This lane provides a Wasm library, not
a deployed web demo or interactive native player.

Quota overflow drops geometry. Some large scenes also exceed the CPU reference's
128 MiB page-list limit. Such measurements are failures or degraded-image rows,
not evidence of crack-free rendering or valid speedups. The current topology filter
is conservative and can keep large early terminal patches; the resulting raster
floor is a real limitation of this experiment.

The error-honesty oracle is one-sided, sampled and uses a 4× refined-error gate.
Coverage tests detect fully missing interior pixels, excluding one pixel of the
MSAA silhouette; they do not prove a global geometric or subpixel error bound.
No geomorphing, occlusion hierarchy, streaming, software rasterizer or fallback
simplifier is implemented. Cross-ISA byte identity is not claimed.
