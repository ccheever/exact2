# Code review: the design tool's half of LLP 1121 stage 11 D on the real server (the stub dropped), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `60bf87b1d8`.
- **Method:** one brief (sha256 `b07cd878e977efe5f68102c7bcee6cf4ea4835fa9d2f0e6745d2a8f51d823de4`), shared by both reviewers, each blind to the other's review; commit 60bf87b1d8. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (after the CLI's own compaction notice).
- **Verdict:** nothing blocks (2 SHOULD-FIX, 4 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - The grid is read after Back and before its undo, and Set is offered again after the package's Back (1).
  - Every source kind is compared (radius.md's measured and guideline cells) (2).
  - The package name comes from the wire (3).
  - radius.full's cell is compared whole; the set note and the field's state are checked (4).
  - The web is shipped, and `shipped` drops an empty column (5).
  - Set's file is compared whole (6).

The review is ready to write once the cited lines are checked against the commit. I'll confirm the test, the tool, and the dropped stub against the files, then give the findings.The new test and `fromOf` match the summary. Next I’ll check the dropped stub test, how the tool reads the server, and whether the clock steps can fail closed.Nothing blocks. Back's undo is exact file equality, and the tool matches the server's `full` / reserved `stretch` / `bases` shape. Two coverage gaps are worth fixing; the rest are small.

1. **SHOULD-FIX** — The grid is never read between Back and undo. `scripts/design-tool-scale.test.mjs:52-53`

   `tap back-space.md`, apply, then three clocks, then `tap undo`, and the next `tree` is `trees[3]` (`:88`), which only checks that the over-line matches the post-Set tree. `writes[1].after` (`:86`) is the file the proxy read when the `remove-token` reply came back (`scripts/design-tool-harness.mjs:95-97`). Scenario: Back restores `app.tamber` and the byte assert passes, while the grid still shows `token-over-space.md` and still hides `set-for-app-space.md` (`apps/design-tool/app.contract:1741-1746` hides Set once `source == "app"`). The removed stub test asserted both of those after Back, before any undo. The package path has the same hole in the other direction: `trees[5]` checks that `token-over-reading.measure` is gone (`:98`) and never checks that `set-for-app-reading.measure` is back.

2. **SHOULD-FIX** — Measured and guideline sources are never compared to the tree. `scripts/design-tool-scale.test.mjs:66-70`

   Full cell equality runs only for `space.md`. On the real table those cells are metric `grouped-row-inset` and terminal `cells` (`kernel/tables/schema.json:5981`). `radius.md` is measured on iOS (`ios-button-medium-corner`) and a guideline on Android (`m3-shape`) (`schema.json:5985`), and the server emits those kinds (`contract/cli/src/tamber_base.rs:26-27`). An overridden base still draws `token-base-*` (`app.contract:1736-1737`), so `token-base-radius.md-ios` and `token-base-radius.md-material` are on `trees[0]`. `cell()` already formats both (`:19`, `:44`). A `fromOf` regression that dropped `measured` or `guideline` (`apps/design-tool/base.ts:18-19`) would still pass. The stub's "Mac 6px (a stretch)…" sentence was stub fiction — the real Mac cell is measured — and should stay dropped.

3. **NIT** — The package name is a literal, because the describe lookup never hits. `scripts/design-tool-scale.test.mjs:45`

   Describe has no top-level `tokens`; package rows are `ir.tokens` (`contract/cli/src/tamber_base.rs:185-192`, marked in `contract/cli/src/tamber_packages.rs:330`). `described.tokens?.find(...)` is always undefined, so `pkgName` is `'@acme/reading'`, and the group, source, and over-line (`:73-74`, `:90`, `:95`) compare the grid to that literal. The tool itself reads `t.package` off the row (`base.ts:48-49`). A tool that ignored the wire and hardcoded `@acme/reading` would pass. The same literal coupling is `editValue` `'640px'` (`:91`), the written kind `: length` (`:94`), and the radius field `'12px'` (`:101`).

4. **NIT** — `radius.full`'s new source arm is a substring. `scripts/design-tool-scale.test.mjs:70`

   This commit's tool change is `case 'full': return 'the pill'` (`base.ts:22`). The test accepts any text containing `the pill`, so `iOS 99px · the pill` passes. iOS is shipped for this fixture, so `cell('radius.full', 'ios')` would be the same check `space.md` already gets. Also dropped from the stub, and still unchecked: `set-for-app-note` (`app.contract:1882-1883`) and `enabled`/`disabled` on `branch-value`. The disabled flag is the same `readOnly != ""` gate as the readonly text (`app.contract:1836`), which the package cell does check (`:90`).

5. **NIT** — The fixture comment names the web, and `shipped` keeps null columns. `scripts/design-tool-scale.test.mjs:24`, `:42`

   Only `app.json` is copied, and that manifest is iOS and macOS (`apps/theme-fixture/app.json:14-37`), so the shipped columns are `ios` then `mac`. `shippedOf` drops an empty column (`base.ts:58`); this `Set` does not. A null `column` is not in this fixture. If one were first, `shipped[0]` would be null and the over-banner (`:72`) would disagree with the tool, so the test would fail rather than pass.

6. **NIT** — "One line added" is a line-set filter. `scripts/design-tool-scale.test.mjs:81`

   A second copy of a line already in the design, including a blank line, is invisible to that filter. The lead's claim does not depend on it: `writes[0..2]` must be `transaction`, `transaction`, `undo` (`:79`), `writes[1].after` must be the original design (`:86`), and `writes[2].after` must be Set's bytes (`:87`). An extra startup transaction would shift those indices and fail the method list. Package Set and Back are full-file equality (`:94`, `:97`).

The clock steps fail closed. `clock data` after Set waits for the propose fetch before `state` and `apply`. Apply stays disabled until the proposal is ready and the key matches (`app.contract:1906`), so an early tap writes nothing and the byte asserts fail. The `clock +1` after apply is what runs `after(1, advance)` (`app.contract:849-850`) so the grid reloads before `tree`. The events poll holds 25s (`scripts/tamber-server.mjs:113`, `:589`) and is not a hold (`host/web-js/agent.js:249`), so a `clock data` overlapping it returns `settled: false` at 20s (`agent.js:365`). The drive records that as a diagnostic and continues (`scripts/agent.mjs:1207`). The design reload finishes during that wait. That is the same pattern as the other real-server design-tool drives, inside the 900s budget. The extra `clock +1` before `state` on the package path (`:55-56`) restarts propose, and the following `clock data` waits it out.

`full` matches the wire (`{kind: "full"}` → "the pill"). The `stretch` arm stays (`base.ts:20`) for a kind the server reserves and does not emit (`docs/tamber-protocol.md:417`; `tamber_base.rs:23-30` has no stretch arm). `explain`'s `bases` is unused. `loadBranch` feeds `names` to `inputsOf` (`app.ts:748`, `branches.ts:60-65`), and the protocol says `names` is as wide as the tokens reached, with `bases` the base-name subset (`docs/tamber-protocol.md:482-487`). This fixture's `radius.md = 12px` names no base, so `bases` is absent on that explain anyway.
