# Code review: setup friction for outside apps and the agent pitfalls doc (151c5c0a5..87cea7f0a), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `87cea7f0a`.
- **Method:** one brief (sha256 `17a78f500883f4c5be4b8d85bdd068a6345351f1dc99305452d57cdd390e7834`), the same one sent to grok; round 1; blind to the other review. Requested by Charlie through the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** every finding checked in the source; fixed in `2ec5d985f` except #4, which is argued:
  1. *The node_modules refusal breaks fixtures.* Fixed: it applies only to a checkout with a `package.json` (`PINNED_BUN`, the Bun-pin guard's test). `bun test scripts/app.test.mjs`: 66 pass, 0 fail (25 failed at `87cea7f0a`).
  2. *A missing root lock.* Fixed: the comparison needs both files.
  3. *An unvalidated cache.* Fixed: the cache is used only when there is no sibling ibex at all (so the engine and `hermesc` never mix), and only when `engine/hermes-input-receipt.json` exists; `js/build.rs` then asserts its `sourceCommit` is `HERMES_PIN`, the check ibex's receipt already gets. This machine's cache had no receipt; one was written naming its source checkout's HEAD (`6badada7`, the pin).
  4. *No rerun on cache changes.* Argued: the sibling ibex engine has the same property on macOS today, and QUEUE's "Hermes incremental rebuild inputs" records why: external `rerun-if-changed` paths make the bake receipt's capture refuse them. The fix belongs with that line, for both sources.
  5. *An empty `--wasm-root` compares nothing.* Fixed in the doc: name the apps, `--build`, a root of your own; "with no apps named, an empty root compares nothing and passes" (reproduced: 0 targets, exit 0).

---

1. **Must-fix — [scripts/app.mjs:419](/tmp/rv-setup1/scripts/app.mjs:419): the dependency guard breaks existing fixtures.** `scripts/app.test.mjs` copies the resolver into minimal workspaces without `package.json` or `node_modules`. Their `resolveApp()` calls now fail before exercising shell repair or workspace resolution. Gate the refusal on the checkout having its package manifest, matching the existing Bun-version guard.

2. **Must-fix — [scripts/app.mjs:197](/tmp/rv-setup1/scripts/app.mjs:197): the lock comparison assumes a root lock exists.** The existing separate-workspace fixture creates `optional/Cargo.lock` but no root `Cargo.lock`. After fixing finding 1, resolution still throws `ENOENT` before checking the valid outside lock. Check that both files exist before comparing; otherwise proceed to `lockedMetadata()`.

3. **Should-fix — [js/build.rs:75](/tmp/rv-setup1/js/build.rs:75), [js/bake/src/lib.rs:393](/tmp/rv-setup1/js/bake/src/lib.rs:393): autodiscovered cache contents are not established as the pinned engine/compiler pair.** Selection checks existence, and the later engine receipt check is optional—the cache on this machine has no receipt. A stale cache can therefore supply another Hermes revision silently; independent engine/compiler fallback can also mix incompatible inputs. Validate the discovered artifacts against the pinned receipt and matching compiler before accepting them.

4. **Should-fix — [js/build.rs:75](/tmp/rv-setup1/js/build.rs:75): Cargo does not track the fallback’s inputs or selection.** After a successful cached build, replacing the cached engine/compiler or provisioning the preferred sibling need not rerun `exact-js`’s build script. Its captured archives and prelude can remain old while `Tools::default()` selects the new compiler for an app bake, causing incompatible bytecode or stale engine use. Add invalidation for the effective artifacts, receipt, and provider selection while preserving the existing `OUT_DIR` capture.

5. **Should-fix — [docs/agent-pitfalls.md:89](/tmp/rv-setup1/docs/agent-pitfalls.md:89): a fresh `--wasm-root` is not an alternative to building.** With an empty root and no named apps, conformance discovers zero targets and succeeds even under `--strict`; named apps instead lack their reference builds. Recommend named apps with `--build` and a private root, or explicitly require that root to contain builds from the current checkout.

Read-only probes confirmed findings 1 and 2 against the base. Changed files satisfy the line cap, and `git diff --check` passes. Full builds/tests were not run.

LAND WITH FIXES
