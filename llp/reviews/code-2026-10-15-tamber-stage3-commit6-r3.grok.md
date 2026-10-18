# Code review: LLP 1121 stage 3, commit 6, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `facad30a0c5f9a4abd7b67fb3ce6a5ab41e1862f20fc7d265fe2028bd0bc2604`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium; Grok: one high, two medium, two low.
- **Disposition:** applied without re-review

---

## Findings

### High — A dark theme block's `var()` is stored as a token name the fold resolves to a different colour

`tamber/src/import/shadcn.rs:363`

`seen` walks a dark theme block as own dark, then the theme's light side, then the default dark side, then the default light side (`shadcn.rs:329`). `colour_of` uses that walk only as a yes/no test, then stores `names[r]` (`shadcn.rs:363`). Emission builds the token with `get(theme, dark).or_else(get(default, dark))` (`shadcn.rs:430`), which never falls through to the theme's light side.

```css
:root { --b: #ff0000; }
.dark { --b: #0000ff; }
.theme-blue { --b: #00ff00; }
.dark .theme-blue { --a: var(--b); }
```

The chain stops on the theme light `#00ff00`. The sides map records `shadcn.b`. That token is `light-dark(#ff0000, #0000ff)` with `when theme=blue: light-dark(#00ff00, #0000ff)`, and `--a` becomes a bare `shadcn.b`. Folded at `theme=blue` in the dark scheme, Tamber shows `#0000ff`. On `.theme-blue` the custom property `--b` is `#00ff00`, so `--a` should be `#00ff00`. `check` still passes, because both values are colours. The same-block case (`--ring: var(--foreground)` → `shadcn.foreground`) is fine and should stay a live reference.

Fix: remember which target supplied the winning declaration. Keep the token name when that target is the same scheme this side will fold (own side, or the default side of the same scheme). When the winner is the other scheme — theme light while reading theme dark — store the literal the chain already resolved.

### Medium — The tuple cap ignores the `default` value reconcile will add

`tamber/src/import/shadcn.rs:268`

`theme_values` is `had.len()` plus theme names not already on the axis. The axis that is actually written always starts with `default` (`shadcn.rs:476`), and `reconcile` adds any missing value. If the design has no `theme` axis, `had` is the synthetic `["default"]` and the count matches. If the design already has a `theme` axis that does not list `default`, the guard is short by one.

With another axis of 20 values, one new theme class, and an existing `theme = light | dark`, the guard sees `20 × 3 = 60` and allows the import. The applied axis is `light | dark | default | <new>`: `20 × 4 = 80`, and `check` refuses it. The declarations are not reported as past the cap.

Fix: when an existing `theme` axis does not already contain `default`, add one to `theme_values` before comparing with `MAX_TUPLES`.

### Medium — An unclosed `{` still drops later declarations, or re-parents them

`tamber/src/import/mod.rs:346`

A balanced `--config: {a: b};` is read correctly: `start` stays put and the next `;` takes the braced value. Two recovery paths still lose or move source values.

- A custom-property `{` with no closer sets `k` to the end of the marks (`mod.rs:347`) and one trailing `declaration` swallows the rest of the block. In `:root { --config: {a: b; --after: #fff; }`, `--after` never increments `source` and is not reported.
- An unclosed rule takes the rest of the file as its body (`mod.rs:350`). A later `:root` or `.dark` is nested under that prelude, so `nested` assigns it to the unclosed theme. `.theme-blue { --a: #111;` followed by `:root { --b: #fff; }` files `--b` under theme blue.

Fix: a `{` whose prelude is a custom property should end the value at the next top-level `;` after the matching `}`, and if there is no matching `}` treat only that value as unclosed — do not give up the later marks. An unclosed rule should contribute its own declarations, then resume the parent so the following rules stay siblings.

## Checked and holding

- Owned-axis unsets use `all` of the condition axes (`mod.rs:156`). A `when` that also names `density` stays. A theme-only `when` the source no longer has is removed. Role offers emit `set-token` plus one `unset-token-when` per when (`shadcn.rs:527`), and `apply_all` re-parses between them, so the lines stay valid.
- `nested` expands comma lists at each level, replaces `&`, and otherwise uses a descendant combinator (`shadcn.rs:52`). `:root, .theme-blue { &.dark, .dark { … } }` becomes the four selectors those combinations mean.
- `flush` before a nested rule keeps source order, so a later declaration in the same block still wins.
- Themes are gathered from every recognised target before colour classification (`shadcn.rs:248`), so a theme that appears only on a length or a nested dark rule still becomes an axis value.
- Each source declaration is mapped or reported once. A selector list counts as one value: `mapped` when any target it still wins is a colour (`shadcn.rs:391`), reported when it wins none or none of those are colours. `source == mapped + reported` holds. A list that is a colour on only some targets does not also report the failing targets; the count forbids counting it twice.
- `reviewShaped` matches the review `importTheme` sends, and the recount `kept.filter(i => i < old.tokens).length` (`scripts/tamber-server.mjs:426`) is the new token prefix because those ops are a prefix and `kept` is sorted.

Low: `nested` splits on every comma, so a comma inside `:is()` becomes two selectors and the theme or `.dark` inside it is missed (`shadcn.rs:58`). `reviewShaped` accepts a negative `tokens` (`scripts/tamber-server.mjs:300`); the recount then treats every kept op as an offer.
