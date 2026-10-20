# Code review: LLP 1122 T2b step 6 (the inspector panel), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `3af389f4d` (compacted once mid-review).
- **Method:** one brief (sha256 `30bf48a7e6f7dcad994d26fb739573ae7659b1af8638092441561516502d8f1f`), shared by both reviewers, blind to the other review, with rounds 1 and 2's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** taken, but Astra 3 and 5 in part. This is the last round.
  - The build facts a selection was made under now read the build's own `plan` and `design` (the server keeps them beside `status`, not in it). A rebuild or a live batch therefore ends the selection, with a test using the wire's shape.
  - A cell the inspector opens carries its column's place, whole: the scheme and every axis's value by name, not the label, which leaves defaults out. A branch read since on another place is refused, a changed default included.
  - With no app axes read yet, or a copy naming an axis value the design does not list, the copy is not placed, said as such ("the app's design is not read yet", "its modes are not a column of the app's design"), and nothing is offered.
  - Reads of the app design's axes (the open design's, or `loadAppAxes` while another file is open) share one counter, so only the newest sets them.
  - Server (scripts/tamber-builds.mjs, docs/tamber-protocol.md): `inspect` off with no build also clears the client's inspect mode on its builds that are away, so one that registers again is told off, not on. Tested in tamber-builds.test.mjs.
  - In part (Astra 3): an accepted `loadAppAxes` does not itself refresh the inspector. A stale view's Edit is still refused at the cell by its place.
  - In part (Astra 5): a presence lapse with no event gap is not seen by the tool. The server turns inspect mode off and publishes nothing, and data sources have no clock to measure the gap. The toggle can say "Stop inspecting" until the next command or a gap; a stale selection still offers nothing.

---

Round 2's taken fixes hold where they were actually wired: a reload that returns to the same status stays stale, `status.mode` is in the key, a trimmed report with no `color_scheme` row offers no edit, `inspectSent` moves the toggle, `selected: null` and a missed poll clear inspect state, and an axes reorder that changes the column label is refused at the opened cell. Two of those fixes do not do what the disposition says they do.

## High

**A new plan or design leaves the previous join current, and Edit stays up.** `apps/design-tool/inspector.ts:30`, `apps/design-tool/app.ts:566`

`buildKey` reads `status.plan` and `status.design`. The server never puts them there. `status` is only scheme, contrast, mode, root font size, and pin revision (`scripts/tamber-builds.mjs:246`). `plan` and `design` sit beside it (`scripts/tamber-builds.mjs:124`, `:247-249`), and `buildOf` keeps `status` and drops those siblings. Both key fields are always null, so a generation or a design digest compares equal. The page applies a rebuild in place: `applied` stores the new generation and sends `status()` without re-registering or clearing the selection (`host/web/dev.js:236`, `:364`). A live batch does the same for the design digest (`host/web/dev.js:281`). `revision` is the pin frame, so it stays put. The server refuses a later report whose plan or design disagrees (`scripts/tamber-inspect.mjs:125`) and does not retract the selection already published. The README and the `buildKey` comment say a new plan or design ends the selection.

Scenario: Inspect the theme fixture, select the card, Edit offered. The app rebuilds, or another edit lands as a live batch. Scheme, mode, and pin revision are unchanged. The panel still shows the previous document's source, rows, and Edit or Open. Apply writes the branch of a join computed against the plan that has gone.

The reload test (`app.test.contract:853`) nulls `status` and restores the same status object, with no `plan` or `design` field, so it stays green.

Fix: keep the build event's top-level `plan` and `design` on the snapshot `buildOf` stores, and put those fields in `buildKey`. A change latches stale until the next `selection`.

## Medium

**The opened cell treats a column label as the tuple, and labels omit every axis at its default.** `apps/design-tool/app.contract:1049`, `apps/design-tool/app.ts:282`

Edit stores `selModes` as the column label (`editFromSelection` at `app.contract:621`). The cell stays open while `branch.modes == selModes`. `columnsOf` builds that label from values other than `values[0]`, so the all-defaults column is always `0-dark` / `0-light` with the label `Dark` or `Light`, whatever those defaults are. `draftStale` compares `branch.when` only after a keystroke, and the keystroke copies the new branch's `when`, so Apply follows the new tuple. The axes-reorder test (`app.test.contract:875`) reverses axis order, which changes `Dark · dark-style=dim` into a different label. A change of `values[0]` does not.

Scenario: Select a copy on the default dark column and click Edit (`selColumn` `0-dark`, `selModes` `Dark`). The design then makes `dim` the first `dark-style` value. The selection panel goes stale once it is read again, but the opened cell's label is still `Dark` and its id is still `0-dark`. Apply writes the dim branch. While another file is open, `inspected` depends on `changes.answer` and `design.answer` (`app.contract:445`) and not on `appAxes`, so after `loadAppAxes` stores the new axes the Edit button itself stays up until the next poll, and that click is accepted.

Fix: store the column's axis values (the tuple) on the click, and refuse unless the branch column's values match. A label match is not a tuple match.

## Medium

**Before the app design has been read, every selection is placed on the default column, and that label then matches the real one.** `apps/design-tool/inspector.ts:51`, `apps/design-tool/app.ts:543`

`designAxes` starts as `[]`. `columnsOf([])` yields one empty tuple per scheme, labelled `Light` and `Dark`. The match is `c.values.every(...)`, and `every` on an empty list is true, so any selection whose scheme is known binds column `0-light` or `0-dark`. A missing axis value also falls back to `values[0]` (`inspector.ts:51`). `loadSelection` runs from the poll (`app.contract:445`) while `design.answer` is still 0.

Scenario: Describe of the app design has not returned (or the first read failed and left `designAxes` empty). Select the Dim card. The panel shows `Dark` and offers Edit. Click it: `selColumn` is `0-dark`, `selModes` is `Dark`. The read then succeeds. The panel latches stale because the axes changed, but the opened cell sees the real default column, whose label is also `Dark`, and shows its editor. Apply writes the default branch for a Dim copy.

Fix: place no column when the axes are empty or the tuple does not name an axis value. Offer no Edit or Open until the selection is placed on the axes the app design actually has.

## Low

**An unplaceable tuple is reported as an unknown scheme.** `apps/design-tool/inspector.ts:56`

When the scheme is known and no column contains the tuple, the view still says "the scheme it is shown in is not known". That string is also the real unknown-scheme message (`schemeOf` returned `''`). No edit is offered.

Scenario: The build's mode is a value the design's axis does not list, and `color_scheme` is `normal` on a light build. The axes are unchanged, so the axes latch does not replace the message. The panel says the scheme is unknown.

Fix: use a separate reason when the scheme is known and the tuple is not a column ("this copy's modes are not a column of the design").

Verdict: Request changes — a rebuild or a new design digest leaves the old selection editable, and the opened cell will write a different tuple that shares the column label.
