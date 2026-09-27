# Review: LLP 1009 The GPU canvas (codex)

- **Family:** codex (OpenAI)
- **Provider/runtime:** `codex exec -s read-only`, model `gpt-5.6-sol`, `model_reasoning_effort=ultra` (to be verified in the run log), workdir = review capsule (an export of the exact2 working tree, 284 files, structurally excluding `llp/reviews/1009*` and every orchestrator note; full read access within it)
- **Method:** llp-super-refine round 1, mutually blind to the same-round grok review (structural).
- **Brief sha256:** d151a197c4e04e75faf07e8b047b7cfa25fcc3808939e565e598c8c1ff4ac318 · **Prompt sha256:** 4f8abcefd357fa6605b6fd69977953f059be3aa5cd446c5974bf5220fc018705 · **Capsule sha256:** c409d2cb38d671569c1ecf6bea5665f941173629ed4235bfb09851e55b9aceec
- **Target (Review, round 1) sha256:** c234d8788bdb15db26ed963fd5a58079aa9c8f101e5e7103e94514415eeb37b5
- **Date:** 2026-08-29
- **Verdict:** (round 1 in flight — recorded on receipt)

---

## Round 1 — verbatim review

## Overall assessment

The leaf-canvas direction is plausible, but the RFC is not ready. It moves forbidden scope without the required trade, and its surface binding, host integration, portability, shader, and extensibility decisions do not yet compose with the landed runtime or the binding rules.

## Findings

1. **MATERIAL —** The Summary says this RFC “decides how [GPU support] is built,” and §4 sequences implementation, but `rules/DEFERRED.md` explicitly excludes both the GPU/WebGPU substrate and the `canvas` component. The RFC records neither the required explanation of what this unblocks nor what leaves the doing-list in exchange. Resolve by recording the binding move-off trade before acceptance, or do not build/accept this scope.

2. **MATERIAL —** D1 says “where wgpu and a browser disagree, the browser is right,” but no browser/toolchain, required feature set, limits, formats, color space, or unsupported-capability policy is defined. Different conforming WebGPU implementations can legally differ, so a single browser readback cannot adjudicate every disagreement. Resolve by defining a portable WebGPU profile and fail-closed capability policy, making the specification/profile authoritative, and restricting parity readback to deterministic rendering into a canonical offscreen format with normalized origin, alpha, and color space.

3. **MATERIAL —** D2’s decided trait is only `render(&mut self, frame, gpu) -> Present`, yet the same decision says a surface is selected by name and receives plan inputs. The trait has no input delivery, registry/factory, per-canvas lifecycle, or failure path for unknown names and bad arguments. Repeated or keyed canvases therefore cannot obtain distinct mutable surface instances coherently. Resolve by deciding the provider/factory seam, per-view identity and destruction, typed argument updates, and fail-closed errors.

4. **MATERIAL —** D3 says the `surface=` binding reuses “the `resource` machinery,” but landed resources settle synchronously and globally to shape-checked `Value`s, and `contract::bake` writes every resource’s boot value into the plan. A surface is mutable code, returns no `Value`, has no shape or baked initial value, must not access a GPU during baking, and may occur within an instantiated `each`. Resolve by specifying exactly what is reused—likely only VM argument evaluation—making surface bindings node-site/per-instance, excluding them from resource settlement and baking, and defining their initial inputs and boot visual.

5. **MATERIAL —** D3 says Canvas is “sized entirely by its style rows,” but a browser `<canvas>` has intrinsic 300×150 sizing while the kernel gives an unmeasured leaf no intrinsic content size. With an `auto` axis, browser and native layout can immediately diverge, contrary to both the web oracle and “GPU content never influences layout.” Resolve by deciding auto/intrinsic/aspect-ratio behavior, drawable content-box mapping, and logical-to-device-pixel sizing, then explicitly reset the web element or declare the necessary deviation.

