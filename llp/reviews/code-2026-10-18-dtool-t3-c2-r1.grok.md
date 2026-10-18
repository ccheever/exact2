# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e7c0a682f`.
- **Method:** one brief (sha256 `f7cc3c2ac9b04484059e9d6aeed2b3c331d3f4222621e52a5e16d99047c87fe3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** request changes (2 high, 2 medium).
- **Disposition:** all taken in `8f3231296` (and `e1aa1a4c9` for params written by their short name): an icon param shows its sources, read only, and no longer throws; a duration's slider and stepper write ms, and kinds the dials have no control for take a text field; a panel edit clears the branch editor's draft and mode scope, so it is never a stale draft, and a scrub on another branch of the same param is cancelled; Only this mode on a param with no line writes its line first (and has no scrub); a token option is matched by its name, a literal by its meaning; the panel is shown on the app's design only, its native mark by hello's design file; the real-server setups that copy the stub's design now copy its app too. Tests: the panel's other kinds and a panel edit beside an open param branch. Kept: a colour param whose value names a token shows 'Follows … / Pin' in the branch editor, as a token's does; pinning a literal there is D14's pinned param.

Two bugs block this commit. Two more break the param rung on paths the tests never open. The op plumbing, the app-design re-read, and the dial-versus-param name split hold.

## Blocking

### 1. With another design file open, the panel describes that file and writes the app design

`loadDesign` builds the panel from the file on screen:

```782:782:apps/design-tool/app.ts
        components: componentsOf(ir.components, file || designFile, names),
```

```71:71:apps/design-tool/components.ts
      native: c.identity.startsWith(`${design}#`),
```

`describe` attaches the app's components for every file (`contract/cli/src/tamber_serve.rs:788-799`), and `set`, `pinned`, and `values` are taken from the file that was parsed (`contract/cli/src/tamber_params.rs:103-109`, `235-238`). A native identity is always the app design's home, `app.tamber#MapCard` (`contract/lower/src/params.rs:89`). `set-param` omits `file`, so the server writes the app design (`apps/design-tool/app.ts:272-274`, `contract/cli/src/tamber_ops.rs:36`).

Open `brand.tamber`. The panel still lists Card and MapCard. MapCard's identity is `app.tamber#MapCard`, the prefix is `brand.tamber#`, and the native mark is gone. `brand.tamber` has no `params` block, so Card.pad shows `8px` and "the default" even though `app.tamber` has `Card.pad = 12px`. Set it to `20px` and Apply. The line lands in `app.tamber`. The `changed` event moves `filesAt` (`app.ts:619-623`, `app.contract:569`, `tamber_serve.rs:679-684`), `loadDesign` reads `brand.tamber` again, and the panel stays on the default. The swatch is folded with `brand.tamber`'s tokens too.

The empty file and an explicit `app.tamber` match `design.home`, so the mark and the `set` line are right there. The file picker appears once hello lists a second design file (`app.contract:1188`), and the panel renders whenever `design.components.length > 0` (`app.contract:1219`). The stub refuses `describe` of anything but `app.tamber` (`stub/tamber-stub.ts:156`), so the tests never see this.

### 2. The generalised dial actions treat two `set-param`s as one edit when only the `when` differs

`dialNumber`, `dialStepped`, and `dialText` cancel a scrub unless `editKind == op` and `editTarget == name`, then force `editWhen = ""` (`app.contract:719-754`). `dialReleased` Keeps on that same pair (`739-745`). A scrub's identity is `[file, kind, target, when]` (`app.ts:856`, `871`, `884`). `draftStale` is also suppressed while `scopeOwn` is set (`app.contract:647`), and these actions never clear `scopeOwn`. The branch title still reads "only this mode" from that flag (`app.contract:1286-1287`).

Same param, two whens. Card.pad already has a line. Open it on column `2-dark`, press "Only this mode", and scrub. That session is `set-param` of `app.contract#Card.pad` with `when` `density=regular`. Drag the panel slider. The op and the name match, so the scrub is not cancelled and `editWhen` becomes `""`. The next frame returns "This scrub is of another edit" and leaves `want` alone (`app.ts:871`). Releasing the slider calls `releaseScrub`, which refuses the same mismatch before writing (`app.ts:884`). Keep stays disabled because `scrub.ok` is false (`app.contract:1382`). The preview stays on the mode value until Cancel.

Same param, no scrub, wrong line. After "Only this mode", drag the slider to 16 and Apply. `scopeOwn` is still true, so `draftStale` stays false and Apply is enabled. The op's `when` is empty, so the write is `Card.pad = 16px`, the param's own line. The title still says "only this mode". The diff shows the unscoped line; the mode flag and the op have come apart.

A dial scrub and a param scrub of the same short name do cancel each other. The check includes the op, and the param control's name is the full name (`app.contract#Card.pad`), not `Card.pad`.

## Param rung

### 3. "Only this mode" on a param that has no `params` line is refused

The button is explicit and off by default (`scopeOwn` starts false; `app.contract:1340-1342`, `835-846`). For a color or length param whose winning `when` is `""`, `branchOf` still computes a non-empty `own` on a moded column (`app.ts:182-185`), so `exact` is false and the button shows. Apply sends one `set-param` with that `when`. The server refuses a `when` when the param has no line yet:

```25:30:tamber/src/edit/params.rs
        let Some(p) = entry else {
            if !when.is_empty() {
                return Err(refuse(format!(
                    "`{name}` has no line in the design's `params` block: set its value before one of its `when` lines"
                )));
```

Card.fill is that case: `stub/dials.tamber` has no `Card.fill` line, and `stub/dials.json` has `set: null`. Open the param rung on column `2-dark`, press "Only this mode", Apply. The proposal error is `` `app.contract#Card.fill` has no line… ``. Card.pad already has a line, so the same gesture works there. A duration or easing has `own === ""`, so the button stays hidden. The client never writes the base line first. The stub's `transaction` returns an inverse for every op (`stub/tamber-stub.ts:180`), so a contract test would not catch the refusal.

### 4. A color param whose value is a token has no value field

`loadBranch` sets `expression` from `inputsOf` (`app.ts:804-809`, `196-204`). `accent.fill` is a token, so Card.fill is an expression. `partsOf` has no color case and returns null (`parts.ts:66-82`), so `typed.ok` is false. The editor then takes the follow branch and draws no value input (`app.contract:1289-1303`): "Follows accent.fill: edit those, or pin a value here." Pin flips to "Pinned here: no longer follows accent.fill" and only then shows the field (`1314-1339`). A param value that names a token is an ordinary `set-param`. The panel already shows it that way: Card.fill is a color field whose text is `accent.fill` (`components.ts:27`, `43-46`).

The inspector half of "beside, never the default" is right: rung `param` is the filled "Edit Card.fill", and the alias is the secondary "Edit the token accent.fill instead" (`app.contract:1109-1114`). The rung test stops at Card.pad's typed fields and then jumps to the token from the inspector (`app.test.contract:1104-1117`). It never opens Card.fill's branch.

Switching cells is safe. `editFromSelection` and `selectCell` clear `editKind`, `rawFor`, `partFor`, and `scopeOwn` (`app.contract:771-786`, `793-807`), and `draftStale` / `shownValue` / `fieldDraft` / `partsStale` all require `editKind == branchOp` (`646-668`). A leftover `set-token` draft is not applied as `set-param`.

## What holds

- `opOf`, `label`, and `opText` cover `set-param` and `unset-param` (`app.ts:272-282`, `590-609`). `proposeEdit` allows an empty value for `unset-param` (`823`). Apply and undo go through `writeEdit`, one inverse, one stack entry (`845-847`). Reset is one `unset-param` (`app.contract:756-765`). The panel lives in `components.ts`. The param is the primary rung button.
- On the app design, `filesAt` does move after a real `set-param`. `commit_reply` emits `changed` when the reply has an inverse (`tamber_serve.rs:679-684`), `noted` bumps `fileChanges` and `lastFiles` (`app.ts:619-623`), and `advance` copies that into `filesAt` (`app.contract:687`). `loadDesign` re-reads `set`, `pinned`, and the slider's number together. A scrub frame does not emit `changed`, so the panel stays on the committed value until Keep, same as a dial.
- The server's `scrub-fold` parses `set-param` and plans it (`contract/cli/src/tamber_scrub.rs:42-92`). `scripts/tamber-scrub.mjs` refuses `class !== "live"`. A duration or easing param is rebuild because the plan marks motion tokens rebuild (`tamber/src/session.rs:217-228`). The client allows a scrub only for `set-dial`, `set-token`, and `set-param` with a value, and only when the chip is live (`app.ts:854`, `app.contract:1390-1396`).
- `controlOf`'s new `same` (`dials.ts:133`) does not change a dial. Dial selects are still literal-filtered (`panelOf` does not pass `takesTokens`). The `numberOf(o) !== null` guard is what stops a param select from matching every token-named option to a non-numeric value. For Card.tone, `#eeeeee` matches no option and is prepended; `surface.card` and `surface.inset` stay. A length param's range is the server's numbers with `px` appended (`components.ts:28`, `36`), which is what the pad row shows: `0px – 24px · step 1px`. A color param whose written value is a token uses `values[0]` for the swatch and falls through to it for `number` (`components.ts:39-46`). `control === "token"` forces the text field (`43`).

## Tests and the stub

The three new tests match the app-design happy path. They miss the failures above, and two assertions pass for a narrow reason:

- `classOf` and the scrub `ask` classify rebuild by `op.name` only (`stub/tamber-stub.ts:37-50`). The fade case expects the target and the "Rebuilds" chip (`app.test.contract:138-143`) and does not expect `editKind == "set-param"`. Any op whose name is `app.contract#Card.fade` chips Rebuilds. The pad case does expect `editKind` before the chip (`127-130`), so that half is real.
- `transaction` returns an inverse and does not emit `changed` or update `FILES` (`stub/tamber-stub.ts:180`). Undo `(1)` then `(0)` (`150-153`) shows that the client pushed one stack entry. It does not show that the wire op was `unset-param`, or that the panel's "set here" line cleared. `describe` is the canned `dials.json` and only for `app.tamber`.

The component-panel test's native line is the empty-file path, where the prefix is hello's `app.tamber`. That assertion is right for the file it opens.
