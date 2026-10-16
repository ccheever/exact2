# Code review: LLP 1122 T2b step 6 (the inspector panel), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `10d7f5f4d`.
- **Method:** one brief (sha256 `83bc54370096448d2b731170f626b17642dc5ec4d0c14d3f0d0770c6e4185106`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 3 medium, 2 low).
- **Disposition:** taken, but Astra 5 in part. The selection keeps what it was joined against: its build (the build's key), the design's axes, and the scheme it is shown in (the node's own `color-scheme`, else its build's). It is marked stale, with no Edit or Open, when its build goes or re-registers, the axes change, the page sends `selected: null`, or the scheme is not known. Its column is found against those axes, and the modes line is that column's label. Edit and Open open the app's design (`file = ""`). The toggle follows the builds the server said it told (`inspect`'s `builds`), and drops a build on its `selected: null`, when it leaves, or on a new epoch. Impact is shown only when `uses.token` is the token asked for; slot and derive vias are labelled. Outline lists the builds that were told, says when there are none, and the stub tells only builds that can inspect. Rows use the author's CSS names. The chain shows full paths. Tests: a light copy on the light column, an unknown scheme, a build gone, `selected: null`; the wasm drive asserts the exact branch label `text.primary · default (Light)` and the matching row. In part: another client's `inspect` for the same build is not seen (the wire has no inspect-state event), so the toggle shows only what this client did.

---

The rung buttons follow each row's `editable` value. The branch they open, and the inspect toggle, do not stay tied to the copy the server joined.

## High

**1. A kept selection is shown as current, and Edit/Open can target the wrong branch.** `apps/design-tool/app.ts:544-547`, `apps/design-tool/app.ts:1142-1144`, `apps/design-tool/app.contract:903-908`

`selectionNow` is replaced only by a newer `selection` event, or cleared when the server epoch changes (`app.ts:1111`). A `selected: null` from the page (Escape or the banner) is ignored. A `builds` list that no longer contains the build, a reload that re-registers it with `status: null` (`scripts/tamber-builds.mjs:213`), and a missed poll gap (`app.ts:1124`) all leave the old join on screen. The comment at `app.ts:1142` says that case is "said as its build's no longer"; the panel still renders `changes.selection.has` with the same Edit/Open buttons.

The column is recomputed on every poll from the build's **current** scheme and from `designAxes`:

```544:547:apps/design-tool/app.ts
  const build = buildsNow.get(String(s.build));
  const dark = build?.status?.scheme === 'dark';
  const values = designAxes.map(a => (dark || a.under !== 'dark' ? s.tuple?.[a.name] : null) ?? a.values[0]);
  const column = columnsOf(designAxes).find(c => c.dark === dark && c.values.every((v, i) => v === values[i]))?.id ?? '';
```

`scheme === 'dark'` is false when the build is missing or its status is still null, so the copy is treated as light. On a light column an `under: "dark"` axis is forced to `values[0]`, and Dim becomes the default tuple. The rows still show the joined copy (the Dim hex, `dark-style=dim` in the tuple line).

`designAxes` updates when `describe` finishes (`app.ts:812`), but the column id is baked into `changes.selection` and is not recomputed until the next long poll returns (up to ~25s). Column ids are `${tupleIndex}-dark|light`. After a value is added or axes are reordered, the grid's `2-dark` is a different tuple from the id the button still submits. `editable.modes` is never read.

Scenario: select a node on a dark Dim build, then the page reloads. The next `builds` event has `status: null`. The panel still shows that node, and Edit `surface.card` opens the light default cell. Apply writes a branch the selected copy was not showing. The same window exists after an axis edit: the grid has the new columns, the button still carries the old id.

Fix: snapshot scheme, status revision, and axes onto the selection when the event is applied. Recompute the column from that snapshot when the design resource updates. If the build is gone, status was unknown, scheme or modes have moved, or the axes no longer produce that column, show the join as stale and render no Edit/Open. Handle `selected: null` the same way.

**2. The inspect toggle is local state, so it drifts from the builds.** `apps/design-tool/app.contract:595-599`, `apps/design-tool/app.contract:915`

`toggleInspect` sets `inspecting` before `sendInspect` returns, and nothing sets it back. A failed `inspect` (client not present, 409) still leaves "Stop inspecting". The page leaving inspect publishes `selected` with `selected: null` and drops ownership (`scripts/tamber-builds.mjs:255-263`); this client never reads `selected`. Another client sending `inspect`, or this client's poll lapsing past presence so the server clears inspect, leaves the same split. A new server epoch clears the selection and not `inspecting`. Outline is enabled exactly when that flag is true (`app.contract:915`), so it runs while no build is inspecting, or stays disabled while one is.

Scenario: turn Inspect on, then press Escape on the canvas banner. The frame leaves inspect mode. The tool still says "Stop inspecting", the last node still offers Edit, and Outline still sends.

Fix: drive the toggle from the server. On `selected: null`, a failed `inspect`, an epoch change, or this client losing the builds it turned on, set `inspecting` from what the server actually holds. Keep Outline disabled unless a build that can inspect is still in this client's inspect mode.

## Medium

**3. Impact and Outline can name the wrong sites, and Outline names no build.** `apps/design-tool/app.contract:914-919`, `apps/design-tool/app.ts:1402`, `apps/design-tool/app.ts:1412`

The heading interpolates `impactToken` while the list is `uses.sites`. A resource keeps its previous value until the next load, and `loadUses` is retried on every poll answer, so the line can read `text.primary: read at N source site(s)` over `surface.card`'s rows. The returned `uses.token` is unused. `via.slot` and `via.derive` add no "through …" label (`app.ts:1402`).

`outlineUses` reports `reply.sites.length`. That array is plan site ids in the current map (`scripts/tamber-server.mjs:539-541`), and `builds` is dropped. The note says the token was outlined when the map yielded sites and every capable build was told, including builds whose inspect mode is off and that draw nothing.

Scenario: Edit `surface.card` (2 sites), then Open `text.primary` before `uses` returns. The heading names `text.primary` and the rows are still the card. Outline then says `Outlined 2 site(s) of text.primary` from the map, with no build id, including when the page has already left inspect mode.

Fix: render `uses.token` and the sites together, and hide them while `pending(uses)` or `uses.token != impactToken`. Have the note list `reply.builds`, and say when that list is empty. Label slot and derive vias.

**4. The token rung writes whichever file is open.** `apps/design-tool/app.contract:602-613`, `apps/design-tool/app.contract:481`

`editFromSelection` sets `selToken` and `selColumn` and leaves `file` alone. `loadBranch` and `proposeEdit` both take `file`. The column id was computed from the app design's axes (`designAxes` updates only when `file` is the app design, `app.ts:812`). On another `.tamber` file the same id can be a different tuple, or a token of the same name in the wrong file. `chooseFile` clears the cell, and the next Edit from the inspector sets it again.

Scenario: open `brand.tamber`, then Edit `surface.card` on a selected Dim copy. Apply writes `brand.tamber` on that file's `2-dark`, which is not the branch the running app resolved.

Fix: open the app design (`file = ""`) before placing the cell, and place the column only against that design's columns.

**5. The tests pass without locking the branch, a stale join, or outline.** `apps/design-tool/app.test.contract:784-794`, `scripts/design-tool.test.mjs:942`

The stub test covers one dark Dim join whose axes are already loaded, expects `selColumn == "2-dark"`, and stops. It never sends scheme `light` with `dark-style=dim` (the light column should be the under-dark default), a build with `status: null`, a `builds` list that drops `web-1`, a `selected: null`, a failed `inspect`, or an axis change. The trailing `POST /__stub/restart` has no assertion after it. The stub's `outline` echoes request indexes and every build id, including `ios-1`, which cannot inspect (`stub/tamber-stub.ts:109`).

The wasm drive's branch check is `toMatch(/^text\.primary · /)`, which accepts the default and the Dim `when`. Impact accepts any digit, including `0` from the previous empty `uses` while the new load is in flight. Outline is not driven on the real server.

Fix: assert the light/under-dark column, unknown status and a departed build render no Edit, Escape clears the toggle, and the opened branch label is the `when` that won (`dark-style=dim` or `default (Light)`). Point the stub's `outline` at the real reply shape and assert the build ids.

## Low

**6. The modes line includes axes the column dropped.** `apps/design-tool/app.ts:551`, `apps/design-tool/app.contract:893`

On a light build the column correctly ignores `dark-style`, and the line still shows `web-1 · dark-style=dim density=regular`. The copy looks like Dim until the cell opens on Light.

Fix: print the column label (the modes that placed the branch), and keep the raw tuple only as secondary detail.

**7. Two call sites can collapse to one row.** `apps/design-tool/app.ts:548`, `apps/design-tool/app.contract:891`

`at()` keeps the file's basename, and the chain is `key=c`. `components/Card.contract:4 · Card` and `pages/Card.contract:4 · Card` are one key, so one call site is dropped.

Fix: key and show the path the join sent.

A new epoch does clear `selectionNow`. With a known `scheme` and current axes, a light build's under-dark axis is placed on the default light column, and a row is Edit, Open, or reason exactly as that row's `editable` says.

Verdict: the panel follows per-row editability, and it will still offer a token edit on the wrong branch for a stale, reloaded, or unplaced copy, with an inspect toggle the builds do not share.
