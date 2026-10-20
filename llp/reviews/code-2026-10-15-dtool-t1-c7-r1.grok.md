# Code review: LLP 1122 T1 commit 7 (T1a, import review), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f42acb24d` (commit `cd635dade`, with later commits' fixes).
- **Method:** one brief (sha256 `4c3b030489ffa8aa55ed3101d8750497f2216f6f7b85b3b53440cbeeda99d0f0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 6 medium, 3 low).
- **Disposition:** taken: an edit carries the ticks to the new proposal, and a recomputed import's kept offers stay on; operations by kind; every mode editable; a note-only offer kept through a recompute (on only if it has items); the recompute notice on the import card; the edit bound to its run, a failed drop said; derived rows marked outside the count; a recomputed card labels the count as the source import's; the grant narrowed to app:/tmp/picked; shadcn's axis note names `tamber-role-axis`; tests as in the astra record.

---

Judged at HEAD `f42acb24d`. The import-review behavior is the T1a commit; later commits on these files do not rewrite it. The shadcn gate path holds: the picker grant can read `app:/tmp/picked/…`, one file is `{source,name}` and several are `{sources}`, a fresh review leaves overrides off, ticking only AccentColor recomputes and the second accept is one transaction, and undo restores the original bytes.

## High

**Editing a mapped value drops every tick, and the note tells you to accept.** `apps/design-tool/app.ts:935`

`editImport` copies the ops, sets `ops[index].value`, and `propose`s them with `hold: true`. That call passes no replaces id (`scripts/tamber-server.mjs:385`), so the new proposal is a fresh import. Ticks live in `picked` under the old proposal id (`apps/design-tool/app.contract:727`, `:740`). `reviewOf` turns every offer on only when `replaces` is set (`apps/design-tool/app.ts:494`), and `acceptImport` treats a non-recomputed proposal as "tokens in, offers out" (`apps/design-tool/app.ts:926`). The note at `app.ts:945` says to review and accept.

Leave out fifty Tailwind tokens, tick AccentColor, change one value, and press Accept as the note says. The checkboxes are back to the fresh-import defaults, so the write contains every mapped token and no role override. The same indices would still be valid: the edit does not reorder ops.

Carry the flipped indices onto the new proposal id (or patch the held proposal in place), keep offers that were ticked on, and change the note so it names the selection that will be written.

## Medium

**The token list treats every op in the token slice as an `add-token`.** `apps/design-tool/app.ts:483`

`reviewOf` reads `op.value` and `op.whens` only. `reconcile` emits `add-axis` first (`tamber/src/import/mod.rs:110`), and a re-import emits `set-token` with a singular `when` (`mod.rs:148`, `contract/cli/src/tamber_import.rs:259`). The accept parser ignores a `value` on `add-axis` (`contract/cli/src/tamber_serve.rs:1155`).

On the Figma fixture the first row is the axis `primitives-completed` (`brutal-theme | modern-theme`), shown as a token with an empty value. Untick that blank row and the mode conditions are written with no axis. Type a value into it and the reviewed text is dropped on accept. Re-import a token that already exists and each mode update is another row labeled `default`; unticking one drops that mode's `set-token` or `unset-token-when`.

Branch on `op`. Show an axis as its name and values, without a value field. Group `set-token` / `unset-token-when` by name and `when`.

**A non-default mode is swatches and a label.** `apps/design-tool/app.contract:729`

The modes loop paints two swatches and, when the label is not `default`, the when text. The text field binds only the mode labeled `default` (`app.contract:735`). D11 asks for the value in each mode, each editable before accept.

Figma `color.brand.radish` is `#ffbe16` on Brutal Theme and `#ffcbb7` on Modern Theme, so the Modern value is a `when` on the `add-token`. The hex can paint as a swatch; the string is not shown and cannot be edited. A `light-dark(figma.color.…, …)` reference does not paint, so that mode value is invisible.

Print each mode's value and edit that entry (`op.whens[i].value` for a when, `op.value` for the default).

**A role the importer cannot offer disappears on the review you accept.** `apps/design-tool/app.ts:494`

The filter keeps an empty offer only when `!p.replaces && o.note`. A partial accept copies those offers with `items: []` and the original note (`scripts/tamber-server.mjs:517`). `on: !!p.replaces` would also mark a note-only row checked if it were shown.

Import shadcn into a design where `shadcn.primary` varies by an app axis, so AccentColor comes back with a note and no ops. Tick another override and Accept. The confirming card, the one that writes, no longer shows the `tamber-role-axis` note.

Keep an offer when `items.length > 0 || o.note`, and set `on` only for an offer that has items.

**The import card never says the proposal was recomputed.** `apps/design-tool/app.contract:719`

`Recomputed from ${h.replaces}…` is in the non-import branch (`app.contract:754`). D10 says the tool shows the recomputed transaction and the person accepts that. The import branch's only signal is `write-note` (`app.contract:694`), above the inbox.

Tick AccentColor and press Accept. Nothing is written. The new card looks like the first review with that override already on. A person who misses the note treats the import as finished, or accepts without seeing that this card is the one that writes.

Show the replaces line on the importer card, and say that the kept overrides are on because they were ticked.

**An edit is not bound to the server run that holds the proposal.** `apps/design-tool/app.ts:942`

`acceptHeld` passes `heldToken` (`app.ts:892`). `editImport` calls `propose` unbound, then `drop`s the old id and ignores the result (`app.ts:944`). An unbound call takes whatever token is on port 7740 now.

Restart the server, or let another app's server take the port, then edit a mapped value. The edited ops are held on the new run. A failed drop leaves the old proposal in place as well, and the note still says to accept.

Pass `heldToken`, refuse the edit when the run has changed, and surface a failed drop.

**The gate fixture's count and its unmapped list disagree on screen.** `apps/design-tool/app.contract:744`

`reviewOf` sets `derived` when `block === '(derived)'` (`app.ts:495`) and the view never reads it. shadcn excludes those rows from `reported` (`tamber/src/import/shadcn.rs:316`). Neutral is 47 source values = 46 mapped + 1 reported, while "Not mapped (5)" lists `--radius` and four derived sizes.

The equation goes red or looks false against the heading, on the fixture the gate uses. The why text mentions "derived from --radius" only on the size rows.

Mark `(derived)` rows and say they sit outside the equation.

**The tests prove the shadcn happy path and a note string.** `scripts/design-tool.test.mjs:552`

The real-server test reads `resources.changes.held`, not the tree. It checks the shadcn count, 23 mapped rows, five offers off, five unmapped names, the AccentColor recompute (`replaces`, that offer on), the written file (AccentColor and `shadcn.background` present, `-exact-background` absent), and undo back to the original bytes. Tailwind is `countOk` and `mapped.length > 200`. Figma is the count string and `unmapped.length === 64`. It never unticks a token, edits a value, or asserts `writes[0].after` is still the original file. The stub test (`apps/design-tool/app.test.contract:339`) checks the three lists, a role-axis note with no checkbox, and that the edit note quotes the typed value and a new id. It never reads the new proposal's op, and the stub's partial accept does not remap review the way `tamber-server.mjs` does.

Assert the edited op on the new proposal, a token left out, `writes[0].after === original`, the Figma axis row, a when-mode value, and a real `tamber-role-axis` note before and after recompute.

## Low

**One refusal note never names the diagnostic.** `tamber/src/import/shadcn.rs:371`

The "varies by an app axis" offer cites LLP 1121 D5 and omits `tamber-role-axis`. The existing-role note includes it (`shadcn.rs:450`), as does `split_roles` (`tamber/src/import/mod.rs:491`). The tool prints `o.note` as sent. Put the diagnostic id in that template.

**The import grant is the whole scratch directory.** `apps/design-tool/app.ts:17`

`fs.read app:/tmp` covers the picker copies under `app:/tmp/picked/`, and also anything else the host places in `app:/tmp`. Narrow it to `app:/tmp/picked`.

**A recomputed review keeps the original count.** `scripts/tamber-server.mjs:515`

The recompute copies `counted` and rewrites `tokens` to the number of kept token ops. Leave out ten tokens and the card still says `47 source values = 46 mapped + 1 reported` over a shorter list. Recompute `counted` from the kept ops, or label the equation as the source import's.

Verdict: The shadcn gate path holds, but an edit before accept writes a different selection than the one reviewed, and the Figma review and the tests both miss that.
