# Code review: LLP 1121 stage 3, commit 6, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `facad30a0c5f9a4abd7b67fb3ce6a5ab41e1862f20fc7d265fe2028bd0bc2604`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium; Grok: one high, two medium, two low.
- **Disposition:** applied without re-review

---

No high-severity findings. Seven **medium** findings remain:

1. **Reference validation does not validate the emitted token graph** — [shadcn.rs:363](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:363).  
   This imports all four declarations without warnings:
   ```css
   :root { --a: red; --b: var(--a); }
   .dark { --a: var(--b); --b: blue; }
   ```
   It emits `a = light-dark(red, b)` and `b = light-dark(a, blue)`. Tamber folds both operands and rejects the cycle. Additionally, `seen()` prefers theme-light over default-dark, while emission at line 433 prefers default-dark, so references can resolve differently from their validation. **Fix:** align reference resolution with emission and validate the resulting graph; inline resolved colours or report references that Tamber cannot represent.

2. **Nested component/state selectors become unconditional theme values** — [shadcn.rs:77](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:77).  
   `:root { --background:white; &:hover { --background:red; } }` imports unconditional red and reports white as superseded. Replacing `&:hover` with `.card` produces the same result. Parent resolution now exposes `:root`, but `target()` ignores the remaining restrictions. **Fix:** accept only selectors whose complete meaning is supported; report component or state restrictions instead of removing them.

3. **Tuple overflow can delete existing theme overrides** — [shadcn.rs:269](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:269).  
   With 32 density values and `theme = default | blue`, importing root, blue and new green declarations exceeds 64 tuples. The fallback removes **all** theme targets, including existing blue. Reconciliation then proposes removing the blue `when` from tokens with retained root declarations—even though blue remains in the source. **Fix:** reject the oversized import before reconciliation, or preserve existing themes and exclude refused scopes from removal.

4. **The tuple recount omits an added `default` value** — [shadcn.rs:268](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:268).  
   A valid design with 32 density values and `theme = blue | green` has 64 tuples. Importing `.theme-blue` passes this calculation, but the emitted axis values include `default`; reconciliation appends it, producing 96 tuples and an unapplicable proposal. **Fix:** count the actual union of existing values, `default`, and imported themes.

5. **Nested selector lists expand exponentially** — [shadcn.rs:55](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:55).  
   Each nesting level materializes the Cartesian product before target deduplication. Thirty nested `&,&` lists produce over a billion strings from roughly 200 bytes of CSS, although every selector identifies the same target. **Fix:** resolve bounded target states without constructing the full product; deduplicate intermediate states and explicitly report expansion limits.

6. **The reader invents declarations inside custom-property values** — [mod.rs:283](/tmp/rv-s3c6/tamber/src/import/mod.rs:283).  
   `:root { --config:[a; --background:red; ]; --after:blue; }` is counted as three declarations. It imports the embedded `--background` and offers a platform-role override. Escaped semicolons cause the same problem. **Fix:** respect balanced square brackets and escapes outside strings when locating declaration boundaries, following [CSS component-value parsing](https://www.w3.org/TR/css-syntax-3/#consume-a-component-value).

7. **Bad-string recovery still swallows subsequent declarations** — [mod.rs:273](/tmp/rv-s3c6/tamber/src/import/mod.rs:273).  
   An unterminated quoted value followed by a raw newline leaves both scanners inside the string. A probe containing that declaration and two subsequent colour declarations returns `source=1`, with the rest of the stylesheet embedded in the first report. CSS terminates a bad string at the newline and continues parsing. **Fix:** implement that recovery in both `strip_comments()` and `marks()`, preserving escaped-newline handling. [CSS string-token rules](https://www.w3.org/TR/css-syntax-3/#consume-a-string-token)

Validation was read-only: all seven prebuilt importer tests for this commit passed. Additional CLI probes reproduced the reference, selector and reader failures; in-memory wire checks passed review-shape rejection and repeated token-count renumbering.
