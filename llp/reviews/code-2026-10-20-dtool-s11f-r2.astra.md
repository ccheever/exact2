# Code review: the design tool's half of LLP 1121 stage 11 D on the real server (the stub dropped), round 2, 2026-10-20 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `7686803747`.
- **Method:** one brief (sha256 `7255fd3a5a7b8f40c05aaca12af468457e43bc769e0f12f2a7999fb539ed0177`), shared by both reviewers, each blind to the other's review; commits 60bf87b1d8 and 7686803747, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (4 SHOULD-FIX).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - Explain's `bases` are inputs of a value, beside its `names`, so an alias over a base name links its base. Round 1's disposition was wrong: they are separate lists. Tested with `card.pad: length = space.md` on the real server (1).
  - Each Apply in the test waits a clock step past its proposal (2).
  - radius.full's Set: the proposal for this draft is active, with its diff; applied, and the app's line checked in the file (3).
  - The newly Set space.md cell edits as the app's line, and the package's Set button goes once set (4).

No BLOCKERs found. The whole-file checks prove Set, Back, Back’s undo, and package Set/Back restore the expected bytes. Round 1 is not fully resolved:

1. **SHOULD-FIX — The rejected `bases` finding remains valid.** [apps/design-tool/app.ts:748](/tmp/rv-dt-s11f/apps/design-tool/app.ts:748) still discards `bases`. For `card.pad: length = space.md` with `space.md` unset, the checker records `bases: ["space.md"]` and `names: []`; these are separate collections ([tamber/src/check.rs:201](/tmp/rv-dt-s11f/tamber/src/check.rs:201)). The editor consequently offers neither the dependency link nor the pinning explanation. The disposition’s claim that `bases` is a subset of `names` is incorrect. Include base names among the candidates filtered by `inputsOf`, and cover this alias against the real server.

2. **SHOULD-FIX — The longer proposal timeout does not remove the test’s timing race.** [scripts/design-tool-scale.test.mjs:54](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:54) proceeds to Apply after one `clock data`. That operation returns `settled: false` after 20 seconds ([host/web-js/agent.js:354](/tmp/rv-dt-s11f/host/web-js/agent.js:354)), and the drive ignores that result. A healthy proposal taking 25 seconds now survives the client timeout, but the test attempts a disabled Apply and fails. Wait for the current proposal’s readiness within the read budget. The `clock +1` steps correctly advance the follow task; they are not the problem.

3. **SHOULD-FIX — The `radius.full` assertion does not prove server acceptance or application.** [scripts/design-tool-scale.test.mjs:123](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:123) checks only `ok: true`, which is also the proposal’s idle value ([apps/design-tool/app.ts:776](/tmp/rv-dt-s11f/apps/design-tool/app.ts:776)). A pending request can therefore leave a passing idle or previous value. The drive then selects `radius.md`, abandoning the draft without applying it. Assert a settled, active proposal matching this draft, then Apply and check the resulting file.

4. **SHOULD-FIX — Some removed transition coverage is still missing.** [scripts/design-tool-scale.test.mjs:54](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:54) never opens newly Set `space.md`; the editable-field assertion covers only the pre-existing `radius.md` override. The package assertions also omit checking that its Set button disappears ([scripts/design-tool-scale.test.mjs:109](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:109)). A newly overridden base remaining read-only, or a package retaining its Set button, would pass. Both checks existed in the removed stub test.

The other recorded fixes are present: package-cell-before-Set, describe-derived sources and column order, shipped web coverage, and async registration. `full` matches the wire; removing the fictional `stretch` fixture is appropriate.

Static review only. No files changed, builds or test suites run.
