# Cluster LOD — offline bake, GPU selection and hardware rasterization

Standalone experiment for LLP 1041.011 O1 / §5 Q2. L1 builds the file and numerical
oracles; L2a adds hardware rasterization; L2b adds core-WebGPU selection,
culling, stable compaction and per-page indirect draws, retaining the CPU oracle.
The vendored meshoptimizer v1.2 and `demo/clusterlod.h` are unchanged.

**Status:** L1/L2a verified; L2b GPU image/selection oracles verified on Apple M5 Max
/ Metal. Initial invalid counter readbacks are documented in decision 27;
final commands report sample validity and independent completed-frame latency.

## Run

Run these commands from `experiments/cluster-lod/` with the launch environment
(debug information and incremental compilation disabled). The only target directory
is this workspace's `target/`.

```sh
cargo run -p clod-bake -- <input.ply> <output.clod> --max-triangles 128 --page-mib 32
cargo run -p clod-bake -- --inspect <output.clod>
cargo run -p clod-bake -- --cut <output.clod> --threshold 0.01 --obj <cut.obj>
cargo run -p clod-bake -- --generate 8 ~/Library/Caches/exact2-cluster-lod/out/sphere.ply
cargo test --workspace --no-fail-fast -- --nocapture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p clod-format --target wasm32-unknown-unknown
```

Bake output is one JSON object. View outputs JSON lines; compare/pop report all
pairs and a summary. Failures produce an `error` object and exit 1.
All meshes, baked outputs, exported cuts and logs belong in
`~/Library/Caches/exact2-cluster-lod/`, never in Git.

### Renderer commands

```sh
cargo build -p clod-view
# Full checks, all 32 timing cases, and 64 cameras × three layouts per real asset:
python3 measure.py verify
python3 measure.py sweep
python3 measure.py oracles
asset="$HOME/Library/Caches/exact2-cluster-lod/out/washington-1.clod"
out="$HOME/Library/Caches/exact2-cluster-lod/out/demo"
target/debug/clod-view render "$asset" --out "$out/lit.png" --path hero --t 0.5
target/debug/clod-view render "$asset" --out "$out/clusters.png" --view clusters --t 0.5
target/debug/clod-view time "$asset" --out "$out/timing.png" --layout grid:400 --select gpu --frames 7
target/debug/clod-view time "$asset" --out "$out/brute.png" --layout grid:400 --select brute --frames 7
target/debug/clod-view render "$asset" --out "$out/overflow.png" --capacity 1
target/debug/clod-view oracle "$asset" --steps 64 --size 256x256
target/debug/clod-view compare "$asset" --out "$out/compare" --threshold-px 0.5,1,2,4,8 --t 0,0.25,0.5,0.75,1
target/debug/clod-view pop "$asset" --out "$out/pop" --threshold-px 1 --steps 240
cargo test --workspace --no-fail-fast -- --nocapture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p clod-view --lib --target wasm32-unknown-unknown
```

Defaults: cluster mode, GPU selection, culling on, lit view, single layout, threshold 1 px, 2560×1440,
45° vertical field of view, hero t=0. `--mode naive` uses the indexed baseline.
Selection: `--select gpu|cpu|brute`; `brute` scans every cluster of each surviving
instance on the GPU. `--cull on|off` controls both passes; overdraw disables culling.
`--capacity N` sets a fixed **per-instance** GPU quota at load time (default rule
in decision 26); zero is clamped to one. CPU reference rendering uses its own
renderer/list buffers. Layouts: `single`, `ring:N`, `grid:N`, `field:N,seed`; N=1..10,000. Views:
`lit|clusters|depth|triangles|instances|overdraw`. Explicit world-space camera:
`--eye x,y,z --target x,y,z --fov degrees`. `--size WIDTHxHEIGHT` caps at 8192².
The library accepts byte slices and caller-owned wgpu devices; I/O, timing,
blocking map polling and PNG encoding live exclusively in the native binary.
The browser's caller supplies cache-optimized baseline buffers if it needs naive
mode; the native-only meshoptimizer FFI is not linked into the library or Wasm.

The CLI is an offscreen demo: render arbitrary `--t` values or walk 240 samples
with `pop`. There is no window or real-time interactive player in this lane.
Runtime shader compilation is performed by wgpu from the build-validated WGSL.

## Format v1

Little endian, magic `CLOD0001`, version 1. `format/src/lib.rs` contains the
authoritative `repr(C)`/`Pod` structs. Header and every top-level section begin on
16-byte boundaries. Padding is zero. Section order is header, clusters, groups,
page table, optional BVH, geometry pages. The header carries counts, byte offsets,
configuration, flags and SHA-256 of the primary source file's exact bytes.
The reader borrows aligned bytes, returns errors on unaligned or malformed input,
and checks all ranges, local indices, digests, group references and BVH reachability.
The writer invokes the reader to validate its own output. No filesystem or native
dependency in the format crate; its `wasm32-unknown-unknown` build passes.

Each 128-byte cluster has its own sphere and normal cone (apex, axis, cutoff),
20-byte simplified and refined bounds (float3 centre, radius, error), group IDs,
page ID, page-relative vertex offset/count, index-byte offset/triangle count,
group depth, and three zero reserved words. `u32::MAX` is the original-geometry
refinement sentinel. Terminal groups store `f32::MAX` error. L2 should use scalar
WGSL arrays/words for these layouts; WGSL `vec3` alignment is not the C layout.

A group is 32 bytes: simplified bounds, depth, first cluster, cluster count.
A BVH node is 32 bytes: bounds, group ID (`u32::MAX` for internal nodes), child
offset/count. This is the vendor's forest: the first `root_count` nodes are roots,
one per depth; leaves address the group table. Flat selection never needs it.

A page-table record is 80 bytes: u64 offset/length, SHA-256, cluster range, vertex
and index counts, vertex/index offsets within the page, two reserved words.
Pages contain only vertices then index bytes, both padded to 16 bytes. Every
cluster has a contiguous vertex array and three `u8` local indices per triangle.
Offsets address vertices or bytes, respectively. There are no cross-page pointers.
The default bound is 32 MiB, configurable from 4 KiB through 128 MiB.

Vertices are 20 bytes: float32 xyz (12), octahedral signed-normalized 16-bit x/y
packed into a u32 (4), RGBA8 (4). Decode normal components by signed division by
32767, unfold the lower hemisphere and normalize (`unpack_normal` is the oracle).
RGBA is low-byte red; absent colors become opaque white and header flag bit 0 is
clear. Source positions are copied without quantization: every copy, across all
clusters and pages, has identical position bits. Normals are area-weighted if
missing, normalized otherwise. Normals and colors enter simplification as float
attributes with weight 0.1 per component.

## Selection

Draw each cluster independently iff `projected(simplified) > threshold` AND
(`refined == ORIGINAL` OR `projected(refined_bounds) <= threshold`). A uniform cut
uses world-space errors directly. Perspective projection follows the vendor:

```
error / max(length(center - camera_position) - radius, positive_near)
    * (cot(fovy / 2) * 0.5) * viewport_height
```

Terminal error is treated as infinity, avoiding overflow/underflow of the stored
finite sentinel. Camera orientation is intentionally absent from this rotationally
invariant size estimate. Frustum/cone culling is a separate L2 operation. Threshold
must be finite, nonnegative and less than `f32::MAX`.

## Decisions

1. Preserve float32 position bits and pack only normals. This gives a direct crack
   guarantee for repeated source positions; independent per-page quantization does
   not. RGBA8 stays present even without colors for one GPU vertex stride.
2. Disable permissive and sloppy simplification, keeping topology-preserving regular
   simplification and locked group borders. This lane's closed-manifold oracle takes
   precedence over obtaining the smallest terminal cut. Use additive error accumulation
   (`max(previous, current) + current`), a conservative setting to test numerically.
3. Store every terminal group first, then descending depth with original group ID as
   tie-breaker. Page 0 must fit the entire terminal cut; fail with a page-budget error
   if it cannot. Nonterminal groups may span pages, but individual clusters never do.
   Group IDs remain vendor emission order; cluster ranges are updated after packing.
4. Lift a zero simplification error to `f32::MIN_POSITIVE` when recording groups so
   threshold zero selects precisely original triangles even for planar geometry.
5. Use single-threaded vendored construction. Hash maps in mesh generation and STL
   welding are lookup-only; emitted order follows source triangles, never map iteration.
6. Keep glTF with default features disabled and only `utils`; no image decoder, renderer,
   or wgpu dependency in L1. Load all triangle primitives of the first mesh, ignoring
   scene transforms/materials. Support GLB, base64 buffers and plain local buffer paths;
   no network/percent-escaped URIs. The source digest is of the primary file; external
   glTF buffers are additional inputs and must also remain unchanged for determinism.
7. Stream PLY, OBJ and STL input through a hashing buffered reader. Support ASCII PLY
   with one element per line, binary little/big-endian PLY, optional normals and uchar
   RGB(A); reject nontriangular PLY faces. OBJ polygons use fan triangulation, positions
   only. Binary STL welds identical float positions (signed zero normalized).
8. The writer assembles a contiguous output vector after building bounded pages.
   This uses more RAM than an on-disk spool but avoids scratch duplication on the
   nearly full disk. Peak process RSS is measured in each CLI run. Build errors are
   reported as errors; the page-0 bound is never silently exceeded.
9. Use a displaced octasphere for reproducible fixtures: 8 subdivisions requests
   524,288 triangles, and 10 requests 8,388,608. The numerical test requests 15
   uniform thresholds and 240 random cameras. It generates orientations but does
   not apply frustum culling: selection is rotationally invariant and culling would
   intentionally open the surface tested for closed edges.
10. Sample 100,000 area-uniform cut points at each of four thresholds. The error
    gate is `4 * maximum selected refined error + 1e-6` world units. Four is a
    deliberately generous falsification threshold for accumulated quadric error,
    which is not a rigorous Hausdorff bound; print maximum/RMS and ratio regardless
    of success. This is cut-to-source sampling, not a bidirectional or pixel-error
    proof. The completed measurements are below.
11. Reconstruct the naive source-resolution mesh once from ORIGINAL clusters,
    weld identical complete vertex records, and run meshoptimizer vertex-cache and
    vertex-fetch optimization. This keeps source geometry/attributes identical
    while giving the baseline real indexed vertex buffers and instancing.
12. Use flat, deterministic CPU selection with group error evaluated once per
    instance, then independent cluster selection and sphere/frustum culling.
    Uniform instance scale affects radius and error. No cone culling is needed
    for the L2a reference; L2b extends it with decision 25.
13. Submit one instanced draw per baked page per pass, including zero-instance
    draws for empty pages. Each instance is one visible (cluster, scene instance)
    pair. Short clusters emit coincident out-of-clip vertices; padding consumes
    vertex invocations but produces no fragments. Report useful and padded counts.

14. Normalize the longest asset dimension to 2 world units and place its bottom on
    Z=0. Smithsonian Washington's source digest identifies its Y-up basis; rotate
    it into the Z-up scene. Other sources default to Z-up. Layout seed uses an
    explicit 32-bit LCG. Grid and field vary positive uniform scale and rotation.
15. Resolve authored 16:9 screen anchors on the two scans' hair to the nearest real
    source triangle at load time. This was chosen after viewing the first PNGs:
    a bounding-box aim landed behind the surface. The smoothstep camera ends
    0.055 world units from the hit, with near=0.002 and no camera cuts. Other
    meshes use a central screen anchor. The path is geometric; it does not morph
    between LOD cuts or guarantee a zero popping metric.
16. Render a 2048² directional shadow map through the same page path at twice the
    main threshold, selected with orthographic projected error. The indexed
    baseline draws its full-resolution mesh in both passes. Report shadow costs
    separately; image differences include shadows as well as main geometry.
    Use nine comparison samples, depth bias, Lambert + GGX dielectric (roughness
    .32, F0 .04), hemisphere ambient, filmic tonemap, an sRGB target and 4× MSAA.
17. Only `time` requests TIMESTAMP_QUERY, and only when the adapter exposes it.
    `render`, `compare`, `pop`, and all image tests request no features. All use
    `Limits::default()` exactly. `time` reports medians of seven measured frames
    after one warmup; GPU values cover the main and shadow passes, excluding
    readback, uploads and CPU selection. `encode_ms` includes upload calls,
    command encoding and submission; separately report both CPU selections.
