# Cluster LOD — offline bake and CPU reference renderer

Standalone experiment for LLP 1041.011 O1 / §5 Q2. L1 builds the file and numerical
oracles; L2a adds Metal/WebGPU hardware rasterization and the CPU reference selector.
The vendored meshoptimizer v1.2 and `demo/clusterlod.h` are unchanged.

**Status:** L1 verified on Linux and this Mac; L2a complete and verified on Apple M5 Max / Metal; Wasm library builds.

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
asset="$HOME/Library/Caches/exact2-cluster-lod/out/washington-1.clod"
out="$HOME/Library/Caches/exact2-cluster-lod/out/demo"
target/debug/clod-view render "$asset" --out "$out/lit.png" --path hero --t 0.5
target/debug/clod-view render "$asset" --out "$out/clusters.png" --view clusters --t 0.5
target/debug/clod-view time "$asset" --out "$out/timing.png" --layout grid:400 --frames 7
target/debug/clod-view compare "$asset" --out "$out/compare" --threshold-px 0.5,1,2,4,8 --t 0,0.25,0.5,0.75,1
target/debug/clod-view pop "$asset" --out "$out/pop" --threshold-px 1 --steps 240
cargo test --workspace --no-fail-fast -- --nocapture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p clod-view --lib --target wasm32-unknown-unknown
```

Defaults: cluster mode, lit view, single layout, threshold 1 px, 2560×1440,
45° vertical field of view, hero t=0. `--mode naive` uses the indexed baseline.
Layouts: `single`, `ring:N`, `grid:N`, `field:N,seed`; N=1..10,000. Views:
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
    for this reference; raster backface culling handles backfaces.
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
(37.18791 B/source triangle), terminal cut 6,088 triangles. First/repeat wall
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

This is an offscreen CPU-reference lane. The browser library compiles but has
not been driven in a browser, and there is no interactive window, streaming,
occlusion culling or LOD morphing. GPU selection belongs to L2b. The declared
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
