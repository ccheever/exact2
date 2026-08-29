# Code review: shader reflection at build and the readback fixture, 2026-08-29 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export
- **Method:** one-shot code review from the brief (sha256 f7f63a040b9e1595101d87e714ba416e7e5fc438dbbbebf6b2b3011bdc59d454), mutually blind to the other family; capsule sha256 c09d9ce76368548570ccd2a1785cd0bee47f27b8ddb4b0734d563a2345db2c75. The code reviewed is the tree at `e0a69bb` (the change landed inside that commit); the brief is `CODE-BRIEF.md` in the capsule.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T13:32Z, verbatim)

## Overall assessment

The current shaders and generated API are correct for their exercised shapes: Naga-derived struct offsets, matrix/array strides, nested writes, runtime-array minimum sizes, transitive visibility, constant descriptors, and padded RGBA readback are sound. The separate Naga-only crate respects dependency direction and leaves boot untouched. However, reflection emits an incorrect layout for a valid class of sampled textures, defeating the build-time correctness guarantee.

## Findings

1. **HIGH —** `gpu/reflect/src/lib.rs:343-350` declares every non-multisampled `texture_*<f32>` filterable. Filterability depends on whether an entry point actually pairs the texture with a sampler; a valid shader using only `textureLoad` can bind an unfilterable format such as `Rgba32Float`, but this generated layout rejects it at first use. `gpu/reflect/tests/reflect.rs:59` merely asserts the over-broad policy. Derive filterability from each entry point’s Naga `sampling_set`, generate entry-point/pipeline-specific layouts where usage differs, and test both sampled and load-only float textures.

2. **MEDIUM —** `gpu/reflect/src/lib.rs:697-703` silently drops a vertex layout when two entry-point names normalize to the same generated type—for example, `foo_bar` and `foo__bar` both become `FooBarInput`. Related derived names are also unchecked: `gpu/reflect/src/lib.rs:193-199` can make compute entry `foo`’s `FOO_WORKGROUP_SIZE` collide with another entry named `foo_workgroup_size`; generated structs can collide with the emitted `entry` module or `wgpu` import. Replace the local collision checks with one registry covering every generated identifier and return a named build error rather than `continue`.

3. **MEDIUM —** `gpu/src/fixture.rs:99-125` promises to read “any four-byte format,” but `block_copy_size == 4` does not mean four RGBA8 channels. This admits BGRA, single-channel `R32*`, packed formats, and depth formats while `Pixels::ppm` treats the bytes as RGB. It also copies every array layer via `texture.size()` while allocating only `row * height`, with no `rows_per_image`. Either restrict `read` to a single-layer `Rgba8Unorm` texture or retain format/dimension metadata, size all layers correctly, and normalize supported formats to RGBA8.

4. **LOW —** `gpu/src/fixture.rs:25-32` does not validate coordinates. For example, `(width, 0)` can return `(0, 1)` rather than fail because the flattened index remains in range. Assert `x < width && y < height` before indexing and add an out-of-bounds test.

5. **MEDIUM —** `gpu/tests/fixture.rs:54-59` and `apps/caltrain/gpu/tests/map.rs:19-25` turn “no adapter” into a passing test, so a green CI run may execute no readback at all. These tests also establish only native broad pixel properties; they do not compare the same fixture with Chrome as LLP 1009 D1 requires. Keep local skipping if desired, but make an asynchronous GPU/parity lane require an adapter and compare canonical native and browser readbacks within declared bands.

6. **LOW —** `gpu/reflect/src/lib.rs:657-729` presents a tightly packed, single-buffer, per-vertex layout as reflection, although WGSL declares only locations and types—not buffer slots, offsets, strides, or `VertexStepMode`. This convention forecloses multiple buffers and instance-rate attributes. Document it explicitly as an Exact convention and separate reflected attributes from the chosen buffer policy so future policies can be added without replacing the reflection API.

## Verdict

NOT READY
---

## Disposition (orchestrator, 2026-08-29)

Verified against the code before folding; each fold is held by a test named here.

1. HIGH (every float texture filterable) — **CONFIRMED, FOLDED.** `binding_type` reads naga's per-entry-point `sampling_set` (transitive through calls, `valid/analyzer.rs:451`): a float texture is `filterable: true` exactly when some entry point samples it with a sampler, `false` when only `textureLoad` reads it or it is multisampled — the rule wgpu applies for `layout: None`. One layout per module (the union over entry points), not per entry point: a texture sampled in one stage and loaded in another is `filterable: true`, which the loading stage also accepts. Held by `gpu/reflect/tests/reflect.rs::a_float_texture_is_filterable_exactly_when_a_sampler_touches_it` (`tex` sampled → true; `lut` loaded → false) and the aurora's `CHILDREN` in `apps/caltrain/gpu/tests/shaders.rs`.
2. MEDIUM (collision gaps) — **FOLDED.** One `Names` registry per generated module, seeded with `SOURCE`, `MODULE`, `entry`, `wgpu`, claims every binding const, `GROUP_n`, buffer struct, and vertex-input struct; the `entry` module has its own for `<NAME>` and `<NAME>_WORKGROUP_SIZE`; a duplicate is a build error naming both sides. A second vertex entry point that would reuse a name is an error unless it takes the same WGSL struct, in which case one Rust struct serves both. Held by `colliding_generated_names_are_refused_by_name` (`foo_bar`/`foo__bar` → `FooBarInput`; `struct entry`; `source`; `a_b`/`aB`) and `two_vertex_entry_points_sharing_one_struct_get_one_rust_struct`.
3. MEDIUM (`read`'s contract) — **FOLDED.** `fixture::read` accepts one 2D layer (mip 0) of `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`, or `Bgra8UnormSrgb`, swizzles BGRA to RGBA, copies exactly `width × height × 1`, and refuses anything else by name; `Pixels` is documented RGBA. Held by `gpu/tests/fixture.rs::a_bgra_texture_comes_back_rgba` and `other_formats_and_array_textures_are_refused_by_name` (`R32Float`; two layers).
4. LOW (`Pixels::at` unchecked) — **FOLDED.** Asserts `x < width && y < height`, naming the coordinate and the picture. Held by `a_pixel_off_the_picture_panics`.
5. MEDIUM (a skip is a pass; no Chrome comparison) — **DECLARED, NOT FOLDED.** A required-adapter mode is a check, and `rules/RULES.md` §Agents says a human asks for one; proposed to Charlie as `EXACT_GPU_REQUIRED=1` making `fixture::device()` fail instead of skip, for a labelled GPU host. The native-versus-Chrome comparison within declared bands is LLP 1009 §4 open question 1, unchanged.
6. LOW (the vertex layout is a convention) — **FOLDED.** The generated struct carries `INPUTS: &[(u32, VertexFormat)]` — what the shader declares — beside `LAYOUT`, whose doc names it as this generator's convention (one per-vertex buffer, tightly interleaved in declaration order) and says another policy is another const beside it; the crate docs say the same. Several buffers and instance-rate attributes remain undone. Held by `entry_points_and_vertex_inputs_are_named_and_packed`.

Verdict NOT READY binds to the reviewed tree; the folds above are unreviewed by this family.
