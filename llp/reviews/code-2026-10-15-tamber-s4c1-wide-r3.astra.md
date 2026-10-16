# Code review: stage 4 commit 1: reclaimable wide colour storage, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Method:** one brief (sha256 `8e6ec018b2d08a34f8cd163865850549b456c8483498a25b35e565e33f9d988a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. Round 1 (x2-perf3's, before the 2026-10-15 restart) left no transcript.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (batches left open by kernels that never commit; one kernel's commit closing another's batch; holder-less decoding; captured values shadowing live ids after a wrap; settled svg pairs not flipping).
- **Disposition:** applied without re-review (the last round): explicit stop lookup for gradients (`resolved_with`, `placed_with`), svg settled to 8-bit pairs, a batch closes with its thread's last holder and protects at most `SPAN` interns, `wide::Batch` around export decode and the document fold. Left to `GRACE`, as the commit says: another kernel's commit on the same thread closing a batch first, and threads with no holder.

---

Blocking issues remain in HEAD `f2b531b`.

1. **P1 — Detached renders leave batches open indefinitely.** [reclaim.rs:378](/tmp/rv-s4-2/kernel/src/style/wide/reclaim.rs:378)  
   Detached kernels never publish their commits; creating the DocTree’s `Keep` also does not close the batch. Dropping the runner removes its holder but leaves the thread’s batch intact. On a persistent render worker, this protects every subsequent intern process-wide, even after the documents are dropped. Repeated renders can exhaust all 4,096 slots and refuse new colours. Completed and abandoned batches need a retirement path independent of another kernel commit or thread exit.

2. **P1 — One kernel’s publication closes another kernel’s pending batch.** [reclaim.rs:366](/tmp/rv-s4-2/kernel/src/style/wide/reclaim.rs:366)  
   `publish()` unconditionally removes the thread’s batch. Parse operations for kernel A, publish kernel B, then intern enough colours to exceed `GRACE` and trigger a sweep: A’s pending colours can be reclaimed before A applies them. B’s mark does not contain A’s operations. The delayed-operation test exercises only `GRACE / 2` interns, so it misses this failure. Protection needs to follow the pending operations’ lifetime.

3. **P1 — Decoding on a thread without a holder remains unprotected.** [reclaim.rs:151](/tmp/rv-s4-2/kernel/src/style/wide/reclaim.rs:151), [export.rs:497](/tmp/rv-s4-2/kernel/src/export.rs:497)  
   With an idle kernel on another thread, decoding a snapshot containing 3,500 distinct wide colours can sweep its earliest colours during decoding. The snapshot’s `Keep` is created only afterward and cannot repair those stale IDs. Decoded wire frames also have no keep. A producer thread exiting after handing off operations removes its batch through `ThreadGone`, leaving the consumer dependent on the bounded grace window. Construction and transfer need protection through publication.

4. **P2 — Captured values can shadow unrelated live colours after generation wrap.** [wide.rs:65](/tmp/rv-s4-2/kernel/src/style/wide.rs:65)  
   The TLS override intercepts every lookup with a matching `u16`, including lookups reached through roles and accents. Retain a gradient containing `Wide(A)` and `AccentColor`; after A’s slot wraps, a live accent B can receive A’s old ID. Inside `Values::read`, resolving B then returns captured A, making both stops use A. This requires no missed holder and produces no stale assertion. Captured lookup must distinguish original stops from live indirect colours. The Values test never reclaims or reuses an ID.

5. **P2 — Settling SVG colours changes retained-region scheme flips.** [marks.rs:223](/tmp/rv-s4-2/kernel/src/style/wide/marks.rs:223), [region.rs:287](/tmp/rv-s4-2/host/linux/src/paint/region.rs:287)  
   `Settle` converts a wide `light-dark()` pair into one `Fixed` colour. While replacement content is pending, `paint_region` reuses the old picture even after the appearance changes. Previously, its SVG colours resolved using the current `painter.dark`; now wide pairs stay in the captured appearance, while legacy `LightDark` pairs still switch. Preserving both resolved halves would avoid losing that behaviour. The new Linux test flips appearance only after replacement content lands, so it does not cover this case.

Source review only: no files changed, and Rust tests or performance benchmarks were not run.
