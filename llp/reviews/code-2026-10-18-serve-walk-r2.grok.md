# Code review: the design server's walk of the Contract sources (serve cache), round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `136c3bd78`, `437d3d4cc`.
- **Method:** one brief (sha256 `3cc06614f0d766a8af82c05124438b8d1ebb5f68e71be77df95ed13b3ba73ca0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (identity manifests; app.json, the walk-time test, the conflict reply).
- **Disposition:** fixed in `e1e4cb506` (see its message).

---

I'll review both commits read-only: first the round-1 change, then whether HEAD actually fixes those findings and leaves any stale-walk or per-request cost.Both commits are in place. Next I’ll read the walk cache and every caller that depends on it, then check each round-1 fix against the failure modes you listed.The package.json identity hole is blocking. The other round-2 fixes hold: design bytes, icon paths, the root, inode changes, and a server rename are picked up on the next request.

**1. Blocking** — `contract/cli/src/tamber_walk.rs:181` (key), manifests at `:105`, consumed at `app_params` `:220`

Adding or renaming a `package.json` above a vendored source refreshes the owned-file list and leaves params and synthetic tokens on the previous identity.

`vendor/p/index.contract` declares `Badge` with a param and is imported with a relative `use`. There is no `package.json`. Resolve returns identity `vendor/p/index.contract#Badge` and token `param.f<hash of that path>.Badge.…`. The next request after writing `vendor/p/package.json` with `{"name":"p"}` drops the file from the owned list, because `fresh` sees the manifest change and walks again. The new walk's key is only source, consulted, and design digests. That `package.json` is not one of them (`home()` reads it during load, and relative resolution never puts it in `consulted`). `app_params` and `synthetic` compare that key, hit the cache, and keep `vendor/p/index.contract#Badge`. The build now names it `p/index.contract#Badge`, so the token hash changes. A later set-param writes the stale token; the compiler applies the new one. The same split happens if the `"name"` field changes from `p` to `q`.

Append each manifest path and digest (empty when the file is absent) to `Walk::key` in the same `path\0digest\n` form as the other files, so params and synthetic tokens reload with the walk.

**2. Should-fix** — `contract/cli/src/sources.rs:266` (`design_path`), walk inputs at `contract/cli/src/tamber_walk.rs:166`

`app.json`'s `"tamber"` field chooses the design file and is not watched. `code_beside` re-reads `app.json` on every key build, but only whether `modules` or `hatches` is non-empty. Changing `"tamber"` from `app.tamber` to `brand.tamber` (a different native component and different icon paths) leaves that boolean alone, leaves every watched stamp alone, and the next resolve still serves `app.tamber`'s params. Before this cache, `params_of` and `source_graph` called `design_path` on every request.

Always stat `app.json`, present or not, the way the root is stat'd, and treat a digest change as a new walk.

**3. Should-fix** — `contract/cli/src/tamber_walk.rs:164` and `:189`

`once` is set when any file's mtime is greater than or equal to a wall-clock stamp taken before `source_graph`. That test is wrong in both directions.

A source dated in the future (`tar`, clock step, `touch`) is `>= began` on every walk, so `once` stays set and every request pays the full parse again.

A replace during the walk whose mtime is older than `began` is kept. The editor writes a temp before the request and renames it over `app.contract` while `source_graph` is still reading the old bytes. The temp's mtime is before `began`. The walk then hashes the new bytes, builds `owned` and the graph from the old bytes, and does not set `once`. The next request sees the stored stamp and digest, treats the walk as fresh, and serves that torn graph until some other stamp moves. `cp -p` during the walk is the same shape: mtime preserved, ctime ignored. `moved` looks only at field `s.1`.

Set `once` when the bytes just hashed are not the bytes `source_graph` parsed (the loader already digests each source; keep those and compare), not when mtime is merely `>= now`.

**4. Should-fix** — `contract/cli/src/tamber_walk.rs:154`, snapshot at `contract/cli/src/tamber_serve.rs:1365`

The once-per-request skip makes the conflict reply list the owned files from the start of the request. `transaction` walks at the opening snapshot. Commit then notices the file changed and calls `snapshot()` again. `checked == asked` returns the cached walk without stating. Bytes of files already in the list are re-read; a `use` added during the request, or a new `package.json`, is not in that list. The conflict body is what the client refreshes from.

Bypass the `checked` short-circuit on that path (set `checked` back so `fresh` runs).

What checked out:

- Design-file bytes are in the params key, and icon paths are sources or consulted entries, so a native-component default and a new or missing SVG invalidate the next request. Synthetic tokens still key only token names plus the sources key, which matches the previous server.
- The root is in the file list even when missing. A graph with errors sets `once`, so the next request walks again. `owned_kept` still fills from the last whole list.
- A new import is an edit of a file already in the walk. A file that resolution already tried (missing `package.json`, missing export, missing icon) goes from stamp `None` to `Some` and forces a walk. `node_modules` entries, including `@scope` and canonicalize targets, are compared every check.
- Another inode or device fails `same_file` and walks even when the bytes match. A same-inode stamp change re-reads and keeps the walk only when the SHA-256 matches. A normal write cannot keep both mtime and ctime, so a same-stamp content edit is not a realistic hole. Symlinks are followed by `metadata`, so retargeting changes the target inode.
- `set-state` renames a temp over the `.contract` file, which changes the inode. The next request fails `same_file` and walks. Startup recovery walks before it applies the journal (`asked` is still 0, so the short-circuit stays off); the first client request sees the new inodes.
- `walk` drops its `RefMut` before it returns the `Ref`, and `sources_key` returns an owned `String`, so `app_params` does not overlap those borrows.
- Steady state does not call `source_graph` or `params_of`. What remains per request is a stat per watched file, a hash of each ancestor `package.json`, and a `node_modules` listing. A graph that never loads cleanly still reparses every request, which is the `once` rule above.
