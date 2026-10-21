# Code review: LLP 1122 T1 commit 4 (editing and undo), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `3ca4c6160` (commit `1eab66090`, with commit 3's round-1 fixes after it).
- **Method:** one brief (sha256 `5d273747773b55e3738f104135723def78d8eb4ad1e74b4a8c6d56cbd6725d2d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** do not land as is (3 high, 4 medium, 3 low).
- **Disposition:** all taken, as in the astra record; also: the branch panel shows only for the cell chosen, and a no-op Apply leaves redo be.

---

Light cells edit the wrong branch, and "Only this mode" on a light column writes a `when` the checker will tie or that covers more than that tuple. A write on "The app's design" can also carry the wrong file's digest.

## High

**Light cells take a `when` the checker does not use for the light side** — `apps/design-tool/app.ts:275`

`loadBranch` treats an `under scheme=dark` axis as fixed at `values[0]` on a light column, then keeps every `when` whose conditions match, most axes named, and on a tie the first in file order. The checker's light side is `select(..., base: true)` (`tamber/src/check.rs:595`): any `when` that names an under-dark axis is skipped, and the light colour comes from what remains (or the default). A `when` that names that axis's default value therefore wins in the tool and loses in the checker.

On the Bluesky design (`dark-style = dim | black`, Dim is the default), "Only this mode" on the Dim column of `contrast_0` writes `when dark-style=dim`. The Light cell then opens that `when`. The swatch is still the default's light side. Changing the dark hex rewrites Dim; changing the light hex is refused with `tamber-under-light`. The same split happens for any hand-written `when dark-style=<default>`.

Skip under-dark `when`s on a light column, the way `select` does with `base`. If two remaining `when`s name the same number of axes, show the ambiguity instead of taking `[0]`.

**"Only this mode" on a light column omits under-dark axes** — `apps/design-tool/app.ts:283`

`own` is built from axes for which `applies` is true, so a light column leaves `dark-style` out. That `when` matches every value of the omitted axis, and it ties any existing under-dark `when` of the same length.

On the fixture, the Light cell of `surface.page` proposes `when density=regular`. That holds for dim+regular as well, where `when dark-style=dim` also holds and names one axis, so the checker reports `tamber-when-ambiguous` and the edit never lands. The Light cell of `surface.inset` has no competing `when`, so the write succeeds and also covers Dark · dim, which is not that tuple. The button is hidden only when `when === own`, so this Light cell still offers it (`exact` is false). The integration test only applies the dark case `cell-surface.card-1-dark`, where every axis is included and the `when` really is unique.

Name every axis at this column's tuple values, including an under-dark axis at its default. That `when` holds for one tuple and cannot tie. On a light cell it only changes the dark side (`under_value`); say that, and don't offer the button when the light swatch would stay on the base branch.

**The default file's write attaches whichever digest sorts first** — `apps/design-tool/app.ts:304`

With the file picker left on "The app's design", the operation has no `file`. `propose` returns every design file (`files_json` of the whole snapshot). `serde_json`'s map is ordered by path (`contract/cli` does not enable `preserve_order`), and the write sends only `Object.keys(reviewed.files)[0]`. The operation still edits the app's own design. A missing digest is a conflict (`contract/cli/src/tamber_serve.rs:714`), and `byWhom` then says that file "was changed on disk".

`app.tamber` plus `accent.tamber`, picker left on the default, any dial Apply: the digest sent is `accent.tamber`'s, `app.tamber` is reported stale, nothing is written. `app.tamber` plus `brand.tamber` works only because `app` sorts first. Choosing the file in the picker sets `op.file` and avoids it.

Send the whole `reviewed.files` map. Digests for files the operation does not edit are not stale-checked. Record `ok.written` from the reply for redo.

## Medium

**A dropped `propose` still overwrites `reviewed`** — `apps/design-tool/app.ts:297`

Digests live in a module global, not on the proposal value the runtime newest-wins. The web target still runs the older call, and that call assigns `reviewed` after the visible proposal has moved on. The idle path (`!kind`) returns without clearing it. Apply then either hits "Review the change before applying it" or sends the older digests for the same key and conflicts.

Type `210` into the hue stepper: three `propose`s overlap, an earlier reply lands last, the chip shows 210, Apply refuses.

Bump a generation in `proposeEdit` and ignore a result that is no longer current. Clearing the edit should clear `reviewed`.

**Undo and redo drop the entry on any error** — `apps/design-tool/app.ts:314`

`undoLast` pops before the call and, on any error, does not push the entry back. The comment assumes a conflict, which can never apply; a timeout or a down server is the same branch, so the inverse id is gone while the edit is still in the file (or was undone, with nothing on the redo stack). `redoLast` pushes back only when `hello` fails. A transaction error drops the redo, and the message is the raw server string, not `byWhom`.

Undo once, let the request time out: Undo shows 0, the hue line is unchanged, there is no way to send that inverse again. Redo after another client writes between `hello` and `transaction`: the entry is gone and the text does not name them.

Put the entry back unless `error.conflict` is set. On a conflict, use `byWhom` for redo as well.

**A refusal is the word "refused"** — `apps/design-tool/app.ts:55`

`call` keeps `error.message`. For a checker refusal the server sets that to `"refused"` and puts the reason in `error.diagnostics`, which the proposal panel never reads. Pinning `accent.soft` to a literal makes `lift` move no token (`tamber-dial-inert` in `apply_all`'s `check`). There is no remove-dial operation, so that pin cannot land. The test taps Pin and asserts the slot; it never Applies. The same blank "refused" is what an ambiguous "Only this mode" and `tamber-under-light` show.

Show each diagnostic's file, line, and message. When a pin would inert a dial, name that dial in the proposal.

**The branch panel edits the retained resource** — `apps/design-tool/app.contract:354`

The panel is shown whenever `branch.token != ""`. A new `loadBranch` keeps the previous cell until `explain` returns, and `cellText` / `onlyThisMode` copy `branch.token`, `branch.when`, and `branch.own` at that moment (`app.ts:212`).

Select `surface.page`'s Dim cell, then quickly select `surface.card` and press "Only this mode" before the second `explain` returns. The proposal is `surface.page`'s `when`. When the new branch arrives, the heading says `surface.card` and Apply still writes `surface.page`.

Show the panel only when `branch.token == selToken` and `branch.column == selColumn`.

## Low

**A dial in "Follows …" is opened as a token** — `apps/design-tool/app.contract:360`

Inputs are `names` and `dials` that occur in the value (`app.ts:287`). The regex boundaries are right (`accent` does not match inside `accent.soft`). Every button calls `selectCell`, and `explain` has no token `lift`. Clicking `lift` on `accent.soft` replaces the branch with "no token `lift`". Focus that dial's control instead.

**The gate tests don't reach the broken cases** — `scripts/design-tool.test.mjs:179`

`changed` is a bag of lines, so a reordering or a second copy of a line can look like a one-line edit. Undo's `toBe(original)` is a real byte check, including the unspaced `text.primary` file, and the Dim cell does write `when dark-style=dim`. Nothing selects a light column, applies "Only this mode" where an under-dark axis is omitted, pins a value through Apply, or writes while a second `.tamber` sorts before the design file.

**A no-op Apply clears redo** — `apps/design-tool/app.ts:310`

A transaction with a null `inverse` (the text did not change) still sets `redoStack.length = 0` and pushes no undo. Undo a hue edit, Apply the same hue again: Redo drops to 0.

Clear redo only when an inverse was stored.

Verdict: The dark-cell edit and the byte-exact undo hold; light-column branches, light "Only this mode", and default-file digests do not, so this should not land as is.
