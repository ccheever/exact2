# Cluster LOD — offline bake and CPU reference renderer

Standalone experiment for LLP 1041.011 O1 / §5 Q2. L1 builds the file and numerical
oracles; GPU rendering, timing and the interactive camera demonstration belong to L2.
The vendored meshoptimizer v1.2 and `demo/clusterlod.h` are unchanged.

**Status:** L1 verified on Linux and this Mac; CPU reference renderer in progress.
The native executable startup fault cleared without changes to L1 code.

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

CLI output is one JSON object; failures produce an `error` object and exit 1.
All meshes, baked outputs, exported cuts and logs belong in
`~/Library/Caches/exact2-cluster-lod/`, never in Git.

## Format v1

Little endian, magic `CLOD0001`, version 1. `clod-format/src/lib.rs` contains the
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
    proof. No measured error claim is made until the oracle actually runs.
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

L2a measurements will be recorded below after running the image oracles.
