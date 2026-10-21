# Code review: LLP 1122 T1 commit 7 (T1a, import review), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `905b0a69b` (commit `cd635dade` and its round-1 fixes).
- **Method:** one brief (sha256 `b71fbea7eb6d2b12adf97adcb474552bdf326727df60a053708cccf2bd662cbe`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 1 medium, 3 low).
- **Disposition:** taken but 4 in part: an accept draws its baseline from the same mark the card does (a recomputed import, or an edit of one: `kept`); the checkboxes are off while a write or an edit is in flight; an edited import names the one it replaced (`from`) and shows the notice and its diff; the role-overrides heading says when they are as ticked; the server refuses several stylesheets (400) and the tool says so; an importer's diagnostics are in the wire's shape, and a string one is shown whole; an unmapped value names its block, keyed by position; an import that maps nothing says so; tests edit the recomputed card through to the bytes, and Figma's references and other modes. 4 in part: a reference is followed to a token the import adds or the design has, and painted; one to anything else stays unpainted.

---

Round 2 of the T1a import review, judged at HEAD `905b0a69b` with the T1a change in `42dd1f1d1`. Request changes: one high remains. Editing the confirming card inverts the role overrides that card shows as on.

## High

**Editing a recomputed import inverts its role overrides.** `apps/design-tool/app.ts:1013`, `apps/design-tool/app.ts:1018`

`reviewOf` turns offers with items on when the proposal has `replaces` or the review has `kept` (`apps/design-tool/app.ts:506`, `apps/design-tool/app.ts:513`). The checkbox follows that: checked when `o.on` disagrees with `picked` (`apps/design-tool/app.contract:780`). `editImport` sets `kept` when the card had `replaces`, and copies only ids already in `picked` (`apps/design-tool/app.ts:1035`, `apps/design-tool/app.ts:1042`). Client `propose` always holds with `replaces` omitted (`scripts/tamber-server.mjs:386`), so the new card has `kept` and no `replaces`. `acceptImport` then sets `recomputed` from `held[].replaces` alone:

```1013:1018:apps/design-tool/app.ts
    const recomputed = !!held.find(h => h.proposal === proposal)?.replaces;
    // ...
    const items = ops.map((_, i) => i).filter(i => (i < tokens || recomputed ? !flipped.has(i) : flipped.has(i)));
```

With `recomputed` false, an offer index is sent only when it is in `picked`. A kept offer starts on, so leaving it checked stores nothing in `picked`, and unticking it does. The box and the items disagree in both directions. Token leave-outs still stick, because those indices use the same rule on every card. A second edit keeps `kept`, so the inversion stays.

Import the shadcn fixture, tick AccentColor, leave `shadcn.foreground` out, and Accept. Nothing is written; the new card has `replaces` set and AccentColor on, which is the path `scripts/design-tool.test.mjs:589` checks. Then change one hex on that card. AccentColor is still checked, the orange recompute line is gone, and Accept sends the token ops without AccentColor. That partial accept recomputes again with the offer's items emptied; the offer has no note (`tamber/src/import/shadcn.rs:411`), so the next card drops the row and the write never contains `AccentColor = shadcn.primary`. Untick AccentColor on that edited card and Accept writes it. The same split hits an Astryx proposal after a partial accept. Tailwind and the Figma fixture have no offers, so they stay on the path the test covers. Undo is still one step; it undoes whichever wrong set was written.

In `acceptImport`, treat `r.kept` the way `reviewOf` does (`replaces` or `kept`), so a checked kept offer is sent and an unticked one is not. Carrying `picked` ids cannot do this: the implicit on-state is not in `picked`. Extend `scripts/design-tool.test.mjs` to edit the confirming card and assert both directions against the written bytes.

## Medium

**That same edit hides the recomputed diff.** `apps/design-tool/app.contract:758`, `scripts/tamber-server.mjs:386`

The import card prints the recompute notice and `h.diff` only when `h.replaces` is set (`apps/design-tool/app.contract:758`). The edited hold is a new plan — `hold` fills `diff` — and D10 says the person accepts that diff. After the edit above, the count still begins `the source import:` because `kept` makes `reviewOf` treat the card as recomputed (`apps/design-tool/app.ts:506`), while the orange line and the diff are gone. The note says to review it and accept (`apps/design-tool/app.ts:1043`). The heading still says the overrides are off (`apps/design-tool/app.contract:776`) while AccentColor is checked.

Show the notice and `h.diff` when the review is `kept`, and keep the predecessor id in the review the edit sends. Client `propose` cannot set the proposal's own `replaces`.

## Low

**The override heading stays "off until ticked" on the card that writes.** `apps/design-tool/app.contract:776`

On the legitimate confirming card (`replaces` set, before any further edit) AccentColor starts checked and the orange line says the kept overrides are on. The heading above the checkbox still says they are off. Say they are on when the card is a recompute.

**An import that mapped nothing says the design already has every mapped value.** `scripts/tamber-server.mjs:622`

`importHeld` returns that sentence whenever `ops` is empty, and the client shows it with the count and the unmapped rows (`apps/design-tool/app.ts:1002`). A sheet that is only `--radius: 0.5rem` maps nothing, yet the note says the design already has every mapped value. The count and the `--radius` row are on screen and contradict it. For a real re-import of an unchanged source the sentence is right, and offers are still withheld when there are no token ops. Say "nothing to propose" and leave the "already present" clause for a re-import whose mapped count is non-zero.

**The gate test never edits the confirming card or a non-default mode.** `scripts/design-tool.test.mjs:589`, `scripts/design-tool.test.mjs:625`

The real-server test does prove the fresh path: mode 0 is edited, token 1 is left out, AccentColor is ticked, the partial accept leaves the file untouched, the second accept writes that selection, and undo restores the original bytes. The Figma assertion is `modes[1].value !== ''` under a comment that the other mode is editable. `editImport` writes `op.whens[mode - 1].value` only when `mode !== 0` (`apps/design-tool/app.ts:1033`), and nothing types into that input. A broken when-edit would stay green, as would the inversion above. Type a non-default mode and edit the card whose `replaces` is set.

Round 1 holds on the paths it fixed. A fresh import carries explicit ticks across an edit, lists ops by kind, shows and edits each mode, keeps a note-only offer through recompute (`tamber-role-axis` is named at `tamber/src/import/shadcn.rs:371`), shows the notice and the diff while `replaces` is set, binds the edit's propose and drop to the holding run, marks derived rows outside the count, reads picks under `fs.read app:/tmp/picked`, takes one stylesheet or several DTCG files, and still shows the count and unmapped rows when there is nothing to hold. A reference stays text, unpainted.

Verdict: request changes — one high: editing the confirming card inverts the role overrides it shows as on.
