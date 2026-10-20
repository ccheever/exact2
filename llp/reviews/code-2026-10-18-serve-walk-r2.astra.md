# Code review: the design server's walk of the Contract sources (serve cache), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `136c3bd78`, `437d3d4cc`.
- **Method:** one brief (sha256 `3cc06614f0d766a8af82c05124438b8d1ebb5f68e71be77df95ed13b3ba73ca0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (identity manifests, native params in the retention, app.json; a torn walk, a retargeted link to a hard link, per-request work).
- **Disposition:** fixed in `e1e4cb506` except the remaining per-request work (a snapshot reads its files; `uses` compiles), as before this change.

---

1. **Blocking — ownership manifests do not invalidate params.** [tamber_walk.rs:181](/private/tmp/rv-sc-2/contract/cli/src/tamber_walk.rs:181)  
   Warm the params cache for `vendor/p/index.contract#Card.radius`, then add `vendor/p/package.json` naming package `p`. The walk and ownership refresh, but `key` and `design` remain identical: `manifests` contributes to neither. Params retain the old identity instead of `p/index.contract#Card.radius`; synthetic retention can retain old param names too. Additionally, identity manifests inside external packages are excluded by line 110. **Fix:** record every manifest inspected by `sources::home`, including absent paths, and include those inputs in the relevant keys.

2. **Blocking — refreshed native params can use stale retention.** [tamber_walk.rs:183](/private/tmp/rv-sc-2/contract/cli/src/tamber_walk.rs:183), [tamber_serve.rs:379](/private/tmp/rv-sc-2/contract/cli/src/tamber_serve.rs:379)  
   After warming `synthetic`, add a native length param to the design without changing its top-level token names. HEAD refreshes `app_params`, but reuses retention computed before that declaration. Where only the native declaration retains that param, proposing its first differing `when` value can return **`live` instead of `rebuild`**: `session::shape` skips the missing retained token. The existing exclusion of design bytes is insufficient for retention. **Fix:** invalidate retention on native declarations and their dependencies, independently of synthetic candidates if necessary.

3. **Blocking — design selection is not watched.** [tamber_walk.rs:166](/private/tmp/rv-sc-2/contract/cli/src/tamber_walk.rs:166)  
   Change `app.json`’s `"tamber"` from `app.tamber` to another existing design, keeping `code_beside()` unchanged. Neither `graph.sources` nor `graph.consulted` records this manifest, so the cached walk continues watching the former design and serving its params. Subsequent edits to the newly selected design are invisible. **Fix:** watch `app.json`, including its absence, and track the selected design. Refresh the server’s selected design consistently or explicitly reject a selection change until restart.

4. **Should-fix — a save during snapshot collection can permanently preserve the wrong graph.** [tamber_walk.rs:174](/private/tmp/rv-sc-2/contract/cli/src/tamber_walk.rs:174)  
   The graph reads an old import, `stamp(p)` captures the old stamp, then an in-place save changes that import before `read(p)` hashes the new bytes. `moved` examines the old timestamp and remains false. Next request sees the newer stamp but matching cached digest, updates the stamp, and keeps the old graph indefinitely. Preserved-mtime replacements provide another route. **Fix:** validate against the bytes and resolution targets the loader actually consumed; do not retain a graph paired with a different snapshot.

5. **Should-fix — equal inode stamps do not establish equal resolution paths.** [tamber_walk.rs:131](/private/tmp/rv-sc-2/contract/cli/src/tamber_walk.rs:131)  
   Let `current` be an app-local directory symlink to `a`, with `a/index.contract` and `b/index.contract` hard links. Retarget `current` to `b`. Their followed stamps are identical, yet relative imports now resolve beside `b`. No watched stamp or ownership manifest need change, leaving params and the writable file list pointing at `a` and its dependencies. **Fix:** retain and compare canonical destinations for resolution paths, even when device/inode match.

6. **Should-fix — substantial O(app) work remains on warm requests.** [tamber_serve.rs:569](/private/tmp/rv-sc-2/contract/cli/src/tamber_serve.rs:569)  
   Every snapshot still reads, hashes, and decodes every owned `.contract` file, including for `resolve` and `hello`. Ownership manifests are also re-read and hashed every request; `uses` still performs a full mapped compilation at line 997. These are remaining costs, not all new regressions. **Fix:** reuse validated snapshot bytes/digests, stamp manifests, and cache mapped/motion results under a complete revision key.

The late-root retry, failed-graph retry, ordinary inode/ctime invalidation, scoped-package checks, and next-request invalidation after `set-state` look sound. I found no current `RefCell` re-entrancy path. Review was read-only; runtime tests were not run.