6. **MATERIAL —** D4 promises visibility-aware scheduling and `exact_readback(view)`, but the landed ownership boundary cannot implement that promise. Swift owns `NSView`/`CAMetalLayer`, clipping, scrolling, and the display link; JavaScript owns DOM canvases and rAF; the Rust hosts currently receive neither target objects nor visibility state. Apple target handoff also requires a new lifetime-sensitive, audited FFI/unsafe boundary that the current ABI and `#![deny(unsafe_code)]` host do not admit. Resolve by deciding target registration and lifetime, visibility ownership and semantics, the combined `motion || canvas-demand` frame signal, and asynchronous readback completion. Raw canvas parity readback must remain distinct from the composited agent screenshot.

7. **MATERIAL —** D4 initializes the device on “the first canvas it presents,” while the Summary and §5 call that “off the boot path” and “never at boot.” An initially visible canvas makes first presentation part of boot unless a blank or placeholder first frame is deliberately chosen; browser adapter/device acquisition is asynchronous as well. Resolve by choosing and specifying post-first-pixel asynchronous initialization plus repaint-on-ready, or include and measure initialization in canvas apps’ cold-start budget. Also state whether the device survives plan reloads, since the current web reload drops its `Host`.

8. **MATERIAL —** D5’s claims that the pipeline set is enumerable and that “runtime shader compilation exists on no native host” do not follow from scanning WGSL modules. Naga emits MSL source, not a `.metallib`; Apple’s offline tools and wgpu’s unsafe passthrough path are additionally required. SPIR-V remains input that a Vulkan driver may compile during runtime pipeline creation, stock wgpu can create internal validation pipelines, and a raw `wgpu::Device` permits arbitrary uncatalogued shaders and pipelines. D5 also explicitly lets the browser compile WGSL despite §1 claiming the unqualified DEFERRED rule is kept verbatim. Resolve by defining and enforcing a finite catalogue of complete pipeline variants and backend artifacts, selecting the exact wgpu flags/passthrough boundary, and either defining an authoritative exception for driver/browser compilation or changing the binding rule through its required process.

9. **MATERIAL —** Open questions 1 and 4 are not independent implementation details. D2 says app code is written against wgpu and OQ1 leans toward raw wgpu types, but OQ4 claims hand-written `navigator.gpu` glue can merely become another `Gpu` implementation without changing app Rust; §5 similarly calls replacing wgpu a host-only change. Concrete wgpu resource types make those claims false. Moreover, wgpu 30’s wasm WGSL path passes WGSL directly to the browser and does not inherently activate optional naga, so §5’s assertion that most of the measured 200 KiB is unavoidable naga is factually unsupported. Resolve the app-facing API boundary now, then remeasure a minimal-feature wgpu build against an actually equivalent wrapper.

10. **MATERIAL —** Section 1 says all five binding GPU constraints are kept, but D6 says extensible animatable rows “come later.” The landed representation is concretely closed: a static schema table, a four-property transition vocabulary, a four-variant motion `Property`, and a fixed four-target kernel seam. “Nothing in v1 forecloses it” is therefore not an integration plan. Resolve by deciding and landing the minimum dynamic-property identity, type, storage, transition, wire, and Surface-input flow with Canvas, or explicitly trade away the binding requirement.

11. **MATERIAL —** Section 4 implements Apple before the web even though D1 declares the browser the oracle and repository rules make web the dev loop with native swept afterward. It also places build-time shaders after the Apple and web demos, although those demos necessarily require shaders. Resolve by landing the web executor and browser fixture first, putting D5’s compliant shader path before the first GPU pipeline, and then sweeping Apple.

12. **MATERIAL —** The header, §4, and ratification note institute a super-refine loop, while `rules/DEFERRED.md` explicitly forbids refine loops and READY verdicts. Unlike LLP 1004, this RFC records no required one-off exception or trade. Resolve by recording an authorized exception through the binding process or removing the loop from the document.

13. **MINOR —** **FLAG:** LLPs 0115, 0512, 0490, 0422, and 0406 are absent from this export, so their characterizations cannot be verified. This is especially relevant to the claims that every Linux canvas is “a plane” and that this canvas device is necessarily the future DRM host’s device; LLP 1000 establishes only that Linux is future work, and hardware planes are not an arbitrary per-node resource. Mark these as hypotheses and leave Linux composition to its lane. The present LLP 0559 also records app WGSL outside its adopted substrate amendment, so it should not be cited as authority for D5’s app-shader policy.

