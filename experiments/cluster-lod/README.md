# Cluster LOD on core WebGPU

Standalone experiment for LLP 1041.011 O1 / §5 Q2: bake a cluster-LOD DAG, select
and cull on the GPU, then draw through ordinary hardware rasterization. No software
rasterizer, 64-bit atomics, optional rendering features, or vendor modifications.
The native CLI renders the two scanned statues and a continuous avenue-to-detail
camera path, including lit cluster colours, baked AO, and a 40-second 60fps H.264
reel. It is an offscreen demo, not an interactive player. L3a uses format v4;
the F2 performance tables below remain historical v3 measurements.

F1 found four-incident edges in regular vendor simplification, including uniform
cuts. F2 establishes that these are balanced pinches (net winding zero), not holes
or a bake defect. The strict-manifold filter was an over-constraint; it is removed.
The bake now rejects only changes to the oriented boundary chain.

watertight: every group transition preserves the oriented boundary chain, so every
cut of a closed mesh is closed; not guaranteed manifold: 503 pinched edges across
501 cuts, worst incidence 4. These are edge-cut occurrences in the 21-fixture sweep;
this statement does not promise absence of geometric self-intersection.

In the historical F2 run, at 400 instances, 2560×1440 and 1 px, main-pass medians are **1.490 vs 78.622 ms
(Gaul)** and **3.852 vs 365.483 ms (Washington)**, GPU cluster vs indexed naive
with the same instance-frustum cull. All four rows have zero overflow. Both
cluster rows use 10,485 slots per instance. Shadows, measured separately, are
0.313 vs 72.492 ms and 2.328 vs 336.622 ms; cluster shadows use 2 px and naive
shadows use full geometry. GPU selection costs 2.114 ms and 0.619 ms respectively
(including shadow selection), separate from the main-pass figures.

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
bun measure.mjs bake # cached scans, two release bakes each
bun measure.mjs verify
bun measure.mjs sweep
bun measure.mjs oracles
bun measure.mjs images # both scans: comparison, 240-frame path, lit/cluster views
```

`verify` runs workspace tests with `--no-fail-fast --nocapture`, clippy with
`-D warnings`, fmt, and both format/view-library wasm32 builds; it reports all
command failures and source-file lengths. `sweep` runs 48 default-quota timing
configurations plus four larger-quota comparison runs; any overflow fails the sweep.
`oracles` checks 64 cameras × three layouts per real asset. The scripts record
commands, PIDs, exit codes and elapsed times in `<cache>/out/L3a/`.

```sh
asset="$HOME/Library/Caches/exact2-cluster-lod/out/washington-4.clod"
out="$HOME/Library/Caches/exact2-cluster-lod/out/reel"
# This Mac's ffmpeg needs the cache-local ABI-215 library (see Decisions).
export DYLD_LIBRARY_PATH="$out/encoder-lib"
target/debug/clod-view reel "$asset" --out "$out" --seconds 40 --fps 60 --size 2560x1440
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
`brute` scans all clusters of surviving instances. Layouts: `single|avenue:N|ring:N|grid:N|field:N,seed`,
N=1..10,000. Views: `lit|clusters|depth|triangles|instances|overdraw|coverage`.
`depth` is DAG depth. `coverage` has white geometry, magenta background, no ground
or shadows. Cameras accept `--eye x,y,z --target x,y,z --fov degrees`; size caps
at 8192². `--capacity N` is a per-instance GPU quota, not a total budget.

The library accepts byte slices and caller-owned wgpu devices. Native I/O, PNG
encoding, blocking readback and meshoptimizer baseline optimization stay in the
binary. A browser supplies its own optimized baseline buffers. `time` and `reel`
require `TIMESTAMP_QUERY`; it errors if unavailable or invalid. Rendering uses
`Limits::default()` and no required features. Bake emits one JSON record; view
commands emit JSON lines, including errors with exit 1. `compare` and `pop` are
measurement reports; the permanent quality gates are the cargo tests.

## Format v4

Little endian, magic `CLOD0004`, version 4. Earlier versions are rejected; the spare
vertex alpha byte now stores ambient visibility, and AO enters simplification. `format/src/lib.rs` defines the `repr(C)`/`Pod` records.
Sections, in order: header, clusters, groups, page table, optional BVH, geometry.
Section starts and page arrays align to 16 bytes; all padding is zero.

| Record | Bytes | Contents |
| --- | ---: | --- |
| Cluster | 128 | Culling sphere/cone, simplified/refined bounds, group IDs, page/ranges, depth, reserved zeros |
| Selection bounds | 20 | Float32 centre/radius/error |
| Group | 32 | Simplified bounds, depth, cluster range |
| BVH node | 32 | Bounds, group or children; one root per depth |
| Page | 80 | u64 extent, SHA-256, cluster/vertex/index ranges, reserved zeros |
| Vertex | 20 | Float32 xyz, octahedral snorm16×2 normal, RGB + AO8 |

A cluster has at most 256 vertices and 128 triangles with three u8 local indices
per triangle. Vertices then indices occupy each page; no cross-page pointers.
Pages default to 32 MiB, configurable from 4 KiB to 128 MiB. All terminal groups
must fit page zero. Positions retain source float32 bits. RGB is low-byte red, AO is the high byte (255 = unoccluded);
`HAS_COLOR` is set iff at least one stored RGB colour differs from white.
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

