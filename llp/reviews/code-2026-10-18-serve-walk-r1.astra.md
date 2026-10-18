# Code review: the design server's walk of the Contract sources (serve cache), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `136c3bd78`.
- **Method:** one brief (sha256 `50594dafa599d41747e043b583a5cfd927a2ac3ebd734eb6637e75bee1bb42db`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (design-derived data, ownership inputs, equal bytes elsewhere, the cap; an unreadable root, same-stamp edits, a torn walk, per-request work).
- **Disposition:** fixed in `437d3d4cc` (see its message).

---

1. **Blocking — design-derived data never invalidates.** [tamber_serve.rs:482](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:482), [tamber_serve.rs:445](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:445)  
   `params_of()` includes components declared in `.tamber`. After warming `MapCard.inset`, changing its design-file default or range leaves `resolve`/`describe` returning the old data. Also, changing an icon’s SVG reference never adds the new asset to the cached graph/key; the old code’s fresh walk did. **Fix:** track the selected design and its dependencies, or cache Contract-derived params separately and merge current design-derived params.

2. **Blocking — cached ownership omits inputs that decide whether files are writable.** [tamber_serve.rs:423](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:423), [tamber_serve.rs:456](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:456)  
   Warm a relative import of `vendor/p/index.contract`, then add `vendor/p/package.json` with a package name. That manifest is not in `graph.consulted`; the file remains “owned,” so `set-state` can write and journal a package file. Adding a link beneath an existing `node_modules/@scope`, or retargeting an unused module link without changing its name, has the same ownership problem. **Fix:** record all ancestor manifest probes, scoped entries, and canonical module-link targets used by `owned()`.

3. **Blocking — equal bytes do not establish equal import resolution.** [tamber_serve.rs:436](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:436)  
   Retarget `current.contract` from `a/button.contract` to byte-identical `b/button.contract`, leaving both files present. The changed inode is accepted because the digest matches. Component identities, relative imports, and owned paths can nevertheless differ. The server continues serving old params and can write the old file. **Fix:** compare canonical resolution targets independently of content hashes and rewalk when they change.

4. **Blocking — the required caps check fails.** [tamber_serve.rs:420](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:420)  
   Running `bun scripts/caps.mjs` reports this file at **1,580 lines**, exceeding 1,500. **Fix:** extract the cache implementation or otherwise reduce the file below the limit.

5. **Should-fix — an unreadable root can freeze the cache permanently.** [tamber_serve.rs:441](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:441)  
   When `app.contract` is missing or invalid UTF-8, `source_graph()` returns empty `sources` and `consulted`. The cached empty file list subsequently passes `.all()` forever while module names remain unchanged. Restoring the root does not refresh params or ownership. **Fix:** always track the root path, including failed reads, and ensure failed graphs can recover.

6. **Should-fix — same-stamp edits bypass validation.** [tamber_serve.rs:436](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:436)  
   Change a param default from `4px` to `8px` in place and restore the original mtime. Length, device, inode, and mtime match, so cached params remain stale. Although snapshots read the new bytes, their digests do not invalidate this cache. **Fix:** include Unix ctime with nanoseconds and reconcile cached inputs with freshly read snapshot digests.

7. **Should-fix — the cached graph and its fingerprints can describe different revisions.** [tamber_serve.rs:440](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:440)  
   If an editor adds an import after the walk reads an existing source but before line 447 fingerprints it, the server stores the old graph with the new file’s stamp/hash. Subsequent requests consider it fresh, omit the newly imported file from ownership, and miss later changes to that dependency. **Fix:** fingerprint the bytes actually consumed by the walker and validate those observations before publishing the cache.

8. **Should-fix — cache hits retain substantial per-request work.** [tamber_serve.rs:433](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:433), [tamber_serve.rs:668](/private/tmp/rv-sc-1/contract/cli/src/tamber_serve.rs:668)  
   A timestamp-only change causes repeated hashing forever: digest equality never updates the saved stamp. Additionally, each `walk()` scans all recorded paths, `sources_key()` copies the whole key, and snapshots still read/hash every owned Contract file. These remain O(app), with multiple validations per request. **Fix:** promote validated stamps, share validation within a request, and reuse safely validated texts/digests.

Ordinary import edits and server `set-state` renames invalidate correctly. I found no re-entrant `RefCell` borrow path. The working tree is unchanged; runtime reproductions requiring writes were not run.
