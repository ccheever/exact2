# Code review, round 3: setup friction for outside apps (2ec5d985f..71a26dc84), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, one fresh session with `--cwd` a detached worktree at `c91c6e97d`.
- **Method:** the shared round-3 brief (sha256 `344edac59df6d5275be0eb6d8d60ced8182e96d3f6521b9aaefb211086c2de77`); blind to astra's round 3. The last round.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** both fixed in `aacdcd75e`:
  1. When the cache is selected, `js/build.rs` requires `engine/hermes-input-receipt.json` naming `HERMES_PIN` right after choosing the engine, before the `EXACT_JS_ENGINE=stub` return, so a stub build cannot bake with an unpinned cache `hermesc` (`js/bake` depends on `exact-js`, whose build script runs first). Driven: a copy of the cache without its receipt under a temporary `HOME` fails `cargo build -p exact-js` with `EXACT_JS_ENGINE=stub`, naming the receipt; with the receipt it builds.
  2. `weatherlightPrerequisite` skips, naming the receipt, when the cache it would use has no receipt naming the pin (`hermesIos().pin`, js/build.rs's).

---

I'll review round 3 read-only: first the prior findings and dispositions, then the fix diff and the whole change.The review notes and the round-3 diff are the next thing to read, so I can check each prior finding against the actual fix.Round-2 asked for four fixes. I’ll read the functions they touch and the rest of the change around them.Round 2 is fixed where it claimed to be. The lock is written back before an offline `exact new` defers, the iOS embed and archive lines name a skipped Metal library, an explicit `EXACT_HERMES_DIR` no longer pulls the cache, Rolldown loads only after `resolveApp`, and the outside-lock comment matches the code. Two of those edits drop the receipt rule that `js/build.rs` now depends on.

1. **Should-fix** — `js/bake/src/lib.rs:393`. A cache is chosen when `EXACT_HERMES_DIR` is unset, there is no sibling ibex, and `~/.cache/exact/hermes-macos/engine` is a directory. The receipt check did not move with that predicate. `js/build.rs:168` returns before its new assert when `EXACT_JS_ENGINE=stub` (the escape hatch that same script prints). The web build still publishes the bake's `app.hbc` as the native bytecode download (`host/web/build.mjs:228`). On a Mac with no ibex, a receipt-less cache directory, and that stub set, round 2 refused the cache compiler because `hermes-input-receipt.json` was missing; this round compiles and ships `app.hbc` from `~/.cache/exact/hermes-macos/hermesc`. When the cache is selected, require that receipt and `sourceCommit` equal to `HERMES_PIN` in `js/build.rs`, and fail with the same error. A normal non-stub build still dies in `js/build.rs` first, because `exact-js-bake` depends on `exact-js`.

2. **Should-fix** — `host/web/tests/document.test.mjs:28`. `weatherlightPrerequisite` treats the cache as usable when `engine` exists and `EXACT_HERMES_DIR` is unset. It no longer requires `engine/hermes-input-receipt.json`. A cache that has the three archives, headers, and `hermesc`, but no receipt, used to skip. `weatherlightCheck` now runs `bun host/web/build.mjs weatherlight --wasm`, `exact-js`'s build script asserts, and the test fails instead of skipping. Require the receipt file in `cached` (and treat a `sourceCommit` other than the pin as missing), so the skip matches the build.

LAND WITH FIXES