18. Baseline cluster/depth/triangle debug views are rejected: core WebGPU exposes
    no primitive ID without an extra feature, and duplicating vertices would
    spoil the indexed baseline. Those debug views operate on the cluster path.
    `depth` means DAG depth. `overdraw` adds linear RGB (.04,.013,.002) with
    depth test Always and no backface culling, producing a saturating heat view.
19. Pixel errors use RGB in the output sRGB PNG, normalized by 255; alpha excluded.
    A pixel differs if any channel differs by >2. Worst compare pair means
    largest mean absolute error. Pop reports both max(MAD(cluster delta) minus
    MAD(naive delta)) and the stronger mean absolute spatial residual of signed
    RGB deltas; saves before/after cluster and naive frames at the latter maximum.
20. The format does not promise pixel-identical rasterization after triangle
    reordering. On the procedural fixture allow threshold-zero max 1/255 and mean
    <1e-6; final measured mean is 0 (earlier camera measured 1.77e-8). The 1 px
    procedural regression gate is mean <.008 and differing-pixel fraction <.20:
    measured .003611/.12291; this bounds image regression,
    not Hausdorff distance or a guarantee that all changed pixels lie within 1 px.
21. Visible-pair storage grows to a power-of-two high-water capacity per page,
    capped at 128 MiB. Reject a larger list rather than adding draws dependent
    on visibility. Baseline chunks cap indices at 128 MiB and vertices at 120 MiB.
    GPU residency counts allocated buffers/textures, with readback separate;
    driver overhead, shader binaries and allocator overhead are not measurable here.
    L2b supersedes the growing GPU list with decision 26; the CPU reference retains
    this growth policy.

22. Keep only the current and worst frame pairs in memory in compare/pop and
    encode the winning PNGs once at the end. This removes repeated PNG writes
    from the camera sweep without changing the error equations. Adapter skips
    write directly to stderr so libtest cannot hide them in its default capture.

23. Evaluate shadow transforms as `light * (model * position)` in both paths and
    mark clip positions invariant. An initial different multiplication grouping
    caused 163/85 changed pixels (>2/255) in Gaul/Washington close-ups at threshold
    zero. The corrected close-ups are exact. A probe preserving source triangle
    order also makes the far images exact; cache optimization leaves one changed
    pixel there (max 18/255 Gaul, 4/255 Washington). Keep the cache-optimized
    baseline and allow at most 8 such pixels, max 20/255, mean <1e-7 on the scans.
    `tests/real_assets.rs` checks both orders at t=0,.5,1; skips loudly if cached
    scans or a GPU are unavailable. The standard procedural test needs no files.

24. L2b uses a runtime error-envelope index, preserving baked cluster order. A suffix
    maximum of simplified error and prefix minimum of refined error are monotone;
    binary searches bound a conservative contiguous candidate range. The enclosing
    sphere includes all selection bounds, not only vertices. This trades extra
    candidates within a depth for stable raster order and no format/re-bake change.
    Pad the enclosing radius by 1.00002 and range thresholds by relative 1e-5,
    only to avoid pruning borderline candidates; the final LOD predicate is exact.
    Terminal sentinels always survive range pruning, including when a very large
    finite threshold overflows its mesh-space conversion.
25. Main culling uses instance/cluster spheres and meshoptimizer's perspective
    apex cone test. Shadows keep L2a's orthographic error at twice the threshold,
    light-frustum spheres, and the directional-light cone test. Camera-facing
    tests are never used for shadow casters. Sphere planes and cone dots use a
    conservative 1e-5 guard in both CPU and WGSL. Culling is disabled for overdraw.
26. Allocate visible storage once: each instance gets min(cluster count,
    floor(4,194,304 / instance count)) slots per pass. `--capacity` overrides
    the per-instance slot count. Stable scans keep earliest coarse-to-fine IDs;
    a full quota drops subsequent finer clusters, counts every drop, and cannot
    overwrite another instance. Unused quotas are not shared. Overflow can make
    holes; it is an explicitly reported degraded image, not a crack-free cut.

27. Metal timing limitation found during L2b: main-pass end counters were zero
    or stale in the expanded timing path (CPU and naive commands also affected). Three diagnostic/fix
    rounds inspected raw counters, reordered query indices into execution order,
    and resolved in a subsequent command buffer. The issue persisted; the fix
    loop stopped. Invalid pairs are printed with all eight raw values and become
    JSON null, never zero or a fabricated duration. A stage median requires all
    measured samples to be valid; valid sample counts are explicit. Valid select/shadow timings
    and CPU costs are still reported. `frame_completion_ms` is a separate host-clock
    latency from encode start through completed RGBA readback: an upper bound
    including submission, GPU work and pixel transfer, not a substitute GPU
    stage time. The unsuccessful extra-submission workaround was removed.

28. Reuse L1's 524,288-triangle (subdivision 8) closed fixture for the GPU edge
    oracle, and share the edge-count implementation through `clod-format::oracle`.
    An initial subdivision-6 fixture exposed 1–2 bad edges in ten unculled camera
    cuts, identically on CPU and GPU (zero set differences). That new-fixture
    bake/topology limitation is retained here as evidence, not blamed on GPU
    selection or hidden by a relaxed edge threshold.

29. Keep topology-preserving bakes and their unchanged SHA-256 digests. The
    optional depth-gated permissive/sloppy experiment is not performed or adopted:
    the immutable vendor's public configuration has only global fallback switches,
    so a depth gate needs a separate builder change. No HZB, streaming or optional
    core features are added. This leaves a measured raster floor, not a claim that
    GPU selection alone makes the 5,000-instance Washington scene reach 60 Hz.
30. The GPU frame uploads two fixed selection uniforms and one render uniform,
    encodes eight fixed compute dispatches and two page-draw loops, and submits.
    Instances and metadata are uploaded only at scene load. CPU frame work is
    O(pages), independent of clusters and instances, for a fixed asset. Diagnostic
    visible/counter/shadow readbacks are explicit post-frame CLI/test operations;
    their results never feed a draw. Reject scenes exceeding the u32 candidate
    counter range rather than silently wrapping a measurement.

31. Correct the inherited Washington floor: 6,088 is the deepest level, not the
    whole terminal cut. `clod-bake --cut ... --threshold 1e30 --obj ...` and both
    selectors at `f32::MAX / 2` give 24,515 triangles, including groups that stopped
    at earlier depths. The 5,000-instance floor is therefore 122,575,000 triangles,
    98.16% of the 124,878,597 main triangles measured at 1 px. The bake is unchanged;
    the earlier floor report was incorrect. Optional fallback remains unmeasured.

## Results

L1 verification, 2026-09-20, from cache `logs/L1-verify-{1,mac}.log`:

| Command / measurement | Linux | Apple Mac |
| --- | ---: | ---: |
| `cargo test --workspace --no-fail-fast -- --nocapture` | 3 tests passed, 16.424 s total | 3 tests passed, 23.573 s total |
| numerical oracle | 6.033 s | 4.96 s |
| `cargo clippy --all-targets -- -D warnings` | passed | passed, 5.71 s |
| `cargo build -p clod-format --target wasm32-unknown-unknown` | — | passed, 3.27 s |

Linux numerical oracle: 524,288 source triangles, 15 uniform thresholds plus 240
cameras, 29,338,902 triangles checked, 0 bad edges, 0 ancestor overlaps, edge use
exactly 2. Camera cuts: 126–524,288 triangles, 120 distinct counts. Four sets of
100,000 sampled points: maximum deviation / claimed error = 0.67745, 0.64592,
0.54846, 0.33381 (thresholds .001, .01, .1, 1); maximum distances .00066282,
.00593142, .04081806, .08612405; RMS .00008838, .00116135, .00830478, .02127320.
26 malformed files rejected, 8 loader formats, 3 CLI commands passed.

`target/release/clod-bake <assets>/smk-dying-gaul-kas1312/smk-190-inv-dying-gladiator.stl <out>/gaul-1.clod`:
4,000,020 triangles, 1,999,991 source vertices, 65,415 clusters, 4,006 groups,
4,591 BVH nodes, depth 15, 4 pages, 138,273,088 bytes (34.56810 B/source triangle).
First/repeat elapsed 9.03/9.33 s; peak RSS 670,662,656/666,861,568 bytes.
First phases: load 1.32038 s, normals .05917 s, build 6.07353 s, encode .74801 s,
write .01847 s. Terminal cut 114 triangles. Both outputs have SHA-256
`879644077118ad27d0c777b988a36134b0be96e2e9f4f8fc475d07e515549eb0`.



### Hero bake (L1, orchestrator-verified)

From cache `logs/L1-hero-bake.log`, command
`target/release/clod-bake <assets>/si-george-washington-greenough/george-washington-greenough-statue-(1840)-master-geometry.obj <out>/washington-1.clod`:
16,860,930 source triangles, 9,022,298 source vertices, 281,343 clusters,
17,204 groups, 19,684 BVH nodes, depth 11, 18 pages, 627,022,800 bytes
(37.18791 B/source triangle), deepest level 6,088 triangles. The complete
terminal cut is 24,515 triangles (L2b correction: earlier groups can also terminate). First/repeat wall
57.52/61.00 s, peak RSS 2,839,658,496/2,843,557,888 bytes. First phases:
load 21.80677 s, normals .10789 s, build 29.09533 s, encode 3.15659 s,
write .62860 s. Repeat: load 12.49021 s, normals .11094 s, build 39.19019 s,
encode 4.49781 s, write .10324 s. Both SHA-256:
`84b20d2f2dfbd329a24d79f9ddb9827f967ac1862a5dcfd02d803878f96811a1`.

### L2a verification

2026-09-20, Apple M5 Max, Metal, wgpu 30.0.1, optimized dev/test profile
(opt-level=2, debug=false, incremental=false). Renderer commit `a7907b88`.
40 final sweep commands, 0 failures: 50 compare pairs, 6 extra threshold-zero
pairs, two 240-frame paths (478 temporal comparisons), 12 render commands,
12 timing commands and 10 debug-view commands. No GPU test was skipped.
Raw logs/PIDs/commands: `<out>/L2a-sweep.jsonl`, `<out>/<asset>-*.jsonl`;
final checks: `<out>/L2a-validation-final.jsonl` and `<out>/final-*.log`.
All images live under `<out>/L2a/{gaul,washington}/`; no binary assets in Git.

| Final command | Result | Wall seconds |
| --- | --- | ---: |
| `cargo test --workspace --no-fail-fast -- --nocapture` | exit 0; 5 tests passed | 13.366487 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 | 0.155357 |
| `cargo fmt --all -- --check` | exit 0 | 0.093154 |
| `cargo build -p clod-view --lib --target wasm32-unknown-unknown` | exit 0 | 1.189273 |

Line-count run: Python `Path.rglob`, `splitlines`, excluding target/vendor:
31 source files, max 622 lines (`view/src/gpu.rs`),
0 files over 1,500. Naga validated 1 WGSL file with 0 failures and no capabilities.
Device oracle: 0 features, 8 storage bindings/stage, 134,217,728-byte storage
binding, 268,435,456-byte buffer limits. Five rotated/translated/scaled cuts
match exactly; scale 2.5 gives projected error 10.686141 px in both frames.

Procedural image oracle: 524,288 source triangles, 384² pixels, 17 pages,
9 rendered images, threshold-zero exact, repeated PNGs byte-identical
(63,207 bytes). At 1 px: 32,688 main triangles + 2,640 padding triangles,
17 main draws at both thresholds, mean 0.003611641235,
max 67/255, differing-pixel fraction 0.122904459635.
The .008 mean/.20 fraction regression bounds are 2.22×/1.63× these measured values.

| Debug view vs lit (procedural, 1 px) | Mean absolute RGB difference |
| --- | ---: |
| Clusters | 0.042653920292 |
| Depth | 0.034482753424 |
| Triangles | 0.038581124543 |
| Instances | 0.028973260130 |
| Overdraw | 0.043558348723 |

Real-asset oracle: 2 scans × 3 cameras × 2 baseline triangle orders = 12
comparisons / 18 GPU frames, 0 failures. The same-order indexed reference is
**pixel-exact in all six cases**. The cache-optimized reference has the following
threshold-zero differences (out of 3,686,400 pixels):

| Asset | t | Mean absolute | Max byte | Pixels >2/255 |
| --- | ---: | ---: | ---: | ---: |
| gaul | 0 | 2.26942628903e-08 | 18 | 1 |
| gaul | 0.5 | 3.54597857662e-09 | 4 | 1 |
| gaul | 1 | 0 | 0 | 0 |
| washington | 0 | 6.38276143791e-09 | 4 | 1 |
| washington | 0.5 | 8.15575072622e-09 | 7 | 2 |
| washington | 1 | 0 | 0 | 0 |

