# Code review: LLP 1121 stage 3, commit 7, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `f93c9248efae2e8c28dbc5d7375c310a14c2ba0553dc68019dd116958e5ad139`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, two low; Grok: two high, four medium, one low.
- **Disposition:** fixed in round 1 (the theme built as Tailwind builds it, whole blocks only, backing variables and inline keys, resets, defaults, none components); named palette steps stay whole names and tracking stays a length, for theme1's RFC wording

---

Found **four medium** and **two low** issues. **No high-severity findings.**

1. **Medium — `@theme default` can overwrite explicit values.** [tailwind.rs:153](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:153) discards theme options, and the engine chooses the last declaration. Consequently, `@theme { --color-brand: red } @theme default { --color-brand: blue }` imports blue. Tailwind preserves the explicit red value regardless of the later default. **Fix:** retain declaration options and apply default precedence before assembling tokens, including when processing resets. Add both declaration orders to the tests. [Tailwind’s implementation](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/tailwindcss/src/theme.ts#L55-L79).

2. **Medium — ordinary `@theme inline` aliases cannot resolve their backing variables.** [tailwind.rs:182](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:182) excludes light `:root` declarations; the namespace classifier also excludes backing variables such as `--ink`. Thus this valid pattern produces no colour token:
   ```css
   :root { --ink: #111; }
   .dark { --ink: #eee; }
   @theme inline { --color-ink: var(--ink); }
   ```
   **Fix:** retain auxiliary custom properties for reference resolution while exporting only mapped theme names. Preserve `inline` information when determining reference scope: it substitutes the expression into utilities, rather than freezing its referenced value. [Tailwind’s reference semantics](https://tailwindcss.com/docs/theme#referencing-other-variables).

3. **Medium — dark resets leave erased colours active.** [tailwind.rs:193](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:193) reports every `initial`, but records resets only when `in_theme && !dark`. For `@theme { --color-x: white } .dark { --color-x: black; --color-x: initial }`, the earlier black still becomes the dark side. An explicit custom-property reset makes that value invalid; it does not preserve black. **Fix:** retain reset markers per target, invalidate earlier winners and dependent references, and report modes that cannot be represented. Test resets followed by fresh declarations too. [CSS reset semantics](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid).

4. **Medium — selector substring matching invents global dark overrides.** [tailwind.rs:109](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:109) accepts any occurrence of `.dark`. This classifies `:root:not(.dark)` as dark and promotes `.dark .card` or `.dark:hover` declarations into unconditional dark tokens. Shadcn’s existing classifier rejects restricted selectors instead. **Fix:** classify selector structure and nesting, supporting the declared theme selectors while reporting restrictions that Tamber cannot represent. Cover negation, component scope and selector lists.

5. **Low — named palette steps lack the requested naming rule.** [tailwind.rs:35](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:35) splits only numeric suffixes. A named step such as `--color-brand-foreground` therefore becomes `tw.brand-foreground`, never a dotted palette step. **Fix:** explicitly define how named steps are distinguished from hyphenated standalone names, implement that convention, and assert both named steps and multi-dash numeric palettes. The current tests leave this D14 ambiguity unresolved.

6. **Low — tracking’s pending kind disagrees with D14.** [tailwind.rs:62](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:62) reports `--tracking-*` as `length`; [D14](/tmp/rv-s3c7/llp/1121-tamber.rfc.md:1075) groups it with numbers. Tailwind’s fixture actually uses lengths such as `-0.05em`. **Fix:** correct D14’s tracking classification to match those values and add an explicit pending-kind assertion.

The engine extraction otherwise appears faithful to shadcn. The 13 `none` colours are appropriately reported under the existing folding limitation. Independently counting the fixture confirms **419 declarations = 275 remaining colours + 13 `none` colours + 131 other declarations**. `unfolded` does introduce a full check per literal resolution, potentially repeated through references; I could not establish a performance regression. Its empty role vocabulary does not newly exclude roles because the preceding colour parser already rejects them.

Caps, formatting checks on the changed Rust files, and diff whitespace checks passed. The two new tests cover the advertised fixtures and unchanged re-imports, but not the cases above. Rust tests were not run: no local `target/` exists, and the read-only sandbox prevents building them. No files were edited.
