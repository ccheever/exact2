# Code review: The importers propose every kind (LLP 1121 stage 6 H), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD (`3c13cec4d`), the importers' kinds besides colour.
- **Method:** one brief (sha256 `34bd99294d64118ee5a226f35846e6aba73d6170234964e4d6b46a19252c49fd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: overrides of a Tailwind type's sub-properties and of a weight it names, a type alias copying weight, a shadcn winner of no kind, Astryx name collisions and a missing side, a Tailwind reference to a collision's loser, quoted family text, a DTCG family string with commas; P1s: a design token of another kind, a colour role over a length, one-hop references).
- **Disposition:** all taken in the first-round commit `4b20b66ce`, each with a regression test in `tamber/tests/it/import_kinds.rs`.

---

1. **P0 — Tailwind ignores overrides of merged type fields.** [tailwind.rs:745](/tmp/rv-s6h-r1/tamber/src/import/tailwind.rs:745)

   Input: `@theme { --text-sm:12px; --text-sm--line-height:1.5; } .dark { --text-sm--line-height:2; }`

   The importer reports the dark declaration but still proposes `tw.type.sm: type = 12px/1.5`. The redefinition check examines only `--text-sm`, so the proposed token has the wrong dark line height. Tailwind reads these sub-properties separately. [Tailwind documentation](https://tailwindcss.com/docs/font-size#customizing-your-theme)

   **Fix:** check every carried sub-property before constructing the type token. Add tests overriding both line height and weight in root and dark blocks.

2. **P0 — Inlined Tailwind weights can use a rejected, stale value.** [tailwind.rs:770](/tmp/rv-s6h-r1/tamber/src/import/tailwind.rs:770)

   Input:
   ```css
   @theme {
     --font-weight-heavy: 700;
     --text-body: 16px;
     --text-body--font-weight: var(--font-weight-heavy);
   }
   :root { --font-weight-heavy: 800; }
   ```

   The weight key is reported as redefined, but `value_of()` still reads its original `700`. The importer proposes `700 16px`; the source resolves to `800`.

   **Fix:** resolve copied values through the same override/refusal tracking as their dependencies. Test a type whose referenced weight is redefined or refused.

3. **P0 — Aliasing a Tailwind font size copies unrelated typography.** [tailwind.rs:752](/tmp/rv-s6h-r1/tamber/src/import/tailwind.rs:752)

   Input: `@theme { --text-base:16px; --text-base--font-weight:700; --text-copy:var(--text-base); }`

   The proposal makes `tw.type.copy = tw.type.base`. Tamber therefore copies weight `700` too, although the source alias contains only the font size.

   **Fix:** distinguish references to source scalar values from references to assembled type tokens. Add a regression asserting the alias’s **folded** weight and leading remain unspecified.

4. **P0 — shadcn ignores unsupported winning overrides.** [shadcn.rs:259](/tmp/rv-s6h-r1/tamber/src/import/shadcn.rs:259), [shadcn.rs:420](/tmp/rv-s6h-r1/tamber/src/import/shadcn.rs:420)

   Input: `:root { --gap:8px; } .dark { --gap:2vh; }`

   Only the recognized length enters `others`. The dark declaration is reported, but excluded from the winner table, so `shadcn.gap: length = 8px` is nevertheless proposed for both schemes.

   **Fix:** inspect every winning declaration before admitting a variable. An unsupported winner must prevent mapping the whole variable. Test unsupported dark and theme overrides.

5. **P0 — Astryx silently merges colliding non-colour names.** [astryx.rs:306](/tmp/rv-s6h-r1/tamber/src/import/astryx.rs:306), [astryx.rs:367](/tmp/rv-s6h-r1/tamber/src/import/astryx.rs:367)

   Give both `light` and `dark` the values `{"--spacing-1":"4px","--spacing-s1":"8px"}`.

   Both names become `astryx.space.s1`. The importer keeps `4px`, counts **both** source tokens as mapped, and reports nothing. The arithmetic passes despite losing `8px`.

   **Fix:** group only genuine parts of the same text style; report other name collisions individually. Extend the existing colour-collision test to lengths and colliding text-style names.

6. **P0 — Tailwind references can resolve to the wrong collision owner.** [tailwind.rs:691](/tmp/rv-s6h-r1/tamber/src/import/tailwind.rs:691)

   Input: `@theme { --radius-1:4px; --radius-s1:8px; --spacing-gap:var(--radius-s1); }`

   `--radius-s1` is reported as colliding, but remains in `base_of`. Consequently, `tw.space.gap` references `tw.radius.s1`, which belongs to `--radius-1` and folds to **4px instead of 8px**.

   **Fix:** build reference mappings from accepted source-name owners, or resolve the original value before substitution. Test references to collision losers.

7. **P0 — Astryx invents values on a missing scheme side.** [astryx.rs:318](/tmp/rv-s6h-r1/tamber/src/import/astryx.rs:318)

   Input: `{"light":{"--spacing-1":"4px"},"dark":{}}`.

   The `(Some(v), _)` arm proposes a constant `4px`, manufacturing a dark value absent from the resolved source.

   **Fix:** require a valid value on both sides, or report the missing side. Tests currently cover differing values, but not missing or non-string sides.

8. **P0 — Shared CSS conversion changes quoted font names.** [kinds.rs:35](/tmp/rv-s6h-r1/tamber/src/import/kinds.rs:35)

   Input: `@theme { --font-brand:"A  B"; }`, with two spaces inside the quotes.

   `split_whitespace()` changes the name to `"A B"`. The subsequent `find("var(")` also substitutes text inside quoted names, where it is literal text rather than a CSS reference.

   **Fix:** preserve strings and escapes, and normalize whitespace/substitute references only outside strings. Add quoted-name tests containing repeated spaces, literal `var()`, and Unicode.

9. **P0 — DTCG single font names containing commas become fallback stacks.** [dtcg/kinds.rs:163](/tmp/rv-s6h-r1/tamber/src/import/dtcg/kinds.rs:163)

   Input: `{"f":{"$type":"fontFamily","$value":"Acme, Display"}}`.

   The proposal’s unquoted `Acme, Display` names two fonts. DTCG defines a string as **one** font name; fallback lists use arrays. [DTCG §8.3](https://www.designtokens.org/tr/2025.10/format/#font-family)

   **Fix:** serialize scalar font names as single CSS family members, quoting when necessary. Test scalar comma-containing names against arrays.

10. **P1 — The synthetic fold check does not ensure the reconciled transaction checks.** [mod.rs:149](/tmp/rv-s6h-r1/tamber/src/import/mod.rs:149), [kinds.rs:169](/tmp/rv-s6h-r1/tamber/src/import/kinds.rs:169)

    Existing design: `tw.radius.md: number = 1`. Import: `@theme { --radius-md:4px; }`.

    Reconciliation emits `SetToken`, preserving `: number`; applying the proposal produces `tw.radius.md: number = 4px`, which fails checking. The isolated probe checked a newly declared **length** instead.

    **Fix:** detect incompatible existing kinds and validate the reconciled result against the actual design. The new tests’ empty-design helper misses this case.

11. **P1 — shadcn offers colour roles for newly imported lengths.** [shadcn.rs:566](/tmp/rv-s6h-r1/tamber/src/import/shadcn.rs:566)

    Input: `:root { --primary:8px; }`.

    The main proposal creates a length token, but `offers()` checks only its name and offers `AccentColor = shadcn.primary`. Accepting that offer fails because the role requires a colour.

    **Fix:** restrict colour-role offers to colour tokens. Add a test applying every nonempty offer after its main transaction.

12. **P1 — shadcn’s documented reference support stops after one hop.** [shadcn.rs:344](/tmp/rv-s6h-r1/tamber/src/import/shadcn.rs:344), [RFC:3000](/tmp/rv-s6h-r1/llp/1121-tamber.rfc.md:3000)

    Input: `:root { --a:4px; --b:var(--a); --c:var(--b); }`.

    `a` and `b` map, but `c` is reported as referencing “no colour”: kind discovery explicitly skips referenced declarations that are themselves references. This contradicts the new section’s claim that a reference takes its target’s kind.

    **Fix:** follow kind references recursively with cycle detection. Add multi-hop, forward-reference, and non-colour-cycle tests.

These findings are from code inspection. `cargo test -p exact-tamber` failed before compilation because the read-only filesystem prevented creating its target directory. I found no definite panic, vector-index mismatch, or independent unchanged-source re-import failure.

WRONG