Additional CLI check: `render gaul-1.clod --layout field:12,7 --view instances
--size 800x600`, twice: 14,765-byte identical PNGs,
SHA-256 `c4b8c553016a20eab0c5852aeff19eb137d99283faf9dbc52b2db5bc47dbf43c`.
`compare gaul-1.clod --layout field:12,7 --size 800x600 --threshold-px 0,1
--t 0 --eye 8,-12,7 --target 0,0,0.6 --fov 50`:

| Threshold px | Cluster triangles | Naive triangles | Mean absolute | Max byte | Fraction >2/255 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 48,000,240 | 48,000,240 | 3.50217864924e-06 | 30 | 6.66666666667e-05 |
| 1 | 65,352 | 48,000,240 | 0.00156669934641 | 120 | 0.035625 |

The multi-instance threshold-zero case has 32 pixels >2/255 and max 30/255;
it is not pixel-exact. The single-instance test's empirical bound is not a
universal bound over different instance/draw orders. Geometry position bits
are identical; source-order tests isolate the remaining order dependence.

### Frame costs at 2560×1440

Commands for every table row: `target/debug/clod-view render <out>/<asset>-1.clod
--mode <mode> --layout <layout> --size 2560x1440 --threshold-px 1 --t 0 --out <png>`;
replace `render` with `time` and append `--frames 7` for timings (one warmup).
Both modes share the same scene and camera. Main and shadow counts exclude the
2-triangle, 1-draw ground plane. Total draws = main + shadow + 1.
The naive mesh uses 1 chunk / 1,999,991 vertices for Gaul and 2 chunks /
9,114,163 vertices for Washington (91,865 repeated vertices at chunk boundaries).
All pages are resident; there is no streaming. Padding costs three vertex
invocations per padded triangle, with no vertex-page fetch or raster fragments.

| Asset / layout / mode | Selected clusters | Main triangles | Main padding | Main draws | Shadow triangles | Shadow padding | Shadow draws | GPU resident bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gaul / single / cluster | 1,281 | 148,718 | 15,250 | 4 | 87,476 | 9,932 | 4 | 287,510,048 |
| gaul / single / naive | 0 | 4,000,020 | 0 | 1 | 4,000,020 | 0 | 1 | 237,488,124 |
| gaul / ring:12 / cluster | 904 | 104,134 | 11,578 | 4 | 84,504 | 7,656 | 4 | 287,502,560 |
| gaul / ring:12 / naive | 0 | 48,000,240 | 0 | 1 | 48,000,240 | 0 | 1 | 237,488,828 |
| gaul / grid:400 / cluster | 3,425 | 408,496 | 29,904 | 4 | 271,040 | 15,680 | 4 | 287,576,544 |
| gaul / grid:400 / naive | 0 | 1,600,008,000 | 0 | 1 | 1,600,008,000 | 0 | 1 | 237,513,660 |
| washington / single / cluster | 4,361 | 467,930 | 90,278 | 18 | 343,882 | 68,278 | 18 | 775,404,056 |
| washington / single / naive | 0 | 16,860,930 | 0 | 2 | 16,860,930 | 0 | 2 | 534,102,484 |
| washington / ring:12 / cluster | 9,627 | 945,870 | 286,386 | 18 | 765,936 | 253,968 | 18 | 775,526,512 |
| washington / ring:12 / naive | 0 | 202,331,160 | 0 | 2 | 202,331,160 | 0 | 2 | 534,103,188 |
| washington / grid:400 / cluster | 156,162 | 13,681,274 | 6,307,462 | 18 | 12,114,320 | 6,112,880 | 18 | 779,549,040 |
| washington / grid:400 / naive | 0 | 6,744,372,000 | 0 | 2 | 6,744,372,000 | 0 | 2 | 534,128,020 |

Readback is another 14,745,632 bytes with timing, 14,745,600 without; ordinary
render residency is 32 bytes below the timing table (query resolve buffer).
CPU and GPU columns are independent medians in ms, so median GPU total need
not equal the sum of the two pass medians. CPU selection includes the flat
reference scan; encode excludes image readback and PNG encoding.

| Asset / layout / mode | CPU main select | CPU shadow select | Encode | GPU main | GPU shadow | GPU total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| gaul / single / cluster | 0.387958 | 0.112917 | 0.330875 | 0.523416 | 0.066875 | 0.589749 |
| gaul / single / naive | 0.000000 | 0.000000 | 0.252833 | 0.998958 | 0.408667 | 1.407625 |
| gaul / ring:12 / cluster | 1.218583 | 0.873042 | 0.283791 | 0.821500 | 0.137542 | 0.958250 |
| gaul / ring:12 / naive | 0.000000 | 0.000000 | 0.360875 | 2.496084 | 2.221500 | 4.775041 |
| gaul / grid:400 / cluster | 41.136750 | 36.521958 | 0.388250 | 1.171833 | 0.280333 | 1.452333 |
| gaul / grid:400 / naive | 0.000000 | 0.000000 | 0.308625 | 74.835041 | 68.507250 | 143.091416 |
| washington / single / cluster | 1.369125 | 1.153916 | 0.538042 | 1.815125 | 0.375125 | 2.190375 |
| washington / single / naive | 0.000041 | 0.000041 | 0.295834 | 1.801375 | 1.150958 | 2.985209 |
| washington / ring:12 / cluster | 15.557625 | 11.600000 | 0.495250 | 1.149458 | 0.395666 | 1.545124 |
| washington / ring:12 / naive | 0.000000 | 0.000042 | 0.348083 | 10.148542 | 8.677917 | 18.802041 |
| washington / grid:400 / cluster | 368.294541 | 345.027375 | 0.759666 | 6.294709 | 9.318000 | 15.719375 |
| washington / grid:400 / naive | 0.000042 | 0.000041 | 0.600250 | 455.821708 | 441.419334 | 881.973750 |

Gaul grid: GPU ratio 98.53×; summed measured CPU selection + encode + GPU medians 79.499 ms cluster vs 143.400 ms naive. This sum is a cost estimate, not measured frame latency. Padding is 6.82% of submitted cluster triangles.

Washington grid: GPU ratio 56.11×; summed measured CPU selection + encode + GPU medians 729.801 ms cluster vs 882.574 ms naive. This sum is a cost estimate, not measured frame latency. Padding is 31.56% of submitted cluster triangles.

The GPU reduction is real, but the CPU reference scan prevents interactive
large scenes: Washington grid spends hundreds of milliseconds selecting each
pass. Moving that work to compute is L2b's measurable target. Small single-mesh
views are much closer on GPU time, because rasterizing the image and ground
still costs time after reducing geometry.

### Image comparison tables

For each asset: `target/debug/clod-view compare <out>/<asset>-1.clod
--threshold-px 0.5,1,2,4,8 --t 0,0.25,0.5,0.75,1 --size 2560x1440
--out <out>/L2a/<asset>/compare`. Mean and fraction are normalized 0..1;
max is printed in byte units (divide by 255 for normalized max).
These include the coarser cluster shadow versus the full-resolution naive shadow.

#### Gaul

| Threshold px | t | Mean absolute | Max byte | Fraction >2/255 | Cluster triangles | Naive triangles | Cluster shadow triangles |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.5 | 0 | 0.000439128 | 75 | 0.013124457 | 329,938 | 4,000,020 | 230,100 |
| 1 | 0 | 0.000574163 | 107 | 0.017065158 | 148,718 | 4,000,020 | 87,476 |
| 2 | 0 | 0.000770215 | 130 | 0.021299642 | 68,412 | 4,000,020 | 36,758 |
| 4 | 0 | 0.001009394 | 124 | 0.026478950 | 33,300 | 4,000,020 | 15,738 |
| 8 | 0 | 0.001458510 | 133 | 0.033742405 | 12,900 | 4,000,020 | 7,042 |
| 0.5 | 0.25 | 0.000551133 | 87 | 0.016104872 | 403,558 | 4,000,020 | 230,100 |
| 1 | 0.25 | 0.000729212 | 101 | 0.021258952 | 173,636 | 4,000,020 | 87,476 |
| 2 | 0.25 | 0.000974700 | 132 | 0.027343750 | 78,690 | 4,000,020 | 36,758 |
| 4 | 0.25 | 0.001303787 | 129 | 0.034737956 | 36,758 | 4,000,020 | 15,738 |
| 8 | 0.25 | 0.001807057 | 131 | 0.043368056 | 18,522 | 4,000,020 | 7,042 |
| 0.5 | 0.5 | 0.000885026 | 91 | 0.024264052 | 615,737 | 4,000,020 | 230,100 |
| 1 | 0.5 | 0.001185656 | 95 | 0.034252387 | 308,203 | 4,000,020 | 87,476 |
| 2 | 0.5 | 0.001551233 | 122 | 0.045083008 | 148,027 | 4,000,020 | 36,758 |
| 4 | 0.5 | 0.001984419 | 120 | 0.054493544 | 78,589 | 4,000,020 | 15,738 |
| 8 | 0.5 | 0.002799733 | 126 | 0.071214735 | 36,653 | 4,000,020 | 7,042 |
| 0.5 | 0.75 | 0.000750771 | 51 | 0.017654351 | 742,530 | 4,000,020 | 230,100 |
| 1 | 0.75 | 0.001542177 | 64 | 0.044855143 | 400,696 | 4,000,020 | 87,476 |
| 2 | 0.75 | 0.002242411 | 90 | 0.067905816 | 227,029 | 4,000,020 | 36,758 |
| 4 | 0.75 | 0.003205039 | 111 | 0.093610569 | 127,962 | 4,000,020 | 15,738 |
| 8 | 0.75 | 0.004758052 | 111 | 0.117682292 | 84,725 | 4,000,020 | 7,042 |
| 0.5 | 1 | 0.001318314 | 21 | 0.057992622 | 434,594 | 4,000,020 | 230,100 |
| 1 | 1 | 0.002094026 | 31 | 0.083525662 | 371,365 | 4,000,020 | 87,476 |
| 2 | 1 | 0.002964863 | 51 | 0.100791829 | 230,455 | 4,000,020 | 36,758 |
| 4 | 1 | 0.004193963 | 51 | 0.132863227 | 149,616 | 4,000,020 | 15,738 |
| 8 | 1 | 0.012004740 | 85 | 0.242714030 | 103,675 | 4,000,020 | 7,042 |

#### Washington

| Threshold px | t | Mean absolute | Max byte | Fraction >2/255 | Cluster triangles | Naive triangles | Cluster shadow triangles |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.5 | 0 | 0.000396037 | 76 | 0.012555881 | 896,890 | 16,860,930 | 706,488 |
| 1 | 0 | 0.000560972 | 90 | 0.018231879 | 467,930 | 16,860,930 | 343,882 |
| 2 | 0 | 0.000772165 | 90 | 0.023257107 | 244,610 | 16,860,930 | 172,572 |
| 4 | 0 | 0.001038898 | 106 | 0.028988715 | 136,708 | 16,860,930 | 96,146 |
| 8 | 0 | 0.001408524 | 120 | 0.034767253 | 82,274 | 16,860,930 | 58,498 |
| 0.5 | 0.25 | 0.000470920 | 67 | 0.014418945 | 1,050,532 | 16,860,930 | 706,488 |
| 1 | 0.25 | 0.000686127 | 89 | 0.021952040 | 547,636 | 16,860,930 | 343,882 |
| 2 | 0.25 | 0.000927901 | 89 | 0.028677843 | 286,588 | 16,860,930 | 172,572 |
| 4 | 0.25 | 0.001287559 | 104 | 0.036565484 | 154,892 | 16,860,930 | 96,146 |
| 8 | 0.25 | 0.001748571 | 119 | 0.044445258 | 92,082 | 16,860,930 | 58,498 |
| 0.5 | 0.5 | 0.000588298 | 63 | 0.016377224 | 1,457,655 | 16,860,930 | 706,488 |
| 1 | 0.5 | 0.000888772 | 76 | 0.026855469 | 803,542 | 16,860,930 | 343,882 |
| 2 | 0.5 | 0.001293488 | 86 | 0.039289551 | 424,701 | 16,860,930 | 172,572 |
| 4 | 0.5 | 0.001739593 | 98 | 0.050486111 | 235,700 | 16,860,930 | 96,146 |
| 8 | 0.5 | 0.002406155 | 107 | 0.062613932 | 141,742 | 16,860,930 | 58,498 |
| 0.5 | 0.75 | 0.000432483 | 73 | 0.007083062 | 846,522 | 16,860,930 | 706,488 |
| 1 | 0.75 | 0.000737666 | 74 | 0.016793349 | 514,744 | 16,860,930 | 343,882 |
| 2 | 0.75 | 0.001137425 | 75 | 0.031436632 | 311,515 | 16,860,930 | 172,572 |
| 4 | 0.75 | 0.001724902 | 159 | 0.049432509 | 190,767 | 16,860,930 | 96,146 |
| 8 | 0.75 | 0.002293859 | 159 | 0.067272407 | 119,276 | 16,860,930 | 58,498 |
| 0.5 | 1 | 0.000992809 | 31 | 0.038136936 | 133,446 | 16,860,930 | 706,488 |
| 1 | 1 | 0.001104147 | 119 | 0.042827148 | 112,176 | 16,860,930 | 343,882 |
| 2 | 1 | 0.001815562 | 120 | 0.064330512 | 83,489 | 16,860,930 | 172,572 |
| 4 | 1 | 0.002758670 | 120 | 0.087069227 | 68,216 | 16,860,930 | 96,146 |
| 8 | 1 | 0.004006336 | 120 | 0.117296821 | 52,213 | 16,860,930 | 58,498 |

