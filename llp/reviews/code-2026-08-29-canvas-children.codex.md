# Code review: canvas children — the app inside the sky, per-child placements, the card deck (LLP 1014 §1a/§1b), 2026-08-29 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only --skip-git-repo-check --ephemeral`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export (the capsule), the brief `CODE-BRIEF-canvas.md` and the whole diff `CHANGE.diff` at its root
- **Method:** one-shot code review from the brief (sha256 948e5deecf8fbde98c1298ecc66e3d1650a437ce9d3d4ee2e347c3cffbac2dfe), mutually blind to the other family; capsule sha256 22c1f53478bd5fb73a35f547137e34d7ec7d3535b5340c795165aa6a29a96dcb (the working tree at `0b7a0c4` plus the staged change, `Cargo.lock`, `target/`, `.build/`, and `.git` excluded, plus both briefs and the generated `shaders.rs`). The code reviewed is `0b7a0c4` plus the staged change of 2026-08-29 (the diff's sha256 1392204fef038cf518bca38001a4e03df9f59cb8cb193e56bce1184d344638ac); this brief covers the canvas half of it.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T19:41:47Z, verbatim; started 2026-08-29T19:31:46Z)

## Overall assessment

The canvas architecture is coherent and its focused GPU/layout tests pass, but the change is not ready: the native ABI violates its stated hostile-caller boundary, AppKit can hit visually absent cards, agent clock advancement does not synchronously settle GPU state, and the required test suite remains red even with Linux packages excluded. Crossfade history and the governing documents also need reconciliation.

## Findings

1. **HIGH —** `gpu/src/native.rs:267`. The C exports construct Rust slices or references before validating caller-controlled pointers and lengths: `gpu_child`, `gpu_texture`, and `gpu_readback` immediately call `from_raw_parts(_mut)`, while `gpu_placement` dereferences an unaligned, nullable fixed-array pointer without receiving an output length. Caller-controlled dimensions can also overflow later byte-count multiplication, and an arbitrary child index can force an unbounded resize. This violates the brief’s requirement that the sole unsafe boundary remain sound against a misbehaving host. Validate nullness, alignment, exact lengths, checked byte counts, dimensions, and declared child indices before creating references; add an explicit placement output length and adversarial ABI tests.

2. **HIGH —** `host/apple/macos/Sources/ExactMac/Presenter.swift:287`. When inverse-mapped placement hit testing misses, `NodeView.hitTest` falls back to `super.hitTest(point)`. Because placed children remain transparent subviews in their original kernel frames, that fallback can hit a card whose placement moved it offscreen, violating “nothing hits a card that is not there.” Once placements exist, fallback must exclude every placed child and consider only genuinely unplaced children or the canvas itself; add a host-level test that taps a departed card’s original frame.

3. **HIGH —** `host/apple/macos/Sources/ExactMac/Agent.swift:155`. `clock` advances runner and motion state, calls `apply`, and replies without synchronously ticking or rendering canvases. Canvas rendering and placement reads occur later through `CADisplayLink`, so a completed clock command can expose stale placement state and the UI can continue changing between subsequent agent operations without another clock advance. Drive capture, render, and placement publication to a bounded fixed point at the landed timestamp before replying.

4. **MEDIUM —** `host/web/tests/agent.rs:45`. The web agent-state test still requires the old four-slot JSON prefix, but this change adds `material`, `deck`, and `focus`; `cargo test --workspace --exclude exact-linux --exclude caltrain-linux` consequently fails here. Update the assertion—preferably by parsing the JSON and checking the intended fields—so the mandatory workspace check passes.

5. **MEDIUM —** `gpu/src/lib.rs:411`. The `previous` texture does not consistently represent the previously displayed image. A size change allocates a new, never-populated previous texture, while a same-size update during an active fade copies the latest upload rather than the currently displayed blend; `GlassSurface` nevertheless treats either generation as valid history. Resizes can therefore fade from blank, and interrupted fades can jump before restarting. Preserve or resample the displayed source, or explicitly invalidate/rebase the fade, and test both resize and mid-fade generation changes.

6. **LOW —** `llp/1014.000-canvas-v1-implementation.spec.md:302`. Section 5 still lists transforms, per-child textures, and nested canvases as excluded from v1 even though Sections 1a/1b describe them as implemented. LLP 1014’s D5 discussion and the canvas entry in `QUEUE.md` retain the same obsolete boundary. Reconcile these documents with the shipped scope and remove or split the completed queue item.

## Verdict

NOT READY

---

## Disposition (orchestrator, 2026-08-29)

Verified against the code before folding; each fold names what holds it. The folds are transcribed in LLP 1014.000 §1c.

1. HIGH (the ABI builds slices before it checks) — **CONFIRMED, FOLDED.** Every host byte range goes through `native::bytes`/`bytes_mut` (a null pointer is a refusal by name, never a slice); `Module::child`/`texture` compute byte counts with `checked_mul`; a child index past the next one is refused ("out of order"), so a host's number never sizes an allocation; `gpu_placement(id, index, out, len)` takes `out`'s length, refuses null or short, and writes unaligned; an ABI refusal is reported once and then the module's error. Held by `gpu/tests/module.rs` (overflowing and zero counts refused by name; a null pointer refused and reported once; a real pointer is the slice). `Gpu.swift` passes the length. Not done: a hostile `Frame` size (wgpu's validation is the guard there).
2. HIGH (the fallback hits a placed child at its kernel frame) — **CONFIRMED, FOLDED.** When placements exist and every inverse misses, `hitTest` tests only the overlay's *unplaced* children, then answers the canvas itself — never `super`, whose order is the kernel column. Held by `scripts/smoke.mjs` step 11 on macOS: at clock 0 the deck is closed (its cards placed in the top half of the canvas) and a `tap` on the canvas's middle — the second card's kernel frame on the reviewed tree — focuses nothing; verified by driving the rebuilt app (focus `null`, then a card's id after a tap on the front card, then unchanged after the middle again).
3. HIGH (`clock` replies before a frame) — **CONFIRMED, FOLDED.** `Canvases.settle(now:)` — pending captures painted, every canvas rendered at the landed clock, placements and nested readbacks within the call, three rounds at most — runs after `tap`, `type`, and `clock` before the reply. Held by smoke step 11: the `layout` right after `tap deck-toggle` carries placed boxes (closed-deck poses, the hidden cards off the canvas) and `clock +2000` moves them. `clock settle` remains motion's fixed point (LLP 1012); a surface's springs run on the clock the agent advances — declared in §1c, with `tests/stack.rs` holding that `clock +60000` lands settled.
4. MEDIUM (`host/web/tests/agent.rs` reads a four-slot prefix) — **CONFIRMED, FOLDED.** The test asserts the `{"clock":0,"slots":{` prefix and each of the seven slots by name. (The workspace run on the reviewed tree also failed `host/linux/tests/host.rs` twice and `host/linux/tests/paint.rs` once against the same app change — folded under the Linux lane's review.)
5. MEDIUM (`previous` is not always the displayed picture) — **CONFIRMED for the resize, FOLDED; the mid-fade restart DECLARED.** `GlassSurface::children` marks a new texture `fresh`, and a fresh texture shows at once like the first upload. A change during a fade restarts from the latest picture — a small jump the `CONTINUOUS_MS` rule already hides for bursts — declared in §1c and §5; no hermetic test of the fade (declared, `QUEUE.md`).
6. LOW (the documents keep the old boundary) — **FOLDED.** 1014.000 §5 lists what is actually not in v1 and says D5 and nested canvases are; the RFC's "not decided" notes the nested build; `QUEUE.md`'s canvas line names what is left. (The finding's filename `1014.000-canvas-v1-implementation.spec.md` is not the file's name; the section is.)

Verdict NOT READY binds to the reviewed tree; the folds above are unreviewed by this family.
