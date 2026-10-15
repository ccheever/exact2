# Code review: LLP 1121 stage 3, commit 7, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `f1c756ace1eb95ef9285ba56d4cfe8f6f1f2f92f127d9c25077b267f87acda31`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: six medium; Grok: four high, one medium, two low.
- **Disposition:** applied without re-review; selector specificity beyond whole root and dark blocks is not modelled (a dark declaration holds the dark side)

---

No high-severity findings. Six **medium-severity** issues remain:

1. **Dropped tokens leave dangling aliases — [engine.rs:277](/tmp/rv-s3c7/tamber/src/import/engine.rs:277).**  
   With `:root { --a: red; --b: var(--a); } .dark { --a: 1rem; }`, `shadcn.a` is dropped, but `shadcn.b = shadcn.a` is still emitted and counted as mapped. Accepting that proposal against an empty design fails validation. A failure in a named theme causes the same problem. **Fix:** propagate exclusions through references before assigning fates, or replace surviving references with safely resolved literals.

2. **Backing-variable overrides inside `@theme` are discarded — [tailwind.rs:473](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:473).**  
   `@theme { --ink: red; --color-a: var(--ink); } :root { --ink: blue; }` imports `tw.a = red`. The new backing-variable support admits `--ink`, but this guard rejects its override as a pending token kind. **Fix:** retain `UNNAMED` backing-variable redefinitions in the engine’s declarations, just as backing variables declared outside `@theme` are retained.

3. **Inline-dependent rejection misses reference chains — [tailwind.rs:407](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:407).**  
   Consider:
   ```css
   @theme inline { --color-a: red; --color-b: var(--bridge); }
   :root { --color-a: blue; --bridge: var(--color-a); }
   ```
   The importer maps `tw.b` to red, although its inline utility reads blue through `--bridge`. The guard checks only a theme value’s immediate reference; backing declarations and references in ordinary overrides escape it. **Fix:** follow dependencies transitively across both declaration sets, reporting affected tokens or separating CSS-variable values from inline-token values.

4. **The custom-variant reader mistakes raw text for declarations — [tailwind.rs:113](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:113).**  
   `@custom-variant  dark (@media print);`—with two spaces—is missed, so subsequent `@variant dark` values become dark-mode colours instead of being reported. Conversely, comments, strings and `dark-*` names can match; later redefinitions are ignored. **Fix:** read actual at-rule tokens, match the exact variant name, and select its last definition. Tailwind’s parser handles whitespace and comments before registering variants. ([Parser](https://raw.githubusercontent.com/tailwindlabs/tailwindcss/main/packages/tailwindcss/src/css-parser.ts), [registration](https://raw.githubusercontent.com/tailwindlabs/tailwindcss/main/packages/tailwindcss/src/index.ts))

5. **Source-order sorting overrides more-specific selectors — [tailwind.rs:496](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:496).**  
   `:root.dark { --color-a: blue; } @theme { --color-a: red; }` imports plain red. The accepted `:root.dark` selector outranks Tailwind’s generated `:root`, so the dark value should remain blue regardless of that ordering. **Fix:** preserve specificity when choosing winners for accepted selectors, or report combinations whose cascade cannot be represented. ([Specificity rules](https://www.w3.org/TR/selectors-4/#specificity-rules))

6. **An empty first `@theme` loses its insertion position — [tailwind.rs:229](/tmp/rv-s3c7/tamber/src/import/tailwind.rs:229).**  
   `@theme {} :root { --color-a: blue; } @theme { --color-a: red; }` imports red instead of blue. `css_blocks()` omits empty blocks, so `first_theme` points to the later declaration-bearing block. Keyframes-only blocks have the same problem. **Fix:** track the first `@theme` from the parsed structure independently of whether it contains custom properties. [Tailwind retains that first insertion point](https://raw.githubusercontent.com/tailwindlabs/tailwindcss/main/packages/tailwindcss/src/index.ts).

All 13 existing importer tests passed using the prebuilt binary from the matching-source worktree. The additional cases above were traced in source. No files changed.