### Temporal comparisons

`target/debug/clod-view pop <out>/<asset>-1.clod --threshold-px 1 --steps 240
--size 2560x1440 --out <out>/L2a/<asset>/pop`. t=step/239. 239 pairs per asset.
The requested metric is the **excess MAD** column; the stronger spatial residual
also exposes redistribution that can cancel in a difference of image means.

| Asset | Max excess MAD | Step | Max temporal residual | Step | Command wall seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| gaul | 0.000113175580 | 220 | 0.003234335640 | 195 | 9.742247 |
| washington | 0.000217941517 | 231 | 0.001760733323 | 173 | 13.902593 |

The metric is nonzero: there is no claim of mathematically pop-free rendering.
All 240 samples of each initial path were reproduced exactly after changing
only PNG output scheduling; the corrected invariant-shadow sweep above is the
final result. It saves `pop-before.png`, `pop-after.png` and both naive frames.

### Visual inspection and limits

Opened the lit full-scene and close-up PNGs for both scans, both worst-pair
reference images, cluster colors and the Washington grid. The statues are
upright, visibly lit, cast shadows and show carved hair at the endpoint;
cluster colors form contiguous patches over the same visible surface. No black
frame, accidental sliver or globally inverted normals was seen. The close-ups
show source facets and visibly coarse shadow edges. At 8 px the worst-pair
images show altered relief/shadow detail; small mean RGB errors do not imply
that every feature is visually identical.

The L2a results above are from the offscreen CPU-reference lane. The browser library compiles but has
not been driven in a browser, and there is no interactive window, streaming,
occlusion culling or LOD morphing. L2b results below cover GPU selection. The declared
regression bounds apply to the measured fixtures/cameras, not arbitrary meshes.

Useful local PNGs: `<out>/L2a/<asset>/render-single-cluster.png`,
`debug-clusters.png`, `compare/worst-{cluster,naive,diff-10x}.png`,
`pop/pop-{before,after}.png`. `<out>` means
`~/Library/Caches/exact2-cluster-lod/out` throughout this report.

<details>
<summary>Exact render and timing JSON lines for all six requested scene/mode pairs per asset</summary>

The original numeric fields are preserved. `out` paths use `<out>` below.

```jsonl
{"asset":"gaul","bytes_resident_gpu":287510016,"command":"render","draws":4,"encode_ms":1.3777920000000001,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2a/gaul/render-single-cluster.png","padding_triangles":15250,"readback_bytes":14745600,"selected_clusters":1281,"selection_ms":0.390125,"shadow_clusters":761,"shadow_draws":4,"shadow_padding":9932,"shadow_selection_ms":0.10162500000000001,"shadow_triangles":87476,"size":[2560,1440],"submitted_vertices_or_indices":491904,"t":0.0,"threshold_px":1.0,"triangles_drawn":148718,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":287510048,"command":"time","draws":4,"encode_ms":0.33087500000000003,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"gpu_main_ms":0.523416,"gpu_ms":0.589749,"gpu_shadow_ms":0.066875,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2a/gaul/time-single-cluster.png","padding_triangles":15250,"readback_bytes":14745632,"selected_clusters":1281,"selection_ms":0.38795799999999997,"shadow_clusters":761,"shadow_draws":4,"shadow_padding":9932,"shadow_selection_ms":0.112917,"shadow_triangles":87476,"size":[2560,1440],"submitted_vertices_or_indices":491904,"t":0.0,"threshold_px":1.0,"triangles_drawn":148718,"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237488092,"command":"render","draws":1,"encode_ms":1.045125,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"naive","out":"<out>/L2a/gaul/render-single-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":4.2e-05,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":4000020,"size":[2560,1440],"submitted_vertices_or_indices":12000060,"t":0.0,"threshold_px":1.0,"triangles_drawn":4000020,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":237488124,"command":"time","draws":1,"encode_ms":0.252833,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"gpu_main_ms":0.9989579999999999,"gpu_ms":1.407625,"gpu_shadow_ms":0.408667,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"naive","out":"<out>/L2a/gaul/time-single-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":4000020,"size":[2560,1440],"submitted_vertices_or_indices":12000060,"t":0.0,"threshold_px":1.0,"triangles_drawn":4000020,"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287502528,"command":"render","draws":4,"encode_ms":1.1888750000000001,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"ring:12","measured_frames":1,"mode":"cluster","out":"<out>/L2a/gaul/render-ring-12-cluster.png","padding_triangles":11578,"readback_bytes":14745600,"selected_clusters":904,"selection_ms":1.198042,"shadow_clusters":720,"shadow_draws":4,"shadow_padding":7656,"shadow_selection_ms":0.80325,"shadow_triangles":84504,"size":[2560,1440],"submitted_vertices_or_indices":347136,"t":0.0,"threshold_px":1.0,"triangles_drawn":104134,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":287502560,"command":"time","draws":4,"encode_ms":0.28379099999999996,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"gpu_main_ms":0.8215,"gpu_ms":0.95825,"gpu_shadow_ms":0.137542,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2a/gaul/time-ring-12-cluster.png","padding_triangles":11578,"readback_bytes":14745632,"selected_clusters":904,"selection_ms":1.218583,"shadow_clusters":720,"shadow_draws":4,"shadow_padding":7656,"shadow_selection_ms":0.873042,"shadow_triangles":84504,"size":[2560,1440],"submitted_vertices_or_indices":347136,"t":0.0,"threshold_px":1.0,"triangles_drawn":104134,"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237488796,"command":"render","draws":1,"encode_ms":1.020291,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"ring:12","measured_frames":1,"mode":"naive","out":"<out>/L2a/gaul/render-ring-12-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":48000240,"size":[2560,1440],"submitted_vertices_or_indices":144000720,"t":0.0,"threshold_px":1.0,"triangles_drawn":48000240,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":237488828,"command":"time","draws":1,"encode_ms":0.360875,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"gpu_main_ms":2.4960839999999997,"gpu_ms":4.775041,"gpu_shadow_ms":2.2215,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"naive","out":"<out>/L2a/gaul/time-ring-12-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":48000240,"size":[2560,1440],"submitted_vertices_or_indices":144000720,"t":0.0,"threshold_px":1.0,"triangles_drawn":48000240,"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287576512,"command":"render","draws":4,"encode_ms":1.848167,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"grid:400","measured_frames":1,"mode":"cluster","out":"<out>/L2a/gaul/render-grid-400-cluster.png","padding_triangles":29904,"readback_bytes":14745600,"selected_clusters":3425,"selection_ms":47.747833,"shadow_clusters":2240,"shadow_draws":4,"shadow_padding":15680,"shadow_selection_ms":46.315416,"shadow_triangles":271040,"size":[2560,1440],"submitted_vertices_or_indices":1315200,"t":0.0,"threshold_px":1.0,"triangles_drawn":408496,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":287576544,"command":"time","draws":4,"encode_ms":0.38825,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"gpu_main_ms":1.171833,"gpu_ms":1.4523329999999999,"gpu_shadow_ms":0.280333,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2a/gaul/time-grid-400-cluster.png","padding_triangles":29904,"readback_bytes":14745632,"selected_clusters":3425,"selection_ms":41.13675,"shadow_clusters":2240,"shadow_draws":4,"shadow_padding":15680,"shadow_selection_ms":36.521958,"shadow_triangles":271040,"size":[2560,1440],"submitted_vertices_or_indices":1315200,"t":0.0,"threshold_px":1.0,"triangles_drawn":408496,"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237513628,"command":"render","draws":1,"encode_ms":1.2495,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"grid:400","measured_frames":1,"mode":"naive","out":"<out>/L2a/gaul/render-grid-400-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":4.1e-05,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.000125,"shadow_triangles":1600008000,"size":[2560,1440],"submitted_vertices_or_indices":4800024000,"t":0.0,"threshold_px":1.0,"triangles_drawn":1600008000,"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":237513660,"command":"time","draws":1,"encode_ms":0.308625,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"gpu_main_ms":74.83504099999999,"gpu_ms":143.09141599999998,"gpu_shadow_ms":68.50725,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"naive","out":"<out>/L2a/gaul/time-grid-400-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":1,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":1600008000,"size":[2560,1440],"submitted_vertices_or_indices":4800024000,"t":0.0,"threshold_px":1.0,"triangles_drawn":1600008000,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":775404024,"command":"render","draws":18,"encode_ms":2.078834,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2a/washington/render-single-cluster.png","padding_triangles":90278,"readback_bytes":14745600,"selected_clusters":4361,"selection_ms":0.550625,"shadow_clusters":3220,"shadow_draws":18,"shadow_padding":68278,"shadow_selection_ms":0.48304100000000005,"shadow_triangles":343882,"size":[2560,1440],"submitted_vertices_or_indices":1674624,"t":0.0,"threshold_px":1.0,"triangles_drawn":467930,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":775404056,"command":"time","draws":18,"encode_ms":0.538042,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"gpu_main_ms":1.8151249999999999,"gpu_ms":2.190375,"gpu_shadow_ms":0.375125,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2a/washington/time-single-cluster.png","padding_triangles":90278,"readback_bytes":14745632,"selected_clusters":4361,"selection_ms":1.369125,"shadow_clusters":3220,"shadow_draws":18,"shadow_padding":68278,"shadow_selection_ms":1.1539160000000002,"shadow_triangles":343882,"size":[2560,1440],"submitted_vertices_or_indices":1674624,"t":0.0,"threshold_px":1.0,"triangles_drawn":467930,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534102452,"command":"render","draws":2,"encode_ms":1.2175420000000001,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"naive","out":"<out>/L2a/washington/render-single-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":4.2e-05,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":0.000125,"shadow_triangles":16860930,"size":[2560,1440],"submitted_vertices_or_indices":50582790,"t":0.0,"threshold_px":1.0,"triangles_drawn":16860930,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":534102484,"command":"time","draws":2,"encode_ms":0.295834,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"gpu_main_ms":1.801375,"gpu_ms":2.9852090000000002,"gpu_shadow_ms":1.150958,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"naive","out":"<out>/L2a/washington/time-single-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":4.1e-05,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":4.1e-05,"shadow_triangles":16860930,"size":[2560,1440],"submitted_vertices_or_indices":50582790,"t":0.0,"threshold_px":1.0,"triangles_drawn":16860930,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":775526480,"command":"render","draws":18,"encode_ms":1.7979999999999998,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"ring:12","measured_frames":1,"mode":"cluster","out":"<out>/L2a/washington/render-ring-12-cluster.png","padding_triangles":286386,"readback_bytes":14745600,"selected_clusters":9627,"selection_ms":13.950707999999999,"shadow_clusters":7968,"shadow_draws":18,"shadow_padding":253968,"shadow_selection_ms":12.137208,"shadow_triangles":765936,"size":[2560,1440],"submitted_vertices_or_indices":3696768,"t":0.0,"threshold_px":1.0,"triangles_drawn":945870,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":775526512,"command":"time","draws":18,"encode_ms":0.49525,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"gpu_main_ms":1.1494579999999999,"gpu_ms":1.545124,"gpu_shadow_ms":0.39566599999999996,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2a/washington/time-ring-12-cluster.png","padding_triangles":286386,"readback_bytes":14745632,"selected_clusters":9627,"selection_ms":15.557625,"shadow_clusters":7968,"shadow_draws":18,"shadow_padding":253968,"shadow_selection_ms":11.6,"shadow_triangles":765936,"size":[2560,1440],"submitted_vertices_or_indices":3696768,"t":0.0,"threshold_px":1.0,"triangles_drawn":945870,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534103156,"command":"render","draws":2,"encode_ms":1.1165829999999999,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"ring:12","measured_frames":1,"mode":"naive","out":"<out>/L2a/washington/render-ring-12-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":202331160,"size":[2560,1440],"submitted_vertices_or_indices":606993480,"t":0.0,"threshold_px":1.0,"triangles_drawn":202331160,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":534103188,"command":"time","draws":2,"encode_ms":0.34808300000000003,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"gpu_main_ms":10.148541999999999,"gpu_ms":18.802041,"gpu_shadow_ms":8.677916999999999,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"naive","out":"<out>/L2a/washington/time-ring-12-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":0.0,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":4.2e-05,"shadow_triangles":202331160,"size":[2560,1440],"submitted_vertices_or_indices":606993480,"t":0.0,"threshold_px":1.0,"triangles_drawn":202331160,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":779549008,"command":"render","draws":18,"encode_ms":2.112208,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"grid:400","measured_frames":1,"mode":"cluster","out":"<out>/L2a/washington/render-grid-400-cluster.png","padding_triangles":6307462,"readback_bytes":14745600,"selected_clusters":156162,"selection_ms":406.417959,"shadow_clusters":142400,"shadow_draws":18,"shadow_padding":6112880,"shadow_selection_ms":360.568542,"shadow_triangles":12114320,"size":[2560,1440],"submitted_vertices_or_indices":59966208,"t":0.0,"threshold_px":1.0,"triangles_drawn":13681274,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":779549040,"command":"time","draws":18,"encode_ms":0.759666,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"gpu_main_ms":6.294709,"gpu_ms":15.719375,"gpu_shadow_ms":9.318,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2a/washington/time-grid-400-cluster.png","padding_triangles":6307462,"readback_bytes":14745632,"selected_clusters":156162,"selection_ms":368.294541,"shadow_clusters":142400,"shadow_draws":18,"shadow_padding":6112880,"shadow_selection_ms":345.027375,"shadow_triangles":12114320,"size":[2560,1440],"submitted_vertices_or_indices":59966208,"t":0.0,"threshold_px":1.0,"triangles_drawn":13681274,"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534127988,"command":"render","draws":2,"encode_ms":1.157959,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"gpu_main_ms":null,"gpu_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"grid:400","measured_frames":1,"mode":"naive","out":"<out>/L2a/washington/render-grid-400-naive.png","padding_triangles":0,"readback_bytes":14745600,"selected_clusters":0,"selection_ms":4.2e-05,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":6744372000,"size":[2560,1440],"submitted_vertices_or_indices":20233116000,"t":0.0,"threshold_px":1.0,"triangles_drawn":6744372000,"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":534128020,"command":"time","draws":2,"encode_ms":0.60025,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"gpu_main_ms":455.821708,"gpu_ms":881.97375,"gpu_shadow_ms":441.419334,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"naive","out":"<out>/L2a/washington/time-grid-400-naive.png","padding_triangles":0,"readback_bytes":14745632,"selected_clusters":0,"selection_ms":4.2e-05,"shadow_clusters":0,"shadow_draws":2,"shadow_padding":0,"shadow_selection_ms":4.1e-05,"shadow_triangles":6744372000,"size":[2560,1440],"submitted_vertices_or_indices":20233116000,"t":0.0,"threshold_px":1.0,"triangles_drawn":6744372000,"view":"lit","warmup_frames":1}
```

