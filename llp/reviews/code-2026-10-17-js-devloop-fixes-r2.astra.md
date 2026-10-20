# Code review: JS dev loop fixes (atomic save, startup lock, stale reader), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `525b91c1a`, `ab217a323`, `263c8ad4c` with round 1 applied.
- **Method:** one brief (sha256 `44dfb4f2410c523c20d8f7167634c35f97aca05a961925d9e8c634772971d947`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES on all three (backend replaced with an older mtime; `.tmp.png`; deleted assets seen in a burst; unrelated crates; LLD; the test passing early).
- **Disposition:** the last round, applied without re-review into each commit (`ab1cc06ef`, `d52432ed5`, `420699cc8`). Lock: backend-origin compared by its bytes and its being there against the last build's, not its mtime, for both event forms. Atomic save: a `.tmp` suffix only hex or numeric (`app.tamber.tmp.png` is an asset); a gone file edits nothing only if born since the last build began (birth time), never a prior input. Reader: the linker text read per missing symbol (macOS `referenced from`, LLD's `undefined symbol` / `referenced by`, GNU ld's `in function`), so another missing symbol's crate is not cleaned. Test: an app of its own in a temp dir (no shared fixture touched); the loop now says `design reader ready` and, when a batch's verifying build keeps it, `edit -> checked ... the batch stands`, and the test waits on those, with no fixed sleeps.

---

Read-only review; line numbers refer to HEAD. I checked extracted code with in-memory stubs. I did not run the integration test because it modifies fixtures.

1. **P0 — Commit 1 still skips backend replacements with older timestamps.** [host/web-js/dev.mjs:485](/tmp/rv-devloop2/host/web-js/dev.mjs:485)  
   Replace `.exact/backend-origin` with a prewritten file containing a different origin whose mtime predates `since`. The bare `.exact` event is rejected here; a direct file event is rejected at line 500. Creating the backend by moving an older `.exact` directory into place also fails that second filter. The app keeps its previous backend. **Fix:** compare the backend’s contents and existence against the build’s snapshot, and bypass the generic timestamp filter for detected changes.

2. **P0 — Commit 2’s temporary regex still suppresses genuine inputs.** [host/web-js/dev.mjs:465](/tmp/rv-devloop2/host/web-js/dev.mjs:465)  
   With `assets/app.tamber` present, delete an existing image named `assets/app.tamber.tmp.png`. The `.tmp(?:[.-]\w+)?` branch accepts `.tmp.png`, so line 492 drops the deletion and the old image remains served. Changing that image is likewise ignored. **Fix:** protect declared or previously built inputs before applying temporary heuristics; do not accept arbitrary file extensions as temporary identifiers.

3. **P0 — Real asset deletions can still become design-only edits; this predates these commits.** [host/web-js/dev.mjs:327](/tmp/rv-devloop2/host/web-js/dev.mjs:327)  
   Deliver a change event for an existing `assets/logo.png`, then delete it and save the design before the burst timer fires. `appeared` contains the image, so `designOnly` becomes true. The verification build can discard its updated asset tree through `keep`, retaining the deleted image. **Fix:** distinguish inputs present in the previous build from newly created temporary files. Seeing a file during the burst does not establish that it was temporary.

4. **P1 — Commit 3 still selects unrelated crates for cleaning.** [host/web-js/dev.mjs:210](/tmp/rv-devloop2/host/web-js/dev.mjs:210)  
   Supply one diagnostic containing a missing `foo.llvm.123` referenced from `exact_kernel`, followed by an ordinary missing external symbol referenced from `exact_runner`. `staleCrates` returns both packages: the LLVM check applies to the entire diagnostic, not each symbol’s references. Healthy runner artifacts are unnecessarily removed. **Fix:** parse individual missing-symbol blocks and collect referencing crates only for the matching `.llvm.<hash>` symbol.

5. **P1 — Commit 3 misses LLD diagnostics.** [host/web-js/dev.mjs:208](/tmp/rv-devloop2/host/web-js/dev.mjs:208)  
   `rust-lld: error: undefined symbol: foo.llvm.123`, followed by an `exact_kernel-<hash>…rcgu.o` reference, returns `[]`; recovery never starts. LLD is the default linker on `x86_64-unknown-linux-gnu`. [Rust release notes](https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/) **Fix:** recognize LLD’s undefined-symbol forms, including hidden symbols, and associate their references with the relevant symbol.

6. **P1 — Commit 2’s test can still pass before an unwanted reload.** [scripts/tamber-server.test.mjs:713](/tmp/rv-devloop2/scripts/tamber-server.test.mjs:713)  
   A batch arrives, but the subsequent rebuild waits on Cargo long enough to finish after the fixed 15-second sleep. Both assertions pass, and teardown kills the pending rebuild. Evaluating the actual test body with delayed callbacks reproduced this. **Fix:** await completion of verification and queued watcher work, then assert that no reload occurred.

The empty-burst guard, one-clean limit, shutdown checks on recovery callbacks, and normal fixture restoration are correct. Package-specific cleaning acquires Cargo’s build lock, resolving that concurrent-build concern. [Cargo source](https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/ops/cargo_clean.rs)

HEAD~3 — **RIGHT WITH CHANGES**  
HEAD~2 — **RIGHT WITH CHANGES**  
HEAD~1 — **RIGHT WITH CHANGES**
