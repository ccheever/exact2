# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `0ef88b7fd`.
- **Method:** one brief (sha256 `37fb25c19813b21bcfcbd05773260b6c34e7582ddc637c660965f83d119165bb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 high, 4 medium, 1 low).
- **Disposition:** all taken in round 1: axis reorders refused and a revert's axis result checked against the old order; a token's kind and metadata part of its identity; a held revert planned again from the commit when its files move (partial accepts on its planned digests); the tool's files served through no link; one file a request for compare and revert-plan, transport errors returned; duplicate declarations refused; `sources` for multi-file DTCG imports; one git diff a file, by literal path.

---

1. **High — Axis reordering is silently omitted from reverts.** [tamber/src/history.rs:241](/tmp/rv-dt-1/tamber/src/history.rs:241)  
   The planner compares axis membership but never restores ordering. If a commit changes `black | dim` to `dim | black` and changes a token, reverting restores the token while leaving the wrong default mode, without a conflict. A reorder-only commit incorrectly reports “nothing to revert.” **Fix:** restore ordering explicitly, or refuse ordering changes until supported. Test the resulting default, not just emitted operations.

2. **High — Reverting a token addition can delete subsequent edits.** [tamber/src/history.rs:401](/tmp/rv-dt-1/tamber/src/history.rs:401)  
   `same_token` checks values and branches but ignores kind and metadata. Suppose a commit adds `card = #303030`, then someone adds `scheme=fixed` or changes its label. Reverting the addition still queues `RemoveToken`, deleting that later work. **Fix:** compare the complete declaration before removing a token or treating a restored token as already correct; report differing metadata or kinds as conflicts.

3. **High — Held reverts lose their conflict checks when files move.** [scripts/tamber-server.mjs:535](/tmp/rv-dt-1/scripts/tamber-server.mjs:535)  
   Reverts become ordinary held operations, without the planning snapshot’s digests. After a stale acceptance, the existing acceptance path calls `propose` with those old operations rather than rerunning `revert-plan`. For example, a held inverse of `#101010 → #202020` will subsequently offer to overwrite a newer `#303030` with `#101010`, instead of refusing the conflicting token. There is also an unguarded gap between initial planning and holding. **Fix:** retain the revert’s original versions and planning digests; replan the inverse whenever its base changes.

4. **Medium — Symlinks escape the static-serving boundary.** [scripts/tamber-server.mjs:270](/tmp/rv-dt-1/scripts/tamber-server.mjs:270)  
   The containment check is lexical; `statSync` and `readFileSync` follow symlinks. A harmless-looking asset symlink can expose a file outside `toolDir`, including a dotfile through a non-dot alias. I reproduced a 200 response using an existing repository symlink whose target was outside the configured directory. Local-only access remains enforced, but filesystem containment does not. **Fix:** reject symlink components, or validate canonical targets and their dotfile restrictions before serving regular files.

5. **Medium — Whole-commit batching breaks history and revert at the stdio limit.** [scripts/tamber-server.mjs:510](/tmp/rv-dt-1/scripts/tamber-server.mjs:510)  
   Both versions of every changed file are serialized into one request. Three 200 KiB files exceed 1 MiB despite each file being small enough individually. `history` then returns 200 with `edits: null` for every file, while `revert` returns 413. These are small HTTP requests concerning otherwise supported designs. **Fix:** batch comparisons by encoded size, aggregate revert planning against one guarded snapshot, and explicitly handle individually oversized pairs. Preserve transport-error status rather than reporting successful history containing only raw diffs.

6. **Medium — Duplicate declarations silently disappear from comparisons.** [tamber/src/history.rs:368](/tmp/rv-dt-1/tamber/src/history.rs:368)  
   Historical versions are deliberately parsed without checking, but `find` selects only the first matching declaration; branch lookup does likewise. Changing the second `edge` declaration therefore produces `edits: []`. Reverting can claim it is already undone or omit that edit while reverting others. **Fix:** detect ambiguous duplicate names and branches and refuse semantic comparison, or represent their occurrences explicitly. Add tests that inspect the actual declarations independently of `edits()`.

7. **Medium — The import endpoint cannot represent multi-file DTCG sources.** [contract/cli/src/tamber_import.rs:147](/tmp/rv-dt-1/contract/cli/src/tamber_import.rs:147)  
   The endpoint always passes exactly one `(name, source)` pair. Figma’s `Collection.Light.json` and `Collection.Dark.json` cannot be imported together; importing them separately treats each as a base and replaces values instead of preserving both modes. Resolver documents referencing sibling files also cannot work. This falls short of the existing importer’s coverage needed for D11’s per-mode review. **Fix:** accept a bounded bundle of named source texts and plan it as one import. Exercise the repository’s multi-file Figma fixture through the endpoint.

8. **Low — Git-quoted filenames lose their raw diffs.** [scripts/tamber-history.mjs:84](/tmp/rv-dt-1/scripts/tamber-history.mjs:84)  
   Matching assumes an unquoted `diff --git a/<path>` header. Git quotes Unicode and control-character paths, so their patches are discarded and returned as `diff: ''`. I reproduced this with the existing `Tokens — Completed.Light.json` fixture. **Fix:** decode Git’s quoted paths correctly or obtain each file’s patch separately using its literal pathspec.

The added tests cover basic histories, but omit these cases. In particular, the “recomputed” revert test changes files **before requesting** the revert, not after holding it. Read-only checks confirmed the symlink escape, quoted-path failure, and stale-revert dispatch sequence. Cargo tests were not run; no files changed.

Verdict: Request changes before landing.