L3a (2026-09-20; supersedes decisions 15–16): use 25 monuments in an avenue, shared-derivative quintic Hermite
camera segments, a static final hold, and a median-edge-derived 1440p distance
floor. Closest framing adds 0.40 world units to that floor to clear the raised
hand during the orbit; an outward waypoint precedes the descent. `reel` measures
closest original-triangle distance at 2,401 path samples and rejects a median-edge
projection above 2.1 px at 1440p. This samples motion; it is not a continuous
collision proof or a bound on the longest source edge. Keep the existing single/ring/grid/field layouts for measurement. `--path
hero` and `reel` choose the avenue unless `--layout` is explicit. One 4096² shadow
map uses 25 weighted PCF taps; its crop follows the hero and retains the scene's
full light depth, with a soft crop-edge fade. No cascade is needed for this reel.
Warm wrapped diffuse, cool sky, warm ground bounce, broad low specular and ACES
fit tonemapping light both the marble and a 12-colour muted debug palette.

AO budget test: `cargo run --release -p clod-bake --example ao_budget -- <source>`:
9,022,298 Washington vertices, 249,990 proxy triangles, 144,356,768 rays,
13.484045458 s, minimum 0/255, mean 194.541543740/255. Accept AO: 16 fixed cosine
hemisphere rays per vertex, radius 3.5% of extent, regular meshoptimizer proxy
with 250,000 target triangles and 0.002 relative error, bias max(0.00008 extent,
1.2 × proxy error), two integer adjacency smoothing passes. Up to 16 workers write
disjoint vertices; sample sets, reductions and output order do not depend on worker
count. No SSAO. Format v4 reassigns the spare alpha byte to AO; RGB remains colour,
source transparency is outside this opaque-scan demo. Rays ignore back-facing proxy exits so an original vertex below a simplified
face does not occlude itself. Ambient light retains a 22% floor. The CLI always bakes AO;
the low-level writer accepts supplied AO, including neutral 255 in frozen geometry
fixtures. Alpha is a weighted simplification attribute (weight 0.1), like RGB.

