# Code review, round 2: setup friction for outside apps (87cea7f0a..2ec5d985f), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `175ed7f99` (the round-1 fixes plus round 1's reviews and dispositions).
- **Method:** brief sha256 `1cb6528aac764076ba53bf66e588258cd3aaecbce221bf1092cadbeb6ed2ff58` (the fixes against round 1's findings and dispositions, then the whole change), the same one sent to grok; blind to grok's round 2.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all four fixed in `71a26dc84`:
  1. A deferred `exact new` writes exact2's lock back after Cargo's failure, so the bytes the first build recognises are exactly the copy, whether or not Cargo wrote the lock first.
  2. The `--embed` line names "no SVG filter kernels (no Metal toolchain)" on iOS, as do the archive and install lines; the unreachable clause on the macOS line is gone.
  3. The cache is chosen only when `EXACT_HERMES_DIR` is unset (and there is no sibling ibex and the cache's `engine` exists); an explicit engine keeps the old optional receipt check. `js/bake` and the document test use the same choice.
  4. `host/web/build.mjs` imports Rolldown after `resolveApp`. With `node_modules` moved away, `bun --no-install host/web/build.mjs caltrain` now prints the `bun install --frozen-lockfile` line.

---

Round 1: I accept Astra #1–2 / Grok #1–3: fixtures bypass the dependency guard, both locks are checked for existence, and valid locks avoid unlocked resolution. I accept Grok #4’s explicit online-repair advice without a stamp. Astra #3 / Grok #6 now select one default Hermes provider and require the cache’s pinned receipt, though the assertion introduces finding 3 below. I accept deferring Astra #4 / Grok #7: [QUEUE.md:823](/tmp/rv-setup2/QUEUE.md:823) documents the existing invalidation problem, receipt-capture constraint, and clean-build workaround. Astra #5 and Grok #9–10 are fixed. Grok #5 and #8 remain incomplete:

1. **Should-fix — [game/new-app.test.mjs:87](/tmp/rv-setup2/game/new-app.test.mjs:87), [game/new.mjs:292](/tmp/rv-setup2/game/new.mjs:292): deferred resolution does not guarantee unchanged lock bytes.** With registry metadata cached but a required crate archive missing, Cargo can rewrite the app’s lock and then fail downloading offline: its [resolver writes the lock before downloading packages](https://doc.rust-lang.org/stable/nightly-rustc/src/cargo/ops/resolve.rs.html). `resolveOffline()` reports deferral without restoring the copy, so the new equality assertion fails and the completion message is false. Restore the original copied bytes on deferred failure, and cover failures both before and after lock writing.

2. **Nit — [host/apple/build.mjs:1007](/tmp/rv-setup2/host/apple/build.mjs:1007): the embed completion still hides the Metal skip.** An `--ios --embed` build without Metal finishes without mentioning the missing kernels. The added condition at line 1317 cannot help: it is inside `if (!ios)`. Add the skip to the embed completion and remove the unreachable macOS condition. Grok #8 is only partially fixed.

3. **Should-fix — [js/build.rs:179](/tmp/rv-setup2/js/build.rs:179): cache availability incorrectly imposes receipt requirements on explicit overrides.** On a Mac without sibling ibex, an available cache sets `from_cache=true` even when `EXACT_HERMES_DIR` and `EXACT_HERMESC` select a separate, provisioned pair. If that explicit engine lacks a receipt—previously permitted—the new assertion rejects it as “cached Hermes.” Track whether the **selected engine** came from the cache and apply the mandatory-receipt assertion to that selection. Cover explicit overrides with an unrelated cache present.

4. **Should-fix — [scripts/app.mjs:427](/tmp/rv-setup2/scripts/app.mjs:427), [host/web/build.mjs:27](/tmp/rv-setup2/host/web/build.mjs:27): the dependency refusal runs too late for web builds.** Rolldown’s static imports resolve before `resolveApp()`. Confirmed here: direct resolution prints the install instruction, but `bun --no-install host/web/build.mjs caltrain` prints only “Cannot find package 'rolldown'.” A fresh offline machine therefore still misses the intended remedy. Load those dependencies dynamically after the preflight, and test the actual entrypoint without installed packages.

Read-only control-flow probes passed for lock repair and optional/required Metal handling. No full builds or test suites ran; `git diff --check` passed and the worktree remains clean.

LAND WITH FIXES
