# Code review: the JS loop's design reader (value-only edits without a build), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `f96c979e88d9019702a0252abdf3a489647e33560fa67826d2d491981ec6917a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (plan structure; another design file; reader freshness; error; cargo on stop).
- **Disposition:** in the first-round commit.

---

No P0/P1 blockers found. Five P2 findings in `host/web-js/dev.mjs`:

- **[144](/tmp/rv-s4-15/host/web-js/dev.mjs:144): Compare full plan structure.** Changing a profile-colour token can preserve `shape` and accent flags while changing `structure`. The fast path batches it and retains dist’s old plan; the slow path would rebuild.

- **[166](/tmp/rv-s4-15/host/web-js/dev.mjs:166): Restrict eligibility to the app’s design file.** A Rust data module using `include_str!("palette.tamber")` stops updating when that file changes: this branch reads `app.tamber` and skips the build that would detect the changed logic digest.

- **[133](/tmp/rv-s4-15/host/web-js/dev.mjs:133): Recheck compiler freshness.** Changes under `contract/` or `kernel/` trigger no watcher here. Subsequent design edits keep using the old optimized compiler indefinitely, potentially accepting values the updated compiler refuses. The previous `webCompiler()` path checks dependencies.

- **[171](/tmp/rv-s4-15/host/web-js/dev.mjs:171): Clear the previous build error on success.** After an invalid design causes a failed build, repairing it through this branch updates the snapshot but leaves `error` set. Newly opened pages still display the old failure.

- **[128](/tmp/rv-s4-15/host/web-js/dev.mjs:128): Stop the Cargo child during shutdown.** `stop()` never terminates `tool.child`. Sending SIGTERM during compilation leaves Cargo running and potentially holding the build lock after the server exits.

In-memory checks confirmed scrub snapshot/base handling and replay of edits arriving during a design read. No worktree files changed.
