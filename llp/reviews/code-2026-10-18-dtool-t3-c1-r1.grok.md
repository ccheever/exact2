# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ebd279c80`.
- **Method:** one brief (sha256 `c226f06af73fa1857c291c7b24e0372b5b94e6933e96d20c4300f662621cc68a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high; smaller points).
- **Disposition:** all taken in `fcfca3753`: the published `selected` report is scrubbed as the join is (the builds channel publishes what the join hands back); IR names are found anywhere in a name, so a synthetic token's expression is named too, and its test fixture is now the whole expression; a file change empties the map until `describe` answers, a failed read retries at the next change, and a new plan reads again; a param with `when` lines aliases no token, a role override aliases by its role's spelling; a row reached through a param links it by its full name; the internal unread mark never leaves the join. `modes` is the copy's tuple, as the token rung's (the tool reads the branch by `explain`): the protocol now says so rather than 'the winning branch'. A package component's change re-reads at the next plan (the map is kept meanwhile). Not taken: the describe cost is one fold per file change, measured at about 118 ms on the clone before rt1's cache.

Two correctness bugs should be fixed before this meets the ruling. The param rung itself joins the right row, and a missing describe fails closed. The raw `selected` event still carries the IR (intermediate representation) name, and a file change can keep serving the previous param map.

## Correctness

**[P1] The raw `selected` event still publishes the plan's IR param name — `scripts/tamber-builds.mjs:327`**

`join` rewrites `token.names` and `token.text` on the `selection` event (`scripts/tamber-server.mjs:219`). After that callback returns, the build channel sends the host report unchanged:

```327:327:scripts/tamber-builds.mjs
        send({ event: 'selected', build: b.id, selected: r.selected ?? null });
```

Select a node whose `background_color` is `param.f3f9a01c2.PostButton.fill`. Every subscriber of `/__tamber/events` gets that string in `selected.rows[].token.names` and `token.text`, on the same report that also produced the scrubbed `selection`. The new server test only stringifies the `selection` event (`scripts/tamber-server.test.mjs:98-100`), so this passes.

**[P1] After the files move, the param map stays the previous describe until a later one succeeds, and a failed describe is never retried — `scripts/tamber-server.mjs:360-364`**

```360:364:scripts/tamber-server.mjs
  const learnParams = files => {
    const key = JSON.stringify(files);
    if (params.key === key) return;
    params.key = key;
    ask({ method: 'describe' }).then(r => { if (params.key === key && r?.ok) params.map = paramsOf(r.ok.ir); }, () => {});
  };
```

The key check drops an older describe that returns after a newer one. The map is left in place when the key changes. A live edit that the build has already taken stays editable (`designMoved` allows that), so the rung is offered from the old map.

Change `Card.fill` from `ink` to `paper` and select before `describe` returns. The row is still `{rung: "param", param: "app.contract#Card.fill", token: "ink"}`. Re-scope edits `ink`. If that `describe` errors, or hits the 120s `answerMs` timeout (the promise resolves once, so the late success is discarded), `params.key` is already the new files and the next hello with those digests returns immediately. The stale alias stays until some other file changes. An empty map on first boot is the safe case: the row is read-only and says it has not been read. A previous map is not.

**[P2] The alias is the params line's unconditional value, so a winning `when` that names another token is ignored — `scripts/tamber-inspect.mjs:121-122`**

`set.whens` on the wire is a count, not the branch values (`contract/cli/src/tamber_params.rs:286`). `paramsOf` reads `p.set?.value ?? p.default` and, if that whole string is a name in `ir.tokens`, stores it.

```
params
  Card.fill = ink
    when dark-style=dim: paper
```

On a dim copy the rung is `{token: "ink", modes: "dark-style=dim"}` (or the full tuple, below). The value on screen is `paper`. Re-scope follows `ink`. A line that is a literal (`6px`) with a `when` that names `radius.md` omits `token` on the copy that does alias a token. A `set` line that names a token does win over the default; that part of the rule is right, and the unit test only covers a literal `set` (`radius` → `6px`).

A param whose value is another param does not leak. `shown` writes the other param's full name, `hide` has removed param tokens from `ir.tokens`, and `tokens.has` is false, so `token` is omitted.

**[P2] A bare role key is not treated as the token it names — `scripts/tamber-inspect.mjs:117` and `:122`**

The IR publishes a role override under its row spelling (`tamber/src/check.rs:143-145`): a token declared `secondary-label` is `ir.tokens[].name === "-exact-secondary-label"`, with `role: "secondary-label"`. Both spellings are real references. `paramsOf` only asks `tokens.has(written)` of `t.name`.

`Card.fill = secondary-label` therefore joins as a param rung with no `token`. `Card.fill = -exact-secondary-label` does alias. The token rung already accepts the pair (`-exact-secondary-label` in the text, `secondary-label` in `names`); the alias check does not.

**[P2] A JS dynamic row keeps the IR name in `token.text` as a custom property — `scripts/tamber-inspect.mjs:138`, `host/web-js/inspect-dev.js:109-112`**

`named` replaces the dotted IR token, and only where that exact string occurs. On the JS target, a row bound by an expression sets `text` from the inline style. The dev custom property is the same token with dots turned into dashes (`host/web-js/src/modes.rs:112-118`): `--x-param-f3f9a01c2-PostButton-fill`.

`color-mix(in oklab, param.fill 50%, black)` is reported with `names` rewritten to `post.contract#PostButton.fill` and `text` still `color-mix(in oklab, var(--x-param-f3f9a01c2-PostButton-fill) 50%, black)`. The wasm and native runners put the plan's dotted text in `token.text`, and that path is rewritten. `value` on both paths is the computed CSS (`#rrggbb`, a length, …), so the hash does not leave through `value`.

## Checked, and smaller

**`modes` is the copy's tuple, the same string the token rung already forwards.** `runner/src/runner/mode.rs:230-236` formats `modes_of(row.tuple)`: every axis, `axis=value` joined by commas. The tuple stored on the row (`runner/src/instance/theme.rs:259`) is the app tuple that row was resolved in. A param is a theme token resolved in that tuple, so this is the param's own copy. It is the full tuple, which is what the token rung carries; the winning `when` clause still comes from `explain`. The protocol line that calls it "the winning branch as the token rung's" matches that field.

**Scrubbing inside `selection` holds on the paths that go through `named`.** `link` is taken from the rewritten names, and `declared` (`scripts/tamber-inspect.mjs:106`) rejects a full name because of `#`, so an expression or an inherited row whose only name is a param gets `link: null` rather than the IR name. A file-moved row uses `offered.token` (`scripts/tamber-inspect.mjs:195`), which is the alias design token. An unread name is removed from `names`, and `text` becomes `a component param`. A state-part row whose whole value is one param is offered the param rung; the `set-state` refusal runs only when `names` is empty, which is already how a state row that is one design token behaves (`scripts/tamber-inspect.test.mjs:87-88`). Outline sites are indexes. `resolve`, `explain`, and `uses` go through `present`, which rewrites IR tokens to full names. The dev-stream batch and snapshot still name theme tokens the way the plan does; the build applies the live table by those names.

**Inherited and dynamic param rows lose the link.** After the rewrite, `names.find(declared)` skips `post.contract#PostButton.fill`. An inherited row that aliases `primary_500` is read-only with `link: null`, so the client cannot follow the alias. The reason for an inherited or dynamic row also does not say the param was unread; that sentence is only reached for an `authored` / `own` row (`scripts/tamber-inspect.mjs:160`).

**`token.paramUnread` is sent on the row.** It is an internal flag, not in the protocol. Clients that ignore unknown fields are fine.

**`describe` still returns the IR name.** `describe` does not go through `present` (`contract/cli/src/tamber_serve.rs:620`), and the server proxies it (`scripts/tamber-server.mjs:572`). `ir.components[].params[].token` is still `param.f<hash>.…`, which is what `paramsOf` keys on, and which the 1.10 protocol already documents. Any tool client that calls `describe` still receives it.

**Cost.** Every hello whose file map string-changes runs a full `describe` (fold the design with every param, emit the IR), 50ms after the watcher settles, on the same stdio queue as writes. A token edit that does not touch a param pays that too, and that duration is the stale-map window above. Key order in `JSON.stringify(files)` can repeat it when the same digests come back in another order.

**Protocol.** The 1.11 bullet and the selection paragraph match the join's happy path: full name, `short` null when two components share it, `token` optional, unread name dropped and the own row made read-only. They describe the `selection` event only. They do not mention the `selected` event, a `when` on the alias, or the dashed custom property.

**Tests.** The join test covers a direct alias, a literal `set`, a null `short`, an expression, an unknown param, and the no-map case, and it asserts the join JSON has no `param.f…` name. It does not cover a `set` that names a token, a `when`, a bare role key, a param whose value is another param, an inherited or file-moved param row, or a JS dynamic `text`. The server test sleeps 20ms and checks one `selection` event against a fake `describe` that answers immediately. It does not look at the sibling `selected` event, a second file generation, or a `describe` that fails.