</details>


## L2b verification and measurements

Run from this workspace: `python3 measure.py verify`, `python3 measure.py oracles`,
then `python3 measure.py sweep`. Raw commands, recorded child PIDs and exit codes
are in `<out>/L2b/{processes,runs}.jsonl`; each command's JSON lines and stderr are
in `<out>/L2b/<case>.log`. Images remain in that cache directory. No vendor,
workspace-root, engine, rule, or LLP file was changed.

### Verification

| Command | Result | Wall seconds |
| --- | --- | ---: |
| `cargo test --workspace --no-fail-fast -- --nocapture` | 6 passed; no GPU skips | 25.375433 |
| `cargo clippy --all-targets -- -D warnings` | passed | 0.794380 |
| `cargo fmt --all -- --check` | passed | 0.117755 |
| `cargo build -p clod-format --target wasm32-unknown-unknown` | passed | 0.104719 |
| `cargo build -p clod-view --lib --target wasm32-unknown-unknown` | passed | 0.697083 |
| `clod-view oracle <out>/gaul-1.clod --steps 64 --size 256x256` | passed | 9.627883 |
| `clod-view oracle <out>/washington-1.clod --steps 64 --size 256x256` | passed | 39.938490 |

All oracle commands request **zero features** and exactly `Limits::default()`;
selection uses seven storage bindings, raster uses five, workgroups contain 256
invocations, and the largest dispatch is 5,000 workgroups in the measured scenes.
Both WGSL files validate with Naga's empty capability set at build time. Source
caps: 38 Rust/WGSL/Python/C++ files, maximum 678 lines, zero over 1,500.

| Fixture | Cameras/layout | Layouts | Identical set/image pairs | Near-boundary differences | Exact color + shadow culling pairs | Identical repeated PNGs | Overflow drops (three tiny-capacity cases) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L1 octasphere, 192² | 64 | 3 | 192 | 0 | 96 | 192 | 4,041 |
| Gaul, 256² | 64 | 3 | 192 | 0 | 96 | 192 | 3,520 |
| Washington, 256² | 64 | 3 | 192 | 0 | 96 | 192 | 276,958 |

The three layouts are `single`, `ring:12`, `grid:400`. Half the cameras follow
far-to-detail; half are inside the scene looking outward. Total: 576 set/image
pairs, 32,243,712 color pixels compared exactly, 288 culling pairs with direct
2048² shadow-depth byte comparisons, and 576 deterministic PNG pairs. No draw-order
pixel exception was needed: stable page/instance/cluster order is preserved.
The 64 unculled procedural GPU cuts checked 5,927,820 decoded triangles with
L1's shared edge oracle: zero bad edges. No 1e-5 boundary exceptions occurred;
when one does, the CLI checks the affected unculled GPU main/light cuts' edges.
The two real scans are not asserted to be closed manifolds.

All nine capacity-one cases matched the CPU's exact retained cluster subsets,
exact drop counts and rendered pixels, without validation errors. They dropped
284,519 selected clusters in total. This verifies safe deterministic degradation;
it does **not** claim that an overflowing cut stays watertight. Normal measurement
scenes have zero overflow in both passes. L1's numerical, corruption, loader and
byte-determinism checks, L2a's procedural images, and its 12 real-asset baseline
comparisons remain green. The subdivision-6 limitation is documented in decision 28.

### Timing method

Every timing row is `target/debug/clod-view time <out>/<asset>-1.clod --layout
<layout> --size 2560x1440 --threshold-px 1 --frames 7 --select <gpu|brute|cpu>
--out <out>/L2b/<case>.png`; naive substitutes `--mode naive`. One warmup, then
seven measured frames; medians are per column, so totals need not sum across
median columns. `GPU select` includes selection and stable compaction for **both**
passes. `CPU` is both selections plus encoding/submission. `Completion` measures
encode start through completed pixel readback; it excludes CPU reference selection,
post-frame counter diagnostics, PNG encoding and scene load. Resident bytes count
allocated buffers/textures, excluding diagnostic staging, driver and shader memory.
All geometry and instance records are resident and immutable after scene load.

Initial main timestamp failures and the three-round stop are documented in
decision 27. Each final JSON reports valid timestamp sample counts explicitly;
a stage with fewer than seven valid measured samples is null, not a partial median.
The completed-frame host-clock number provides an independent latency measurement.

Final 32 rows have **7/7 valid samples for every timing column**, zero invalid
counter lines, and zero overflow. The GPU/brute rows were refreshed after the
terminal-sentinel guard; CPU/naive code was unchanged. Measured PNGs are byte-identical
for all 16 GPU-vs-CPU and GPU-vs-brute comparisons, including both 5,000-instance
fields. Main/shadow cluster, useful-triangle and padding counts also match.
Six additional endpoint-threshold cases agree exactly: zero selects
524,288 / 4,000,020 / 16,860,930 triangles; `f32::MAX / 2` selects
126 / 114 / 24,515, for the fixture / Gaul / Washington, in both passes.

| Asset / layout / selector | CPU ms | GPU select ms | GPU main ms | GPU shadow ms | GPU total ms | Completion ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| gaul / single / gpu | 0.502791 | 0.424125 | 0.532042 | 0.066791 | 1.022126 | 3.901209 |
| gaul / single / brute | 0.522625 | 1.958334 | 0.477792 | 0.060250 | 2.495542 | 5.076250 |
| gaul / single / cpu | 1.686917 | 0.000000 | 1.352208 | 0.154167 | 1.509458 | 4.507125 |
| gaul / single / naive | 0.429334 | 0.000000 | 0.982292 | 0.435833 | 1.468416 | 4.871083 |
| gaul / ring:12 / gpu | 0.490584 | 0.244041 | 0.810542 | 0.138083 | 1.193917 | 3.848709 |
| gaul / ring:12 / brute | 0.441666 | 0.997791 | 0.175958 | 0.031875 | 1.205916 | 3.894583 |
| gaul / ring:12 / cpu | 4.844125 | 0.000000 | 0.812166 | 0.140333 | 0.952125 | 4.294291 |
| gaul / ring:12 / naive | 0.413833 | 0.000000 | 2.456833 | 2.225791 | 4.674041 | 8.165000 |
| gaul / grid:400 / gpu | 0.453875 | 2.067667 | 0.272291 | 0.164959 | 2.504917 | 4.852750 |
| gaul / grid:400 / brute | 0.422833 | 4.274542 | 0.242875 | 0.061333 | 4.581791 | 6.910250 |
| gaul / grid:400 / cpu | 92.764084 | 0.000000 | 1.165208 | 0.282375 | 1.447583 | 5.008375 |
| gaul / grid:400 / naive | 0.453042 | 0.000000 | 76.843792 | 68.361459 | 147.098750 | 151.425542 |
| gaul / field:5000,1 / gpu | 0.435125 | 5.505250 | 0.319917 | 0.110208 | 5.932291 | 8.442875 |
| gaul / field:5000,1 / brute | 0.445917 | 46.030417 | 0.322916 | 0.109958 | 46.463376 | 48.958917 |
| gaul / field:5000,1 / cpu | 1275.701542 | 0.000000 | 1.484750 | 0.514875 | 1.999917 | 13.590708 |
| gaul / field:5000,1 / naive | 0.812417 | 0.000000 | 1148.077583 | 1055.947083 | 2205.199417 | 2221.871500 |
| washington / single / gpu | 0.549166 | 3.368291 | 0.635500 | 0.127250 | 4.179708 | 6.616042 |
| washington / single / brute | 0.505042 | 4.329042 | 0.398875 | 0.078917 | 4.806626 | 7.240792 |
| washington / single / cpu | 13.314292 | 0.000000 | 1.816292 | 0.369334 | 2.185292 | 5.791041 |
| washington / single / naive | 0.458541 | 0.000000 | 1.866917 | 1.218125 | 3.098124 | 6.628167 |
| washington / ring:12 / gpu | 0.538292 | 0.319791 | 0.887208 | 0.306375 | 1.512791 | 4.109500 |
| washington / ring:12 / brute | 0.515499 | 4.199542 | 0.455417 | 0.152208 | 4.798708 | 7.290084 |
| washington / ring:12 / cpu | 42.050208 | 0.000000 | 2.121709 | 0.722291 | 2.853208 | 6.817459 |
| washington / ring:12 / naive | 0.386250 | 0.000000 | 10.057334 | 8.542875 | 18.755084 | 21.450916 |
| washington / grid:400 / gpu | 0.723000 | 0.620416 | 3.934208 | 2.238334 | 6.788916 | 9.748042 |
| washington / grid:400 / brute | 0.548833 | 18.986000 | 3.894584 | 2.255333 | 25.136875 | 27.720667 |
| washington / grid:400 / cpu | 995.470999 | 0.000000 | 6.329708 | 9.326833 | 15.645416 | 19.490667 |
| washington / grid:400 / naive | 0.424792 | 0.000000 | 349.522209 | 330.212125 | 679.557000 | 686.124417 |
| washington / field:5000,1 / gpu | 0.613958 | 6.948000 | 34.271083 | 23.094708 | 64.421874 | 67.651167 |
| washington / field:5000,1 / brute | 0.602625 | 198.440208 | 34.221042 | 23.042208 | 255.820626 | 260.308500 |
| washington / field:5000,1 / cpu | 18963.177751 | 0.000000 | 34.168334 | 32.682500 | 66.797583 | 100.922125 |
| washington / field:5000,1 / naive | 0.911167 | 0.000000 | 4794.789875 | 4259.548667 | 9066.564792 | 9199.697708 |

