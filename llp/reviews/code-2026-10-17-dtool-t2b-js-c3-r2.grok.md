# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `faf272b86`.
- **Method:** one brief (sha256 `87d029298cdfcc7ad14f2a3192cdc293a089387b8936d5df36e7a492e004491f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (several medium, 2 low).
- **Disposition:** taken in most part.
  - The dev client's early gate is complete while the module loads: it installs a cover and a guest-frame shield, stops pointer movement too, and lets Escape go (it clears the kept state). A module that fails to load lets go too. The module removes the stand-ins when it installs.
  - Focus events stop for the inspector's own controls too; only the app's are blurred.
  - A colour's `currentcolor` is a clear, as the runner treats it, and an empty pin sets nothing.
  - After a live edit, a token row keeps the table's value when its tokens' slices are as they were built. A changed token's value is written as `node` writes it (a colour, a number, a px length as the table's kind), else as the computed CSS, said so.
  - The table must be the page's plan's (`plan`); a page whose plan was rebuilt says to select again after it reloads.
  - A kept selection names its inspect command: a new command clears it, and a reload restores it only under the same one.
  - An app with no axes reports `modes: {}`, as `node` does.
  - The test now compares `testId` and `computed` too.
  - Limits, for §10: a style row's fallback against an expression's row above; the CSS map's gaps (step 2's); SVG geometry written as attributes (`attribute_rows`: `x`, `y`, `width`, `height` on svg and its kin, and a radial gradient's `cx`, `cy`, `r`); outline marks and scrolling in document order, not the runner's site order; `from` and `node` are the page's view ids.

---

I'll review HEAD as a read-only pass: the JS inspect client against the runner's `node` report, round 1's dispositions, and whether the dev-only gate and tests actually hold.The commit is the JS-page inspect client. I'll read round 1's dispositions first, then compare the new module to the wasm inspect path and the runner's `node` report.Round 1 named specific gaps in the report and the input gate. I'll check those fixes against the runner's `node` path and the DOM the page actually emits.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll confirm the remaining findings against the current files, then write the round-2 report.Request changes. The round-1 reload click is swallowed once the classic script is in place, and the seven theme-fixture nodes can match on the fields the test keeps. These holes remain.

## Medium

**1. An empty or whitespace pin is reported as the boundary’s own mode row.** `host/web-js/inspect-dev.js:81` and `:122`.

`modes.js:16` treats `""` and whitespace as unset (`UNSETS`), leaves `$xbad` unset, and keeps the surrounding tuple (`modes.js:77-80`). `PropHooks` still writes `data-x-pin=""` (`modes.js:242`). `CLEARS` does not match `""`, so `setNow` is true and `ownRow` reports that surrounding tuple plus one as `dynamic`. `pinned` (`runner/src/bridge.rs:317`) returns none for an empty pin, so `node` has no own mode row.

Scenario: `-exact-mode=""` or a binding that sets `data-x-pin=""`. The report shows an own mode row; `node` shows the ancestor’s mode or initial `0`, and descendants inherit that.

Fix: use the same unset test as `modes.js` (empty, whitespace, `unset`, `inherit`) and require a pin `pinned` would accept.

**2. `currentcolor` and `color-scheme: normal` stay own dynamic rows.** `host/web-js/inspect-dev.js:81` and `:125`.

`text_unsets` (`kernel/src/style.rs:335-346`) clears `currentcolor` on text color and `normal` on color scheme. Off a boundary, `Sx` still writes `currentcolor` (`modes.js:139-151`), and `CLEARS` does not match it, so `setNow` keeps the row. The value is the computed color with `computed: true`. On a boundary, `$xi` is unset and the row is correctly omitted.

Scenario: `color={flag ? "currentcolor" : "text.primary"}` on a node that is not a mode boundary. The report says `dynamic`. `node` does not set the row, and descendants stop their walk at this node.

Fix: treat `currentcolor` as a clear for text color, and `normal` as a clear for color scheme, the same way `text_unsets` does.

**3. After any live edit, a one-token literal row takes the snapshot’s wire text, which is not `row_json`.** `host/web-js/inspect-dev.js:46` and `:107-114`. Style rows never take this path (`:173-178`).

`valueOf` uses `token.slices?.[n] ?? token.value` from `design::tokens` (`host/web-js/src/modes.rs:531`, `runner/src/design.rs:126`). `asRow` only turns `Npx` into a number and strips an opaque `#rrggbbff`. `measures.rs:36-46` leaves `rem` as `rem`. `row.u` is set only when the literal text itself contains a relative unit (`host/web-js/src/inspect.rs:186-194`), so `padding="space.md"` with `space.md` equal to `0.5rem` is not on that path. The table’s value is the kernel’s pixel number until the design digest changes; then every single-token row whose `row.x === row.k[0]` switches to the wire text, including tokens that were not edited. A role spelling (`row.x !== row.k[0]`) falls through to computed CSS with `computed: true` on a literal row. Style rows stay at the build-time number.

Scenario: edit any color, then read a spacing row whose token is `0.5rem`. The report value becomes the string `"0.5rem"` (or `"#fff"` for a shorthand color) while `node` still says `8` or `"#ffffff"`. A text style fed by a token stays at the old table number. The seven-node test uses literals and does not edit the design; the commit says live values are checked by eye.

Fix: keep the table’s `row_json` value unless that token’s text changed, and resolve a changed value the way `row_json` does (pixels, `#rrggbb` / `#rrggbbaa`). Apply the same resolution to style rows that came from a token. Do not set `computed: true` on a literal row.

**4. Input is not fully gated for the whole time inspect mode is on.** `host/web-js/dev.mjs:468-469`, `host/web-js/inspect-dev.js:306-309`.

The classic script does run before the deferred app module, and a replayed `click` is swallowed. `capture.js` is still registered in the head and runs first. The early listener skips `pointermove`, installs no iframe shield and no cover, and has no Escape path. `import('/__dev/inspect.js')` has no `.catch`. `focusing` returns without stopping when the target is inside the banner.

Scenario: reload while inspecting. Until `inspect.js` finishes, a click or key inside a guest frame reaches the guest (the parent listener does not see that document), `pointermove` reaches the app, and Escape does nothing. If the import fails, `sessionStorage['exact.dev.inspect']` stays set, every later reload keeps swallowing parent input, and Escape cannot clear it. After the module loads, `chrome()` reparents the banner into an open dialog and guest-frame recovery calls `close.focus()`; that `focusin` still reaches the app’s focus trap. A keystroke into an already-parsed control before the end-of-body script is written into the control by `rt.js:1458` even though the replayed event is later swallowed.

Fix: in the classic script, install the iframe shield, stop `pointermove` as well, and handle Escape (clear the storage and stop gating) before the import. On import failure, leave inspect mode. In `focusing`, stop the event for banner targets too.

## Low

**5. The named limit “a dynamic circle’s `r`” covers the wrong node and only one attribute.** `host/web/src/element.rs:739-751`, written by `svgGeometry` at `host/web-js/src/rows.rs:158-167`. `setNow` only reads inline CSS (`inspect-dev.js:125`).

`attribute_rows` is `x`, `y`, `width`, `height` for svg, use, mask, pattern, foreignObject, filter, and fe, and `cx`, `cy`, `r` for a radial gradient. A `circle` is not in that set. A dynamic `width`, `x`, or radial-gradient `r` is omitted and the report shows inherited or initial. `node` includes the row.

Fix: read the attribute `svgGeometry` wrote for those rows, and name that whole set in the limit.

**6. Outlines follow document order, so the copy that scrolls can differ from the wasm page.** `host/web-js/inspect-dev.js:383-384`. `host/web/dev.js:652-660` walks the runner’s `sites` list and scrolls `found[0]`. The JS page uses `querySelectorAll("[data-site]")`. With several copies of one site, those orders differ, and a virtualized list has only the mounted rows. The JS test checks that the outline count is at least 1.

Fix: scroll and order the marks in the same site order the runner’s `sites` answer uses.

## Round-1 fixes that hold

The CSS property comes from `table.css`. A dynamic row is dropped when the inline value is `inherit`, `unset`, `initial`, `revert`, or `revert-layer`, when `$xbad` is set, or, on a boundary channel, when `$xi` is unset. A dynamic ancestor whose inline value names a token produces an implied row in the boundary’s tuple. A style row uses the table value in the selected element’s tuple, and `from` starts at the element itself. The trim keeps `computed`. The selection, the click session, and the local-exit replay are stored and found again by site and instance path. Tuple numbering matches `pinned` (first axis most significant, mode value is that tuple plus one) when `data-x-modes` is the tuple the pin produced.

These limits match the code: a control start is recorded only when a static ancestor sets the row or the control’s value differs from the global initial (`inspect.rs:120-131`); an expression’s token text is the CSSOM string, and a single `var()` becomes the canonical name; the test’s row compare drops `from` and `computed`; live edits are not in the test. The editable compare still embeds `inherited from #${from}` (`scripts/tamber-inspect.mjs:112`), so a green run means those seven ancestors happened to share view ids. `$exactViewId` is the runtime’s own counter (`rt.js:743-745`), not the runner’s view id, so `from` and `selected.node` are not `node`’s ids on a node inspected before the tree has been numbered the runner’s way.

## Security and the tests

`/__dev/inspect.js` is served only by the dev loop, after `gate.check` (`dev.mjs:554-567`), `cache-control: no-store`. `build.mjs` does not copy `inspect-dev.js`. `dev-inspect.json` is copied only when `devReload`. `$exactViewId` and `$exactRowOf` sit in `// dev-only` blocks that `withoutDev` strips. Nothing here is on a release page. The release test checks those source strings, not a built tree. The route is the dev loop’s host gate, the same as the other `/__dev` routes.

“inspect on the JS page” compares the seven literal theme-fixture nodes on site, type, instance, tuple, value, source, and token, plus the join’s line, read-only flag, and editability, and it checks that Swap pins does not run, that one outline exists, and that Escape leaves. It does not cover an expression row, an implied row under a dynamic color, a text style, an empty pin, `currentcolor`, a live edit, an `each` or virtualized path, a report over 60 KiB, a reload, or a guest frame. I did not run it.

Verdict: Request changes; empty pins, `currentcolor`, live token values, and guest-frame input still disagree with `node` and with the wasm page.