14. **MINOR —** Section 5’s native measurements exercise runtime WGSL→MSL compilation, not D5’s proposed precompiled artifact path, and the scratch configuration is absent. They are prototype upper bounds, not measurements of the selected design. Re-measure that path and make “only canvas-declaring apps pay” contingent on explicit Cargo-feature propagation plus a fail-closed plan/feature mismatch; a Contract tag cannot activate a Cargo feature itself.

## Verdict

NOT READY
---

## Round 2 (delta round) — record

- **Target (Review, r2) sha256:** 5485aeb472cbec80c0942f3bb1ad277dad5a6eeb0af5d7be95218058d39781cf · delta r1→r2 enumerated in the round's instructions (sha256 6e032dbe5b52b63b3e5cbab44e3de355f02e4782c115f9567b752a44559f69df)
- **Prompt sha256:** 124ded698e8b7aecd90f539a0f6ddab6f5ee436fba8980ff09e7bd567afedf5f · **Capsule sha256:** 690bb8ed332728f86f8185286988ef888e5994d2bdc32f5c6ee9313912d41cef
- **Method:** delta round — full document as context, the r1→r2 diff, the delta enumeration; every finding classified IN-DELTA or PRE-EXISTING; same runtime and blindness as round 1.
- **Date:** 2026-08-29
- **Verdict:** (round 2 in flight — recorded on receipt)

## Round 2 — verbatim review

## Overall assessment

r2 materially improves the profile, plan-row, startup, sequencing, and extensibility decisions. It is still not implementable as written: the lazy lifecycle, web sizing and motion paths, frame scheduler, native shader strategy, and binding DEFERRED trades contain unresolved MATERIAL contradictions. The absent exact1 documents were not treated as authority.

## Findings

1. **MATERIAL —** **IN-DELTA.** D2 lines 138–153 says surfaces are created with their canvas nodes and unknown names are a “boot-time typed refusal,” but the provider roster exists only in a module loaded after first pixel (D2 lines 145–150; D4 lines 196–202). The landed runner realizes the tree during boot, before that module can be inspected. Resolve this by embedding and boot-validating generated roster plus module-ABI/profile identity, while treating actual surfaces as pending until module load; alternatively, define mismatch as a post-paint surface failure rather than a boot refusal.

2. **MATERIAL —** **IN-DELTA.** D3 lines 160–165 says the web host sizes the canvas backing store from “the kernel’s frame.” LLP 1007 establishes that the browser performs web layout and the web host never calls `Kernel::compute_layout`; those frames therefore are not the web box geometry. Resolve by deriving backing dimensions from the browser’s content box and DPR—covering resize and DPR changes—and passing that host-owned size in `Frame`.

3. **MATERIAL —** **IN-DELTA.** D3 lines 172–180 calls `bind` as soon as arguments change, while D4 lines 183–202 schedules frames only for `Present::another_frame` or “unbound inputs.” Once an idle surface binds successfully, neither condition remains, so its changed content need not render. Conversely, an offscreen surface requesting another frame, or an unbound surface awaiting the asynchronous device, can keep the aggregate signal active indefinitely. Resolve D4 around a persistent dirty/unrendered latch, gated by visibility and readiness, with visibility/device-ready events scheduling a frame.

4. **MATERIAL —** **IN-DELTA.** D5 lines 207–229 catalogues shader modules while leaving pipelines runtime-created, but its generic Metal AOT path needs pipeline-specific information. wgpu’s Metal lowering consumes explicit resource mappings, vertex layouts, constants, topology, and device MSL version; passthrough rejects implicit layouts and warns that bindings are unreliable outside SPIR-V. The proposed MSL fallback has the same binding problem as metallib, so measuring one line-map bind group does not establish the declared WebGPU-shaped API. Resolve this with enumerable per-pipeline artifacts and explicit layouts/reflection, or a proven fixed binding ABI before Acceptance.