CPU-reference submissions are separated by much longer CPU work than GPU-selected
submissions. These are measured frame costs, not a controlled clock experiment;
the different raster timings do not imply different triangle work.

### Candidate scaling and resident geometry

`Candidates` and `clusters` below are main/shadow pairs. CPU and brute test the
same candidate count at these cameras. Geometry and draws are identical across
all three selectors. Draw columns exclude the one additional ground draw.

| Asset / layout | GPU candidates | CPU/brute candidates | Visible clusters | Useful triangles | Padding triangles | Main/shadow draws | GPU/brute resident bytes | CPU resident bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gaul / single | 16,861 / 1,618 | 65,415 / 65,415 | 1,218 / 751 | 142,297 / 86,701 | 13,607 / 9,427 | 4 / 4 | 289,317,940 | 287,510,112 |
| gaul / ring:12 | 1,487 / 720 | 784,980 / 784,980 | 895 / 720 | 103,957 / 84,504 | 10,603 / 7,656 | 4 / 4 | 303,711,352 | 287,502,624 |
| gaul / grid:400 | 3,788 / 2,240 | 26,166,000 / 26,166,000 | 3,408 / 2,240 | 408,445 / 271,040 | 27,779 / 15,680 | 4 / 4 | 371,966,248 | 287,576,608 |
| gaul / field:5000,1 | 5,528 / 5,000 | 327,075,000 / 327,075,000 | 5,457 / 5,000 | 628,496 / 570,000 | 70,000 / 70,000 | 4 / 4 | 372,769,448 | 287,936,544 |
| washington / single | 209,901 / 23,885 | 281,343 / 281,343 | 4,160 / 3,143 | 452,524 / 339,461 | 79,956 / 62,843 | 18 / 18 | 783,208,660 | 775,404,120 |
| washington / ring:12 | 59,732 / 37,692 | 3,376,116 / 3,376,116 | 9,475 / 7,880 | 940,530 / 763,947 | 272,270 / 244,693 | 18 / 18 | 845,109,928 | 775,526,576 |
| washington / grid:400 | 371,502 / 245,520 | 112,537,200 / 112,537,200 | 155,455 / 141,700 | 13,674,953 / 12,112,720 | 6,223,287 / 6,024,880 | 18 / 18 | 861,672,472 | 779,549,104 |
| washington / field:5000,1 | 2,496,205 / 1,949,105 | 1,406,715,000 / 1,406,715,000 | 1,496,826 / 1,489,948 | 124,878,597 / 124,177,569 | 66,715,131 / 66,535,775 | 18 / 18 | 864,021,272 | 809,203,632 |

| Asset / layout | Naive triangles per pass | Naive draws per pass | Naive resident bytes | Brute / range selection time ratio |
| --- | ---: | ---: | ---: | ---: |
| gaul / single | 4,000,020 | 1 | 237,488,188 | 4.617351× |
| gaul / ring:12 | 48,000,240 | 1 | 237,488,892 | 4.088620× |
| gaul / grid:400 | 1,600,008,000 | 1 | 237,513,724 | 2.067326× |
| gaul / field:5000,1 | 20,000,100,000 | 1 | 237,808,124 | 8.361186× |
| washington / single | 16,860,930 | 2 | 534,102,548 | 1.285234× |
| washington / ring:12 | 202,331,160 | 2 | 534,103,252 | 13.132146× |
| washington / grid:400 | 6,744,372,000 | 2 | 534,128,084 | 30.602048× |
| washington / field:5000,1 | 84,304,650,000 | 2 | 534,422,484 | 28.560767× |

Washington `grid:400` meets the target in this run: **6.788916 ms GPU,
0.723000 ms CPU**, with 9.748042 ms completed-frame
latency including pixel transfer. Its two selections test 617,022 candidates
instead of 225,074,400 (364.775324× fewer), and selection/compaction is
30.602048× faster than brute force. Total GPU time
is 100.098013× below the measured naive path. These are medians
on this machine, not a cross-device guarantee.

Washington `field:5000,1` remains raster-limited: 124,878,597 useful main triangles
plus 66,715,131 padding triangles, and 124,177,569 useful shadow triangles plus
66,535,775 padding triangles. GPU selection removes the CPU scan but cannot remove
this geometry floor. Its complete terminal cut is 296 clusters / 24,515 triangles /
34,635 copied vertices; `clod-bake --cut <out>/washington-1.clod --threshold 1e30
--obj <out>/L2b-washington-terminal.obj` produced those counts. The original
6,088-triangle figure counted only depth 11. The optional fallback experiment
was not performed; decision 29 records why.

### Images inspected and remaining limits

Opened `<out>/L2b/{gaul,washington}-close-{lit,clusters}.png` (2560×1440),
`washington-grid-400-gpu.png`, `gaul-field-5000-1-gpu.png`, and the initial
384² Gaul frame. The close-ups show carved relief, source facets and coarse
shadow edges; cluster colors cover contiguous patches of the same surface.
Washington has a small separate surface patch in the close-up; the camera
oracles found no GPU-vs-CPU image difference. The grid shows repeated upright statues with shadows. The
far field's individual statues are only a few pixels wide. The inherited large
ground plane shows a stepped distant clipping boundary in wide views. No
GPU-selection-only holes or garbage triangles were observed at normal capacity;
the numerical image checks are stronger evidence than that visual inspection.

This remains an offscreen demo with a continuous authored camera path. There is
no interactive window, browser runtime validation, HZB, streaming, LOD morphing,
or mathematical no-pop guarantee; L2a's nonzero temporal error measurements still
apply. Capacity overflow intentionally drops geometry and is reported. Initial
counter failures and the subdivision-6 bake's edge defects remain documented;
all final required commands and measured camera oracles pass. No claim is made
that arbitrary new meshes inherit the sampled image or topology bounds.

<details>
<summary>Exact final timing JSON for all 32 scene/selector cases and four inspected close-ups</summary>

