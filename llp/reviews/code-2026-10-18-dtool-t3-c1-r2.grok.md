# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `bd246a813ee1e4efb8288a9fc64e02041bdd3f893f5c6596727c73e253cc2f65`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high; smaller points).
- **Disposition:** all taken in `a90dbbf0c`: a new plan's read keeps the names but offers no alias until it answers, and a failed read (an error or a 504) clears the plan stamp too, so the next selection reads again; the IR name is matched as a whole word of its real shape (`param.f<8 hex>.<Component>.<param>`), so a token that only contains one is left be; the custom property a JS page writes for a param (`var(--x-param-…)`, modes.rs) is named in the text too; a role override aliases by any spelling of its role. The protocol says a row whose one name is a param links it, and that a client that knows no param rung offers no edit. Not taken: an icon param aliases no token (no operation writes an icon).

One bug blocks. A plan-triggered `describe` that fails leaves the previous param map in place and does not try again.

## Correctness

**A failed re-read for a new plan keeps the old alias, and the next selection does not retry.** `scripts/tamber-server.mjs:372-376`

`learnParams(null, planDigest)` is how a package component change is picked up: owned `hello.files` do not include that file, so only the new plan digest starts a read (`scripts/tamber-server.mjs:221`). That call passes no files, so line 369 does not empty the map. The plan stamp is written before `describe` returns (`:370`).

On failure the handler only clears the file key:

```372:376:scripts/tamber-server.mjs
    ask({ method: 'describe' }).then(r => {
      if (at !== params.reading) return;
      if (r?.ok?.ir) params.map = paramsOf(r.ok.ir);
      else params.key = null;
    }, () => { if (at === params.reading) params.key = null; });
```

`ask` resolves a timeout as `{status: 504}` (`:335`); it does not reject. Either way `params.plan` stays at the new digest and the map stays.

Scenario: `Card.fill` aliases `ink` under plan P1. A package default changes to `paper`, the app rebuilds as P2, and no owned file changes. The selection calls `learnParams(null, "P2")`, keeps `{token: "ink"}`, and `describe` errors or hits the 120s timeout. The next click is still P2, so line 221 does not call `learnParams` again. The watcher does not run, because it watches owned files. The rung still offers `ink`, and re-scope edits `ink`. A late success after the timeout is dropped, because the promise already settled. The stale alias lasts until some owned file changes.

A file-change read does fail closed: line 369 empties the map before the request, and a failure nulls the key so the next hello retries. The plan path does not.

## Round 1, and the rest of the checklist

These are fixed and the fix matches the code:

- The `selected` event publishes `joined.publish`, the same scrub as `selection` (`scripts/tamber-builds.mjs:327-329`, `scripts/tamber-server.mjs:239`). A refusal is still `{status}` and is returned as the HTTP response.
- A synthetic name is scrubbed anywhere inside the string, not only when the name starts with `param.f…`. The inspect fixture is the whole `color-mix(…)` expression.
- A file change empties the map until `describe` answers. An older reply is dropped with `params.reading`. A restart starts from an empty map. Two overlapping reads keep the later one.
- A param with `set.whens > 0` gets no alias. `modes` is the row's tuple, and the protocol says the client reads the winning branch with `explain`.
- A role override aliases: `label` and `-exact-label` both resolve to the row spelling. An unoverridden role is absent from `ir.tokens`, so it aliases nothing.
- An inherited or file-moved row whose name is the full param links that name (`scripts/tamber-inspect.mjs:171`, `:217`). `unread` is stripped before the row is sent.
- Callers of `buildChannel` that still return `null` publish the raw report (`joined?.publish ?? r.selected`). The inspect test does that. The only production callback is the server, which returns `{publish}`.

**The regex covers real IR names.** Production names are built by `token_name` in `contract/lower/src/params.rs:25-36`: `param.` + `f` + eight lowercase hex digits + `.` + component ident + `.` + param ident. The hash is FNV-1a. Component and param are Contract idents: an ASCII letter or `_`, then letters, digits, `_`, and `-` only before a letter (`contract/syntax/src/lexer.rs:247-263`). `param\.f[0-9a-f]+\.[A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)*` matches every such name, including a hyphenated one like `My-Button.font-size`. It does not stop early inside an ident, so a real token is not split and the lookup key is the whole token.

`tamber/src/params.rs:168` is a unit-test helper. It uses the file stem (`param.fpost.Card.inset`). That string is not what the compiler emits, and the regex would miss it because `o` is not hex.

The regex is wider than real names: no left boundary, any hex length, and any number of extra dotted segments. A non-param that merely contains `param.f<hex>.Ident` is treated as unread and replaced with "a component param". Design tokens cannot start with `param.`, and rewrite only substitutes a whole `param.<ident>`, so compiler output does not emit that shape except for a real param. The failure mode is a false unread row, and the dotted spelling is still removed.

Dotted `param.f<hash>.…` does not remain in `selection` or `selected`, or in `link` or `reason`. `reason` is either a fixed unread sentence or the rewritten names. `link` is a full name or a declared token.

## Smaller

- **JS dynamic text still carries the hash, in the dashed custom property.** `host/web-js/inspect-dev.js:109-112` sets `token.text` from the inline style when it is not a single `var(--…)`. `host/web-js/src/modes.rs:110-120` turns `param.f3f9a01c2.PostButton.fill` into `--x-param-f3f9a01c2-PostButton-fill`. A dynamic row whose style is `color-mix(in oklab, var(--x-param-f3f9a01c2-PostButton-fill) 50%, black)` keeps that string in `token.text` on both events. The regex looks for dots. Round 1 reported this; `98557610a` does not change it. A single `var(--x-param-…)` is fine: text is replaced with `names[0]`, then scrubbed.
- **Role case.** `paramsOf` stores `t.role` and `t.name` exactly (`scripts/tamber-inspect.mjs:120`). The checker resolves `Label` through an ASCII-lowercase role lookup (`tamber/src/check.rs:346-354`, `:1254-1257`). A default written `Label` gets a param rung and no alias. `label` and `-exact-label` work, which is what the protocol states.
- **An expression row does not link the param.** `isParam` requires the name to equal the full name (`scripts/tamber-inspect.mjs:170-171`). The synthetic name is the whole `color-mix(…)`, so `link` is null. The protocol says a row reached through a param links it by its full name. Inherited and file-moved direct rows do.
- **An icon alias is missed.** `paramsOf` indexes `ir.tokens` only. An icon lives in `ir.icons` (`tamber/src/ir.rs:169`). `Button.glyph = icon.heart` gets no `token`.
- **Protocol.** The 1.11 paragraph matches the join for the dotted name, `whens`, role spelling, `modes`, and both events. It says a failed read is tried again. A failed plan read is not, until an owned file changes.
- **Tests.** The inspect test covers the expression, the role spelling, `whens`, the inherited link, the moved-file link, and `published`. The server test checks both events and a failed hello followed by a new plan. That retry works because `params.plan` is still null, so the selection looks like a new plan. It does not fail a `describe` after the plan is already stamped, and it never changes files, so it does not catch the stale map. The `param\.f[0-9a-f]+\.` assertion also does not see `--x-param-f…`.
