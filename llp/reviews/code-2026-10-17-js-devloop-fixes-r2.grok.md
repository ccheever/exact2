# Code review: JS dev loop fixes (atomic save, startup lock, stale reader), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `525b91c1a`, `ab217a323`, `263c8ad4c` with round 1 applied.
- **Method:** one brief (sha256 `44dfb4f2410c523c20d8f7167634c35f97aca05a961925d9e8c634772971d947`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT / RIGHT WITH CHANGES / RIGHT WITH CHANGES (LLD; the shared fixture; fixed sleeps; `.tmp` suffixes).
- **Disposition:** the last round, applied without re-review into each commit (`ab1cc06ef`, `d52432ed5`, `420699cc8`). Lock: backend-origin compared by its bytes and its being there against the last build's, not its mtime, for both event forms. Atomic save: a `.tmp` suffix only hex or numeric (`app.tamber.tmp.png` is an asset); a gone file edits nothing only if born since the last build began (birth time), never a prior input. Reader: the linker text read per missing symbol (macOS `referenced from`, LLD's `undefined symbol` / `referenced by`, GNU ld's `in function`), so another missing symbol's crate is not cleaned. Test: an app of its own in a temp dir (no shared fixture touched); the loop now says `design reader ready` and, when a batch's verifying build keeps it, `edit -> checked ... the batch stands`, and the test waits on those, with no fixed sleeps.

---

1. **P1 — On Linux the stale-link recovery never runs.** `host/web-js/dev.mjs:208`

   `staleCrates` returns `[]` unless the linker text matches `Undefined symbols` or `undefined reference`. The toolchain is Rust 1.97 (`rust-toolchain.toml`), and there is no linker override. Since Rust 1.90 the default linker for `x86_64-unknown-linux-gnu` is rust-lld, whose diagnostic is `undefined symbol` and `referenced by`. A failing input is:

   ```
   rust-lld: error: undefined symbol: _ZN12exact_kernel…E.llvm.15874425473274385624
   >>> referenced by kernel/src/lib.rs:10
   >>>               libexact_kernel-0c8e4f2a1b3d4e5f.rlib(exact_kernel-0c8e4f2a1b3d4e5f.1brnvk2mjzqx5b9n.rcgu.o):(sym)
   ```

   The gate rejects that text, so `cargo clean` is never started. The page still builds with the debug compiler, and every later design edit takes the full build. macOS `Undefined symbols` and GNU ld `undefined reference` still recover `exact-kernel` and drop `std`. Accept `undefined symbol` in the gate. The `.rcgu.o` arm already keeps the object line, and the crate regex already maps `exact_kernel` through `cargo metadata` to `exact-kernel`.

2. **P1 — The new test rewrites the shared theme fixture for the whole run.** `scripts/tamber-server.test.mjs:691`

   The test renames `apps/theme-fixture/.exact` aside, recolors `app.tamber` from `#16181c` to `#101010`, and deletes `.exact` in `finally`. Bun runs test files concurrently, and the async lane uses `bun test --parallel`. `scripts/design-tool.test.mjs:58` copies that `app.tamber` and line 558 expects `light-dark(#f1f3f5ff, #16181cff)`. A copy taken while this test holds `#101010` fails that assertion. Lines 962 and 970 start a wasm loop and `tamber tool` on the same `apps/theme-fixture`, so this test’s rename and `rmSync` of `.exact` remove that server’s lock. Restore runs only after `loop.stop()` returns. `stop` calls `dev.kill`, which throws if the process has already exited, and a timeout kill skips `finally`. Either one leaves `app.tamber` recolored and the original `.exact` sitting in `.exact.x2-test-<pid>`. Point the loop at a copy of the app.

3. **P1 — The test’s waits are fixed sleeps, so a late rebuild passes.** `scripts/tamber-server.test.mjs:706`

   After the lock file exists it sleeps 8 seconds and requires that `edit →` is absent. The first `plan ready` (line 154) is logged by the debug compiler and can finish while `exact-web-js` is still linking for host-dev. A lock rebuild that prints `edit → plan ready` after those 8 seconds satisfies the assertion, and the save then happens on top of it. The save waits for `/edit → \w+ ready/` (line 711), which matches a batch line or a plan line, then sleeps 15 seconds (line 713). The fast path logs `edit → batch ready` before the verifying build. A `edit → plan ready` from that build, or from the late lock rebuild, that lands after 15 seconds leaves both assertions green. A save taken while `toolFresh()` is still false correctly does a full rebuild, and the test fails because it required a batch and forbade a plan. Wait until the reader binary is ready, then wait for `edit → batch ready`, then wait until the verifying build has finished. Check the absence of `edit → plan ready` after that, with no fixed sleep.

4. **P2 — A `.tmp` suffix that is a real extension is treated as an editor temporary.** `host/web-js/dev.mjs:465`

   `(?:[.-]\w+)?` after `.tmp` matches any word. With `app.json` containing `"tamber": "assets/brand.tamber"` and `assets/brand.tamber` present, deleting `assets/brand.tamber.tmp.png` (or writing `assets/brand.tamber.tmp.contract`) makes `temporaryOfDesign` return true at line 492. The path never enters `edited`, so the page keeps the deleted asset. `brand.tamber.png` and `icons.app.tamber.ts` no longer match. Allow a suffix only when it is an editor token, or reject a suffix that is an input extension (`png`, `json`, `contract`, `ts`, and the rest).

`525b91c1a` (its own design server's lock is no edit): **RIGHT**.

`ab217a323` (an atomic save over the design is a value edit): **RIGHT WITH CHANGES**.

`263c8ad4c` (a design reader with stale incremental state is cleaned and built again, once): **RIGHT WITH CHANGES**.
