# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `5b87dfbc8eb802f9d73cf36e37ab3a52ad380fb58f76e833c0cc28f6ef4ba2d9`), shared by both reviewers, blind to the other review; commits 82d98133ff. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 blocking; smaller points).
- **Disposition:** taken in `bc485dd65f`: a held look's row is set-row's only where it is written under the node in that state (a hover's row with no place of its own is the base binding, so no template address points at the base), its `instances` the held node's own call-site chains, an unread param still refusing it; a string's escapes are decoded one by one as the lexer does (`\n \t \" \\ \` \$ \u{…} \uXXXX`; another refused); a positional argument in parentheses or a template after a plain value; the fixture's rows by their wire names (`border_radius_top_left`, `border_color_top`); the held inline check passes `app` and asserts the template rung; the doc says the state rung's text is the plain value's and a layout row still offers its token. Not taken: `#12`/`1.`/`.5` accepted by the reader but not the lexer (a built map never carries them).

One issue blocks. The held template rung counts the selected node's call-site chains, so a hold on a different node publishes the wrong `instances`.

## Held `instances` is the selected node's

`copies` is computed from the selected node and then passed into the held row:

```350:366:scripts/tamber-inspect.mjs
  const copies = Math.max(1, new Set(sitesAt(map, node).filter(i => same(map.nodes[i], node)).map(i => JSON.stringify((map.nodes[i].chain ?? []).map(c => [c.file, c.line, c.col])))).size);
  // ...
      const offered = !named || readOnly ? null : editable({ ...r, source: 'authored', state: b?.state }, b?.origin ?? null, params, false, stateRung(r, b, app, read, own), rowRung(r, b, map, app, copies, read, own));
```

`node` is `map.nodes[selected.site]` (line 340). The owner is a different map node when the build names `held.site` (line 344). `instances` on that rung is defined as the copies of the source node the edit writes (`docs/tamber-protocol.md:1114-1116`). The inspector prints it as "This template · N instances" (`apps/design-tool/inspector.ts:59-61`), including on held rows (line 103).

Scenario. `Pressable` declares `slot`, then a `button` whose `:active` sets `background-color="accent.pressed"`. `Page` uses `Pressable` twice, and only the first call slots `text "Save"`. The selection is that text; the hold names the button. The fill is expanded under the caller's instance (`contract/syntax/src/inline/use_node.rs:327`, `contract/syntax/src/inline.rs:468`), so the text's chain does not include the `Pressable` use and its span has one chain. The button is stamped with `Pressable`'s instance (`inline.rs:551`, after the child's instance is installed at `use_node.rs:333`), once per use, two chains. `set-row` at the button's attribute rewrites the source both buttons compile from.

A throwaway `join` of that map returned `instances: 1` on the held template rung (`rung: "template"`, `row: "background-color"`, `state: "active"`). Selecting the button itself returned `2`. A `text "OK"` written in the button's own view, sharing those two chains, returned `2` on the held row. The address is the button's; the count is the text's. The held fixture (`scripts/tamber-inspect.test.mjs:105-109`) gives both spans one chain and never reads `editable.instances`.

Count the owner's chains when the held row is the one offered. The selected node's own rows already use the right count, and a generated `button "…"` label still shares the button's span and chain (`contract/syntax/src/parser/view.rs:215-229`).

## Taken points that hold

Inline state on the node is a template rung. `rowRung` no longer requires `attr`; a state binding uses the authored name (`scripts/tamber-inspect.mjs:232-234`, `AUTHORED` at 332), and the rung carries `state` (line 251). The test locks `background-color` / `state: "active"` (`tamber-inspect.test.mjs:89-95`). A style's `:active` still returns null from `rowRung` (line 231) and stays on `stateRung`. The held path calls `rowRung` (line 366); the write address in the scenario above is that place.

