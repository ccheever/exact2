# Review: LLP 1009 The GPU canvas (grok)

- **Family:** grok (xAI)
- **Provider/runtime:** `grok -p` (headless single-turn), model `grok-4.6`, `--reasoning-effort xhigh`, `--disable-web-search`; document-embedded prompt, no repository access, terminal ban
- **Method:** llp-super-refine round 1, mutually blind to the same-round codex review (structural).
- **Brief sha256:** d151a197c4e04e75faf07e8b047b7cfa25fcc3808939e565e598c8c1ff4ac318 · **Prompt sha256:** b5de204b0286a368d33cdbf1c62715ec847404d6c85c34ef926a7ba7107b01ce
- **Target (Review, round 1) sha256:** c234d8788bdb15db26ed963fd5a58079aa9c8f101e5e7103e94514415eeb37b5
- **Date:** 2026-08-29
- **Verdict:** (round 1 in flight — recorded on receipt)

---

## Round 1 — verbatim review

The full review brief was truncated in the message, so I'll read the offloaded prompt first and then write the review from that text only.## Overall assessment

The RFC is at the right altitude: six named decisions, explicit rejections of exact1’s extra layers, and a real cost measurement. The NOT-DOING GPU door is mostly honored as a *shape* (leaf box, content does not layout, host owns the frame, `@property` left unblocked, shaders meant to be built ahead of time). What fails is coherence among the decisions themselves and with the landed Apple/runner contracts they claim to extend. Several “decisions” either contradict an open question, reuse a seam whose boot/settlement semantics cannot apply, or sequence the work so the stated oracle and the shader rule cannot hold. Those are RFC-level defects, not spec transcription.

## Findings

1. **MATERIAL —** Summary (“GPU code is Rust in the app’s data crate, **against wgpu**”), D2 (`gpu: &Gpu`), §6 question 1 (leaning “the raw wgpu device”), and §6 question 4 (“whether the web host uses wgpu’s WebGPU backend from wasm or a hand glue over `navigator.gpu` … **not decided here**”) cannot be true together. If `Gpu` is a wgpu type (Q1’s lean, the Summary’s “against wgpu”), Q4’s hand glue is not “which object implements `Gpu` in wasm”; it is a second app-facing API or a wgpu backend. If the point of D1/D2 is one Rust `Surface` on every host, `Gpu` has to be an Exact-owned, WebGPU-shaped handle that wgpu and `navigator.gpu` both implement — which is the option Q1 defers. Leaving Q1 open while D2 and Q4 assume opposite answers is a decision the RFC fails to make at its own altitude. **Resolve:** decide Q1 here. Either (a) `Gpu` is Exact-owned and Q4 stays a host-only measurement, or (b) `Gpu` is wgpu and Q4 is closed (wgpu-in-wasm, 200 KiB gzip). Drop “against wgpu” from the Summary unless (b).

2. **MATERIAL —** D2: “its **inputs are state** — the values the plan passes, as a `resource`’s arguments are.” The trait it publishes is `fn render(&mut self, frame: &Frame, gpu: &Gpu) -> Present` — no arguments, no bind method, no name. D3 then says the compiler lowers `surface=` with “**the `resource` machinery, reused**.” LLP 1004 D4’s resource machinery is `DataSource::query(name, args) -> Value`, constant resources **baked into the plan’s data segment** so the first frame needs no seam, settlement synchronous. A canvas has no such value: there is no boot pixel, and `contract-lower` cannot honestly `query` a GPU surface at build. Reusing that machinery without saying what is *not* reused leaves the boot path and the runner’s settlement model unspecified at the seam this RFC exists to name. **Resolve:** reuse only the plan-row shape (name + argument expressions) and the invalidation rule (re-render when those expressions change). State that a surface is **not** a `DataSource`, is **not** baked, and has **no boot value**; first pixels come from the first host-driven `render` after lazy device init. Put `args` (plan values) on `render` or on an explicit bind, the way `query` takes `args`.

