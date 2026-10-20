# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `922498a9a9`.
- **Method:** one brief (sha256 `7465de06fcd80ec7e1b8db57a0140ca408227012f8e67bbde045f766f6fdd7d0`), shared by both reviewers, blind to the other review; commits bc485dd65f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 blocking).
- **Disposition:** taken in `e237f00e45`: a held row's unread param is read from the host's own report before its names were scrubbed, so it still refuses; a positional name after a plain value (not an operator word) keeps the template rung (and a param row's template address); held `instances` tested with the press's own copies differing from the label's; the doc's `:active` sentence names the template rung for a row written under the node. Also (review2's pre-read, `6b10281723`): a style is the nearest `style` line above its row only through indented lines, blank ones and comments. Not taken: `opacity=0.5("hello")` with no space (the parser's own positional rule needs none, but no compiled source writes that) — recorded.

**Request changes. One issue blocks.** A held row still offers an edit while a param it reads has not been loaded. The other round 2 points that were taken hold, and the one that was left holds for the reason given.

## Unread is read after the held token is scrubbed

`heldOf` scrubs the held rows before the new flag is computed. `named` records `unread` only while the IR name is still in `token.names`, and it drops that name from the token it returns:

```163:169:scripts/tamber-inspect.mjs
  for (const n of token.names) {
    const r = rename(n, params);
    if (r.unread) unread = true;
    else names.push(r.out);
  }
  const text = token.text == null ? token.text : rename(token.text, params).out;
  return { token: { ...token, names, ...(text == null ? {} : { text }) }, unread };
```

The selection rows keep that flag at the same time as the scrub (`scripts/tamber-inspect.mjs:358`). The held path does not. `join` builds `base.held` through `heldOf` (`:357`), then asks again:

```389:390:scripts/tamber-inspect.mjs
      const unread = named(r.token, params).unread;
      const offered = !heldHere || readOnly ? null : editable({ ...r, source: 'authored', state: b?.state }, b?.origin ?? null, params, unread, stateRung(r, b, app, read, own), b?.state ? rowRung(r, b, map, app, copiesOf(owner), read, own) : null);
```

The second `named` sees `names: []` and text `"a component param"`, so `unread` is false. `editable` would have refused at `:312`.

Scenario. `Pressable` writes `:active` / `background-color="param.fill"`, and `Page` uses it twice. Describe has not returned (`params` empty). The selection is the button, held on `:active`, and the row's token name is `param.f1a2b3c4d.Pressable.fill`. On that one selection the ordinary row is `{rung: null, reason: "it reads a component param this server has not read yet…"}`. The held row is `{rung: "template", row: "background-color", state: "active", text: "param.fill", instances: 2, at: {line: 5, col: 9}}`. Releasing an edit `set-row`s the attribute and overwrites the param reference. The same selection on a style part (`style Pressable` / `:active` / `background-color="param.fill"`) refuses the ordinary row and offers the held row as `{rung: "state", style: "Pressable", state: "active", text: "param.fill"}`, which is `set-state`. A custom property `var(--x-param-f1a2b3c4d-Pressable-fill)` does the same. Keep the flag from the original token the way line 358 does, and pass that into `editable`.

## Taken points that hold

A held look is `set-row`'s only where the binding carries `state` (`:390`). A hover whose map row is the base binding (no `state`) does not get a template address. With the hold reporting `background_color` as the token `accent.fill` and the base attribute `background-color="blue"`, the held offer is the token rung, and `source` is the node's span, not that attribute. An inline `:active` binding still offers the template rung at the part, with `state: "active"`. The new check passes `app` and expects that rung (`scripts/tamber-inspect.test.mjs:112-113`).

`instances` on that rung is `copiesOf(owner)` (`:372`, `:390`). Two `Page` calls of `Pressable`, selection on a slotted child with one chain, hold on the button: the held template reports `instances: 2`. A generated label that shares the button's span and chain does not add a copy. The selected node's own rows still use `copiesOf(node)`.

`unescaped` (`:257-270`) follows `escape` in `contract/syntax/src/lexer.rs:500-536`. One pass, so a backslash before `u` stays a backslash: `marker-end="url(#a\\u{62})"` is the text `url(#a\u{62})`, not a backspace. `\n \t \" \\ \` \$`, `\u{9}`, `\u{a}`, `\u{0}`, `\u{2011}`, `\u00e9`, `\u{1F600}` decode; `\q`, `\r`, `\u12`, `\u{}`, `\u{1234567}`, `\u{D800}`, and `\u{110000}` are null. The new test covers the backslash case, `\$`, `\u{9}`, `` \` ``, and `\q` (`tamber-inspect.test.mjs:399`).

A positional after whitespace may be `"`, `` ` ``, or `(` (`:300`). `opacity=0.5 ("hello")` is `0.5`, and `color="param.ink" ("Hi")` is `param.ink`, so the param rung keeps its `template`. `color="red" \`hi\`` is `red`.

The fixture rows are `border_radius_top_left` and `border_color_top` (`tamber-inspect.test.mjs:345-361`), the wire names.

The state rung's `text` is documented as the plain value, omitted otherwise (`docs/tamber-protocol.md:1100-1101`), matching `stateRung` (`tamber-inspect.mjs:205-207`). A layout row is documented as no `set-row`, with a param or token it reads whole still offered (`docs/tamber-protocol.md:1137-1139`).

## Not taken

`plainAt` still accepts `#12`, `1.`, and `.5` (`:295-296`). The lexer does not (`lexer.rs:280-287` requires a digit after the dot and rejects a leading dot; `:359-377` allows only 3, 4, 6, or 8 hex digits). A map is produced only from a file the lexer accepted, and a file that no longer matches its digest is `readOnly` before a rung is shown (`tamber-inspect.mjs:407`). Leaving those forms in the reader is sound.

## Regressions

None in the other offers. A known param on the held `:active` row stays the param rung, with the template address and the owner's `instances` beside it. The round 2 literals still read: an exponent, nested parentheses, a minus, a bare `#1f9d62`, a following quoted string, and `-( -0.5)` is still null, as `plain` is once the parentheses unwrap to a second unary (`tamber_rows.rs:89-94`).

## Smaller points

`opacity=0.5("hello")` with no space, `color="red" name`, and `opacity=0.5 mix("a")` are still null. The parser treats those as a plain value plus a positional (`parser.rs:1215-1216`). The forms named in this fix, written with a space, pass.

The held-inline assertion never passes an unread token, and both nodes in that fixture have one chain, so `instances: 1` does not show that the count is the owner's. The hover check is only that the rung is not `"template"` (`tamber-inspect.test.mjs:117`).

`docs/tamber-protocol.md:1132-1134` still says an `:active` press row's edit is the state rung. For a row written under the node, the edit is the template rung (`:1107-1109`).
