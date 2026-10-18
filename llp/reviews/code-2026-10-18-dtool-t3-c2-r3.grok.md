# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, the same throwaway worktree at `5c7138e94` (`2d59d8578` after the rebase).
- **Method:** one brief (sha256 `bb57ca48414d0065e045f26fdcb4fc834e22ec017d783fd3f5d5f01d390f91ae`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** nothing blocks; smaller points.
- **Disposition:** taken in `f044d508a`: a param's first when line is proposed only from the params of the proposal's own revision (else 'the design changed: reading it again'); a default that reads another param (named in full) is no line Only this mode can write, and the proposal says so; a param named in full is an expression's input in the branch editor (Follows / Pin); a length or number slider or stepper with no amount to start from (a rem, a token) is a field; the stub's duration param rebuilds. Kept: the two-operation edit's disabled Scrub has no sentence of its own (the button is disabled, nothing is sent).

Nothing blocks. At `5c7138e94` (which contains `471ce36a4`; later commits do not undo these fixes), every round-1 and round-2 correctness point is fixed, and the fix matches the server.

`~/x2idea/SIGNOFFS.md` is not on this machine, so the 10:36 theme2 / review2 text was not read. The kept Follows / Pin behaviour matches D6 in `llp/1122-design-tool.rfc.md` and the disposition recorded on the round-2 reviews.

## Earlier points

Round 1 holds.

- An icon param no longer reaches `numberOf`. `paramRow` passes a string into `controlOf`, then forces `control: 'icon'` and shows the sources from `values[0]` (`apps/design-tool/components.ts:45-56`, `72-74`). `DialControl` has no input for that control (`app.contract:1642-1657`). The panel test expects `sf star · material star` and no glyph field.
- A panel edit clears `draftFor`, `scopeOwn`, and `editWhen` (`app.contract:726-739`, and the same in `dialStepped`, `dialText`, `resetParam`). `draftStale` requires `draftFor != ""` (`app.contract:654`), so editing `MapCard.zoom` while `Card.pad`'s branch is open does not disable Apply. The rung test at `app.test.contract:1133-1138` covers that pair.
- "Only this mode" on a param with no line prepends the base line. `opsOf` does that when the settled read has `set === null`, using `p.default` and the short name (`app.ts:278-281`, `165-167`). `Card.fill` is that case (default `accent.fill`). The server still refuses a lone `when` (`tamber/src/edit/params.rs:26-30`). Apply, undo, and redo keep the reviewed pair (`app.ts:864-872`, `1006-1016`).
- A duration slider or stepper writes `` `${n}ms` `` (`app.contract:740`, `755`). `MapCard.pan` shows `0ms – 1000ms · step 50ms`. A select or a token field keeps the written string (`components.ts:44-45`, `56`). `Card.fade` (`control=token`) stays a field.
- The panel is built only for the app design, including when that file is chosen by name (`app.ts:812-813`: empty `components` when `file` is set and is not `designFile`). The native mark uses hello's design file (`components.ts:93`), so `MapCard` stays `app.tamber#MapCard`. An inspector param edit sets `file = ""` (`app.contract:807`), so the next read is the app design.
- A scrub cancels when the edit identity changes, including `when`. Panel actions cancel unless the open scrub is already this op, this name, and `editWhen == ""` (`app.contract:728`). `cellText`, `partEdit`, and `onlyThisMode` cancel unless the open scrub is already that branch (`app.contract:844`, `861`, `875`).
- A token option matches by trimmed name. Colour folding runs only when neither side is a token name (`dials.ts:134-135`). Ordinary dials still call `controlOf` without `takesTokens` (`dials.ts:172`).
- Every real-server setup that copies the stub design also copies `STUB_APP` (`scripts/design-tool.test.mjs:160`, `303`, `387`, `441`, `513`, `1373`).
- A colour param whose value is a token still says "Follows accent.fill" until Pin (`app.contract:1332-1346`). "Only this mode" in that view writes the declared value `accent.fill` as both the base line and the `when` line. That is the kept disposition.

Round 2 holds.

- The in-flight read no longer drops the base operation. `loadDesign` still does `++issued` before the snapshot exists, but `opsOf` reads `appParams`, and that map is replaced only when a read of the app design settles (`app.ts:797-801`). A read of another file, and an overtaken read, leave it alone. Apply writes `review.ops` (`app.ts:869-872`) and refuses when that list is missing (`app.ts:869`). Redo replays `top.ops`.
- `ms` is stripped only for a duration slider or stepper (`components.ts:44-47`). A select keeps `c.value` (`components.ts:56`). `duration.fast` and a token named `brand.ms` stay intact on a select or a field. The server's default `control` is `"token"` (`contract/cli/src/tamber_params.rs`), so the `!p.control && range` slider clause does not run on a real `describe`.
- "Only this mode" after a panel edit starts a branch draft. The keep-the-typed-value condition now requires `draftFor != ""` (`app.contract:880`). A panel edit sets `draftFor` to `""`, so the branch value is taken and `draftFor` becomes `branchKey`. A value already typed in the branch editor is kept.
- Two operations are not scrubbable. `proposeEdit` sets `scrubbable` from `ops.length === 1` (`app.ts:862`), and Scrub live is disabled on that flag (`app.contract:1434`). The new test (`app.test.contract:1144-1172`) opens `Card.fill` on `2-dark`, presses "Only this mode", and expects a Live chip with Scrub live disabled.

## Smaller

The Live two-op edit still has no sentence explaining the disabled Scrub live button. `scrub-unavailable` is rendered only when the chip is not Live (`app.contract:1438-1439`). A colour `when` line is Live, so "Only this mode" on `Card.fill` shows a disabled Scrub live and nothing else. `scrubFrame` still recomputes `opsOf` and, if a scrub ask arrived with two operations, would set "Review the change before scrubbing it." (`app.ts:909-917`). The button no longer sends that ask. Pressing it does not write the `when` line alone.

The two-op write is still untested on the wire. The new test checks `editWhen`, the Live chip, and the disabled button. It does not read `review.ops` or Apply's body. The stub's `propose` diff is `ops[0]` only, and its `transaction` returns an inverse without applying, so a missing base line would still look fine there. The `12px` assertion at `app.test.contract:206-213` is the same shape: after the gap dial moves to 12, "Only this mode" on `space.edge` expects `12px`, which is also that token's committed value.

Reset of the panel line is one `unset-param` with an empty `when`. The server removes that line and every `when` line under it (`tamber/src/edit/params.rs:108-115`). That matches "the default applies again" for a param that has mode lines.