The reel uses one raster pass with a screen-space diagonal palette wipe, a built-in
5×7 font and a trailing 30-frame GPU-time median in the caption. Raw per-frame
measurements remain in frames.jsonl. PNGs are the encoding source; two H.264 CRF16
yuv420p outputs use the installed ffmpeg. All media stays in the cache. The installed ffmpeg 8.1 references missing
libx265.215 (system x265 is ABI 217). Supply the exact ABI-215 library from the
[Homebrew 4.1 bottle](https://ghcr.io/v2/homebrew/core/x265/manifests/4.1)
in cache `out/reel/encoder-lib`, via `DYLD_LIBRARY_PATH`; no system file, binary,
or signature is altered. Bottle SHA-256:
`b8a5e68579e954f4bfd2917891880f5861537f87c6787caaf72af7419747450f`.
The runtime was verified with `/opt/homebrew/bin/ffmpeg -version`, exit 0.


F2 evidence: `cargo test -p clod-bake --test topology -- --nocapture`, with the
rejection call temporarily removed, checked 21 fixtures, 10,815 cuts and
614,133,420 triangle occurrences in 31.628988 s. All 503 offending edge occurrences
in 501 cuts were (incidence 4, winding 0); zero non-zero-winding edges. The old
strict assertion intentionally exited 101. [Per-fixture evidence](results/f2-unfiltered.json).

F2 decisions: compare signed position-welded boundary chains, discarding zero
coefficients and incidence counts; zero-length edges contribute zero. Retain the
existing dependent-transition stop for a real chain change. Record direct rejects
and dependent stops separately. Bump the bake to v3 because the accepted DAG changes.
Freeze three valid cuts from sweep cameras 36, 9 and 16 to cover all four
distinct observed pinches in twelve aimed 32×32 image windows; this intentionally
magnifies the pinches without automatic
refinement removing them. CPU uploads each cut to the ordinary cluster hardware
path; the permanent GPU sweep independently checks GPU-selected cuts.
Target subdivision 6/seed 0 for pinch images: it has the most edge-cut pinch
occurrences (109) and the largest simultaneous count (2, tied). Keep F1 costs as
historical results. Do not add a manifold mode. Make sweep exit nonzero on any
overflow so a green
sweep certifies complete measured rows; keep the same 52 configurations for F1
comparison. Bake with the release baker, matching the previous bake measurements.

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
32. Compare oriented boundary chains of every replacement patch; balanced pinches
    cancel at any incidence. F2 removes F1's nonmanifold-edge signature.
    Stop invalid transitions and dependent ancestors, remove their replacements and
    retain remaining finer clusters as terminals. Vendor unchanged; F2 format bumped to v3.
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
    nonempty mixed-depth cuts. Do not require a zero boundary chain on open scans.
40. Use Bun for measurement orchestration. Preserve superseded measurements in an
    explicitly historical archive; current tables must come from v3 and reversed-Z.
41. Equality oracles reserve the full 128 MiB core visible-list binding budget,
    divided per instance. This was needed by F1's inflated terminal patches and
    remains the complete-cut oracle budget, independent of performance quotas.
    Default-quota timings remain separate, and the one-slot overflow oracle remains.
42. Disable shadow cone rejection in CPU/WGSL: on Washington's grid it removed a
    rasterized texel (depth 0.48806113 → 0.48824522), while sphere-only culling was
    exact. Keep the exact-depth oracle; retain hardware backface culling. Re-measure
    timings after this change. The shader no longer carries an unused light direction.
43. Check CPU/GPU images even when a set difference is within the existing 1e-5
    numerical boundary band. Closed-edge diagnostics apply only to closed fixtures;
    the real scans use image equality and localized coverage.

## Results

### L3a museum reel (format v4)

Commands: `bun measure.mjs bake`, `bun measure.mjs verify`, `bun measure.mjs oracles`,
and the `reel` command above. Logs and media are in `<cache>/out/reel/`; command
runner logs are in `<cache>/out/L3a/`. The previous v3 tables start below.

AO uses 16 rays/vertex, a 250k-triangle target proxy, 3.5%-extent radius and up to
16 workers. `cargo test -p clod-bake --test ao -- --nocapture` checks 3,362 corner
vertices / 6,400 proxy triangles: crease 248/255 < convex bump mean 255/255;
109,776 baked bytes repeat exactly, zero failures. Saved and opened
`reel/ao-on.png` and `reel/ao-off.png` at hero t=.75: the former reveals the neck,
fingers and cloth recesses; the latter fills those creases with sky light.

| Asset | Vertices | Source triangles | AO proxy triangles | AO rays | AO seconds, first / repeat | File bytes | Mean AO /255 |
|---|---:|---:|---:|---:|---:|---:|---:|
| Gaul | 1,999,991 | 4,000,020 | 249,998 | 31,999,856 | 12.990291 / 18.732870 | 138,247,344 | 214.679651 |
| Washington | 9,022,298 | 16,860,930 | 249,990 | 144,356,768 | 12.107022 / 13.116628 | 626,622,224 | 195.170646 |

Both AO minima are 0/255. Washington total process wall time was 55.939556 /
57.376931 seconds; the first run's load/build/encode/write were 14.503294 /
24.069113 / 2.563524 / 0.226996 seconds. Peak RSS was 2,782,674,944 bytes.
Both assets were baked twice, then independently SHA-256 checked from disk:

- `gaul-4.clod` and `gaul-4-repeat.clod`:
  `8d602c2339227d4e8a36c3d5b8fc13629dd52792190d958607869a28d481e004`.
- `washington-4.clod` and `washington-4-repeat.clod`:
  `431fbed95fc834a5185f4a36fe6dee29540768aecee925ff7fd8a79191e9abb4`.

Washington has 280,965 clusters / 17,147 groups / 18 pages / 26,998 terminal
triangles; 17,132 topology transitions checked, six direct rejects and seven total
stops. AO changes the accepted simplified geometry; the historical F2 counts
are not reused for v4. Low-level frozen topology fixtures still use neutral AO.

Camera preflight: median source edge **0.00102409895** world units; derived
minimum distance **0.890061677** at 1440p/45°/2 px. At **2,401** sampled times,
closest original triangle **0.905398488**, worst t **0.632083356**, projected
median edge **1.966121435 px**. Neither this statistic nor the bake error claims
bounds every source triangle; the scan has variable density.

Real-asset GPU oracle commands each test 64 cameras × three layouts. Combined:
384 exact CPU/GPU image pairs, 384 deterministic repeats, 192 culling-image pairs,
25,165,824 pixels, zero failures. Real-asset oracles also retain deliberate quota
overflow tests; these are separate from the zero-overflow reel.

`clod-view compare <washington-4.clod> --path hero --t 0,0.45,0.75,1
--threshold-px 1 --out <cache>/out/reel/comparison`:

| t | Drawn triangles | Naive visible triangles | Mean absolute RGB /255 | Max byte | Fraction >2/255 |
|---|---:|---:|---:|---:|---:|
| 0 | 1,498,330 | 286,635,810 | 0.001223758 | 113 | 0.037222493 |
| .45 | 617,769 | 50,582,790 | 0.000812995 | 72 | 0.030031738 |
| .75 | 488,844 | 33,721,860 | 0.000498463 | 65 | 0.014690484 |
| 1 | 758,433 | 16,860,930 | 0.001762086 | 78 | 0.050848796 |

### Historical F2 results (format v3)

Apple M5 Max / Metal, wgpu 30.0.1. Viewer/test opt-level=2; release baker;
debug info and incremental compilation disabled. `<out>` means `<cache>/out/F2/`.
The [F1 manifold-cost archive](results/f1-manifold.md) preserves the superseded
measurements, including the 5,000-instance failures. [Earlier measurements](results/round-1.md)
and [F1 details](results/f1-details.md) remain historical.

### Boundary-chain evidence

Command: `cargo test -p clod-bake --test topology -- --nocapture`.
The unfiltered run checked 614,133,420 triangle occurrences in 10,815 cuts
(315 uniform, 10,500 camera cuts). All 503 offending occurrences were balanced
pinches across 501 cuts, representing 16 distinct fixture/edge pairs. No ancestor
overlaps or sphere-containment violations were observed. The final chain-filter
run checks the same geometry and cuts and passes in 31.825762 s.

| Incidence | Net winding | Edge-cut occurrences | Closed-cut failures under F2 |
| --- | --- | --- | --- |
| 4 | 0 | 503 | 0 |
| any | non-zero | 0 | 0 |

| Input | Transitions checked | Direct chain rejects | Total stopped, including dependents |
| --- | --- | --- | --- |
| 21 fixtures | 8,425 | 0 | 0 |
| Gaul | 4,004 | 0 | 0 |
| Washington | 17,186 | 3 | 3 |

Washington still needs three genuine boundary-chain rejections; its existing
boundary/defects must be preserved. Neither scan is asserted to be closed.
The counts above cover every candidate replacement in each bake. Five oracle
controls also distinguish closed edges, a balanced pinch, a missing face, a
flipped face, and odd incidence: all five pass with the expected winding counts.

### Pinches under inspection

Command: `cargo test -p clod-view --test pinches -- --nocapture`.
Subdivision 6 / seed 0 has 109 pinch occurrences, the largest fixture total,
and four distinct pinched edges. Three valid cuts from sweep cameras 36, 9 and 16
cover all four. Freeze them for inspection so the aimed close cameras do not
refine away those edges. The cluster path uses CPU-uploaded selection and ordinary
hardware rasterization; this is a magnified diagnostic, not a 1 px selection claim.

100,000 area-weighted cut→source samples per cut; 1,025 additional samples along
each pinched edge (300,000 +4,100 samples total). All maxima are below claim and
pass the existing 4×claim +1e-6 gate. Zero winding errors or ancestor overlaps.

| Cut camera | Triangles | Pinches | Claimed error | Max distance | RMS | Edge max | Max / claim | Gate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 36 | 6456 | 2 | 0.025093328207731247 | 0.016419235482643745 | 0.0023954809377335446 | 0.005297556592517174 | 0.6543267336528513 | 0.10037431283092499 |
| 9 | 1020 | 1 | 0.0682688057422638 | 0.03970591802658101 | 0.007744344005375535 | 0.016327933736090972 | 0.5816114343128154 | 0.27307622296905515 |
| 16 | 510 | 1 | 0.10654407739639282 | 0.059685048141485196 | 0.011571682463316281 | 0.030042413606826406 | 0.5601911396672895 | 0.42617730958557126 |

| Cut / edge / camera | Mean absolute RGB /255 | Max byte | Fraction >2/255 | Interior / missing pixels |
| --- | --- | --- | --- | --- |
| 36 / 0 / 0 | 0.012201286764705882 | 8 | 0.654296875 | 1024 / 0 |
| 36 / 0 / 1 | 0.012538296568627452 | 8 | 0.646484375 | 1024 / 0 |
| 36 / 0 / 2 | 0.012271497140522876 | 8 | 0.666015625 | 1024 / 0 |
| 36 / 1 / 3 | 0.0350796568627451 | 16 | 0.99609375 | 1024 / 0 |
| 36 / 1 / 4 | 0.015607128267973856 | 7 | 0.90234375 | 1024 / 0 |
| 36 / 1 / 5 | 0.013802083333333333 | 7 | 0.84765625 | 1024 / 0 |
| 9 / 0 / 0 | 0.02497702205882353 | 25 | 0.845703125 | 1024 / 0 |
| 9 / 0 / 1 | 0.023012408088235296 | 27 | 0.822265625 | 1024 / 0 |
| 9 / 0 / 2 | 0.021343954248366014 | 25 | 0.796875 | 1024 / 0 |
| 16 / 0 / 0 | 0.013978247549019608 | 18 | 0.861328125 | 1024 / 0 |
| 16 / 0 / 1 | 0.010926011029411764 | 18 | 0.5986328125 | 1024 / 0 |
| 16 / 0 / 2 | 0.0063751021241830064 | 16 | 0.2421875 | 1024 / 0 |

All twelve windows are 32×32, centered on the known pinched edge: 12,288 interior
pixels, zero missing. The largest local max is 27/255; mean differences range
from 0.0063751021241830064 to 0.0350796568627451. Images are in
`<out>/pinches/cut-{36,9,16}/` with naive, cluster and cluster-colour panels.
I inspected cut 36's `edge-0-camera-1-crops.png` and `edge-1-camera-3-crops.png`:
the first shows a modest shading shift; the second a narrow crease/shading change.
Neither shows a background gap. I also inspected `edge-0-camera-1-crops.png` for
cuts 9 and 16: darker facet variation in 9, a small highlight/crease difference in
16, no visible gap in either. Pinches do not make these images identical to naive.

### Verification and numerical oracles

`bun measure.mjs verify` exits 0: 20 tests pass, zero GPU/asset skips.
46 source files; maximum 705 lines (`view/src/gpu.rs`). Two WGSL shaders validate
with no capabilities. Rendering requires zero optional features, eight storage
bindings/stage, a 134,217,728-byte storage binding and a 268,435,456-byte buffer.
Only timing uses `TIMESTAMP_QUERY`.

| Command | Exit | Wall seconds |
| --- | --- | --- |
| `cargo test --workspace --no-fail-fast -- --nocapture` | 0 | 67.0259235 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0.5496247500000027 |
| `cargo fmt --all -- --check` | 0 | 0.13465475000000152 |
| `cargo build -p clod-format --target wasm32-unknown-unknown` | 0 | 0.08787870799998927 |
| `cargo build -p clod-view --lib --target wasm32-unknown-unknown` | 0 | 0.08884245900000678 |

The procedural camera oracle checks 240 cameras, 109 distinct triangle counts,
126–524,288 triangles and 28,820,546 triangle occurrences including 15 uniform
cuts: zero non-zero-winding edges or ancestor overlaps, worst incidence 4.
Threshold-zero scan tests check 12 comparisons at 2560×1440, both triangle orders,
t=0/.5/1; every RGB max/mean/fraction is zero (18 GPU frames).

`cargo test -p clod-view --test coverage -- --nocapture`: 512², one-pixel
silhouette exclusion, fully covered MSAA interior. Both injected one-pixel cracks
are detected. Each has mean 0.0000012715657552083333, fraction
0.000003814697265625, max 255/255 and would pass the old mean gate.

| Asset | Cameras / mixed | Interior pixels | Missing | Injected cracks detected |
| --- | --- | --- | --- | --- |
| gaul | 48 / 48 | 3,094,126 | 0 | 1 |
| washington | 48 / 48 | 3,985,199 | 0 | 1 |

`bun measure.mjs oracles` exits 0. Procedural GPU selection is included in `verify`.

| Asset | Cuts / exact images | Pixels | Cull pairs | Deterministic PNG pairs | Boundary pairs | One-slot drops | Failures |
| --- | --- | --- | --- | --- | --- | --- | --- |
| procedural | 192 / 192 | 7,077,888 | 96 | 192 | 0 | 4,384 | 0 |
| gaul | 192 / 192 | 12,582,912 | 96 | 192 | 0 | 3,779 | 0 |
| washington | 192 / 192 | 12,582,912 | 96 | 192 | 0 | 286,764 | 0 |

One-slot drops are deliberate overflow-control tests, with exact subset/pixel
checks. They are separate from performance rows. The procedural GPU oracle also
checks 5,968,822 decoded triangles. Closed-edge checks do not apply to open scans.

`cargo test -p clod-bake --test oracles -- --nocapture`: 100,000 samples per row,
one-sided cut→source. The 0.1 cut contains a pinch and its maximum is below claim.

| Threshold | Triangles | Pinches | Claimed error | Max distance | RMS | Max / claim | 4× +1e-6 gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0.0010000000474974513 | 259552 | 0 | 0.0009929724037647247 | 0.0007128301127924904 | 8.782397147560541e-05 | 0.7178750487827139 | 0.003972889615058899 |
| 0.009999999776482582 | 16332 | 0 | 0.009320216253399849 | 0.005378318276093772 | 0.0011488024152074676 | 0.577059386806809 | 0.0372818650135994 |
| 0.10000000149011612 | 1018 | 1 | 0.071912482380867 | 0.05255345970332874 | 0.0084438157398557 | 0.7307974632970129 | 0.287650929523468 |
| 1.0 | 126 | 0 | 0.25400853157043457 | 0.1036819308005273 | 0.023436139602799295 | 0.4081828675576478 | 1.0160351262817382 |

### Repeated v3 bakes

Command: `bun measure.mjs bake`. Four release bakes; each scan's pair has the
same SHA-256. v3 rejects v1/v2; no compatibility path. Sources remain in
`<cache>/assets/`, outputs in `<cache>/out/<asset>-3{,-repeat}.clod`.

| Asset | Source triangles / vertices | Clusters / groups / BVH | Depth / pages | File bytes | Bytes/source triangle | Terminal triangles |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 4,000,020 / 1,999,991 | 65,403 / 4,005 / 4,597 | 15 / 4 | 138,273,552 | 34.568215158924204 | 114 |
| washington | 16,860,930 / 9,022,298 | 281,147 / 17,201 / 19,679 | 11 / 18 | 626,912,848 | 37.18139201099821 | 25,356 |

| Asset/run | Load s | Normals s | Build s | Encode s | Write s | Peak RSS bytes |
| --- | --- | --- | --- | --- | --- | --- |
| gaul/0 | 1.5544447080000001 | 0.099407083 | 6.748234625 | 0.657382541 | 0.02965175 | 664,813,568 |
| gaul/1 | 1.301903125 | 0.063758625 | 6.161743292 | 0.58652025 | 0.031260167 | 679,968,768 |
| washington/0 | 9.422958125 | 0.101013625 | 24.856583042 | 3.20343025 | 0.188847375 | 2,907,389,952 |
| washington/1 | 9.716125333 | 0.088989041 | 23.765434959 | 2.651840209 | 0.172433667 | 2,902,884,352 |

gaul: source `smk-dying-gaul-kas1312/smk-190-inv-dying-gladiator.stl`, SHA-256
`4246ebd08faad0d3a83adf9d77e1117a509e98cfe93afe5e3ca36abc1ef7c2b2`. Both v3 outputs:
**`78f9666e7240af766b236679bcc9dd737a66fb3f8bfb5f5b3b04a64e07f02dc7`**.
Triangles per depth: 4000020, 1999614, 999162, 499210, 249416, 124610, 62254, 31110, 15544, 7762, 3880, 1940, 970, 484, 242, 114.
Clusters per depth: 31491, 16776, 8553, 4294, 2148, 1070, 538, 268, 137, 65, 32, 16, 8, 4, 2, 1.

washington: source `si-george-washington-greenough/george-washington-greenough-statue-(1840)-master-geometry.obj`, SHA-256
`ce558e481460d31678b1e31d4a18602cbede7a89fa83adddf109c3bd5be53cb9`. Both v3 outputs:
**`51e90f84dac67153ac17afe6423295be433fb1b6c11f0c1c06b6830727285455`**.
Triangles per depth: 16860930, 8428004, 4210912, 2103774, 1051012, 525064, 262312, 131102, 66766, 37294, 18878, 1075.
Clusters per depth: 133228, 72356, 37149, 18880, 9581, 4872, 2475, 1269, 684, 413, 227, 13.

### Timings

Command: `bun measure.mjs sweep`; **exit 0, all 52 configurations complete, zero
main or shadow overflows, zero limit failures**. 2560×1440, t=0, main 1 px;
one warmup +7 measured frames, median milliseconds. Every row has seven valid
samples for each reported GPU stage. Both paths use the same instance-frustum
cull. GPU select includes main +shadow selection; CPU includes selection and
encode/submit. Cluster shadows use 2 px; naive uses full geometry. Individual
stage medians need not sum to the total median. The table includes all original
F1 layouts, selectors and quotas; no dropped-image row supports the headline.

| Scene | Path / shadows / quota | Main ms | Shadow ms | GPU select ms | CPU ms | Main triangles | Drops main / shadow |
| --- | --- | --- | --- | --- | --- | --- | --- |
| gaul/single | gpu / on / 65403 | 1.252334 | 0.151083 | 0.972250 | 0.540126 | 138,145 | 0 / 0 |
| gaul/single | brute / on / 65403 | 0.535291 | 0.065958 | 1.970874 | 0.676832 | 138,145 | 0 / 0 |
| gaul/single | cpu / on / — | 1.252459 | 0.152708 | 0.000000 | 1.839209 | 138,145 | 0 / 0 |
| gaul/single | naive / on / — | 1.067917 | 0.412042 | 0.000000 | 0.242833 | 4,000,020 | 0 / 0 |
| gaul/single | gpu / off / 65403 | 0.851542 | 0.000000 | 0.791583 | 0.345376 | 138,145 | 0 / 0 |
| gaul/single | naive / off / — | 1.218833 | 0.000000 | 0.000000 | 0.251917 | 4,000,020 | 0 / 0 |
| gaul/ring:12 | gpu / on / 65403 | 0.825459 | 0.138875 | 0.241542 | 0.431792 | 101,537 | 0 / 0 |
| gaul/ring:12 | brute / on / 65403 | 0.823625 | 0.138875 | 4.627542 | 0.498917 | 101,537 | 0 / 0 |
| gaul/ring:12 | cpu / on / — | 0.825958 | 0.141209 | 0.000000 | 3.184749 | 101,537 | 0 / 0 |
| gaul/ring:12 | naive / on / — | 2.474375 | 2.206792 | 0.000000 | 0.224458 | 48,000,240 | 0 / 0 |
| gaul/ring:12 | gpu / off / 65403 | 0.146000 | 0.000000 | 0.042208 | 0.312458 | 101,537 | 0 / 0 |
| gaul/ring:12 | naive / off / — | 2.414500 | 0.000000 | 0.000000 | 0.253416 | 48,000,240 | 0 / 0 |
| gaul/grid:400 | gpu / on / 10485 | 1.489791 | 0.312833 | 2.114208 | 0.700375 | 419,476 | 0 / 0 |
| gaul/grid:400 | brute / on / 10485 | 0.317791 | 0.068125 | 4.328958 | 0.600083 | 419,476 | 0 / 0 |
| gaul/grid:400 | cpu / on / — | 1.491042 | 0.314875 | 0.000000 | 97.228583 | 419,476 | 0 / 0 |
| gaul/grid:400 | naive / on / — | 78.622333 | 72.491792 | 0.000000 | 0.342292 | 1,600,008,000 | 0 / 0 |
| gaul/grid:400 | gpu / off / 10485 | 0.239625 | 0.000000 | 0.245375 | 0.473083 | 419,476 | 0 / 0 |
| gaul/grid:400 | naive / off / — | 80.138875 | 0.000000 | 0.000000 | 0.419417 | 1,600,008,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / on / 838 | 0.327667 | 0.108709 | 5.503625 | 0.594499 | 634,384 | 0 / 0 |
| gaul/field:5000,1 | brute / on / 838 | 0.330208 | 0.109208 | 47.131332 | 0.655583 | 634,384 | 0 / 0 |
| gaul/field:5000,1 | cpu / on / — | 1.527791 | 0.510667 | 0.000000 | 1118.367792 | 634,384 | 0 / 0 |
| gaul/field:5000,1 | naive / on / — | 1083.980000 | 1003.870083 | 0.000000 | 0.523583 | 20,000,100,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / off / 838 | 0.255958 | 0.000000 | 2.753625 | 0.318959 | 634,384 | 0 / 0 |
| gaul/field:5000,1 | naive / off / — | 1014.531250 | 0.000000 | 0.000000 | 0.252542 | 20,000,100,000 | 0 / 0 |
| gaul/field:5000,1 | gpu / on / 3000 | 0.325750 | 0.109083 | 5.514668 | 0.570875 | 634,384 | 0 / 0 |
| gaul/field:5000,1 | gpu / off / 3000 | 0.394208 | 0.000000 | 4.296500 | 0.311708 | 634,384 | 0 / 0 |
| washington/single | gpu / on / 281147 | 0.759375 | 0.158834 | 2.743249 | 0.596666 | 449,499 | 0 / 0 |
| washington/single | brute / on / 281147 | 0.387250 | 0.079708 | 4.316416 | 0.532167 | 449,499 | 0 / 0 |
| washington/single | cpu / on / — | 0.787000 | 0.202791 | 0.000000 | 10.041585 | 449,499 | 0 / 0 |
| washington/single | naive / on / — | 1.854584 | 1.133708 | 0.000000 | 0.244249 | 16,860,930 | 0 / 0 |
| washington/single | gpu / off / 281147 | 0.566917 | 0.000000 | 2.326333 | 0.370875 | 449,499 | 0 / 0 |
| washington/single | naive / off / — | 1.724291 | 0.000000 | 0.000000 | 0.179167 | 16,860,930 | 0 / 0 |
| washington/ring:12 | gpu / on / 281147 | 0.908292 | 0.306541 | 0.276208 | 0.537500 | 912,031 | 0 / 0 |
| washington/ring:12 | brute / on / 281147 | 0.457667 | 0.153542 | 4.238001 | 0.464291 | 912,031 | 0 / 0 |
| washington/ring:12 | cpu / on / — | 2.129667 | 0.724500 | 0.000000 | 28.103084 | 912,031 | 0 / 0 |
| washington/ring:12 | naive / on / — | 10.185167 | 8.592500 | 0.000000 | 0.275292 | 202,331,160 | 0 / 0 |
| washington/ring:12 | gpu / off / 281147 | 0.735542 | 0.000000 | 0.164042 | 0.429542 | 912,031 | 0 / 0 |
| washington/ring:12 | naive / off / — | 9.911292 | 0.000000 | 0.000000 | 0.248875 | 202,331,160 | 0 / 0 |
| washington/grid:400 | gpu / on / 10485 | 3.852416 | 2.328167 | 0.619042 | 0.567750 | 13,806,771 | 0 / 0 |
| washington/grid:400 | brute / on / 10485 | 3.876333 | 2.326416 | 19.008668 | 0.499917 | 13,806,771 | 0 / 0 |
| washington/grid:400 | cpu / on / — | 6.251333 | 9.431291 | 0.000000 | 657.641750 | 13,806,771 | 0 / 0 |
| washington/grid:400 | naive / on / — | 365.483000 | 336.621958 | 0.000000 | 0.304125 | 6,744,372,000 | 0 / 0 |
| washington/grid:400 | gpu / off / 10485 | 3.787875 | 0.000000 | 0.319208 | 0.501917 | 13,806,771 | 0 / 0 |
| washington/grid:400 | naive / off / — | 338.740541 | 0.000000 | 0.000000 | 0.229125 | 6,744,372,000 | 0 / 0 |
| washington/field:5000,1 | gpu / on / 838 | 34.949000 | 23.737083 | 6.758333 | 0.594625 | 130,563,783 | 0 / 0 |
| washington/field:5000,1 | brute / on / 838 | 34.948750 | 23.766917 | 199.201541 | 0.542458 | 130,563,783 | 0 / 0 |
| washington/field:5000,1 | cpu / on / — | 34.857000 | 32.849792 | 0.000000 | 7973.888792 | 130,563,783 | 0 / 0 |
| washington/field:5000,1 | naive / on / — | 4608.793917 | 4092.343000 | 0.000000 | 0.474958 | 84,304,650,000 | 0 / 0 |
| washington/field:5000,1 | gpu / off / 838 | 34.807541 | 0.000000 | 3.372500 | 0.396541 | 130,563,783 | 0 / 0 |
| washington/field:5000,1 | naive / off / — | 4861.361708 | 0.000000 | 0.000000 | 0.300958 | 84,304,650,000 | 0 / 0 |
| washington/grid:400 | gpu / on / 12000 | 3.873500 | 2.329500 | 0.617583 | 0.707999 | 13,806,771 | 0 / 0 |
| washington/grid:400 | gpu / off / 12000 | 3.765375 | 0.000000 | 0.313875 | 0.415957 | 13,806,771 | 0 / 0 |

The default 838-slot quotas now complete both 5,000-instance GPU scenes.
Washington's CPU reference also completes its former 128 MiB page-list failure;
its selection still scans every candidate and costs 7,973.889 ms/frame here.
This is not the interactive path. The field GPU main pass costs 34.949 ms for
Washington and 0.328 ms for Gaul; selection adds 6.758 and 5.504 ms respectively.
The restored Gaul grid main pass measures 1.490 ms in this run; the earlier
0.27 ms figure was not reproduced and is not reused as the headline.

Historical F1 cost of requiring manifold interiors (separate runs, same machine):

| Asset | F1 terminal triangles | F2 terminal triangles | F1 zero-drop main / naive ms | F1 shadow / naive ms | F1 quota |
| --- | --- | --- | --- | --- | --- |
| Gaul | 296,378 | 114 | 22.331875 / 74.608667 | 15.619375 / 68.396459 | 10,485 |
| Washington | 1,249,766 | 25,356 | 115.171750 / 428.439500 | 70.338875 / 357.158209 | 12,000 |

These F1 numbers record the cost of the over-constraint, not an isolated
same-clock comparison. Full F1 tables, including the overflows and failed CPU
row, remain in [the archive](results/f1-manifold.md).

### Image and continuous camera-path reports

Command: `bun measure.mjs images`, exit 0. Each scan: five hero positions ×five
thresholds at 2560×1440, plus a 240-frame path and close lit/cluster-colour renders.
The table selects the largest mean error among the five positions at each
threshold; max and fraction refer to that same pair. Full position-by-position
numbers are in [the detailed results](results/f2-details.md).

| Asset | Threshold px | t | Mean absolute RGB /255 | Max byte | Fraction >2/255 | Cluster main triangles |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 0.5 | 1.0 | 0.001495685253267974 | 35 | 0.06139512803819445 | 301154 |
| gaul | 1.0 | 1.0 | 0.002215701876815541 | 46 | 0.08418592664930556 | 270973 |
| gaul | 2.0 | 1.0 | 0.0030394830530591868 | 41 | 0.11152316623263889 | 195231 |
| gaul | 4.0 | 1.0 | 0.005211344932938454 | 56 | 0.13700113932291666 | 135795 |
| gaul | 8.0 | 1.0 | 0.016049382361451526 | 85 | 0.2602316623263889 | 96925 |
| washington | 0.5 | 1.0 | 0.0009235270714188454 | 27 | 0.03732638888888889 | 53483 |
| washington | 1.0 | 1.0 | 0.0019767486638752724 | 119 | 0.07094780815972222 | 50347 |
| washington | 2.0 | 1.0 | 0.001992602379493464 | 120 | 0.07128228081597222 | 46501 |
| washington | 4.0 | 1.0 | 0.0034199247117828614 | 120 | 0.10945149739583333 | 41595 |
| washington | 8.0 | 1.0 | 0.003831886928671932 | 120 | 0.11935112847222222 | 35788 |

At 1 px, the largest means are 0.002215701876815541 (Gaul) and
0.0019767486638752724 (Washington), both at t=1. Maxima at those positions are
46/255 and 119/255. Across **all** five 1 px positions, the maximum byte errors
are 107/255 (Gaul, t=0) and 119/255 (Washington, t=1). Thus 1 px selection is
not a per-channel colour-error bound. Coverage still finds zero missing interior
pixels in its separate 96 mixed-cut camera checks.

Pop remains a report, not a no-popping guarantee. Two paths, 478 temporal pairs:

| Asset | Pairs | Max spatial temporal residual | Worst step | Max excess MAD | Excess step | Failures |
| --- | --- | --- | --- | --- | --- | --- |
| gaul | 239 | 0.0032067663654003265 | 195 | 0.0002564891407952005 | 220 | 0 |
| washington | 239 | 0.0018351027766430647 | 174 | 0.00018020308528503975 | 232 | 0 |

Close views: `<out>/<asset>-close-{lit,clusters}.png`. Winning compare/pop frames
are under `<out>/<asset>-{compare,pop}/`; every per-frame record and command is
in the cache. [Detailed results](results/f2-details.md) retain all timing, memory,
padding, per-fixture, sampling, crop and comparison numbers. Source meshes,
baked files, images and raw logs are never committed.

## Known limits

All pages are resident. Exact GPU residency is reported in the detailed timing
tables. A browser tab may not hold the whole Washington scan reliably; this lane
provides a Wasm library, not a deployed web demo or interactive native player.

Quota overflow still drops geometry and the CPU reference still has a 128 MiB
page-list limit. Neither limit is hit in the F2 sweep. The deliberate one-slot
oracle continues to verify overflow reporting. Washington retains three
boundary-chain stops; the bake does not repair input defects.

The error-honesty oracle is one-sided, sampled and uses a 4× refined-error gate.
Coverage tests detect fully missing interior pixels, excluding one pixel of the
MSAA silhouette; they do not prove a global geometric or subpixel error bound.
Pinches can change shading, as the aimed crops show. No geomorphing, occlusion
hierarchy, streaming, software rasterizer or fallback simplifier is implemented.
No no-popping guarantee or cross-ISA byte identity is claimed. Timing rows use
one warmup and seven samples from one run, without machine-wide load control.