5. **MATERIAL —** **IN-DELTA.** §1 lines 41–49 and D5 lines 214–229 redefine the binding “compiles no shaders at runtime” rule as compilation not performed “by our code.” The MSL fallback expressly invokes Metal source compilation at runtime, Linux drivers compile SPIR-V at pipeline creation, and pipeline-state creation remains runtime compiler work. LLP 0559 F8, while research, confirms that the cited rationale was enumerable, precompilable pipelines and no runtime compilation. Consequently §5’s claim that D5 removes the measured cold path is conditional, not decided. Resolve with an actually AOT/prewarmed native path or an authoritative amendment/trade in the binding rule rather than an RFC-local reinterpretation.

6. **MATERIAL —** **IN-DELTA.** D6 lines 231–245 says `exact_motion::Property::Custom` produces presentation values that reach `Surface::bind` each frame. On the web, however, LLP 1002 D2 and LLP 1007 require CSS to execute motion with no per-frame `exact-motion` evaluator; a browser-animated custom property does not automatically enter the separate GPU wasm. Resolve the executor-level fork: specify CSS registration plus per-canvas sampling/wakeup into `bind`, or explicitly authorize a different web executor. Property registration scope, inheritance, and name-to-`Custom(u16)` identity also need CSS-compatible decisions.

7. **MATERIAL —** **IN-DELTA.** §7 lines 338–351 says only `canvas` and refine loops move from DEFERRED, but this RFC also moves the Runtime “GPU / WebGPU substrate” item itself. The refine-loop bullet names no unblock or doing-list removal, and the proposed `Svg` exchange is already described by LLP 1001 §9 as outside v1. Resolve §7 by accounting for every moved no-list item and recording genuine doing-list scope removed in the same PR, as the binding rule requires.

8. **MINOR —** **IN-DELTA.** D1 lines 84–102 calls raw `rgba8unorm` readback “sRGB-encoded.” WebGPU distinguishes linear `rgba8unorm` from `rgba8unorm-srgb`; alpha association is likewise not established by the storage format. Specify an sRGB view or explicit conversion and a defined straight-alpha conversion before comparison.

9. **MINOR —** **IN-DELTA.** D2 lines 141–149 claims exactly two audited `unsafe` boundaries, but D5’s `Device::create_shader_module_passthrough` is itself unsafe and bypasses wgpu validation. Name and audit that boundary, including artifact/reflection/layout invariants.

10. **MINOR —** **PRE-EXISTING.** D2 lines 153–156 rejects a data-only render list partly because “the plan cannot express a shader.” D5 itself demonstrates that a plan or render list can reference a catalogued shader by name without carrying source. The second-scene-format objection may stand, but the alternative should be rejected on its actual complexity and limitations rather than this impossibility claim.

11. **MINOR —** **PRE-EXISTING; FLAG.** §1 fact 4 and §3 characterize LLP 0406’s future Linux host, but LLP 0406 is absent from this export and could not be verified. r2 correctly makes the claim non-load-bearing; mark it explicitly as memory/unverified, as was done for the other absent exact1 documents.

## Verdict

NOT READY
---

## Close-out (orchestrator, 2026-08-29)

**Final verdicts bind to r2** (sha256 5485aeb472cbec80c0942f3bb1ad277dad5a6eeb0af5d7be95218058d39781cf): round 2 NOT READY — 7 MATERIAL, all IN-DELTA; 2 MINOR PRE-EXISTING; no MATERIAL concern in pre-existing text (the delta-round convergence condition met). **r3** (sha256 fe4f6e24485512a5773d25e95cfb88453cf3bff26a3c740811d9edd54350a6c0) folds the round-2 findings and is **unreviewed**; the loop closed at the two-round budget the author set. Launches: round 1 needed three (the first refused a non-git capsule; the second ended in a 'collab: Wait' state with no message; neither counted). Status stays Review; the loop never sets Accepted.

