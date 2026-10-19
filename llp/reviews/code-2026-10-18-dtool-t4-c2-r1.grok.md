# Code review: LLP 1122 T4 step 2, the state rung, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `0c44ecbba5`.
- **Method:** one brief (sha256 `56c57eb6ec6dac5156741507bff4956b33db7d4ff9ce2da9c324f67f669912c1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** request changes (3 high; smaller points).
- **Disposition:** taken in `ab3c809bed`: the state rung is the default with the token offered beside it (theme2's 16:18 correction, which the commit predated); the style is named as its file declares it (the `style` line above the part's row), not the compiler's renamed class; the rung carries the row's value as written (`text`), which the editor starts from, so Apply without typing writes the token or literal as written, never the resolved paint; a package's style (outside the app, or under node_modules) answers a reason; held rows are offered only where the build names the held node (stage 9 E's `held.site`), joined at that node's bindings through `editable`, so a token is offered beside there too; a state edit is bound to the copy it was opened on and stale (Apply disabled) once that copy is not current; opening it closes a token cell in the branch editor; numbers with exponents are numbers. Doc: only an `:active` part's rows carry their place in the map, so `:hover`, `:focus-visible`, `:disabled` and `:checked` parts have no state rung yet (a §10 limit). Kept: the hold replay's diagnostic is the server's (`hold-cleared`), shown by the tool.

Two correctness bugs block this wire. Hover and focus-visible never get a state rung, and Apply on the state field writes the resolved paint, so a token-valued row is replaced by that paint.

## Correctness

**[P1] Hover, focus-visible, disabled, and checked never get a state rung — `contract/lower/src/nodes.rs:503`**

The join only builds a state rung when the map binding has `state` and `at` (`scripts/tamber-inspect.mjs:191`). The compiler sets `Origin::State` only for `:active` press rows, and only then does `contract/cli/src/map.rs:228` emit those fields. `:hover` and `:focus-visible` stay in `Folded::looks` (`contract/lower/src/states.rs:16`). `:disabled` and `:checked` fold into the base row with no `Origin::State`.

`set-state` already accepts those states (`contract/cli/tests/it/tamber_serve_contract.rs` writes `hover` and `focus-visible` opacity). The inspector cannot address them.

Scenario: `style PostButton` with `:hover background-color = yellow`. Hold Hovered. `heldOf` reports `{name: 'background_color', value: 'rgb(255, 255, 0)'}` (`host/web/input-glue.js:382`). The join looks that name up on the base `background_color` binding, which has no `state` or `at`, so `stateRung` returns null (`scripts/tamber-inspect.mjs:253`). The held row shows the yellow and no "Edit PostButton :hover background-color". The protocol says a held state's rows carry the same rung (`docs/tamber-protocol.md:1018`).

`:active` scale is not this bug. `ACTIVE_ROWS` lowers `-exact-press-scale` to one `press_scale` row, and `PRESSED` maps it back to `-exact-press-scale` (`scripts/tamber-inspect.mjs:183`).

**[P1] The state field writes the resolved value, so Apply replaces a token — `apps/design-tool/app.contract:1241`**

The button calls `editState(r.state, r.value)`. `r.value` is the host's resolved row (`#0f73ffff`, `rgba(...)`), not `token.text`. `opOf` leaves a non-numeric string as a string (`apps/design-tool/app.ts:237`), and the server quotes every string (`contract/cli/src/tamber_contract.rs:137`), which is what a Contract color wants (`background-color="accent.pressed"`).

Scenario: the part is `background-color="accent.fill"`. The selection value is `#0f73ffff`, with the token rung beside it. Click "Edit PostButton :active background-color" and Apply without typing. The write is `background-color="#0f73ffff"`. Typing `accent.pressed` does quote correctly. A number such as `0.8` is sent as a JSON number and written unquoted, which matches opacity and `-exact-press-scale`. The contract test types `0.7` over `0.8` and never asserts the operation (`apps/design-tool/app.test.contract:1244`).

**[P1] A held token row skips the token rung — `scripts/tamber-inspect.mjs:253`**

Selection rows go through `editable`, which keeps the token rung and puts the state rung on `editable.state` (`scripts/tamber-inspect.mjs:216`). Held rows attach `stateRung(...)` directly. The host's held payload is CSS text with no token names (`host/web/input-glue.js:375`), and a `var(` or `light-dark(` value is left as that text, with `provenance: "incomplete"`.

Scenario: hold `:active` on a press background whose source is `accent.fill`. The selection offers "Edit accent.fill" plus the state field. The held row offers only the state field, seeded with `var(--x-...)` or `rgba(...)`. Apply quotes that into the part. The new test's held row is a literal opacity (`scripts/tamber-inspect.test.mjs:197`), so this path is untested. The doc's "held state's rows carry the same" (`docs/tamber-protocol.md:1018`) covers this case.

## Smaller

**[P2] A token cell left open sits beside the state editor — `apps/design-tool/app.contract:818`**

`editState` sets `editKind` to `set-state` and does not clear `selToken`. The branch editor stays up whenever `selToken != ""` (`app.contract:1406`), and the state editor is a later sibling (`app.contract:1486`). Apply follows the current `editKind`. Typing in the still-visible branch field runs `cellText`, which sets `editKind` back to the token op (`app.contract:886`). Open `accent.fill`, then "Edit PostButton :active opacity", then type in the token field: Apply writes the token.

The open state editor is also not cleared when the selection goes stale or is replaced (`apps/design-tool/app.ts:1034`, `1038`, `1078`). The buttons hide, the field stays, and Apply still submits the reviewed `PostButton|active|opacity|…` ops. A successful Apply does clear `editKind` (`app.contract:999`).

**[P2] The selection paragraph contradicts the join in two places — `docs/tamber-protocol.md:1018`**

"A held state's rows carry the same" is false for hover, focus-visible, and a token-valued held press row, as above. The next sentence (`docs/tamber-protocol.md:1021`) still says an `:active` row lowered to the press paint is edited by `set-state` unless it is one token. An inline `:active` (`origin` `own`) is read-only and points at `set-row` (`scripts/tamber-inspect.mjs:226`), which the sentence just above already says. The 1.12 bullet (`docs/tamber-protocol.md:75`) says a style's state part row is edited by `set-state`; only `:active` press rows have the map fields that produce the rung.

**[P2] The file is relative only as a string prefix of `app` — `scripts/tamber-inspect.mjs:194`**

`at.file` is a canonical absolute path (both bakes canonicalize `app.contract` before compile). The strip keeps a relative path when `at.file` starts with `${app}/`. `resolveApp` realpaths `app.dir` (`scripts/app.mjs:474`), and that is what the dev loops pass, so an owned file such as `ui/post.contract` matches a snapshot key and `set-state` accepts it. Anything else is sent through and refused: "`…` is no Contract source of the app's own" (`contract/cli/src/tamber_contract.rs:581`). A package style under `node_modules`, or a link outside the app, is still offered, then refused. The inspect test builds the map with the same prefix as `app`, so it never sees a compiler path.

The editor says a hold comes back (`app.contract:1489`). Replay looks up the element's span, not the shared style span (`scripts/tamber-server.mjs:211` calls `sitesAt` on the node; `scripts/tamber-inspect.mjs:273`). Two uses of one style do not collapse to "no single site". A style in another file leaves the element's line alone, so the hold replays. A same-file format that shifts that line clears it, and the tool already shows `hold-cleared` (`apps/design-tool/app.ts:1071`, `holds.ts`). There is no test that the hold returns.

## Checked, not bugs

Hyphenated style names pass the origin regex and the `|` split (`class:Post-Button`). Inline `:active` stays read-only toward `set-row`. A conditional `class:A/B` offers nothing. `PROTOCOL` and the hello assertions in `tamber_serve.rs`, `tamber-server.test.mjs`, and `design-tool.test.mjs` are `tamber/1.12`. Nothing still expects `1.11` on hello. The stub greeting is still `tamber/1.1` and does not gate the tool. The operations table marking `set-state` as 1.11 matches when that op landed.
