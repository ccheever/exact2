# Code review: setup friction for outside apps and the agent pitfalls doc (151c5c0a5..87cea7f0a), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, one fresh session with `--cwd` a detached worktree at `87cea7f0a`. `--always-approve` because plan mode cancels shell reads; the brief instructs read-only, and the worktree was clean afterwards.
- **Method:** the shared brief (sha256 `17a78f500883f4c5be4b8d85bdd068a6345351f1dc99305452d57cdd390e7834`); round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the text before "1." is progress narration and the CLI's own compaction notice.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** fixed in `2ec5d985f` except #7, argued:
  1, 2. The same as astra #1 and #2; fixed.
  3. Fixed: `lockedMetadata` runs first; the copied lock is resolved only when that fails and the bytes still match exact2's, then the locked check runs again. A second worktree at the same commit is never rewritten.
  4. Fixed without a stamp: once exact2's lock has moved, the refusal now says to run `cargo metadata --format-version 1` without `--offline` "when Cargo's cache lacks a crate, as after an offline `exact new`". A stamp file would be new apparatus for a case the message covers.
  5. Fixed: `game/new-app.test.mjs` asserts the copied bytes when `createApp` reports the deferral, and checks `--locked` only after a successful offline resolve.
  6. Fixed, as astra #3: one source for both, the cache only with a receipt naming the pin.
  7. Argued, as astra #4 (QUEUE "Hermes incremental rebuild inputs").
  8. Fixed: both completion lines say "no SVG filter kernels (no Metal toolchain)" when the library was skipped.
  9. Fixed: "Every date in a screenshot is 1 January 2026 (31 December 2025 west of UTC)".
  10. Fixed: `host/web/tests/document.test.mjs` finds the engine and `hermesc` with the same fallback.

---

I'll review the `151c5c0a5..HEAD` diff against the repo rules, starting with `AGENTS.md`, `rules/RULES.md`, and the review skill.The rules are loaded. Next I’ll read the full `151c5c0a5..HEAD` diff and the surrounding code those changes touch.The diff is in eight files. I’ll read each changed region in context and check the pitfalls doc against the code it cites.The four setup fixes and the pitfalls doc are the review surface. I’ll read the call sites around lock resolution, Hermes lookup, Metal, and `resolveApp`.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is ready to write once the two breakages in `resolveApp` are checked against the current lines. I'll confirm those call sites, then deliver the findings.1. **Must-fix** — `scripts/app.mjs:419`. `resolveApp` throws when the directory that contains the loaded `scripts/app.mjs` has no `node_modules`. `scripts/app.test.mjs` `fixture()` copies `app.mjs` into a temp root with no `node_modules` and no `package.json`, then every `app()` call throws `has no node_modules`. `bun test scripts/app.test.mjs` fails on a machine whose real checkout already installed packages, including game and Rust-only resolutions that never run the TypeScript bake. Refuse only when that root has a `package.json` and no `node_modules`. A real checkout has `package.json`; the fixture does not.

2. **Must-fix** — `scripts/app.mjs:197`. When an outside `Cargo.lock` exists, `checkOutsideLock` always `readFileSync`s this module's `ROOT/Cargo.lock`. The same fixture's test "ordinary resolution refuses missing and stale locks without writing them" (`scripts/app.test.mjs:703`) generates `game/Cargo.lock` and then expects `app('plain').cargoPackage('web')` to succeed. The fixture root has no `Cargo.lock`, so the compare throws `ENOENT` and the outside lock is never checked. Skip the compare when the root lock is absent.

3. **Should-fix** — `scripts/app.mjs:198`. Any outside workspace whose lock is byte-identical to this checkout's gets an unlocked `cargo metadata` (offline, then online with no `--locked`) on every `resolveApp`, before the locked check. A second exact2 worktree at this commit, opened with `EXACT_APP_DIR`, is that case: the lock is already valid, and the comment at line 192 says finding an outside app never changes its lock. Unlocked `cargo metadata` is allowed to rewrite it. Call `lockedMetadata` first. Run the resolving metadata only when that fails and the bytes still match, then lock again.

