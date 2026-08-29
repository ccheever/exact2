# Review: LLP 1011 Image v1 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 7fc1a2fe2b2621214eac4022ef6840e6433bd9f2bdd6a44aed08880796276aad; target sha256 19572c0995448b6684bfc5928f9bc66c0f5fe13fcb6c57fff13e6c8a02094657.
- **Verdict (round 1):** NOT READY — binds to the r1 target hash above. r2 (sha256 90aee95ec50bb19c9fc151aa2aeca7592bd78ee1f4316d080bcd02c0879b4375) folds the findings and is unreviewed.

---

## Round 1 (received 2026-08-29T11:03Z, verbatim)

## Overall assessment

The core measured-leaf design, intrinsic-ratio layout, Contract lowering, asset resolution, and Apple ABI are substantially true to the code. The spec is not ready because it misstates rehydration and failed-image sizing, overclaims CSS `object-fit` parity and test coverage, and leaves accessibility behavior inconsistent across hosts.

## Findings

1. **MATERIAL —** §1 says intrinsic size “is not exported, and a rehydrated kernel does not know it” (`llp/1011-image-v1.spec.md:31`). Export omission is correct, but rehydration retains it: `NodeArena` is cloned with its `intrinsic` column (`kernel/src/arena.rs:22`, `kernel/src/arena.rs:37`), and `Kernel::rehydrate` clones the arena before rebuilding Taffy (`kernel/src/kernel.rs:319`). Resolve by changing “rehydrated” to “fresh/replayed after restart,” or clearing intrinsic state during rehydration and adding a test.

2. **MATERIAL —** §4 claims CSS `object-fit` painting (`llp/1011-image-v1.spec.md:102`), but Apple fits and clips against the entire `bounds`, which is the border box (`host/apple/macos/Sources/ExactMac/Presenter.swift:237`). It also paints borders first (`Presenter.swift:225`) and then can paint the image over them. CSS fits replaced content inside the content box, excluding padding and borders. Resolve by fitting/clipping to an inset content rectangle and testing padding and borders across all five fit values; LLP 1008 §5 repeats the same overclaim and should be corrected too.

3. **MATERIAL —** “A broken `<img>` is 0×0” and “a failed load is 0×0” (`llp/1011-image-v1.spec.md:17`, `llp/1011-image-v1.spec.md:130`) describe the intrinsic contribution, not the final box. Explicit dimensions and parent sizing still apply: the Apple test pins the unloaded Caltrain image at `96×0` (`host/apple/tests/host.rs:262`), while block flow can supply a nonzero width. The web additionally delegates broken-image presentation to the browser, which the harness does not characterize. Resolve by saying the missing intrinsic measurement is `0×0`, specifying how styled boxes remain sized, and declaring or testing the browser’s broken-image behavior. LLP 1001 §1 needs the same wording correction.

4. **MATERIAL —** The accessibility account is both incomplete and contradicted by the code. §2 calls `label` the web’s “`alt`/`aria-label`,” but the web emits only `aria-label`, never `alt` (`host/web/src/host.rs:328`). Conversely, §6 says accessibility beyond `label` is absent (`llp/1011-image-v1.spec.md:136`), although Contract admits `hint`, `role`, and `headingLevel` on images (`contract/lower/src/tags.rs:135`) and the web lowers them to ARIA attributes (`host/web/src/host.rs:331`); Apple applies only the label (`host/apple/macos/Sources/ExactMac/Presenter.swift:151`). Resolve by defining and testing a per-host image accessibility contract—normally `label → alt` on `<img>`—and explicitly declaring any remaining host asymmetry.

5. **MATERIAL —** §1 and §6 say neither host lowers `tint_color` (`llp/1011-image-v1.spec.md:58`, `llp/1011-image-v1.spec.md:128`). The web explicitly skips it (`host/web/src/css.rs:79`), but the Apple host serializes color rows, including `tint_color`, into the presenter dictionary (`host/apple/src/style.rs:31`); the presenter merely ignores it. Resolve by saying “web skips it; Apple transports but does not paint it,” or make Apple report it as skipped.

6. **MATERIAL —** §5 overstates what verification holds (`llp/1011-image-v1.spec.md:112`). The web unit test checks emitted attributes and CSS only (`host/web/tests/host.rs:248`), and the web smoke contains no image-load or image-geometry assertion (`host/web/smoke.mjs:64`). Apple smoke checks decoded size and frame, not painted pixels or `object-fit` (`host/apple/smoke.mjs:24`). Screenshots are optional and no script compares them; both smoke files also state they are not blocking checks (`host/web/smoke.mjs:2`, `host/apple/smoke.mjs:2`), so they are not “under the five checks.” The 177 Rust-test count is consistent with the source. **FLAG:** the dated claim that both screenshots showed the same picture in the same place cannot be verified because this export contains no screenshots. Resolve by splitting assertions, observations, and untested behavior as LLP 1010 §4 does, and either add a deterministic visual comparison or remove the parity claim. The Apple unit test should also identify the particular downstream frame that moved rather than merely count more than one frame op (`host/apple/tests/host.rs:279`).

7. **MINOR —** The unit terminology is internally confusing: §1 calls intrinsic dimensions points (`llp/1011-image-v1.spec.md:31`), while §4 calls them pixels, not points (`llp/1011-image-v1.spec.md:106`). The ABI documentation also says points (`host/apple/include/exact.h:71`), although the presenter passes `pixelsWide`/`pixelsHigh` (`host/apple/macos/Sources/ExactMac/Presenter.swift:99`). The intended behavior is discernible—raw pixel counts are treated one-for-one as layout units—but an iOS or Linux implementer could choose a platform image’s point size instead. Resolve by naming the value “raw pixel dimensions mapped 1:1 to CSS-pixel/point layout units” consistently.

## Verdict

NOT READY
---

## Disposition (orchestrator, 2026-08-29; r2)

1. MATERIAL (rehydration) — **FOLDED at §1**: rehydration keeps the column, replay does not; pinned by `kernel/tests/image.rs`.
2. MATERIAL (`object-fit` box) — **FOLDED in code and at §4**; LLP 1008 §5 corrected.
3. MATERIAL (0×0 wording) — **FOLDED at §1/§6** (an unknown axis measures 0; a `width` row still sizes the box; the browser's broken-image presentation declared); LLP 1001 §1 corrected.
4. MATERIAL (accessibility) — **FOLDED**: `label` → `alt` on the web (code + §3); `hint`/`role`/`headingLevel` stated per host (§2, §3, §4) and the macOS gap declared (§6).
5. MATERIAL (`tint_color`) — **FOLDED at §1** ("web skips it; Apple transports, does not paint").
6. MATERIAL (§5 overstated) — **FOLDED at §5**: asserted vs observed split, the smokes named non-blocking, the screenshot claim stated as the author's observation; the Apple test now names the station-name frame. FLAG (screenshots not in the export): correct, and the spec no longer presents them as a check.
7. MINOR (units) — **FOLDED at §1** and in `exact.h`, `Kernel::set_intrinsic_size` docs: pixel counts one-for-one as layout units, never divided by the backing scale.
