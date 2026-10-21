# Code review: stage 4 commit 2: the closure, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `d1cca8ee4562547760f0927f2c51491b9124d8042a88b7a070bf57c0c673de18`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (an unknown closure answered live; the cache key's resolution inputs; comments and the profile rule).
- **Disposition:** applied without re-review in the third-round commit

---

**Blocking correctness issues remain.**

1. **P1 — Failed candidate discovery still permits `live`.** [contract/cli/src/tamber_serve.rs:362](/tmp/rv-s4c2c/contract/cli/src/tamber_serve.rs:362)  
   When compilation stops before theme lowering, the cleared candidate table yields `[]`. Although that result isn’t cached, it still reaches `plan` as a complete synthetic list. With a last-good app containing a mix over `brand`, introduce an unrelated Contract syntax error, then propose changing `brand`: the answer can be `live` with no synthetic update. Unavailable closure information must force rebuild or refusal.

2. **P2 — The cache omits inputs that determine import resolution.** [contract/cli/src/tamber_serve.rs:333](/tmp/rv-s4c2c/contract/cli/src/tamber_serve.rs:333)  
   The key excludes `graph.consulted`, including package manifests. If two Contract files are already imported through explicit subpaths, changing a package’s default export between them can change which component—and synthetic expression—the app uses without changing the ordered source paths or their digests. The cache then keeps the old candidates indefinitely, producing incomplete or incorrect live answers. Include resolution inputs or resolved import edges in the key.

3. **P3 — Comments can incorrectly disable live editing.** [tamber/src/session.rs:121](/tmp/rv-s4c2c/tamber/src/session.rs:121)  
   The profile guard searches the entire source text. A comment such as `// future: color(--brand …)` makes ordinary colour edits rebuild even though no token uses a profile. Inspect parsed values instead.

The role-chain check, strict/loose folding rules, and gained/lost candidate comparison otherwise appear consistent with the compiler.

Static review only; builds/tests were not run under the read-only constraint. `git diff --check HEAD~3 HEAD` passed. No files changed.