4. **Should-fix** — `scripts/app.mjs:197` and the error at line 205. `exact new` leaves exact2's lock in place (`game/new.mjs:237`). The new resolve runs only while that file stays byte-identical to the checkout's current `Cargo.lock`. After exact2's lock changes, the copy no longer matches, the resolve is skipped, and line 205 tells the user to run `cargo metadata --offline`, which is the command that already failed here. The outside app stays unbuildable. Have `exact new` write a stamp that a later exact2 lock change does not erase, and resolve when that stamp is set. Keep the stale-lock refusal in `scripts/app.test.mjs:716`, which bumps a crate version and requires the lock untouched.

5. **Should-fix** — `game/new-app.test.mjs:87`. On a cold Cargo cache, `createApp` now succeeds and keeps exact2's lock (`game/new.mjs:292`). This test then requires `cargo metadata --offline --locked --no-deps` to exit 0. That file is exact2's workspace lock, so `--locked` fails in the new app workspace, which is the machine this change is for. When `createApp` reports the deferred lock, assert the copied bytes and the message. Run the `--locked` dependency check only when offline resolution actually succeeded.

6. **Should-fix** — `js/build.rs:75` and `js/build.rs:173`. The engine and `hermesc` are chosen independently, and the pin check runs only when `hermes-input-receipt.json` reads successfully, and only for the engine. A partial ibex tree (framework present, `hermesc-macos-<arch>` absent, cache `hermesc` present) links one source and compiles bytecode with the other. A cache engine with no receipt is accepted at any pin. `~/.cache/exact/hermes-macos` has no pin in the path. Choose one source for both artifacts. When the cache supplies the engine, require `sourceCommit` == `HERMES_PIN`. `js/bake/src/lib.rs:388` needs the same choice.

7. **Should-fix** — `js/build.rs:152`. macOS emits `cargo:rerun-if-changed` for the iOS static dir only. Cache archives are copied into `OUT_DIR` when the build script runs, and nothing fingerprints `~/.cache/exact/hermes-macos`. `js/bake` resolves `hermesc` at bake time (`js/bake/src/lib.rs:383`). Replacing the cache and then editing the app rebakes bytecode with the new `hermesc` while `exact-js` stays linked to the previous VM. Emit `cargo:rerun-if-changed` for the engine archives and the `hermesc` actually selected.

8. **Should-fix** — `host/apple/build.mjs:102`. Without the Metal toolchain, an app build warns and returns null, and every copy is guarded (`1000`, `1345`, `1357`). Host tests still pass `required=true` (`1446`). The warning is printed before the Cargo and Swift build, and the completion lines (`1007`, `1317`, `1398`) mention `ExactSvgFilter.metallib` only when it was produced. A finished iOS build of a filter-using app reads as success while every SVG filter picture draws with Core Image. Put the skip on those completion lines.

9. **Nit** — `docs/agent-pitfalls.md:76`. The agent epoch is `2026-01-01T00:00:00Z`, which is this year. The symptom says every screenshot date reads as last year. The cause and the `--epoch` / `--time-zone` fix are right. Say the dates read as 1 January 2026.

10. **Nit** — `host/web/tests/document.test.mjs:27`. `weatherlightPrerequisite` still requires the sibling ibex engine and `hermesc-macos-<arch>`. On a cache-only machine the document test reports the toolchain missing and skips, while `js/build.rs` would build. Use the same cache fallback.

The other pitfalls entries match the current tree: the cited attributes, defaults, flags, and commits `32805146` and `c03685dc` are real, and the top add/remove rule is clear. Line counts stay under the 1,500-line cap (`host/apple/build.mjs` is 1,498).

LAND WITH FIXES