3. **MATERIAL —** D4: “Visible canvases are rendered from the host’s **existing frame source** — the display link on Apple (LLP 1008 §5), `requestAnimationFrame` on the web … only while a surface’s `Present` asks for another frame or its inputs changed.” LLP 1008 §1 (the text provided; **FLAG** that §5 was not in the brief) enables the display link **only when the motion engine is not quiescent**, and the 250 ms clock only for runner timers. A map that returns `Present` with no `transition` row has no wakeup under that contract. D4 therefore adds a third reason to run the frame source and does not say so. The same gap hits startup: D4 creates the device on “the **first canvas it presents**, never at boot,” citing LLP 1008 §6 (~11 ms “ours” in a ~200 ms platform floor; device “must not join the boot path”). §4’s first demo is the Caltrain line map — likely on the first screen. “Presents” on that path **is** first frame. Cold device+pipeline is measured at 115 ms; even the 11–13 ms warm figure doubles “ours.” RULES.md’s 100 ms p50 cold start is the constraint they invoked. **FLAG** on LLP 1007’s rAF policy (not in the brief). **Resolve:** (i) the Apple (and web) frame source also runs when any surface returns “want another frame” or canvas inputs have settled — a third batch flag beside `timers` and `motion`; (ii) first *pixel* is not blocked on device init: the kernel box can paint empty, device+first `render` are a reported post-boot phase, even if a canvas is on screen.

4. **MATERIAL —** D4 puts instance/device creation on **the host**. §5 says “**Only an app that declares a canvas links either (a cargo feature on the app crate)**; the Caltrain app today does not, and its wasm does not change until it draws.” If the host owns the device, wgpu lives in `exact-apple` / the web wasm host, and every consumer of that host pays unless the *host* is feature-gated. An app-crate feature cannot implement D4 as written. **Resolve:** name the feature gate on the host (and whatever re-exports `Gpu` to the data crate), fail-closed if a plan names a surface and the host was built without GPU, and restated the “Caltrain wasm does not change” claim as a host-feature fact, not an app-crate fact.

5. **MATERIAL — FLAG —** D5: “on Apple, to produce a **Metal library** the surface loads instead of compiling WGSL at runtime … on Linux, **SPIR-V** … **Runtime shader compilation exists on no native host.**” Those artifacts are not the same, and the wgpu 30 API that would load them is not cited. A `.metallib` can skip Metal’s source compiler; SPIR-V is still compiled by the Vulkan driver at pipeline creation; wgpu’s usual path is naga → MSL → `newLibraryWithSource` (the 115 ms §5 attributes to “runtime WGSL → MSL”). I cannot verify that wgpu 30 will load a precompiled Metal library without WGSL/naga at runtime; **FLAG**. If it will not, D5 as written needs a vendored wgpu patch or a side load that D1 said app code would not have. Enumerable *shader modules* in `build.rs` also are not an enumerable *pipeline* set (LLP 0559 F8 is **FLAG** — research text not provided; the RFC’s gloss may be right). **Resolve:** state the native bar in platform terms — no WGSL/naga at runtime; Apple loads a cited wgpu-30 (or patched) metallib path; Vulkan consumes SPIR-V and **permits** driver/PSO compile; pipeline objects may be created on first present. If wgpu cannot take the Apple artifact, say so and pick the artifact it can take. Do not claim “compilation exists on no native host” unless that sentence is narrowed.

6. **MATERIAL —** D1: “the **browser is the oracle**” and “where wgpu and a browser disagree, the browser is right.” RULES.md: “**Web is the standard and the dev loop; native is swept.**” LLP 1000: web, then Apple, then Linux. §4 does the opposite: Apple wgpu first (step 2, including the Caltrain map), Chrome fixtures in step 3, **“Build-time shaders (D5) before the first shader ships”** as step 4. The map in step 2 *is* a shipping shader. Native is implemented before the oracle exists, and D5 is scheduled after the first shader-using demo. **Resolve:** either web GPU + Chrome readback before Apple (matches D1 and the web-first rule), or an explicit, named exception for why Metal is first. Move D5 before any app shader, including the map. Name **Chrome** as the oracle in D1 (as step 3 already does); “the browser” includes Firefox, whose WebGPU stack *is* wgpu.

7. **MATERIAL —** D3 introduces Contract tag `canvas`. `rules/NOT-DOING.md` §Components: “No `video`, `webview`, **`canvas`**, ….” §Runtime invites an owned-pixel leaf when GPU is built, and moving something off the list requires “**one line naming what it unblocks, and take something off the doing-list in the same PR.**” The RFC never records that trade or disambiguates the two uses of “canvas.” **Resolve:** in this RFC, state that landing D3 moves `canvas` off §Components, name what it unblocks (Caltrain line map / the GPU door), and name what comes off the doing-list — or state that §Runtime’s leaf is not the forbidden tag and pick a different tag.