`plainAt` matches the forms the disposition names. The slice is a UTF-8 byte column and the attribute name has to match there (`tamber-inspect.mjs:259-262`). The test covers a column after `你好` (`tamber-inspect.test.mjs:383-386`), and the same probe read `-exact-press-scale=-0.9` as `-0.9`, `#1f9d62`, `2.5E-3`, `(- ( 0.5 ))` as `-0.5`, `\n` `\t` `\"` `\\`, `\u{2011}` and `\u2011`, and a following positional string. A name miss returns null. `-( -0.5)` returns null, as `plain()` does once the paren unwraps to a nested unary (`contract/syntax/src/parser.rs:1231-1246`, `tamber_rows.rs:89-94`).

`instances` on the selected node is one per call-site chain. The third node in the fixture shares the span and chain and the template rung reports 2 (`tamber-inspect.test.mjs:347-362`).

A non-state written row carries `layout: true` on the selection (`tamber-inspect.mjs:379`). State bindings omit it; the map never sets `layout` on `Origin::State` (`contract/cli/src/map.rs:260-287`).

A refusal whose whole value is one token is the token rung (`tamber-inspect.mjs:318-319`): layout (`tamber-inspect.test.mjs:373`), a package file (line 378), or a value `plainAt` refuses. A package layout row still hits the layout return before `ownFile` (line 237).

## Not taken

The empty own-sources window is still the right call. `join` receives `own: null` while `known` is empty (`scripts/tamber-server.mjs:235`); `ownContracts` is that set (`tamber-server.mjs:897`). With `own` null, `ownFile` (`tamber-inspect.mjs:212-215`) treats an app-relative file outside `node_modules` as own. This commit does not change that line. It is the window `stateRung` already had.

## Regressions

T4's state text now comes from `plainAt` (`tamber-inspect.mjs:206`). A bare `background-color = accent.fill` seeds nothing; `"accent.fill"`, `"a \"b\""`, and `0.8` still seed (`tamber-inspect.test.mjs:241-253`). The rung is still offered. The state-rung sentence still says the value as written (`docs/tamber-protocol.md:1099-1100`); the plain-only sentence is in the template paragraph (lines 1117-1120). That is a doc mismatch with a behavior the new test records. It does not block.

The param rung's `template` still attaches when the written rung is `template` (`tamber-inspect.mjs:314`). A param's canonical value is a quoted string, which `plainAt` accepts, so `rowRung` still returns that rung. The direct case is `tamber-inspect.test.mjs:380-382`.

## Smaller points

`plainAt` still rejects some strings the lexer accepts (`contract/syntax/src/lexer.rs:491-497`). The probe returned null for `` color="x\`y" ``, `color="\${n}"`, `color="a\u{a}b"`, `color="\u{0}"`, and `color="\\u{41}"` (the braced rewrite consumes the escaped backslash, then `JSON.parse` throws). Four-digit `\u000A` and ordinary `\n` work. On a paint row that is not a single token, the join then says the attribute is an expression. These are outside the forms the new test lists.

The same reader accepts source the lexer does not: `#12`, `1.`, `.5`. A successful build's map does not carry those, and a moved file is `readOnly` before the rung is shown.

The trailing check (`tamber-inspect.mjs:281`) allows end of line, `//`, another `name=`, or a positional `"…"`, and it requires whitespace. `color="red" \`hi\``, `color="red" name`, and `color="red"opacity=1` return null. The disposition's positional case is the quoted string, which passes.

`docs/tamber-protocol.md:1134-1135` still says `layout: true` on the selection row means the client offers no edit. Lines 1123-1124 say that row offers the token rung when its whole value is one token. The row can carry both. The design tool does not read `layout` yet.

A package style's state refusal still returns from `editable` before the token fallback (`tamber-inspect.mjs:301-303`). That path is `set-state`. The package token fallback the disposition took is the set-row path, and the test covers it.

`col()` in the row fixture is `String.indexOf` (`tamber-inspect.test.mjs:333`). Those lines are ASCII; the wide column is a direct `plainAt` call. The selection object's `layout` field is not asserted, only the refusal reason. The expression reason always says "or over more than one line" (`tamber-inspect.mjs:243`), including for a single-line expression.
