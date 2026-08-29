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