8. **MINOR —** D1: “reached from the app’s wasm through **wgpu’s WebGPU backend**.” §6.4 leaves that path undecided. The parenthetical is not part of the representation/executor decision; strike it from D1.

9. **MINOR —** D1’s “one representation, two executors” is only partly the motion/layout shape. Motion fixtures pin closed-form numbers to CSS; WebGPU adapters differ in limits, formats, and blending. A **readback** with per-fixture tolerance (§6.2) is the right instrument; “the browser is right” is not, for MSAA/blending/precision. Tighten D1 to Chrome readback within a declared band, and to a portable WebGPU core (no wgpu-only extensions). That also stops Q1-raw from leaking a second representation.

10. **MINOR —** D3: “sized entirely by its style rows **like an `Image`**; … **never measured**, and its content never influences layout.” NOT-DOING says **like `NativeView`**. **FLAG** LLP 1001 Image intrinsic size (spec not in the brief). If `Image` is measured from pixels, the simile contradicts the layout clause. Use NativeView’s box rule: style rows only; missing size does not come from GPU content.

11. **MINOR —** D4 “a canvas that is **offscreen** or unchanged costs nothing.” LLP 1008’s window is an `NSScrollView` the kernel does not own (“scroll always wins”). “Visible” is not a kernel frame test. Name the host as authority (intersection with the platform viewport, occlusion, `display`/`visibility`). Inferable from “host owns the frame,” but the cost sentence is operational.

12. **MINOR —** Summary / facts §3 / §3: “the device this RFC builds is **the one the Linux host will use**” (LLP 0406, **FLAG** — research, not provided). A DRM swapchain for a GPU presenter is not an app `Canvas` / `Surface`. Linux can own pixels without this leaf; this leaf can ship without Linux. It is not a reason to sequence Apple before the oracle. The real why-now is integration of the NOT-DOING door and the line map.

13. **MINOR —** D2 `Frame` includes “the clock,” and “never because a timer fired.” If a `Surface` samples wall time or GPU timestamps for animation, the agent `clock` operation cannot read a deterministic canvas (RULES: flake is worse than no check). Require that presentable time is `Frame`’s clock.

14. **MINOR — FLAG —** Characterizations of LLP 0115, 0512, 0490, 0422, 0559 F8, and 0406 were not checkable from the brief (research, never authority). They do not block, but they also cannot be used as proof that D5’s pipeline rule or the Linux justification is what those documents said.

## Verdict
NOT READY

---

## Round 2 (delta round) — record

- **Target (Review, r2) sha256:** 5485aeb472cbec80c0942f3bb1ad277dad5a6eeb0af5d7be95218058d39781cf · delta r1→r2 enumerated in the round's instructions (sha256 6e032dbe5b52b63b3e5cbab44e3de355f02e4782c115f9567b752a44559f69df)
- **Prompt sha256:** 0d12763b9891e2b29585329e3b55219a9308e919909c03bfb3162a1b66dbf575
- **Method:** delta round — full document as context, the r1→r2 diff, the delta enumeration; every finding classified IN-DELTA or PRE-EXISTING; same runtime and blindness as round 1.
- **Date:** 2026-08-29
- **Verdict:** (round 2 in flight — recorded on receipt)

## Round 2 — verbatim review

The review brief requires the full document. I’ll read the offloaded prompt so the review is based on the complete RFC, not the truncated message.The RFC is truncated at the costs section. I’ll read the rest of the offloaded prompt so D5–D7 and the r1→r2 diff are in the review.## Overall assessment

r2 turns LLP 1009 into a real decision document: a declared WebGPU profile with fail-closed adapters, an exact-owned `Gpu` handle with two executors, a non-resource `surfaces` row, host-owned frame/visibility/target, a narrowed shader rule with a named driver-compile exception, D6 landed as `@property`-shaped rows, web-first sequencing, and honest re-attribution of the wasm number. Those fixes meet the round-1 questions in the brief. Two architecture contradictions in the delta are still unresolved: D2 describes an in-process `DataSource`-shaped seam in the app data crate while also placing GPU code in a separately loaded module the runner cannot call at boot, and D6 feeds surfaces from motion presentation values in a way that does not work with LLP 1002’s CSS-delegated web executor or D4’s `canvas` signal.

