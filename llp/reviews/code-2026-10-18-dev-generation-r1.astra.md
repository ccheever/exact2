# Code review: the dev loop's generation identity, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `5862196aa`.
- **Method:** one brief (sha256 `b12254251206a7dc3fe87e3a7c2dc5d7d041fe822f8b7b8bac44bf8cc14f8271`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (entry plans refused by the web page, pre-existing).
- **Disposition:** fixed in `8830a5c32`.

---

1. **Should-fix — [host/web/dev.js:57](/private/tmp/rv-ws-1/host/web/dev.js:57): generations containing entry plans are rejected.** `captureGeneration` includes `plans/widget.plan` in the asset roster, but `validName` accepts only `assets/`, `deck/`, and `shaders/`. I reproduced `invalid or duplicate asset name` on initial fetch and subsequent edits, despite the digest matching. **Fix:** admit `plans/` with the existing path checks, and include an entry plan in the [regression test](/private/tmp/rv-ws-1/scripts/caps.test.mjs:405). This also fails before this commit.

The digest change otherwise checks out: in-memory execution of `captureGeneration` and `fetchGeneration` matched across initial, asset-edit, logic-edit, Rust, and mixed-app paths, including UTF-8 asset ordering and Rust targets. Apple’s canonicalization matches by inspection. Linux and Android share a separate plan/Rust fingerprint and do not verify `dev.generation`.

Retention uses epoch/sequence paths; receipts bind plan and payload digests. I found no dependency on the old URL-inclusive identity.

The new test is sound for the reported regression: it passes now and fails with the old hashing logic. Full suites and native execution were not run under the read-only constraint.
