# Code review, round 2: setup friction for outside apps (87cea7f0a..2ec5d985f), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, one fresh session with `--cwd` a detached worktree at `175ed7f99`.
- **Method:** the shared round-2 brief (sha256 `1cb6528aac764076ba53bf66e588258cd3aaecbce221bf1092cadbeb6ed2ff58`); blind to astra's round 2.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three fixed in `71a26dc84`:
  1. The cache is selected when no engine is named, there is no sibling ibex and `~/.cache/exact/hermes-macos/engine` exists; a selected cache without a receipt fails naming the receipt and `EXACT_HERMES_DIR`, and its pin is checked. An explicit `EXACT_HERMES_DIR` keeps the old optional check and never takes the cache's `hermesc`. Same choice in `js/bake` and `weatherlightPrerequisite`.
  2. The embed and archive lines name the skip (the same as astra #2).
  3. The comment now names the deferred `exact new` copy as the exception.

---

I'll review the round-2 fixes against the round-1 findings and the full change, read-only. First I'm loading the review instructions and the disposition notes.The review notes and both diffs are next, so I can check each round-1 finding against the actual fix.The fix diff is small. I'll read the changed functions and the round-1 arguments against the current code.Round-1 dispositions, then what is still open.

**Accepted as fixed.** Grok 1 and Astra 1: the `node_modules` refusal is gated on `PINNED_BUN`, which is null when the copied fixture has no `package.json`. Grok 2 and Astra 2: both lock files are checked with `existsSync` before `readFileSync`. Grok 3: `lockedMetadata` runs first, and the unlocked resolve runs only after that fails and the bytes still match. Grok 5: a deferred `createApp` only checks the copied bytes. Grok 9 and Astra 5: the epoch line and the empty `--wasm-root` line match the code (`conform.mjs` with no named apps and an empty root has zero targets and exits 0). Grok 10: the document test uses the same no-sibling-ibex plus receipt-file fallback.

**Accepted as argued.** Grok 4: after exact2's lock moves, the copy is no longer auto-resolved; the error tells you to rerun `cargo metadata` and to drop `--offline` when the cache lacks a crate. A stamp file would be new apparatus for that window. Grok 7 and Astra 4: `scripts/app.mjs` receipt capture throws `compiler input has no captured source identity` for a path outside the checkout, the app, and the iOS Hermes cache (`nameOf` around line 866). `QUEUE.md` already records that external `rerun-if-changed` lines are refused. The sibling ibex engine has the same gap.

**Not fully fixed.** Grok 6 and Astra 3, for the partial-ibex mix and the pin check on a cache that is actually selected. The receipt gate is attached to the wrong condition, below. Grok 8: only the simulator/device install line got the skip note.

1. **Should-fix** — `js/build.rs:79` and `js/build.rs:179`, same predicate in `js/bake/src/lib.rs:393`. `from_cache` is "no sibling ibex and `engine/hermes-input-receipt.json` exists", and it is computed before `EXACT_HERMES_DIR` is applied. A populated `~/.cache/exact/hermes-macos/engine` with no receipt never sets `from_cache`, so the new "has no receipt" assert is unreachable. The build fails with "no Hermes" at `../ibex/ios/Frameworks-vanilla/macos-static` and the hint names the cache directory that is already there. The other way around: no ibex, a cache receipt present, and `EXACT_HERMES_DIR` pointing at an engine tree that has archives but no receipt (this used to link). `from_cache` is still true, the receipt is read from the override, and the assert fires calling that directory the cached Hermes. `hermesc` is also taken from the cache while the VM comes from the override. Select the cache only when the override is unset and the cache archives exist; if that cache is selected and the receipt is missing or `sourceCommit` is not `HERMES_PIN`, fail with that. Leave an explicit `EXACT_HERMES_DIR` on the old optional receipt check. Point `js/bake` and `weatherlightPrerequisite` at the same choice.

2. **Should-fix** — `host/apple/build.mjs:1007` and `host/apple/build.mjs:1387`. An iOS build without the Metal toolchain warns once, before Cargo and Swift, then finishes. `--ios` install prints `no SVG filter kernels (no Metal toolchain)` at line 1398. `--embed` returns at line 1007 and only omits `ExactSvgFilter.metallib` from the file list. `--device --archive` returns at line 1387 and prints the `.ipa` line with no skip. Both are finished iOS artifacts; filter pictures then draw with Core Image (`SvgFilterMetal.library` logs that once at launch). The clause added at line 1317 never runs: that log is inside `if (!ios)`. Put the same skip text on the embed and archive lines.

3. **Nit** — `scripts/app.mjs:192`. The comment says finding an outside app never changes its lock. The branch at line 198 runs unlocked `cargo metadata`, which rewrites a byte-identical copy. Say that the deferred `exact new` copy is the exception.

LAND WITH FIXES
