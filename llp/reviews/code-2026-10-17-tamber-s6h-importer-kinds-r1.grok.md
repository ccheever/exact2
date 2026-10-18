# Code review: The importers propose every kind (LLP 1121 stage 6 H), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** HEAD (`3c13cec4d`), the importers' kinds besides colour.
- **Method:** one brief (sha256 `34bd99294d64118ee5a226f35846e6aba73d6170234964e4d6b46a19252c49fd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P1s: shadcn reference chains, a DTCG percentage line height, a Tailwind leading over a token and a weight chain, a design token's written kind; P2: the D14 table and the protocol's kind limit).
- **Disposition:** all taken in the first-round commit `4b20b66ce`, each with a regression test in `tamber/tests/it/import_kinds.rs`.

---

1. **P1** — `tamber/src/import/shadcn.rs:343`. A pure `var()` returns immediately. The target is classified from the first declaration whose value is not itself a `var()`, and a chain of references yields no kind, so the variable’s own name is never considered. Failing input:

```css
:root {
  --font-sans: 16px;
  --font-sans: Inter, sans-serif;
  --font-body: var(--font-sans);
}
```

`--font-sans` is a family (the `16px` declaration is superseded). `--font-body` is reported `` `--font-sans` is no colour where this block reads it ``. The same happens for `--shadow-sm: var(--shadow-md)` when `--shadow-md` is `var(--shadow-lg)` and `--shadow-lg` is `0 1px 2px #111`: `--shadow-sm` is not proposed. Follow the reference chain to the winning declaration (cycle-safe). If that fails, classify by this variable’s name (`--shadow-*`, `--font-*`) and by whether the winning value is a length.

2. **P1** — `tamber/src/import/dtcg/kinds.rs:237`. A typography `lineHeight` that is a dimension object is passed to `length()`, so `{value: 120, unit: "%"}` is written `16px/120%`. A type’s leading accepts a number, a px/rem/em length, or `normal`, and the token is reported `not a type Tamber folds`. The string form `"120%"` already becomes the ratio `1.2` (`kinds.rs:116`). A standalone `$type: "lineHeights"` object hits `number()` (`kinds.rs:105`), which only accepts a JSON number or a string, and is reported the same way. Failing input: `{ "$type": "typography", "$value": { "fontSize": {"value": 16, "unit": "px"}, "lineHeight": {"value": 120, "unit": "%"} } }`. When the unit is `%`, divide by 100 and emit a unitless ratio, in both `typography` and `number("lineHeights", …)`.

3. **P1** — `tamber/src/import/tailwind.rs:757` and `:767`. A leading that contains whitespace or `(` is folded as a number in a one-token probe, so a calc that names another token is refused even though it would fold beside the import. A weight `var()` is one raw lookup, not the key’s number. Failing inputs, inside `@theme`:

```css
--leading-tight: 1.25;
--text-body: 1rem;
--text-body--line-height: calc(var(--leading-tight) * 2);
--font-weight-bold: 700;
--font-weight-heading: var(--font-weight-bold);
--text-body--font-weight: var(--font-weight-heading);
```

The leading becomes `calc(tw.leading.tight * 2)` and the probe has no `tw.leading.tight`, so the type is reported. The weight is stored as `var(--font-weight-bold)`, which is not a weight, and the type is reported. Fold that calc in a probe that contains the theme’s number tokens, and resolve a weight `var()` to a number before composing the type.

4. **P1** — `tamber/src/import/mod.rs:152`. `set-token` replaces the value and leaves an explicit kind in place (`tamber/src/fmt.rs:221`, applied at `tamber/src/edit.rs:555`). A design that already has `shadcn.radius: color = #fff`, imported with `:root { --radius: 8px; }`, is proposed `set-token` value `8px`. Applied, the line is `shadcn.radius: color = 8px`, which does not check. An empty design still checks, because `add-token` writes the kind. When the design token’s written kind differs from the kind being proposed, report that instead of emitting the set-token.

5. **P2** — `llp/1121-tamber.rfc.md:1096` and `docs/tamber-protocol.md:299`. The D14 table still says `--tracking-<n>` is a number and that a shadow’s dark redefinition is mapped. The importer proposes tracking as a length (`tailwind.rs:73`, `-0.025em`) and reports a non-colour redefined outside `@theme` (`tailwind.rs:553`). The protocol limits bullet still says a token’s kind is `color` until stage 6; this commit’s import section (`docs/tamber-protocol.md:534`) says `add-token` writes the kind. Update the table row and that bullet to match stage 6 H.

**Tests.** `import_kinds.rs` covers one-level `var()`, the default theme, derived radii, scheme-sided lengths, and re-import of the op text. Nothing covers a multi-hop shadcn `var()`, a Tailwind weight `var()` chain, a line-height `calc()` that names another token, a DTCG line-height dimension in `%` (object form, or `$type: "lineHeights"`), or `set-token` against an existing token whose explicit kind is different. A non-colour `when` line is checked for Figma numbers only; a shadow or type that differs by an axis is untested.

RIGHT WITH CHANGES
