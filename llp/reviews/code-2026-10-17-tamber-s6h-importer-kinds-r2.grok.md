# Code review: The importers propose every kind (LLP 1121 stage 6 H), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD (`3c13cec4d`, `4b20b66ce`), the importers' kinds and the first round's fixes.
- **Method:** one brief (sha256 `0394d5a2e4579a776d9a98d8b6cc1565cf140cad0159c32b76e1da2b5e36a37e`), shared by both reviewers, blind to the other review, with the first round's records beside the worktree. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P0: a type alias through two hops; P1s: a leading or weight naming another type's sub-property, a bare zero length, a type size naming a length key; P2s: D14's alias bullet, a typography's `normal` leading and zero letter spacing).
- **Disposition:** the last round, applied without re-review in `88e35b8ec`, each with a regression test.

---

1. **P0** — `tamber/src/import/tailwind.rs:792`. A type size that is `var(--text-<m>)` is peeled once: `value_of` returns that key’s raw value, then `kinds::bare` substitutes token names. One hop onto a literal size stays the size (`import_kinds.rs:295`). A second hop substitutes the assembled type token, and `tamber/src/typeface.rs:75` copies that token’s whole type, weight and leading included. The probe design contains both tokens, so the check passes.

```css
@theme {
  --text-sm: 16px;
  --text-sm--line-height: 1.4;
  --text-sm--font-weight: 700;
  --text-base: var(--text-sm);
  --text-copy: var(--text-base);
}
```

`--text-base` becomes `16px`. `--text-copy` becomes `tw.type.sm`, which folds to `700 16px/1.4`. The source is only a font-size alias of `16px`. The same arm with `--text-copy--font-weight: 400` builds `400 tw.type.sm`, which fails the fold and is reported instead. Resolve a type-to-type `var()` until the value is the font-size property, and never substitute the assembled type token.

2. **P1** — `tamber/src/import/tailwind.rs:747` (`value_of`), used by `written_out` at `:806` and `:815`. `value_of` returns a value only when the exact variable is an owning base key. A leading or weight `var()` that names another type’s sub-property is refused with `` `--text-sm--line-height` makes no token of another kind ``. Stage 6 H says those `var()`s are written as what the keys hold. Base keys such as `--leading-tight` still work (`import_kinds.rs:375`); this is the hole left by narrowing `value_of` to `base_of`.

```css
@theme {
  --text-sm: 16px;
  --text-sm--line-height: 1.5;
  --text-sm--font-weight: 700;
  --text-body: 16px;
  --text-body--line-height: var(--text-sm--line-height);
  --text-body--font-weight: var(--text-sm--font-weight);
}
```

Expected `tw.type.body: type = 700 16px/1.5`. Actual: the type is reported. Return the raw theme value of any key in `others` whose base owns the token, and keep refusing a key redefined outside `@theme` or a collision loser (`--radius-s1` when `--radius-1` owns the name).

3. **P1** — `tamber/src/import/shadcn.rs:576`. `looks_like_length` accepts only `px`/`rem`/`em`/`%` and `calc(`. A unitless `0` is a length: `tamber/src/measure.rs:213` folds it to `0px`. `:root { --radius: 0; }` and `:root { --gap: 0; }` (also `0.0`) are reported “not a colour, nor a length, shadow or family by its name and value”, and `--radius: 0` derives no `sm`/`md`/`lg`/`xl`. `--radius: 0px` would propose the length and, because it is px, all four derived sizes. Treat a trimmed value that parses as zero as a length and propose `0px`. Leave any other unitless number reported.

4. **P1** — `tamber/src/import/tailwind.rs:796`. The type-size special case applies only when the referenced key is itself a type. A size that is `var()` of a length key goes through `kinds::bare`, so the whole type value is that length’s token name. `tamber/src/typeface.rs:75` then reads it as a type alias, while `typeface.rs:168` would accept the same name as a size word.

```css
@theme {
  --spacing-4: 1rem;
  --text-body: var(--spacing-4);
}
```

Reported: `` `tw.space.s4` is not a type token ``. The font-size is `1rem`, which folds as a type. Write that length out (`1rem`), the same way a one-hop type reference writes the raw size.

5. **P2** — `llp/1121-tamber.rfc.md:1125`. D14 still says every `{a.b}` alias becomes a token reference. Stage 6 H (`:2987`) and `tamber/src/import/dtcg/kinds.rs:1` write a non-colour alias out, including inside a composite. Colour aliases stay references (`dtcg.rs:1010`). Update the D14 bullet to say that.

6. **P2** — `tamber/src/import/dtcg/kinds.rs:251`. A typography `lineHeight` string is passed to `number()`. `"normal"` is a weight name only when the type starts with `fontWeight` (`:153`), so `{ "fontSize": "16px", "lineHeight": "normal" }` is reported `` `normal` is no number ``. A type leading accepts `normal` (`tamber/src/typeface.rs:207`). The letter-spacing quiet list (`:220`) is exact strings, so `"0.0px"` also refuses a typography that sets nothing. Accept `normal` as a leading, and treat a zero letter-spacing dimension as quiet after parsing it.

**Tests.** `import_kinds.rs` covers the round-1 cases: one-hop type size, collision loser, quoted family, DTCG comma family and `%` line height, kind mismatch, shadcn `vh` override, colour-role offers, reference chains, Astryx collision and missing side. Nothing covers a two-hop `--text-*: var(--text-*)` whose end key has a weight or leading, a leading or weight `var(--text-<m>--line-height)` / `--font-weight`, a type size `var(--spacing-*)`, or shadcn `--radius: 0` / `--gap: 0`. These were not executed here.

RIGHT WITH CHANGES