### Disposition ledger — round 1 (r1 → r2)
1. MATERIAL (DEFERRED trades unrecorded) — RECORDED-AS-OBLIGATION at §7.
2. MATERIAL (no profile/limits/formats/capability policy) — FOLDED at D1 (profile table, fail-closed, canonical readback).
3. MATERIAL (trait has no inputs/factory/lifecycle/failure path) — FOLDED at D2 (`bind`, `SurfaceProvider`, per-node instances, typed refusal — refined in r3 to a load-time reported failure).
4. MATERIAL (resource machinery misuse; baking; `each`) — FOLDED at D3.
5. MATERIAL (`<canvas>` intrinsic size vs unmeasured leaf) — FOLDED at D3 (declared deviation; drawable size — host-owned in r3).
6. MATERIAL (visibility/target/frame signal/readback ownership; unsafe boundary) — FOLDED at D4 and D2 (module holds the audited `unsafe`).
7. MATERIAL (first canvas on the boot path; async device; reload) — FOLDED at D4.
8. MATERIAL (pipeline set not enumerable; runtime compilation remains) — FOLDED at D5 in r2; round 2 held it still insufficient; r3 catalogues pipelines with explicit layouts and puts the rule's wording to Charlie (§7) rather than reinterpreting it.
9. MATERIAL (Q1/Q4 not independent; "mostly naga" unsupported) — FOLDED at D2 and §5: re-measured and re-attributed (169 KiB code; 212 KiB wasm-bindgen describe exports); the handle is exact-owned.
10. MATERIAL (D6 deferred is not integration) — FOLDED at D6 (properties table, `Property::Custom`, seam; step 1).
11. MATERIAL (Apple before web; D5 after demos) — FOLDED at §4.
12. MATERIAL (refine loop without recorded exception) — RECORDED at Status and §7 (as LLP 1004).
13. MINOR/FLAG (exact1 docs absent; Linux plane claim; 0559 as authority) — FOLDED at §1 and §3 (marked from memory; non-load-bearing; 0559 cited as research only).
14. MINOR (measurements are prototype upper bounds; feature propagation) — FOLDED at §5 and D2 (module, not feature; re-measure in step 1).

### Disposition ledger — round 2 (r2 → r3, unreviewed)
1. MATERIAL IN-DELTA (roster after first pixel vs boot refusal) — FOLDED at D2 (embedded roster digest in plan and module; pending surfaces; reported load failure, never a boot refusal).
2. MATERIAL IN-DELTA (web backing store from a kernel frame the web never computes) — FOLDED at D3 (drawable size host-owned: content box + DPR via ResizeObserver; carried in `Frame`).
3. MATERIAL IN-DELTA (scheduler: idle-after-bind never renders; offscreen/awaiting keeps signal on) — FOLDED at D4 (per-surface dirty latch gated by visibility and readiness; events set latches).
4. MATERIAL IN-DELTA (Metal AOT needs per-pipeline information; bindings unreliable) — FOLDED at D5 (pipelines catalogued with explicit layouts, `auto` forbidden; passthrough named as audited unsafe; step 1 measures) — the "proven fixed binding ABI before Acceptance" part remains an obligation of step 1, stated in D5.
5. MATERIAL IN-DELTA (rule reinterpreted rather than amended) — FOLDED: D5 no longer reinterprets; §7 puts the amendment to Charlie; D5 is the stricter reading until then.
6. MATERIAL IN-DELTA (custom properties on the web; registration/identity) — FOLDED at D6 (declared exception: `exact-motion` executes custom properties on every host; plan-scoped identity by table index; nothing registered with CSS).
7. MATERIAL IN-DELTA (§Runtime line unaccounted; refine-loop bullet lacks unblock; `Svg` frees nothing) — FOLDED at §7 (all four moved lines listed; unblock lines; `Svg` withdrawn; doing-list candidates for Charlie).
8. MINOR IN-DELTA (rgba8unorm vs -srgb; alpha) — FOLDED at D1.
9. MINOR IN-DELTA (passthrough is a third unsafe) — FOLDED at D5.
10. MINOR PRE-EXISTING (render-list rejection reason) — FOLDED at D2 (rejected on computation and second-format grounds, not impossibility).
11. MINOR PRE-EXISTING/FLAG (0406 unverified) — FOLDED at §1 fact 4.

**r4 (2026-08-29, unreviewed):** Charlie asked for a simpler document; r4 (sha256 e2673b0414c4c44645259afacaac8287de40ed23f2c97042dde7fd6762b17d14) cuts r3 to five decisions — wgpu as the one API on every host, an on-demand per-app module, the canvas leaf and its plan row, host-owned frame/visibility/target with the device after first pixel, shaders validated at build and compiled at first use — and drops the exact-owned handle, the profile table, the shader/pipeline catalogue, and declared properties. Findings above that concerned the dropped machinery are moot in r4; those about the seam, boot path, sizing, and the DEFERRED trades remain folded.

