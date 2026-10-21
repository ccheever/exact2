# Code review: LLP 1121 stage 2: scheme=fixed, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f81cd3f48c1c2b4fa2d31f2f911e53992dbb59c1ba0d3346bb393e0538e755cd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, two low; Grok: four medium, two low.
- **Disposition:** fixed in round 1 except an empty scheme= (a value error), named

---

## Summary

`scheme=fixed` is parsed as trailing token metadata (the same `name=value` words dials use, and `=` inside parentheses or quotes stays in the colour), checked against folded tuples, and emitted as an optional `"scheme": "fixed"` field. Named one-scheme tokens move from a warning on every row to one declaration-site line; literals in rows, pure aliases by the declared name, inline themes, and the post-cap `one_scheme` path all match that. The holes are an alternate spelling that still warns through a fixed token, role values whose nested token names are no longer scanned, and a mark that is only half-validated.

## Issues

### Issue 1 -- Severity: bug
- File: contract/lower/src/warnings.rs:953
- Description: Medium: A fixed token is not treated as fixed when an alias reaches it through the synthetic `-exact-` spelling. `fixed_tokens` stores the declared name (`label`), but every entry is also indexed as `-exact-{name}` (`warnings.rs:1088`) and `entry_named` returns whichever key matched. `entry_colours` then compares that key to the fixed list, so `ink = -exact-label` (or `card = -exact-white_100`) where `label` / `white_100` is `scheme=fixed` still collects `#…` and warns. A row that names the fixed token by its declared name stays silent; only the other spelling in the alias chain leaks. That breaks "aliases through a fixed entry are silent."
- Suggestion: Resolve a hit to the entry's declared name before the fixed test (or put every spelling of a fixed token in the set, including `-exact-{name}` and a bare role name). Add a case whose alias uses the spelling the row did not declare.
- Status: open

### Issue 2 -- Severity: bug
- File: contract/lower/src/warnings.rs:1120
- Description: Medium: Role overrides no longer see a one-scheme token nested in the value, so that colour is reported nowhere. The role loop used to call `colours(&e.value)`, which splits the string on spaces and commas and expands each theme word. It now calls `entry_colours`, which `literal_colours`s the whole value and then looks up that entire string (`warnings.rs:956-957`). `Canvas = "color-mix(in srgb, ink 40%, white)"` with `ink = "#111111"`, and no row naming `ink`, used to warn; `color-mix` is not a literal colour, the next lookup is the whole function text, and `ink` is never pushed into `named`, so the declaration pass skips it too. A value that is only `ink` or only `#111111` still warns. `?:`, template text, and `fn` arguments were not regressions: templates never resolved theme names, and `Expr::Call` still does not walk its arguments.
- Suggestion: After the fixed check, run the same word split `colours` uses on the entry value (and record those names), so a role value and a declaration walk agree on nested tokens.
- Status: open

### Issue 3 -- Severity: bug
- File: contract/lower/src/warnings.rs:1190
- Description: Medium: A one-scheme role override that a row also names never gets the `scheme=fixed` repair. The role loop pushes a `token: false` site first (`warnings.rs:1122`); the declaration loop then pushes `token: true` on the same span and name (`warnings.rs:1179`). Dedup keeps the first, so the warning stays "`#…` on `label` stays the same in dark mode: name a role…", not "`label` is one colour in every mode… `scheme=fixed`". The mark itself still silences the role entry. Unnamed role overrides were already on the old text; this drops the new text for the ones rows name.
- Suggestion: Prefer the token site when both exist, or fold `scheme=fixed` into the role repair, and dedup on `(span, name)` only after choosing the message.
- Status: open

### Issue 4 -- Severity: bug
- File: tamber/src/check.rs:164
- Description: Medium: Only the first `scheme` metadata entry is checked. `scheme()` uses `find`, so `ink = #000000 scheme=fixed scheme=dark` is accepted and marked fixed whenever the folded colour does not differ, and the later word is ignored. `fixed_tokens` (`theme.rs:689`) uses `any`, so it agrees on that accept. The reverse order (`scheme=dark scheme=fixed`) is refused, but the warning pass still treats the token as fixed because any pair equals `scheme=fixed`. The new comment says any other `scheme=` word is refused; `label=…` beside a single `scheme=fixed` is fine and should stay.
- Suggestion: Refuse unless every `scheme` value is `fixed`. Share that rule with `fixed_tokens` so a rejected mark cannot silence `lower-colour-one-scheme` on a failed compile.
- Status: open

### Issue 5 -- Severity: bug
- File: tamber/src/syntax.rs:408
- Description: Low: `scheme=` (empty) is not a scheme mark, so it is not `tamber-scheme-fixed`. `meta_word` requires a non-empty value, the suffix is left on the colour, and the checker reports `tamber-syntax` from CSS (`=` is not value text) at the colour span. `scheme=dark` and `scheme=true` are refused with the right id; `scheme=""` unquotes to empty and is refused correctly. A when-line trailing `scheme=fixed` is also left in the colour (`when` does not strip metadata); the token-line mark itself does not steal the predicate.
- Suggestion: Treat a trailing `scheme` with an empty or unquoted-empty value as `tamber-scheme-fixed` on the token name, the same as any word other than `fixed`.
- Status: open

### Issue 6 -- Severity: bug
- File: contract/lower/src/warnings.rs:1259
- Description: Low: Two or more declaration-site token warnings still get the old closer: "these N colours stay the same… name a role for each or leave it unsaid." Each token line already says `light-dark` or `scheme=fixed` (`warnings.rs:1218`); the closer contradicts that, and it is the line an agent sees last on a design full of unmarked one-scheme tokens. A single token returns early and does not hit this. The count itself is not doubled: one token is one warning, and the closer is the existing multi-site extra. One-scheme warnings are still emitted after the 20-warning truncate, same as the old row walk.
- Suggestion: If every remaining site is a token, skip the closer or repeat the token repair (mark `scheme=fixed`, or give it two scheme values) instead of "name a role for each."
- Status: open