## Findings

1. **MATERIAL —** IN-DELTA. D2 says “The app’s rendering code is Rust in its data crate” and that the app implements `Surface` “reached through a seam the runner defines beside `DataSource`”; D3 says “the runner evaluates the arguments against state after each commit and calls `bind` when they changed.” The same decision then says “`<app>-gpu` is a second artifact per app … a `dylib` … loaded with `dlopen` on the first canvas; a second wasm on the web, fetched on the first canvas,” and D4 says that load happens *after* the first pixel. Those cannot be the same seam. LLP 1004 already links the data crate into every host; the web runner and the Apple static library cannot `call bind` on objects that live in another wasm/dylib. Unknown names as “a **boot-time typed refusal**, the kernel-schema digest’s rule” is the same collision: a schema digest is checked at plan decode, before first pixel, without `dlopen`; the module roster does not exist until after first pixel. Resolve it in D2/D3: the app GPU crate is not the host-linked data crate; the runner evaluates `surface=` arguments (and a plan-side name catalogue) and emits them on the batch; `Surface` / `SurfaceProvider` live behind the module C ABI the presenter calls; name mismatches are a compile- or decode-time catalogue check, not a module-load check.

2. **MATERIAL —** IN-DELTA. D6: “their presentation values reach a surface as inputs through `bind` each frame the engine changes them.” D3 only calls `bind` when `surface=` arguments change after a commit. D4’s `canvas` signal is “true while any surface returned ‘another frame’ or has unbound inputs,” not while a custom property is in motion. LLP 1002 D2: on the web the host “emits the rows as CSS and does nothing per frame.” There is no web-side engine producing per-frame presentation values in Rust, CSS `@property` cannot express D6’s v1 `vec2`, and a surface that returned idle will not be `render`ed while `glow` transitions. Native can cheat if `present` ops are forwarded into `gpu_bind`; the web executor cannot. Resolve it as a decision, not a spec footnote: the bind input vector (argument values vs `Custom` presentation values, and in what order); that a `present` of `Property::Custom` on a canvas node is a bind+render of that surface; and how the web executor obtains those values — either a declared exception that surface-feeding custom properties run through `exact-motion` on all hosts, or that the web host samples them under rAF with the `canvas` signal forced true for the transition.

3. **MINOR —** IN-DELTA. D1’s canonical readback format is “`rgba8unorm`, straight alpha, sRGB-encoded, top-left origin,” while the profile lists `rgba8unorm` / `bgra8unorm` and not `rgba8unorm-srgb`. In WebGPU those are different formats. Pin whether fixtures compare storage-unorm bytes or sRGB display-referred bytes (and whether `rgba8unorm-srgb` is in the profile).

4. **MINOR —** IN-DELTA. D4: “the presenter registers the platform target with the module when the canvas node is created.” Canvas nodes are created at boot; the module is loaded after first pixel. Queue registration until `gpu_load`, then register live canvases and later creates.

5. **MINOR —** IN-DELTA. §5 still quotes **Native: 1.7 MiB (the Metal backend plus naga, stripped)** for the per-app module. D5 removes runtime WGSL→MSL; naga should not be in the loaded dylib if that decision holds. Remeasure the module without the runtime WGSL path, or say the 1.7 MiB figure is the pre-D5 scratch crate.

6. **MINOR —** IN-DELTA. §5: the web executor is “on the order of the motion glue.” Motion glue emits CSS; a `navigator.gpu` ABI is buffers, textures, encoders, async device, and error scopes. The “only canvas apps pay” claim still holds; drop the motion-glue size analogy and keep the step-2 measurement.

7. **MINOR —** IN-DELTA. FLAG: §7 proposes taking **the `Svg` node type (`schema.json` id 7)** off the doing-list, citing LLP 1001 §9 (SVG rasterization not in v1) and a 2D surface “when one exists.” §3 defers that 2D layer, and 1001 §9 / `schema.json` are not in this packet. If SVG is already not in v1, the trade does not free v1 capacity; Charlie should confirm the id and take something that is actually on the v1 doing-list, or state that the unused node type is deleted from the schema.