---

## Round 3 (extension round on r4, a fresh full review) — record

- **Target (Review, r4) sha256:** 27c6c48cd92f30cd86701718e4eb916781449d9c17dd32a6230a18992b207bab · **Brief sha256:** d7e2ed61f64ad4cea7abe7713e1ac56f6dee1d16b68dd4769ede57001c02ed85
- **Prompt sha256:** 4f8abcefd357fa6605b6fd69977953f059be3aa5cd446c5974bf5220fc018705 · **Capsule sha256:** 09decbfc592ba293a9d14efcf3cfb8eba0d9207aaa6220511c092929e0fa56bf
- **Method:** authorized by Charlie in-session ("one round … to see if there are any major issues"); same runtime and blindness as rounds 1–2.
- **Date:** 2026-08-29
- **Verdict:** (round 3 in flight — recorded on receipt)

## Round 3 — verbatim review

## Overall assessment

The core direction—wgpu, an on-demand app module, per-node surfaces, and host-owned scheduling—is feasible. However, r4 has unresolved binding-rule, error-boundary, and readback feasibility defects that prevent implementation from being safely transcribed.

## Findings

1. **MATERIAL —** §1 D2 defines `bind(&mut self, inputs: &[Value])` without a surface signature or error result, while registration supplies only a name. The compiler therefore cannot check surface-specific arity or types, and the module cannot report invalid inputs through the chosen trait. The RFC also decides missing-name behavior but not module-load, adapter/device-creation, or device-loss behavior; device loss invalidates every surface’s device-owned resources. Resolve this by defining input validation and reported failure semantics, classifying module/device failures, and stating that wgpu objects remain module-internal while the C/wasm ABI carries only encoded values and fixed-width handles.

2. **MATERIAL —** D2 says the runner evaluates surface arguments “after each commit” and puts a `surface` op “on the batch,” but the landed runner’s atomic batch is the kernel’s closed op list, while presenter batches are host-owned and created only after successful kernel apply. Evaluating afterward can trap after the kernel has changed; treating `surface` as a kernel op contradicts LLP 1001. Resolve by deciding that candidate surface deltas are evaluated per realized node before apply, published through a separate runner side-output only after successful apply, exposed on boot, and discarded on refusal.

3. **MATERIAL —** §1 D3 chooses “no intrinsic size” and merely promises that the `<canvas>` 300×150 deviation will be recorded later. The binding web-standard rule requires the CSS behavior unless an unavoidable deviation is declared with a reason. A fixed 300×150 fallback does not make GPU content influence layout, so the RFC has not justified rejecting it. Follow the web default or give and record a concrete unavoidable reason in the governing exact2 decision.

4. **MATERIAL —** **FLAG:** D1/D4 require readback from “the canvas’s own texture.” Locally available wgpu 30.0.1 advertises only `RENDER_ATTACHMENT` for its WebGPU surface, not the `COPY_SRC` usage normally required for readback (`wgpu-30.0.1/src/backend/webgpu.rs:4172-4179`). Chrome may accept an unadvertised configuration, but that could not be verified offline and is not a supported portable wgpu path. Resolve this with a pinned Chrome/wgpu proof or use a module-owned copyable texture for fixture rendering/readback.

5. **MATERIAL —** D5 promises that “Every WGSL module” is build-validated, but D1 gives surfaces a raw `wgpu::Device`, whose safe `create_shader_module` accepts inline or dynamically generated WGSL. An app `build.rs` cannot inventory those strings, so an invalid shader need not fail the build as claimed. Restrict the admitted shader sources to an enforceable build-declared set, or narrow D5 and explicitly acknowledge runtime-only validation paths.

6. **MATERIAL —** §1 “Not decided here” defers extensible animatable properties and claims the seam remains open because “a property is more state.” Committed target state is not presentation state: LLP 1002 deliberately separates them, the native motion vocabulary is closed to four properties, and the web delegates interpolation to CSS. Nothing currently delivers an app property’s display-rate value to `Surface`. Resolve by deciding the minimal presentation-value extension point now, or explicitly amend and trade the binding extensibility clause; detailed authoring syntax may remain later.

