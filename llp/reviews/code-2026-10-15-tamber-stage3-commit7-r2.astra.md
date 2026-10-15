# Code review: LLP 1121 stage 3, commit 7, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `d19ae85ffd4b3bd47f352b8ff7bd6f1918bec4c07dbe675c4d184b99624d4a92`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, three low; Grok: three medium, three low.
- **Disposition:** fixed in round 2

---

No high-severity findings. Seven medium and three low findings remain.

1. **Medium — Root overrides leave stale dark values.** [tailwind.rs:408](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:408)  
   `@theme { --color-a: red; } :root { --color-a: blue; }` produces `light-dark(blue, red)`, although the unconditional root override applies in both schemes. Theme declarations populate both targets, but root declarations replace only light. Preserve the distinction between unconditional root and light-only selectors, and apply unconditional overrides on both sides.

2. **Medium — Separating theme and ordinary declarations loses cascade order.** [tailwind.rs:344](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:344)  
   `:root { --color-a: blue; } @theme static { --color-a: red; }` should resolve to red. The importer always appends ordinary declarations after theme declarations, so blue wins its light side. Preserve the position and context where Tailwind emits the completed theme: the first `@theme` rule. [Tailwind implementation](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/tailwindcss/src/index.ts#L603)

3. **Medium — Single-key resets bypass `default` precedence.** [tailwind.rs:283](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:283)  
   `@theme { --color-a: red; } @theme default { --color-a: initial; }` removes `a`. Tailwind retains red: its default guard runs before single-key deletion. Namespace resets intentionally run before that guard. Split these cases rather than treating every `initial` identically. I confirmed this distinction by executing Tailwind’s `Theme.add`. [Tailwind implementation](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/tailwindcss/src/theme.ts#L55)

4. **Medium — Ordinary CSS `initial` incorrectly restores an older colour.** [tailwind.rs:330](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:330)  
   With `@theme { --color-a: white; } .dark { --color-a: initial; }`, removing the dark declaration resurrects white. Outside `@theme`, `initial` makes an unregistered custom property invalid; it does not undo the declaration. Preserve an explicit invalid value in resolution and report the affected colour instead of importing white. The assertion at [import.rs:555](/tmp/rv-s3c7/tamber/tests/import.rs:555) currently enshrines the incorrect result. [CSS custom-property semantics](https://www.w3.org/TR/css-variables-1/#guaranteed-invalid-value)

5. **Medium — Dropping inline-key redefinitions also breaks references to those keys.** [tailwind.rs:338](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:338)  
   For `@theme inline { --color-a: red; --color-b: var(--color-a); } .dark { --color-a: blue; }`, `a`’s utility stays red, but `b`’s utility reads the overridden CSS variable and becomes blue. The importer drops the override globally and exports `b = tw.a`, making both red. Keep CSS-variable resolution separate from inline utility values, or report dependent mappings that cannot be represented. [Tailwind inline semantics](https://tailwindcss.com/docs/theme#referencing-other-variables)

6. **Medium — `@variant dark` ignores its custom definition.** [tailwind.rs:189](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:189)  
   `@custom-variant dark (@media print);` followed by `:root { @variant dark { --color-a: black; } }` becomes a dark-side override even though it applies to print. Custom selectors can likewise make it narrower than a whole theme. Read the definition before classifying uses; recognize supported definitions and report the others. [Tailwind variant registration](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/tailwindcss/src/index.ts#L328)

7. **Medium — A dark-only aux cycle is silently filled with the light colour.** [engine.rs:233](/tmp/rv-s3c7/tamber/src/import/engine.rs:233)  
   Example: `@theme inline { --color-a: var(--x); } :root { --x: red; } .dark { --x: var(--y); --y: var(--x); }`. Cycle detection terminates correctly, but the declaration is counted as mapped because light resolves. `pair` then converts the missing dark result into red everywhere. Distinguish an absent side from a failed resolution, and report the affected theme key when a required side fails.

8. **Low — Backing variables declared inside `@theme` never reach aux resolution.** [tailwind.rs:347](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:347)  
   `@theme { --ink: red; --color-a: var(--ink); }` reports `--ink` and consequently fails to resolve `a`; placing `--ink` in `:root` works. Keep surviving non-colour theme declarations in the resolution environment as aux values, while reporting their own namespace once.

9. **Low — `none` normalization depends on whitespace and component position.** [mod.rs:226](/tmp/rv-s3c7/tamber/src/import/mod.rs:226)  
   It fixes the default palette’s trailing `none`, but misses `hsl(none 0% 50%)`, uppercase `NONE`, and `/none`. Thus shadcn’s valid bare HSL `none 0% 50%` still fails mapping. Replace component tokens case-insensitively rather than splitting on spaces. [CSS missing components](https://www.w3.org/TR/css-color-4/#missing)

10. **Low — Selector expansion overflow becomes an unconditional root block.** [tailwind.rs:240](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:240)  
    `engine::nested` returns an empty vector above 1,024 selectors. The subsequent `.all(...)` succeeds vacuously, accepting an oversized list of component selectors as root declarations. Reject an empty expansion explicitly; distinguish it from the single empty selector representing no enclosing rule.

No cache-context correctness issue found: `unfolded` always checks the same isolated probe with an empty vocabulary.

Caps, importer formatting, and `git diff --check` passed. The fixture census matches **419 declarations: 288 colours, 131 others**, including 13 colours with `none`. Rust tests were not run: this worktree has no build artifacts and the sandbox forbids writes. No files were changed.
