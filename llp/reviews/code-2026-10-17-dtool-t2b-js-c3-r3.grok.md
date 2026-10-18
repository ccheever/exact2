# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `b1c22c4b5`.
- **Method:** one brief (sha256 `895dd9525b2f5ebb495b487f8ffc636ab468817b881a623d4b151720ac9ffe26`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (several medium, 1 low).
- **Disposition:** taken in part (the last round).
  - A live edit of a token that doesn't vary by axis is seen: the table now records each token's build-time value and slices, and the page compares both. (Step 2's table, a one-line change, folded there.)
  - A written clear that inherits (`inherit`, `unset`) bypasses a control's start and a text style, as the runner keeps it written.
  - A colour scheme's `normal` is a clear. A font family's `initial` is its own row.
  - Escape before the module loads records the exit, so the server's replay is answered as the exit. A guest frame that takes focus meanwhile is blurred.
  - A boundary's implied row is `dynamic` when the site binds that row by an expression, as `node` says.
  - A report is read again if a live batch moves the design while it reads, and is made under the facts it read under.
  - A later click or refresh supersedes an earlier one still reading.
  - Limits, for §10: an expression row's ownership is read from what the page wrote, so an animation's `none` (no declaration) and a tint's `unset` (mapped to a value) disagree with `node`. The fixture test compares both pages' identities, not an expected one.

---

I'll review this commit read-only: the inspect-mode port, the two earlier review rounds, and whether the JS page's report matches the runner's `node` op.Round 1 and 2 asked for specific fixes. I'll check those against the runner's `node` path and the page's DOM, then look for anything new.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll re-check the four disputed spots in the current tree so the line numbers and the round-3 verdict match the code.The review skill writes files; this pass stays read-only. I'm reading the report builder, the early gate, and the runner's clear/start rules so the findings cite the current lines.Request changes. A written clear on a control is reported as that control's start, and a live edit of an ordinary token is reported as the build-time value. `color-scheme: normal` is still an own row, and Escape during load does not survive the server's replay.

## High

**A written clear is reported as the control start or the text style.** `host/web-js/inspect-dev.js:189` and `:190`, reached from `setNow` at `:138`.

`setNow` treats `inherit`, `unset`, `initial`, `revert`, and `revert-layer` (and `currentcolor` on text color) as "not set". `nodeOf` then applies `entry.i` and `entry.s` before it walks ancestors. The table records both from the static tree (`host/web-js/src/inspect.rs:111`), where a dynamic binding is absent, so a button's start and a font shorthand are recorded. At runtime an authored clear is `ClearStyle { written: true }` (`runner/src/instance.rs:1072`). A start and a text style apply only while that bit is off (`kernel/src/arena/defaults.rs:192` and `:276`). `kernel/tests/it/native_fields.rs:222` locks the field case: a written color clear follows the parent, and only removing the declaration restores the platform color.

Scenario: parent `text-transform="uppercase"`, button `text-transform={flag ? "lowercase" : "unset"}`, flag false. The page writes inline `unset`. The runner's row is inherited uppercase, `from` the parent. The page reports `{value: "none", source: "initial"}`. The same split hits a field `color={flag ? "red" : "inherit"}` (the neutral field start's label color is reported instead of the parent's ink) and `font-weight={flag ? 700 : "unset"}` on a node whose font shorthand supplies the weight (the page keeps the style row). A class that is off removes the property and `written` is false, so `entry.i` is right in that case. The named limit covers a style row losing to an expression on an ancestor, which is the walk at `inspect-dev.js:199`. It does not cover a clear on the selected node.

Fix: when the dynamic binding's inline value is a clear keyword, skip `entry.i` and `entry.s` and inherit. Use them only when the property is absent.

## Medium

**A live edit of a non-varying token keeps the table value.** `host/web-js/inspect-dev.js:120`.

"Unchanged" is slices-only: `t.tokens[name]` is the slices array (`host/web-js/src/inspect.rs:242`). A token that does not vary by axis is stored with `slices: []` and its text in `value` (`plan/src/builder.rs:327`, `runner/src/design.rs:126`). The dev loop treats a change as value or slices (`host/web-js/dev.mjs:167`). After the edit, slices are still `[]`, so `same` stays true and the report keeps `pick(row.v, n)`. The page's custom properties and the wasm `node` op have the new text. A token that varies per tuple is detected. A relative length marked `row.u` is read from computed style first and is unaffected.

Scenario: edit `surface.card` from `#ffffff` to `#111111`. The card's background row still reports the build-time color.

Fix: compare the live token's `value` as well. The table has to record the build-time value; it currently records slices only.

**`color-scheme: normal` stays an own dynamic row.** `host/web-js/inspect-dev.js:87`.

`text_unsets` clears `normal` only on color scheme (`kernel/src/style.rs:345`). `clears` does not. Round 2 named this beside `currentcolor`; the disposition fixed `currentcolor` and empty pins. The browser accepts `normal`, so `setNow` is true.

Scenario: parent `color-scheme="light"`, child `color-scheme={flag ? "dark" : "normal"}`, flag false. The runner inherits `light`. The page reports `{source: "dynamic", value: "normal", computed: true}`.

Fix: treat `normal` as a clear for `color_scheme` only.

**Escape before the module loads does not answer the replay.** `host/web-js/dev.mjs:468`.

`letGo` removes `exact.dev.inspect` and does not set `exact.dev.inspect.left`. `install` restores `left` only from that key (`host/web-js/inspect-dev.js:447`). On register the server sends `{kind: "inspect", on: true, seq}` whenever inspect is still on (`scripts/tamber-builds.mjs:242`). `frame` treats that as a local exit only when `m.seq === inspecting.left` (`inspect-dev.js:417`). The EventSource is opened before the import, and register waits until the app is ready, so the replay arrives after `letGo`.

Scenario: reload while inspecting, press Escape before `/__dev/inspect.js` finishes. The cover drops. The module then loads, sees no exit, and the replay turns inspect back on. The module's own Escape records `left` and the replay is answered as the exit.

Fix: on that early Escape, store the seq just removed as `exact.dev.inspect.left`, then remove `exact.dev.inspect`. Leave the import-failure path clearing the key without recording an exit.

## Low

**`font-family: initial` is dropped.** `host/web-js/inspect-dev.js:85`.

`CLEARS` matches `initial` on every row. On font family the runner does not clear it: `initial` is an own row, the kernel's family 0 (`runner/src/bridge.rs:723`).

Scenario: `font-family={flag ? "Georgia" : "initial"}`, flag false. The runner reports the initial family as the node's own row. The page omits it and inherits.

Fix: do not treat `initial` as a clear for `font_family`.

## Fixes from rounds 1 and 2 that hold

The CSS name comes from `table.css`. A dynamic row is dropped for `inherit`, `unset`, `initial`, `revert`, `revert-layer`, and for text color `currentcolor`. A pin is set only when its trimmed value is non-empty, not a clear keyword, and `$xbad` is unset. On a boundary a channel is own only when `$xi[j]` is set. A dynamic ancestor whose inline value names a `var()` produces an implied row in the boundary's tuple. A style row uses the table value in the selected element's tuple, and `from` starts at that element. The trim keeps `computed`. The selection is stored with its seq; a new seq clears it; a reload restores it only for the same seq. The table is fetched for `exact.plan`. With no axes, `tuple` is `{}`. The early gate installs the cover, the iframe shield, `pointermove`, and `focusin`, and a failed import lets go. `focusin` is stopped on the banner too. A guest frame's blur inside the gate focuses the close control.

## Limits, as named

These match the code. A style row is applied before the ancestor walk, so an expression that sets the row on an ancestor is reported as the static text style. SVG geometry written as attributes (`x`, `y`, `width`, `height`, and a radial gradient's `cx`, `cy`, `r`) is invisible to `setNow`. Outlines follow `querySelectorAll("[data-site]")` and scroll the first match. `from` and `selected.node` are `$exactViewId`. Step 2's gaps still apply: an expression-only ancestor does not record a control start, `css_property` misses press scale, control size, legacy line clamps, and timelines, and a mixed token-and-text expression has no `row_token` text.

## Security and the test

`/__dev/inspect.js` is served only after `gate.check` (`host/web-js/dev.mjs:554`), with `cache-control: no-store`, by reading the source file. `build.mjs` copies `dev-inspect.json` only for `--dev` and does not copy `inspect-dev.js`. `$exactViewId` and `$exactRowOf` sit in dev-only blocks stripped unless `--dev`. Nothing on this path is copied into a release page. The test checks those source strings; it does not build a release tree.

"inspect on the JS page" compares seven literal theme-fixture nodes on site, type, `testId`, instance, tuple, and each row's value, source, and token, plus the join. It does not cover a written clear, `color-scheme: normal`, a value-only live edit, Escape during load, a guest frame, or a report over 60 KiB. I did not run it.

Verdict: request changes — a written clear on a control and a live edit of a non-varying token still disagree with `node`, `color-scheme: normal` is still an own row, and Escape during load does not survive the server's replay.