7. **MATERIAL —** §5 records obligations, not completed trades. GPU/canvas offers candidates rather than selecting an offset; deferring iOS merely reorders doing-list work, and screenshot removal is not chosen. D5 still conditionally violates the binding runtime-compilation sentence, while the refine-loop bullet names neither what the loop unblocks nor an offset. Before acceptance, select the concrete removal or removals, record the corresponding DEFERRED amendments in the same PR, and let the RFC wait if the shader amendment is refused.

8. **MINOR —** The post-first-pixel design is sound in principle, but the current enforcement path is unresolved. `scripts/boot.mjs` rejects every literal dynamic import in initial glue regardless of control flow, and the existing single `requestAnimationFrame` stamp runs before that frame is painted. Step 2 should specify an auditable post-paint loader boundary and the associated boot-check treatment. The Summary should also replace “boots exactly as fast” with the narrower claim that no GPU fetch, initialization, device creation, or shader work enters the pre-pixel path.

9. **MINOR —** **FLAG:** §3’s GPU figures are not reproducible from this export: the current lockfile contains no GPU dependencies, and no scratch source, command, profile, machine, or measurement artifact is present. The 829 KiB web figure is honestly provisional before wasm-bindgen processing, but the ~100-crate dependency’s effect on the repository’s blocking-loop budgets is also unmeasured. Record the measurement method and gate-time delta; remove the unsupported qualitative claim that 1.7 MiB is “nothing.”

## Verdict

NOT READY
### Round 3 close-out (orchestrator, 2026-08-29)
Round 3 bound to r4 (27c6c48cd92f30cd86701718e4eb916781449d9c17dd32a6230a18992b207bab): NOT READY (7 MATERIAL, 2 MINOR), with the review's own assessment that "the core direction … is feasible". r5 (7d32b27bdc4219c4c2b9419223ccc99ae6dca858c39854dbc5871473f69b6094) folds it, unreviewed.
1. MATERIAL (no error result, no arity, module/device failures, device loss) — FOLDED at D2 (`bind -> Result`; name + arity registration with a compiler check; failures reported per canvas; device-loss rule; wgpu objects never cross the ABI).
2. MATERIAL (surface evaluation vs the kernel's atomic batch) — FOLDED at D2 (evaluated per realized node before apply, as a binding; published as a runner side-output after a successful apply and at boot; discarded on refusal; never a kernel op).
3. MATERIAL (300×150 deviation unjustified) — FOLDED at D3 (tag defaults `width=300 height=150`; no deviation).
4. MATERIAL/FLAG (surface textures are not COPY_SRC) — FOLDED at D4 (fixtures render into a module-owned copyable texture).
5. MATERIAL (build validation unenforceable on a raw device) — FOLDED at D5 (narrowed to the build-declared `shaders/` set; runtime strings acknowledged).
6. MATERIAL (extensible properties not delivered by "more state") — FOLDED: the minimal extension point decided (`Property::Custom`, a plan table, presentation values as trailing `bind` inputs on every host); syntax and web sampling later.
7. MATERIAL (§5 obligations, not trades) — FOLDED at §5 (one concrete proposed take; the refine-loop unblock named; the shader amendment remains Charlie's, and the RFC waits if refused — stated).
8. MINOR (boot check rejects literal dynamic imports; paint stamp timing) — FOLDED at D4 (injected script element after the paint-stamp frame; the Summary's "boots exactly as fast" kept, with the exact claim stated in D4).
9. MINOR/FLAG (measurement provenance; gate-time delta) — FOLDED at §3; the scratch crate's manifest and commands: `wgpu = { version = "30", default-features = false, features = ["wgsl", "webgpu", "metal"] }`, `opt-level = "z"`, fat LTO, `cargo build --release --target wasm32-unknown-unknown` then `wasm-opt -Oz`; native `cargo build --release` (cdylib) and a C `main` timing `probe_init`; macOS 26.6, Apple Silicon.