```jsonl
{"asset":"gaul","bytes_resident_gpu":289317940,"candidates_tested":16861,"capacity_per_instance":65415,"command":"time","cpu_ms":0.5027910000000001,"culling":true,"draws":4,"encode_ms":0.5026660000000001,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"frame_completion_ms":3.901209,"gpu_main_ms":0.532042,"gpu_ms":1.0221259999999999,"gpu_select_ms":0.424125,"gpu_shadow_ms":0.066791,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-single-gpu.png","overflow":0,"padding_triangles":13607,"readback_bytes":14745664,"selected_clusters":1218,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":1618,"shadow_clusters":751,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":9427,"shadow_selection_ms":4.2e-05,"shadow_triangles":86701,"size":[2560,1440],"submitted_vertices_or_indices":467712,"t":0.0,"threshold_px":1.0,"triangles_drawn":142297,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":289317940,"candidates_tested":65415,"capacity_per_instance":65415,"command":"time","cpu_ms":0.522625,"culling":true,"draws":4,"encode_ms":0.522625,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"frame_completion_ms":5.07625,"gpu_main_ms":0.477792,"gpu_ms":2.495542,"gpu_select_ms":1.958334,"gpu_shadow_ms":0.06025,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-single-brute.png","overflow":0,"padding_triangles":13607,"readback_bytes":14745664,"selected_clusters":1218,"selection_ms":0.0,"selector":"brute","shadow_candidates_tested":65415,"shadow_clusters":751,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":9427,"shadow_selection_ms":0.0,"shadow_triangles":86701,"size":[2560,1440],"submitted_vertices_or_indices":467712,"t":0.0,"threshold_px":1.0,"triangles_drawn":142297,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287510112,"candidates_tested":65415,"capacity_per_instance":null,"command":"time","cpu_ms":1.686917,"culling":true,"draws":4,"encode_ms":0.376583,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"frame_completion_ms":4.507125,"gpu_main_ms":1.3522079999999999,"gpu_ms":1.509458,"gpu_select_ms":0.0,"gpu_shadow_ms":0.154167,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-single-cpu.png","overflow":0,"padding_triangles":13607,"readback_bytes":14745664,"selected_clusters":1218,"selection_ms":0.913667,"selector":"cpu","shadow_candidates_tested":65415,"shadow_clusters":751,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":9427,"shadow_selection_ms":0.45079199999999997,"shadow_triangles":86701,"size":[2560,1440],"submitted_vertices_or_indices":467712,"t":0.0,"threshold_px":1.0,"triangles_drawn":142297,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237488188,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.429334,"culling":true,"draws":1,"encode_ms":0.429334,"eye":[1.4864147901535034,-2.797957420349121,1.971238136291504],"frame_completion_ms":4.871083,"gpu_main_ms":0.9822919999999999,"gpu_ms":1.468416,"gpu_select_ms":0.0,"gpu_shadow_ms":0.43583299999999997,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"naive","out":"<out>/L2b/gaul-single-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":4.2e-05,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":1,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":4000020,"size":[2560,1440],"submitted_vertices_or_indices":12000060,"t":0.0,"threshold_px":1.0,"triangles_drawn":4000020,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":303711352,"candidates_tested":1487,"capacity_per_instance":65415,"command":"time","cpu_ms":0.49058399999999996,"culling":true,"draws":4,"encode_ms":0.490542,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"frame_completion_ms":3.848709,"gpu_main_ms":0.810542,"gpu_ms":1.1939170000000001,"gpu_select_ms":0.24404099999999998,"gpu_shadow_ms":0.13808299999999998,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-ring-12-gpu.png","overflow":0,"padding_triangles":10603,"readback_bytes":14745664,"selected_clusters":895,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":720,"shadow_clusters":720,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":7656,"shadow_selection_ms":0.0,"shadow_triangles":84504,"size":[2560,1440],"submitted_vertices_or_indices":343680,"t":0.0,"threshold_px":1.0,"triangles_drawn":103957,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":303711352,"candidates_tested":784980,"capacity_per_instance":65415,"command":"time","cpu_ms":0.441666,"culling":true,"draws":4,"encode_ms":0.441666,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"frame_completion_ms":3.894583,"gpu_main_ms":0.175958,"gpu_ms":1.205916,"gpu_select_ms":0.9977909999999999,"gpu_shadow_ms":0.031875,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-ring-12-brute.png","overflow":0,"padding_triangles":10603,"readback_bytes":14745664,"selected_clusters":895,"selection_ms":0.0,"selector":"brute","shadow_candidates_tested":784980,"shadow_clusters":720,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":7656,"shadow_selection_ms":0.0,"shadow_triangles":84504,"size":[2560,1440],"submitted_vertices_or_indices":343680,"t":0.0,"threshold_px":1.0,"triangles_drawn":103957,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287502624,"candidates_tested":784980,"capacity_per_instance":null,"command":"time","cpu_ms":4.844125,"culling":true,"draws":4,"encode_ms":0.481834,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"frame_completion_ms":4.294291,"gpu_main_ms":0.8121659999999999,"gpu_ms":0.9521249999999999,"gpu_select_ms":0.0,"gpu_shadow_ms":0.14033299999999999,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-ring-12-cpu.png","overflow":0,"padding_triangles":10603,"readback_bytes":14745664,"selected_clusters":895,"selection_ms":2.335542,"selector":"cpu","shadow_candidates_tested":784980,"shadow_clusters":720,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":7656,"shadow_selection_ms":1.878625,"shadow_triangles":84504,"size":[2560,1440],"submitted_vertices_or_indices":343680,"t":0.0,"threshold_px":1.0,"triangles_drawn":103957,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237488892,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.413833,"culling":true,"draws":1,"encode_ms":0.413833,"eye":[12.100516319274902,-22.777442932128906,12.585339546203613],"frame_completion_ms":8.165000000000001,"gpu_main_ms":2.456833,"gpu_ms":4.674041,"gpu_select_ms":0.0,"gpu_shadow_ms":2.225791,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"naive","out":"<out>/L2b/gaul-ring-12-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":1,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":48000240,"size":[2560,1440],"submitted_vertices_or_indices":144000720,"t":0.0,"threshold_px":1.0,"triangles_drawn":48000240,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":371966248,"candidates_tested":3788,"capacity_per_instance":10485,"command":"time","cpu_ms":0.45387500000000003,"culling":true,"draws":4,"encode_ms":0.45387500000000003,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"frame_completion_ms":4.85275,"gpu_main_ms":0.272291,"gpu_ms":2.504917,"gpu_select_ms":2.067667,"gpu_shadow_ms":0.164959,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-grid-400-gpu.png","overflow":0,"padding_triangles":27779,"readback_bytes":14745664,"selected_clusters":3408,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":2240,"shadow_clusters":2240,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":15680,"shadow_selection_ms":0.0,"shadow_triangles":271040,"size":[2560,1440],"submitted_vertices_or_indices":1308672,"t":0.0,"threshold_px":1.0,"triangles_drawn":408445,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":371966248,"candidates_tested":26166000,"capacity_per_instance":10485,"command":"time","cpu_ms":0.42283299999999996,"culling":true,"draws":4,"encode_ms":0.422791,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"frame_completion_ms":6.91025,"gpu_main_ms":0.24287499999999998,"gpu_ms":4.581791,"gpu_select_ms":4.274542,"gpu_shadow_ms":0.061333,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-grid-400-brute.png","overflow":0,"padding_triangles":27779,"readback_bytes":14745664,"selected_clusters":3408,"selection_ms":4.1e-05,"selector":"brute","shadow_candidates_tested":26166000,"shadow_clusters":2240,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":15680,"shadow_selection_ms":4.1e-05,"shadow_triangles":271040,"size":[2560,1440],"submitted_vertices_or_indices":1308672,"t":0.0,"threshold_px":1.0,"triangles_drawn":408445,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287576608,"candidates_tested":26166000,"capacity_per_instance":null,"command":"time","cpu_ms":92.764084,"culling":true,"draws":4,"encode_ms":0.613417,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"frame_completion_ms":5.008375,"gpu_main_ms":1.165208,"gpu_ms":1.447583,"gpu_select_ms":0.0,"gpu_shadow_ms":0.282375,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-grid-400-cpu.png","overflow":0,"padding_triangles":27779,"readback_bytes":14745664,"selected_clusters":3408,"selection_ms":45.784625000000005,"selector":"cpu","shadow_candidates_tested":26166000,"shadow_clusters":2240,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":15680,"shadow_selection_ms":40.625959,"shadow_triangles":271040,"size":[2560,1440],"submitted_vertices_or_indices":1308672,"t":0.0,"threshold_px":1.0,"triangles_drawn":408445,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237513724,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.453042,"culling":true,"draws":1,"encode_ms":0.453,"eye":[51.26447296142578,-96.09046936035156,51.605613708496094],"frame_completion_ms":151.425542,"gpu_main_ms":76.843792,"gpu_ms":147.09875,"gpu_select_ms":0.0,"gpu_shadow_ms":68.361459,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"naive","out":"<out>/L2b/gaul-grid-400-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":4.2e-05,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":1,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":1600008000,"size":[2560,1440],"submitted_vertices_or_indices":4800024000,"t":0.0,"threshold_px":1.0,"triangles_drawn":1600008000,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":372769448,"candidates_tested":5528,"capacity_per_instance":838,"command":"time","cpu_ms":0.43512500000000004,"culling":true,"draws":4,"encode_ms":0.43512500000000004,"eye":[216.71762084960938,-408.0025634765625,217.35040283203125],"frame_completion_ms":8.442875,"gpu_main_ms":0.319917,"gpu_ms":5.932290999999999,"gpu_select_ms":5.50525,"gpu_shadow_ms":0.110208,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-field-5000-1-gpu.png","overflow":0,"padding_triangles":70000,"readback_bytes":14745664,"selected_clusters":5457,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":5000,"shadow_clusters":5000,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":70000,"shadow_selection_ms":0.0,"shadow_triangles":570000,"size":[2560,1440],"submitted_vertices_or_indices":2095488,"t":0.0,"threshold_px":1.0,"triangles_drawn":628496,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":372769448,"candidates_tested":327075000,"capacity_per_instance":838,"command":"time","cpu_ms":0.44591699999999995,"culling":true,"draws":4,"encode_ms":0.44587499999999997,"eye":[216.71762084960938,-408.0025634765625,217.35040283203125],"frame_completion_ms":48.958917,"gpu_main_ms":0.322916,"gpu_ms":46.463376,"gpu_select_ms":46.030417,"gpu_shadow_ms":0.109958,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-field-5000-1-brute.png","overflow":0,"padding_triangles":70000,"readback_bytes":14745664,"selected_clusters":5457,"selection_ms":0.0,"selector":"brute","shadow_candidates_tested":327075000,"shadow_clusters":5000,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":70000,"shadow_selection_ms":0.0,"shadow_triangles":570000,"size":[2560,1440],"submitted_vertices_or_indices":2095488,"t":0.0,"threshold_px":1.0,"triangles_drawn":628496,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":287936544,"candidates_tested":327075000,"capacity_per_instance":null,"command":"time","cpu_ms":1275.7015420000002,"culling":true,"draws":4,"encode_ms":0.679458,"eye":[216.71762084960938,-408.0025634765625,217.35040283203125],"frame_completion_ms":13.590708,"gpu_main_ms":1.48475,"gpu_ms":1.999917,"gpu_select_ms":0.0,"gpu_shadow_ms":0.514875,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/gaul-field-5000-1-cpu.png","overflow":0,"padding_triangles":70000,"readback_bytes":14745664,"selected_clusters":5457,"selection_ms":676.224792,"selector":"cpu","shadow_candidates_tested":327075000,"shadow_clusters":5000,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":70000,"shadow_selection_ms":598.8202500000001,"shadow_triangles":570000,"size":[2560,1440],"submitted_vertices_or_indices":2095488,"t":0.0,"threshold_px":1.0,"triangles_drawn":628496,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":237808124,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.812417,"culling":true,"draws":1,"encode_ms":0.812417,"eye":[216.71762084960938,-408.0025634765625,217.35040283203125],"frame_completion_ms":2221.8714999999997,"gpu_main_ms":1148.077583,"gpu_ms":2205.199417,"gpu_select_ms":0.0,"gpu_shadow_ms":1055.947083,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"naive","out":"<out>/L2b/gaul-field-5000-1-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":4.1e-05,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":1,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":20000100000,"size":[2560,1440],"submitted_vertices_or_indices":60000300000,"t":0.0,"threshold_px":1.0,"triangles_drawn":20000100000,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":783208660,"candidates_tested":209901,"capacity_per_instance":281343,"command":"time","cpu_ms":0.549166,"culling":true,"draws":18,"encode_ms":0.549083,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"frame_completion_ms":6.616042,"gpu_main_ms":0.6355,"gpu_ms":4.179708,"gpu_select_ms":3.3682909999999997,"gpu_shadow_ms":0.12725,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-single-gpu.png","overflow":0,"padding_triangles":79956,"readback_bytes":14745664,"selected_clusters":4160,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":23885,"shadow_clusters":3143,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":62843,"shadow_selection_ms":0.0,"shadow_triangles":339461,"size":[2560,1440],"submitted_vertices_or_indices":1597440,"t":0.0,"threshold_px":1.0,"triangles_drawn":452524,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":783208660,"candidates_tested":281343,"capacity_per_instance":281343,"command":"time","cpu_ms":0.505042,"culling":true,"draws":18,"encode_ms":0.505042,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"frame_completion_ms":7.240792,"gpu_main_ms":0.398875,"gpu_ms":4.806626,"gpu_select_ms":4.329041999999999,"gpu_shadow_ms":0.078917,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-single-brute.png","overflow":0,"padding_triangles":79956,"readback_bytes":14745664,"selected_clusters":4160,"selection_ms":0.0,"selector":"brute","shadow_candidates_tested":281343,"shadow_clusters":3143,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":62843,"shadow_selection_ms":0.0,"shadow_triangles":339461,"size":[2560,1440],"submitted_vertices_or_indices":1597440,"t":0.0,"threshold_px":1.0,"triangles_drawn":452524,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":775404120,"candidates_tested":281343,"capacity_per_instance":null,"command":"time","cpu_ms":13.314292,"culling":true,"draws":18,"encode_ms":0.5555,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"frame_completion_ms":5.791041,"gpu_main_ms":1.816292,"gpu_ms":2.185292,"gpu_select_ms":0.0,"gpu_shadow_ms":0.369334,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-single-cpu.png","overflow":0,"padding_triangles":79956,"readback_bytes":14745664,"selected_clusters":4160,"selection_ms":6.703291999999999,"selector":"cpu","shadow_candidates_tested":281343,"shadow_clusters":3143,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":62843,"shadow_selection_ms":6.010458,"shadow_triangles":339461,"size":[2560,1440],"submitted_vertices_or_indices":1597440,"t":0.0,"threshold_px":1.0,"triangles_drawn":452524,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534102548,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.458541,"culling":true,"draws":2,"encode_ms":0.45849999999999996,"eye":[1.727691650390625,-3.2521255016326904,2.727691650390625],"frame_completion_ms":6.6281669999999995,"gpu_main_ms":1.866917,"gpu_ms":3.098124,"gpu_select_ms":0.0,"gpu_shadow_ms":1.218125,"ground_draws":1,"layout":"single","measured_frames":7,"mode":"naive","out":"<out>/L2b/washington-single-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":2,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":4.1e-05,"shadow_triangles":16860930,"size":[2560,1440],"submitted_vertices_or_indices":50582790,"t":0.0,"threshold_px":1.0,"triangles_drawn":16860930,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":845109928,"candidates_tested":59732,"capacity_per_instance":281343,"command":"time","cpu_ms":0.538292,"culling":true,"draws":18,"encode_ms":0.53825,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"frame_completion_ms":4.109500000000001,"gpu_main_ms":0.887208,"gpu_ms":1.5127910000000002,"gpu_select_ms":0.319791,"gpu_shadow_ms":0.306375,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-ring-12-gpu.png","overflow":0,"padding_triangles":272270,"readback_bytes":14745664,"selected_clusters":9475,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":37692,"shadow_clusters":7880,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":244693,"shadow_selection_ms":4.2e-05,"shadow_triangles":763947,"size":[2560,1440],"submitted_vertices_or_indices":3638400,"t":0.0,"threshold_px":1.0,"triangles_drawn":940530,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":845109928,"candidates_tested":3376116,"capacity_per_instance":281343,"command":"time","cpu_ms":0.5154989999999999,"culling":true,"draws":18,"encode_ms":0.515458,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"frame_completion_ms":7.290083999999999,"gpu_main_ms":0.45541699999999996,"gpu_ms":4.7987079999999995,"gpu_select_ms":4.199542,"gpu_shadow_ms":0.15220799999999998,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-ring-12-brute.png","overflow":0,"padding_triangles":272270,"readback_bytes":14745664,"selected_clusters":9475,"selection_ms":4.2e-05,"selector":"brute","shadow_candidates_tested":3376116,"shadow_clusters":7880,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":244693,"shadow_selection_ms":4.1e-05,"shadow_triangles":763947,"size":[2560,1440],"submitted_vertices_or_indices":3638400,"t":0.0,"threshold_px":1.0,"triangles_drawn":940530,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":775526576,"candidates_tested":3376116,"capacity_per_instance":null,"command":"time","cpu_ms":42.050208,"culling":true,"draws":18,"encode_ms":0.665834,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"frame_completion_ms":6.8174589999999995,"gpu_main_ms":2.121709,"gpu_ms":2.853208,"gpu_select_ms":0.0,"gpu_shadow_ms":0.722291,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-ring-12-cpu.png","overflow":0,"padding_triangles":272270,"readback_bytes":14745664,"selected_clusters":9475,"selection_ms":22.692,"selector":"cpu","shadow_candidates_tested":3376116,"shadow_clusters":7880,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":244693,"shadow_selection_ms":19.788957999999997,"shadow_triangles":763947,"size":[2560,1440],"submitted_vertices_or_indices":3638400,"t":0.0,"threshold_px":1.0,"triangles_drawn":940530,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534103252,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.38625,"culling":true,"draws":2,"encode_ms":0.386167,"eye":[11.758244514465332,-22.133167266845703,12.758244514465332],"frame_completion_ms":21.450916,"gpu_main_ms":10.057333999999999,"gpu_ms":18.755083999999997,"gpu_select_ms":0.0,"gpu_shadow_ms":8.542875,"ground_draws":1,"layout":"ring:12","measured_frames":7,"mode":"naive","out":"<out>/L2b/washington-ring-12-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":4.1e-05,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":2,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":4.1e-05,"shadow_triangles":202331160,"size":[2560,1440],"submitted_vertices_or_indices":606993480,"t":0.0,"threshold_px":1.0,"triangles_drawn":202331160,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":861672472,"candidates_tested":371502,"capacity_per_instance":10485,"command":"time","cpu_ms":0.723,"culling":true,"draws":18,"encode_ms":0.722958,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"frame_completion_ms":9.748042,"gpu_main_ms":3.934208,"gpu_ms":6.788916,"gpu_select_ms":0.620416,"gpu_shadow_ms":2.238334,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-grid-400-gpu.png","overflow":0,"padding_triangles":6223287,"readback_bytes":14745664,"selected_clusters":155455,"selection_ms":4.1e-05,"selector":"gpu","shadow_candidates_tested":245520,"shadow_clusters":141700,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":6024880,"shadow_selection_ms":0.0,"shadow_triangles":12112720,"size":[2560,1440],"submitted_vertices_or_indices":59694720,"t":0.0,"threshold_px":1.0,"triangles_drawn":13674953,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":861672472,"candidates_tested":112537200,"capacity_per_instance":10485,"command":"time","cpu_ms":0.548833,"culling":true,"draws":18,"encode_ms":0.548833,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"frame_completion_ms":27.720667000000002,"gpu_main_ms":3.8945839999999996,"gpu_ms":25.136875,"gpu_select_ms":18.986,"gpu_shadow_ms":2.255333,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-grid-400-brute.png","overflow":0,"padding_triangles":6223287,"readback_bytes":14745664,"selected_clusters":155455,"selection_ms":0.0,"selector":"brute","shadow_candidates_tested":112537200,"shadow_clusters":141700,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":6024880,"shadow_selection_ms":0.0,"shadow_triangles":12112720,"size":[2560,1440],"submitted_vertices_or_indices":59694720,"t":0.0,"threshold_px":1.0,"triangles_drawn":13674953,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":779549104,"candidates_tested":112537200,"capacity_per_instance":null,"command":"time","cpu_ms":995.470999,"culling":true,"draws":18,"encode_ms":0.972333,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"frame_completion_ms":19.490667,"gpu_main_ms":6.329708,"gpu_ms":15.645415999999999,"gpu_select_ms":0.0,"gpu_shadow_ms":9.326832999999999,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-grid-400-cpu.png","overflow":0,"padding_triangles":6223287,"readback_bytes":14745664,"selected_clusters":155455,"selection_ms":501.655834,"selector":"cpu","shadow_candidates_tested":112537200,"shadow_clusters":141700,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":6024880,"shadow_selection_ms":494.404125,"shadow_triangles":12112720,"size":[2560,1440],"submitted_vertices_or_indices":59694720,"t":0.0,"threshold_px":1.0,"triangles_drawn":13674953,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534128084,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.424792,"culling":true,"draws":2,"encode_ms":0.424709,"eye":[51.163719177246094,-95.86027526855469,52.075775146484375],"frame_completion_ms":686.124417,"gpu_main_ms":349.522209,"gpu_ms":679.557,"gpu_select_ms":0.0,"gpu_shadow_ms":330.21212499999996,"ground_draws":1,"layout":"grid:400","measured_frames":7,"mode":"naive","out":"<out>/L2b/washington-grid-400-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":2,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":0.0,"shadow_triangles":6744372000,"size":[2560,1440],"submitted_vertices_or_indices":20233116000,"t":0.0,"threshold_px":1.0,"triangles_drawn":6744372000,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":864021272,"candidates_tested":2496205,"capacity_per_instance":838,"command":"time","cpu_ms":0.613958,"culling":true,"draws":18,"encode_ms":0.613958,"eye":[216.61434936523438,-407.53985595703125,217.83889770507812],"frame_completion_ms":67.651167,"gpu_main_ms":34.271083,"gpu_ms":64.42187399999999,"gpu_select_ms":6.9479999999999995,"gpu_shadow_ms":23.094708,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-field-5000-1-gpu.png","overflow":0,"padding_triangles":66715131,"readback_bytes":14745664,"selected_clusters":1496826,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":1949105,"shadow_clusters":1489948,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":66535775,"shadow_selection_ms":0.0,"shadow_triangles":124177569,"size":[2560,1440],"submitted_vertices_or_indices":574781184,"t":0.0,"threshold_px":1.0,"triangles_drawn":124878597,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":864021272,"candidates_tested":1406715000,"capacity_per_instance":838,"command":"time","cpu_ms":0.602625,"culling":true,"draws":18,"encode_ms":0.6025419999999999,"eye":[216.61434936523438,-407.53985595703125,217.83889770507812],"frame_completion_ms":260.3085,"gpu_main_ms":34.221042,"gpu_ms":255.820626,"gpu_select_ms":198.44020799999998,"gpu_shadow_ms":23.042208,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-field-5000-1-brute.png","overflow":0,"padding_triangles":66715131,"readback_bytes":14745664,"selected_clusters":1496826,"selection_ms":4.1e-05,"selector":"brute","shadow_candidates_tested":1406715000,"shadow_clusters":1489948,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":66535775,"shadow_selection_ms":0.0,"shadow_triangles":124177569,"size":[2560,1440],"submitted_vertices_or_indices":574781184,"t":0.0,"threshold_px":1.0,"triangles_drawn":124878597,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":809203632,"candidates_tested":1406715000,"capacity_per_instance":null,"command":"time","cpu_ms":18963.177751000003,"culling":true,"draws":18,"encode_ms":6.494375,"eye":[216.61434936523438,-407.53985595703125,217.83889770507812],"frame_completion_ms":100.92212500000001,"gpu_main_ms":34.168334,"gpu_ms":66.797583,"gpu_select_ms":0.0,"gpu_shadow_ms":32.6825,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"cluster","out":"<out>/L2b/washington-field-5000-1-cpu.png","overflow":0,"padding_triangles":66715131,"readback_bytes":14745664,"selected_clusters":1496826,"selection_ms":9360.104041999999,"selector":"cpu","shadow_candidates_tested":1406715000,"shadow_clusters":1489948,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":66535775,"shadow_selection_ms":9596.606375000001,"shadow_triangles":124177569,"size":[2560,1440],"submitted_vertices_or_indices":574781184,"t":0.0,"threshold_px":1.0,"triangles_drawn":124878597,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"washington","bytes_resident_gpu":534422484,"candidates_tested":0,"capacity_per_instance":null,"command":"time","cpu_ms":0.9111670000000001,"culling":true,"draws":2,"encode_ms":0.911,"eye":[216.61434936523438,-407.53985595703125,217.83889770507812],"frame_completion_ms":9199.697708,"gpu_main_ms":4794.7898749999995,"gpu_ms":9066.564792,"gpu_select_ms":0.0,"gpu_shadow_ms":4259.548667,"ground_draws":1,"layout":"field:5000,1","measured_frames":7,"mode":"naive","out":"<out>/L2b/washington-field-5000-1-naive.png","overflow":0,"padding_triangles":0,"readback_bytes":14745664,"selected_clusters":0,"selection_ms":4.2e-05,"selector":"gpu","shadow_candidates_tested":0,"shadow_clusters":0,"shadow_draws":2,"shadow_overflow":0,"shadow_padding":0,"shadow_selection_ms":4.2e-05,"shadow_triangles":84304650000,"size":[2560,1440],"submitted_vertices_or_indices":252913950000,"t":0.0,"threshold_px":1.0,"triangles_drawn":84304650000,"valid_timestamp_samples":{"gpu_main_ms":7,"gpu_ms":7,"gpu_select_ms":7,"gpu_shadow_ms":7},"view":"lit","warmup_frames":1}
{"asset":"gaul","bytes_resident_gpu":289317876,"candidates_tested":64884,"capacity_per_instance":65415,"command":"render","cpu_ms":2.675959,"culling":true,"draws":4,"encode_ms":2.675875,"eye":[-0.4810411334037781,-0.33051377534866333,0.9211194515228271],"frame_completion_ms":24.703166,"gpu_main_ms":null,"gpu_ms":null,"gpu_select_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2b/gaul-close-lit.png","overflow":0,"padding_triangles":6195,"readback_bytes":14745600,"selected_clusters":2159,"selection_ms":4.2e-05,"selector":"gpu","shadow_candidates_tested":1618,"shadow_clusters":751,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":9427,"shadow_selection_ms":4.2e-05,"shadow_triangles":86701,"size":[2560,1440],"submitted_vertices_or_indices":829056,"t":1.0,"threshold_px":1.0,"triangles_drawn":270157,"valid_timestamp_samples":{"gpu_main_ms":0,"gpu_ms":0,"gpu_select_ms":0,"gpu_shadow_ms":0},"view":"lit","warmup_frames":0}
{"asset":"gaul","bytes_resident_gpu":289317876,"candidates_tested":64884,"capacity_per_instance":65415,"command":"render","cpu_ms":2.375958,"culling":true,"draws":4,"encode_ms":2.375917,"eye":[-0.4810411334037781,-0.33051377534866333,0.9211194515228271],"frame_completion_ms":26.030542,"gpu_main_ms":null,"gpu_ms":null,"gpu_select_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2b/gaul-close-clusters.png","overflow":0,"padding_triangles":6195,"readback_bytes":14745600,"selected_clusters":2159,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":1618,"shadow_clusters":751,"shadow_draws":4,"shadow_overflow":0,"shadow_padding":9427,"shadow_selection_ms":4.1e-05,"shadow_triangles":86701,"size":[2560,1440],"submitted_vertices_or_indices":829056,"t":1.0,"threshold_px":1.0,"triangles_drawn":270157,"valid_timestamp_samples":{"gpu_main_ms":0,"gpu_ms":0,"gpu_select_ms":0,"gpu_shadow_ms":0},"view":"clusters","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":783208596,"candidates_tested":279955,"capacity_per_instance":281343,"command":"render","cpu_ms":2.213334,"culling":true,"draws":18,"encode_ms":2.213334,"eye":[-0.2631993591785431,-0.17961221933364868,1.773664951324463],"frame_completion_ms":40.062167,"gpu_main_ms":null,"gpu_ms":null,"gpu_select_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2b/washington-close-lit.png","overflow":0,"padding_triangles":1807,"readback_bytes":14745600,"selected_clusters":404,"selection_ms":0.0,"selector":"gpu","shadow_candidates_tested":23885,"shadow_clusters":3143,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":62843,"shadow_selection_ms":0.0,"shadow_triangles":339461,"size":[2560,1440],"submitted_vertices_or_indices":155136,"t":1.0,"threshold_px":1.0,"triangles_drawn":49905,"valid_timestamp_samples":{"gpu_main_ms":0,"gpu_ms":0,"gpu_select_ms":0,"gpu_shadow_ms":0},"view":"lit","warmup_frames":0}
{"asset":"washington","bytes_resident_gpu":783208596,"candidates_tested":279955,"capacity_per_instance":281343,"command":"render","cpu_ms":2.9559580000000003,"culling":true,"draws":18,"encode_ms":2.9558750000000003,"eye":[-0.2631993591785431,-0.17961221933364868,1.773664951324463],"frame_completion_ms":39.6735,"gpu_main_ms":null,"gpu_ms":null,"gpu_select_ms":null,"gpu_shadow_ms":null,"ground_draws":1,"layout":"single","measured_frames":1,"mode":"cluster","out":"<out>/L2b/washington-close-clusters.png","overflow":0,"padding_triangles":1807,"readback_bytes":14745600,"selected_clusters":404,"selection_ms":4.2e-05,"selector":"gpu","shadow_candidates_tested":23885,"shadow_clusters":3143,"shadow_draws":18,"shadow_overflow":0,"shadow_padding":62843,"shadow_selection_ms":4.1e-05,"shadow_triangles":339461,"size":[2560,1440],"submitted_vertices_or_indices":155136,"t":1.0,"threshold_px":1.0,"triangles_drawn":49905,"valid_timestamp_samples":{"gpu_main_ms":0,"gpu_ms":0,"gpu_select_ms":0,"gpu_shadow_ms":0},"view":"clusters","warmup_frames":0}
```

</details>
