# Code review: Tamber importers, the rest of rt1's QUEUE entry (budget calibrated to the isolate, CSS importers linear and bounded, marks), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6ad67867b9`.
- **Method:** one brief (sha256 `dc66edc08eac9c91aa15058492605bab53f564028c244d22b8c6a574254d39ca`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two blocking (per-level marks storage past 1 GiB and unmatched-brace rescans; label copies outside the copy cap), two should-fix (Tailwind inline-key propagation cubic; wide imports quadratic), one nit (an unread rule's line)
- **Disposition:** all taken in d7594a29a5: marks read once with each brace matched once, rules over ranges; the copy cap counts labels and reports; Tailwind keys by name and readers found once; shadcn and kinds::unfolded indexed; the prelude's line

- **Blocking — nesting cap still permits >1 GiB of parser storage.** [import/mod.rs:497](/tmp/rv-imp1/tamber/src/import/mod.rs:497): every recursive `body` retains another full marks vector. Sixty-four enclosing rules around a custom property containing 520,000 balanced brace pairs total **1,040,201 input bytes**, but retain approximately **1,032 MiB** of populated mark entries. Also, unmatched custom-property braces repeatedly run `matching`: 100,000 braces cause approximately five billion visits. **Fix:** scan punctuation once, precompute matching indices, and recurse over ranges. Add both fixtures.

- **Blocking — `COPIED` excludes per-declaration label copies.** [import/mod.rs:399](/tmp/rv-imp1/tamber/src/import/mod.rs:399), [shadcn.rs:190](/tmp/rv-imp1/tamber/src/import/shadcn.rs:190): a 200,000-character `@layer` name around 6,000 declarations costs only about 200 KB against `COPIED`, while declaration labels alone consume **1.2 GB**, from a roughly 277 KB stylesheet. Tailwind has the same copying pattern. **Fix:** charge retained copies before allocation, or share labels and bound their eventual report expansion.

- **Should-fix — Tailwind’s inline-key propagation remains cubic.** [tailwind.rs:470](/tmp/rv-imp1/tamber/src/import/tailwind.rs:470): redefine the first inline colour key outside `@theme`, then chain subsequent keys through it. Each round discovers one key while repeatedly scanning the growing `shadowed` vector; 2,000 keys require roughly four billion comparisons, before the new caps apply. **Fix:** reverse-reference adjacency plus a visited-set worklist.

- **Should-fix — ordinary wide imports remain quadratic.** [shadcn.rs:510](/tmp/rv-imp1/tamber/src/import/shadcn.rs:510) scans all `others` for every variable; [tailwind.rs:416](/tmp/rv-imp1/tamber/src/import/tailwind.rs:416) scans existing keys per declaration; [kinds.rs:252](/tmp/rv-imp1/tamber/src/import/kinds.rs:252) recounts the entire accumulated text per token. **Fix:** index declarations/keys and maintain a line counter. Consequently, [§10’s “each linear and bounded” claim](/tmp/rv-imp1/llp/1121-tamber.rfc.md:4843) is premature.

- **Nit — unread-rule locations identify the brace, not the prelude.** [import/mod.rs:558](/tmp/rv-imp1/tamber/src/import/mod.rs:558): `.deep\n{` beginning on line 65 reports line 66; copy-limit reports similarly use the body offset. **Fix:** retain the trimmed prelude’s starting offset and test multiline preludes with comments.

The memoization, Tarjan, failure-order, and index changes showed no equivalence mismatch in inspection and read-only algorithm models. Nested `Work` preserves the outer limit. However, the budget test asserts message/timing, not RSS, and omits an over-limit fresh-import assertion.

No files changed or builds run. Memory figures above are allocation calculations, not measured RSS.
