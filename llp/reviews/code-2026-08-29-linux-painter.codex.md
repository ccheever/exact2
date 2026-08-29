# Code review: the Linux host r2 — one walk over a backend, vello on the GPU, tiny-skia on the CPU, the screen over VNC (LLP 1015 r2), 2026-08-29 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only --skip-git-repo-check --ephemeral`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export (the capsule), the brief `CODE-BRIEF-linux.md` and the whole diff `CHANGE.diff` at its root
- **Method:** one-shot code review from the brief (sha256 7157aa18cbc04cc125afa57f814a5460e443059993b9a4cd3d9a88890429d153), mutually blind to the other family; capsule sha256 22c1f53478bd5fb73a35f547137e34d7ec7d3535b5340c795165aa6a29a96dcb (the working tree at `0b7a0c4` plus the staged change, `Cargo.lock`, `target/`, `.build/`, and `.git` excluded, plus both briefs and the generated `shaders.rs`). The code reviewed is `0b7a0c4` plus the staged change of 2026-08-29 (the diff's sha256 1392204fef038cf518bca38001a4e03df9f59cb8cb193e56bce1184d344638ac); this brief covers the linux half of it.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T19:40:51Z, verbatim; started 2026-08-29T19:31:49Z)

## Overall assessment
The backend abstraction is coherent, but this change is not ready: the current Linux paint suite fails, the unauthenticated VNC server permits remote resource exhaustion, and GPU recovery and parity guarantees are incomplete.

## Findings
1. **HIGH —** `host/linux/tests/paint.rs:127`. `cargo test -p exact-linux --test paint` fails deterministically: `backgrounds_land_in_their_boxes_with_their_radii` expects `(247,247,247)` but receives white. The test uses the mutable Caltrain app as its fixture and is now stale despite LLP 1015 claiming it “never failed.” Move this assertion to a stable dedicated fixture or update it to the intended current composition.

2. **HIGH —** `host/linux/src/vnc.rs:320`. `ClientCutText` trusts a network-supplied `u32` length and allocates it directly. Because `EXACT_VNC=1` binds unauthenticated service to `0.0.0.0:5900`, a client can request roughly 4 GiB and exhaust or abort the display process. Reject oversized messages or drain ignored payloads through a bounded buffer.

3. **MEDIUM —** `host/linux/src/vnc.rs:228`. Every client gets a writer thread that may wait forever on either condition variable. When the reader sees disconnect, line 332 merely drops the `JoinHandle`; it neither signals nor terminates the writer. Repeated connections therefore leak threads and retained state. Add shared cancellation, notify both waits, shut down the socket, and join the writer.

4. **MEDIUM —** `host/linux/src/presenter.rs:367`. GPU render, poll, or readback failure produces a blank frame with no painted boxes and then sets `dirty = false`; CPU fallback only exists during `Gpu::new`. A device loss therefore blanks the application and disables hit-testing indefinitely. In auto mode, replace the failed backend with `Raster` and repaint; forced-GPU mode should surface the failure explicitly.

5. **MEDIUM —** `host/linux/src/gpu.rs:247`. The image cache uses the raw address of an `Rc<Pixmap>` as permanent identity. After an image or reload drops that `Rc`, allocator address reuse can make an unrelated new image retrieve the old `ImageBrush`, showing stale pixels. Store a `Weak<Pixmap>` and verify identity/liveness, or key the cache by a stable image generation.

6. **MEDIUM —** `host/linux/tests/paint.rs:29`. The promised CPU-oracle parity band is not tested. The suite independently applies broad assertions to each available painter, never compares CPU and GPU frames, and skips GPU entirely when no adapter exists; substantial glyph, caret, clip, image, or blending divergence can pass. Render identical focused fixtures through both backends and enforce an explicit per-pixel band.

7. **LOW —** `host/linux/src/gpu.rs:164`. Pipeline-cache bytes are written directly to the final file, non-atomically, with errors discarded. A crash or concurrent launch can leave a truncated cache that the next boot reports as “cached” merely because the file exists, although wgpu must fall back and compile again. Write a temporary sibling file, sync as appropriate, rename atomically, and only report a usable cache.

## Verdict
NOT READY

---

## Disposition (orchestrator, 2026-08-29)

Verified against the code before folding; each fold names what holds it. The folds are transcribed in LLP 1015 §2a.

1. HIGH (`paint.rs` fails deterministically) — **CONFIRMED, FOLDED.** `backgrounds_land_in_their_boxes_with_their_radii` now takes its radius assertions from an inline `CARD` fixture (an opaque `#f7f7f7`, radius 16, on a white page) — the app's panels became translucent white over the sky in LLP 1014 §1a — and keeps the app's button; the "never failed" sentence in §8 now says what happened. The same app change had also broken two tests in `host/linux/tests/host.rs` (the root is the viewport's height now; the wheel goes to the app's `scroll` node and the page stays) and one in `host/web/tests/agent.rs`; all folded, and a `--no-fail-fast` workspace run is what found them (a fail-fast run had stopped at the first binary).
2. HIGH (`ClientCutText` allocates a wire-supplied `u32`) — **CONFIRMED, FOLDED.** `drain` reads and discards in 4 KiB pieces; `SetEncodings` and `FixColourMapEntries` (previously an unknown message) go the same way. No VNC client harness in the tree to hold it (declared; the scripted RFB client of the DRM run lives in a session scratchpad).
3. MEDIUM (the writer thread outlives its client) — **CONFIRMED, FOLDED.** A `closed` flag set under both locks the writer waits on, `notify_all` on both condition variables, the socket shut down, the writer joined.
4. MEDIUM (a failed GPU frame blanks the app and its boxes) — **CONFIRMED, FOLDED.** Under `Auto`, the CPU painter replaces the GPU's from the failed frame on and paints it (a note on stderr); a forced painter that fails keeps the last frame's boxes. Not held by a test — a failing backend cannot be injected through `boot_with` (declared).
5. MEDIUM (the image cache keys a reusable address) — **CONFIRMED, FOLDED.** Each entry holds the `Rc<Pixmap>` it keys, so the address cannot be reused while cached; `begin` drops entries nobody else holds.
6. MEDIUM (parity asserted, never tested) — **CONFIRMED, FOLDED.** `the_two_painters_agree_within_a_band` renders the app through both backends and asserts a band on the frame — measured on Metal at mean 3.24/255 with 2.98% of pixels differing by more than 32, asserted at 5 and 6% (§8 records it). It still skips without an adapter (declared, as before).
7. LOW (the cache file is written in place) — **CONFIRMED, FOLDED.** Written beside and renamed into place; `cached` is documented as "a cache file was found and handed to the driver", with the shaders' milliseconds as the measure of use.

Verdict NOT READY binds to the reviewed tree; the folds above are unreviewed by this family.
