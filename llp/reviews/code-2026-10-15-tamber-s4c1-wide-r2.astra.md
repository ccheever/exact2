# Code review: stage 4 commit 1: reclaimable wide colour storage, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Method:** one brief (sha256 `d606b85060061a33eb0aaa7089326302bf20ac19296dc32483ac0e44e73ac4d3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. Round 1 (x2-perf3's, before the 2026-10-15 restart) left no transcript.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (big batches parsed before a commit lost to GRACE; the document tree's keep marked an empty tree; settled gradients moved polar ramps; a refused full table never aged; a cloned arena's keep pinned; export snapshots unkept).
- **Disposition:** fixed in the version reviewed in round 3.

---

**Blocking findings remain in HEAD `dfb3e705`.**

1. **[P1] Uncommitted batches can lose colours during parsing.** [kernel/src/style/wide/reclaim.rs:172](/tmp/rv-s4-1/kernel/src/style/wide/reclaim.rs:172)  
   With an empty registered kernel, parsing 3,073 distinct wide colours before applying them causes the 3,073rd intern to reclaim the first 2,048. The runner accumulates precisely this kind of unmarked batch before [its first apply](/tmp/rv-s4-1/runner/src/runner.rs:1020); Apple and web use that path. `apply_frame` similarly decodes the whole frame first. Applying these valid batches then encounters stale IDs. Concurrent interns can also exhaust GRACE during a much smaller parse or thread handoff. Protection must cover construction through publication; a keep added after parsing is too late.

2. **[P1] The DocTree keep always marks an empty tree.** [runner/src/instance/document.rs:166](/tmp/rv-s4-1/runner/src/instance/document.rs:166)  
   `build.doc.nodes` is empty when `Keep::of` visits it. `build.children(...)` populates it afterward, and the keep is never rebuilt. Consequently, kernel-free rendering retains unprotected IDs: subsequent or concurrent colour churn can invalidate a document before projection. Mark the populated tree and protect colours while constructing it.

3. **[P2] Settling changes polar-gradient colours substantially.** [kernel/src/style/wide/marks.rs:255](/tmp/rv-s4-1/kernel/src/style/wide/marks.rs:255)  
   Preserving the interpolation setting does not preserve endpoint precision. For the Linux-admitted gradient  
   `linear-gradient(90deg in hsl, color(srgb 0.5001 0.5 0.5), blue)`,  
   settlement rounds the first endpoint to achromatic `#808080`. The mixer then borrows blue’s hue instead of interpolating from its original red hue. Following the mixer’s arithmetic, the midpoint changes from approximately `#bf40bf` to `#4040bf`. This affects ordinary Linux gradient capture too. Preserve precise endpoints or sample the ramp before quantization.

4. **[P2] Refusals can leave the table permanently unable to admit new colours despite retired slots.** [kernel/src/style/wide/reclaim.rs:141](/tmp/rv-s4-1/kernel/src/style/wide/reclaim.rs:141)  
   Hold 3,072 permanent colours, then replace one additional colour with 1,024 distinct values, trimming retired styles after each commit. The table fills with 3,073 held colours and 1,023 retired but recent entries. Further admissions fail without advancing `interns`, so those retired entries never age beyond GRACE. Repeated attempts cannot recover while the permanent colours remain held. Capacity recovery needs progress independent of successful new admissions.

5. **[P2] Cloned arenas permanently pin their original colours after reset and trim.** [kernel/src/arena.rs:174](/tmp/rv-s4-1/kernel/src/arena.rs:174)  
   `_kept` records the clone’s original colours and is never updated or cleared. `Kernel::rehydrate` retains this keep inside the new kernel. Rehydrate a colour-filled kernel, drop the original, then reset and trim the replacement: its nodes and cached styles disappear, but the original slots remain marked until the replacement kernel is dropped. A clone made with all 4,096 slots occupied can therefore prevent subsequent admissions even after becoming empty.

6. **[P2] Decoded snapshots are another unmarked holder.** [kernel/src/export.rs:436](/tmp/rv-s4-1/kernel/src/export.rs:436)  
   `export::decode` reconstructs wide IDs into `Snapshot.styles`, but `Snapshot` has no keep. Retaining a snapshot after its source kernel resets or drops allows another kernel’s churn to reclaim its colours. Later reads encounter stale IDs. The decoded snapshot needs lifetime protection, including its construction and clones.

The tests leave relevant gaps: the in-flight test advances only 512 interns; the settled-gradient test covers one default-Oklab example; and the [“row keeps its value” assertion](/tmp/rv-s4-1/kernel/tests/wide.rs:199) never applies the failed patch to the kernel it checks. The Linux test verifies reclaimed A in its original appearance, then checks a scheme flip using newly accepted B.

I found no additional mismatch in the SVG `Colors`/`Settle` traversal, including markers, or additional retained IDs in the Linux content-region workers. Generation arithmetic, free-list bookkeeping, `by_text`, live counting, and `stale()` lock ordering looked consistent.

Read-only review; no files changed. Sweep and interpolation arithmetic were checked with in-memory models. Rust/host tests and performance measurements were not run because there are no built artifacts and filesystem writes are prohibited.