8. **MINOR —** IN-DELTA. FLAG: D5 cites wgpu 30 `Device::create_shader_module_passthrough` (`Features::PASSTHROUGH_SHADERS`, `metallib`) and the SPIR-V bind-group caveat. That API is not in this packet. The MSL-passthrough fallback still makes a native path feasible; step 1 should treat a missing or weaker passthrough API as taking the fallback, not as a third architecture.

## Verdict
NOT READY

---

## Close-out (orchestrator, 2026-08-29)

**Final verdicts bind to r2** (sha256 5485aeb472cbec80c0942f3bb1ad277dad5a6eeb0af5d7be95218058d39781cf): round 2 NOT READY — 2 MATERIAL, both IN-DELTA; no MATERIAL concern in pre-existing text (the delta-round convergence condition met). **r3** (sha256 fe4f6e24485512a5773d25e95cfb88453cf3bff26a3c740811d9edd54350a6c0) folds the round-2 findings and is **unreviewed**; the loop closed at the two-round budget the author set. Status stays Review; the loop never sets Accepted.

### Disposition ledger — round 1 (r1 → r2)
1. MATERIAL (Gpu handle vs Q1/Q4) — FOLDED at D2: the handle is exact-owned and WebGPU-shaped, two implementations; Q1 decided, Q4 closed by measurement (§5).
2. MATERIAL (resource machinery, boot value) — FOLDED at D3: a `surfaces` row, not a resource; not baked; no boot value; only argument evaluation and invalidation reused; `bind(inputs)` on the trait.
3. MATERIAL (frame source wakeup; first-screen device) — FOLDED at D4: the `canvas` signal; device after first pixel with repaint-on-ready and a reported phase.
4. MATERIAL (host feature gate) — FOLDED at D2: a separately loaded per-app module; apps without a canvas load nothing.
5. MATERIAL/FLAG (metallib path) — FOLDED at D5 with wgpu 30's `create_shader_module_passthrough` (`PASSTHROUGH_SHADERS`, `metallib`) cited from the crate source and its binding caveat; the fallback measured in step 1; the rule's wording put to Charlie in §7 (r3).
6. MATERIAL (web first; D5 before shaders; Chrome named) — FOLDED at §4 and D1.
7. MATERIAL (NOT-DOING §Components `canvas`) — RECORDED-AS-OBLIGATION at §7 (r2; the `Svg` swap withdrawn in r3 per round 2).
8. MINOR (wgpu parenthetical in D1) — FOLDED at D1.
9. MINOR (readback bands; portable core) — FOLDED at D1 (profile table, per-fixture bands).
10. MINOR (Image simile) — FOLDED at D3 ("exactly as `NativeView`").
11. MINOR (visibility is the host's) — FOLDED at D4.
12. MINOR (Linux justification) — FOLDED at §1 fact 4 and §3 (demoted, unverified).
13. MINOR (presentable time is Frame's clock) — FOLDED at D2's trait doc.
14. MINOR/FLAG (exact1 characterizations) — FOLDED at §1 (marked from memory, non-load-bearing).

### Disposition ledger — round 2 (r2 → r3, unreviewed)
1. MATERIAL IN-DELTA (seam across the module boundary; boot-time refusal impossible) — FOLDED at D2 (runner half emits a `surface` op; presenter half calls the module; pending surfaces; roster digest; reported load failure).
2. MATERIAL IN-DELTA (custom properties cannot reach a surface through CSS on the web) — FOLDED at D6 (declared exception to LLP 1002 D2: `exact-motion` executes custom properties on every host; bind vector order; identity by table index).
3. MINOR IN-DELTA (rgba8unorm vs -srgb) — FOLDED at D1 (linear storage bytes compared; sRGB views never in fixtures).
4. MINOR IN-DELTA (registration before load) — FOLDED at D4 (queued until `gpu_load`).
5. MINOR IN-DELTA (1.7 MiB includes naga) — FOLDED at §5 (labeled the scratch figure; module measured in step 1).
6. MINOR IN-DELTA (motion-glue size analogy) — FOLDED at §5 (dropped).
7. MINOR IN-DELTA/FLAG (`Svg` swap frees no v1 capacity) — FOLDED at §7 (withdrawn; candidates put to Charlie).
8. MINOR IN-DELTA/FLAG (passthrough API not in packet) — the citation stands from the crate source (`wgpu-types-30.0.1/src/shader.rs`, `CreateShaderModuleDescriptorPassthrough { metallib, msl, spirv, … }`; `Features::PASSTHROUGH_SHADERS`); the fallback rule as the finding asks — FOLDED at D5.

**r4 (2026-08-29, unreviewed):** Charlie asked for a simpler document; r4 (sha256 e2673b0414c4c44645259afacaac8287de40ed23f2c97042dde7fd6762b17d14) cuts r3 to five decisions — wgpu as the one API on every host, an on-demand per-app module, the canvas leaf and its plan row, host-owned frame/visibility/target with the device after first pixel, shaders validated at build and compiled at first use — and drops the exact-owned handle, the profile table, the shader/pipeline catalogue, and declared properties. Findings above that concerned the dropped machinery are moot in r4; those about the seam, boot path, sizing, and the NOT-DOING trades remain folded.

---

## Round 3 (extension round on r4, a fresh full review) — record

- **Target (Review, r4) sha256:** 27c6c48cd92f30cd86701718e4eb916781449d9c17dd32a6230a18992b207bab · **Brief sha256:** d7e2ed61f64ad4cea7abe7713e1ac56f6dee1d16b68dd4769ede57001c02ed85
- **Prompt sha256:** 18ac69b4934491568369beab8506b6329dd3b5dffadaf0646dec258abd5358a8
- **Method:** authorized by Charlie in-session ("one round … to see if there are any major issues"); same runtime and blindness as rounds 1–2.
- **Date:** 2026-08-29
- **Verdict:** (round 3 in flight — recorded on receipt)

## Round 3 — verbatim review

The review brief was truncated in this message. I’ll read the offloaded prompt so the review is against the full document, then respond with the review only.## Overall assessment

r4 is a real decision RFC at the right altitude: D1–D5 each name an alternative and a reason, and the host-owned leaf, the on-demand module, and “device after first pixel” line up with `NativeView`, the `DataSource` seam, LLP 1008’s batch/frame model, and the boot budget. D5’s conflict with NOT-DOING is stated rather than papered over; deferring app-declared properties does not close that door, because `bind` already takes state. Two decisions as written cannot be implemented without violating the same document: D1 puts rendering code in the host-linked data crate while D2 keeps wgpu out of the hosts, and §5 moves GPU/`canvas` onto the doing-list without a valid named take.

## Findings

1. **MATERIAL —** D1, “App rendering code is Rust in the app’s data crate (LLP 1004 D4), written against wgpu’s own types,” versus D2, “`<app>-gpu` is a second artifact: wgpu plus the app’s surfaces” and “The core hosts stay as they are … an app without a canvas carries nothing,” plus §3, “the Caltrain wasm today (172 KiB gzip) is untouched.” LLP 1004 D4’s data crate is the `DataSource` implementation “linked into every host.” If Surface impls and wgpu types live in that crate, wgpu lands in `exact-web` / `exact-apple` and in the 172 KiB wasm, and D2/§3 are false. If they live only in `<app>-gpu`, D1’s crate name is false. That is a crate-membership contradiction, not missing spec detail. Resolve in one sentence: Surface implementations and wgpu compile only into `<app>-gpu`; the host-linked `DataSource` crate does not depend on wgpu (a `gpu` feature or a split crate, if they share types).

2. **MATERIAL —** §5, “Candidates to come off the doing-list: iOS on the Apple host’s shape (deferred behind this lane), or the agent API’s screenshot operation (the hosts’ capture plus readback cover it).” The binding rule is “name what it unblocks, and take something off the doing-list in the same PR. If nothing can come off, the answer is no.” Unblocking the Caltrain line map is named; the take is not. Deferring iOS is a reorder, not a take: NOT-DOING’s v1 bar still requires “web, macOS, iOS, and Linux,” and LLP 1000 already has iOS after Apple. Dropping `screenshot` is not a take either: it is one of the eight agent operations, and “hosts’ capture plus readback cover it” aliases that operation (window capture is what `screenshot` is; readback is D4’s fixture path, “not the agent’s screenshot”). Resolve by naming one doing-list item that is actually removed in the same PR — not a v1-bar surface, not a synonym for remaining work.

3. **MINOR —** D1, “the module loads after first pixel, where the boot rule does not reach,” and D2, “on the web a second wasm with its generated glue, fetched when needed.” That is the right boot call: RULES forbids app JS *before* first pixel, and `boot` counts the graph reachable before first pixel. wasm-bindgen glue is still JS. If it is a static import from host glue, the `boot` graph grows even for the empty-box frame; if it is a dynamic fetch started after that frame, it does not. **FLAG:** `host/web` and `scripts/boot.mjs` are not in this packet, so reachability of the glue is unverified. Resolve by stating that the GPU wasm and its wasm-bindgen JS are a dynamic fetch, not a static member of the `boot` graph.

4. **MINOR —** D2, `fn render(...) -> bool` (“Returns whether another is wanted”), and D4, “new inputs, a wanted frame, a new size, or the device just became ready.” A `true` return is a vsync subscription. LLP 1002 D3 and the agent `clock` op make presentation a closed-form function of the seekable clock; a surface that redraws from wall time is not. That does not block v1 if the line map is a function of `bind` inputs (clock-driven state in, one render, `false`). Resolve by saying v1 surfaces are a function of `bind` inputs plus `Frame` size, and that `true` means “load/device/size not finished,” or by putting the app clock in `Frame` and requiring `render` to be a function of it.

5. **MINOR —** D4, “under a third batch signal, `canvas`, beside `timers` and `motion`.” In LLP 1008, `timers` and `motion` are trailers from the Rust host *before* the presenter applies. `gpu_render`’s bool is born in the module when the presenter applies. Treating `canvas` as the same kind of host-batch trailer has no path for that bit unless the presenter feeds it back. Resolve by saying the presenter keeps the existing frame source (rAF / display link) armed while any surface wants a frame, and not implying a host-side trailer computed before apply.

6. **MINOR —** D5, “rather than met with a precompilation pipeline whose feasibility with bind groups is unproven.” The decision to amend rather than meet “compiles no shaders at runtime,” and to keep compile off the boot path, is the right call and is honestly stated: on the web, WebGPU compiles at pipeline creation, and “web is the standard,” so a portable “no runtime compile” door cannot be met. The bind-group sentence is a weaker reason and looks like leftover machinery. **FLAG:** LLP 0559 F8 is not in this packet; the characterization is taken from NOT-DOING. Resolve by justifying the amendment with the WebGPU/boot constraint, and leaving precompilation as a later native measured trade.

7. **MINOR —** §3, “829 KiB raw before wasm-bindgen’s post-processing, of which 169 KiB is code; the processed size is measured in step 2,” and “11–13 ms warm, 115 ms cold” for “adapter + device + one pipeline from WGSL + one draw on Metal.” Those numbers are used soundly as a floor after first pixel, not as a boot cost, and they do not pretend the processed wasm is known. They are still a one-pipeline Metal microbenchmark. A map with several pipelines, plus fetch of processed wasm-bindgen output, is the web cost that lands on the first GPU frame. Do not imply 169 KiB or 115 ms is that frame.

## Verdict
NOT READY

### Round 3 close-out (orchestrator, 2026-08-29)
Round 3 bound to r4 (27c6c48cd92f30cd86701718e4eb916781449d9c17dd32a6230a18992b207bab): NOT READY (2 MATERIAL, 5 MINOR); "neither on architecture" is the orchestrator's reading — see the verbatim text above. r5 (7d32b27bdc4219c4c2b9419223ccc99ae6dca858c39854dbc5871473f69b6094) folds it, unreviewed.
1. MATERIAL (data crate vs module crate membership) — FOLDED at D1/D2 (the GPU crate, sibling of the data crate; wgpu never in the host-linked crate).
2. MATERIAL (the take must be named) — FOLDED at §5 (proposed take: the three gradient style rows; iOS/screenshot withdrawn). Charlie applies.
3. MINOR (dynamic fetch vs boot graph) — FOLDED at D4 (post-paint injected script element; smoke asserts).
4. MINOR (wall-time surfaces vs the seekable clock) — FOLDED at D2 (trait doc: the host's presentable clock).
5. MINOR (the `canvas` signal is the presenter's) — FOLDED at D4 wording ("the presenter keeps the frame source armed").
6. MINOR (D5 amendment honestly stated) — no change needed; noted.
7. MINOR (numbers as a floor, one pipeline) — FOLDED at §3 (method recorded, gate-time delta to measure).
