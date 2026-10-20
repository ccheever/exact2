# Code review: The importers propose every kind (LLP 1121 stage 6 H), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD (`3c13cec4d`, `4b20b66ce`), the importers' kinds and the first round's fixes.
- **Method:** one brief (sha256 `0394d5a2e4579a776d9a98d8b6cc1565cf140cad0159c32b76e1da2b5e36a37e`), shared by both reviewers, blind to the other review, with the first round's records beside the worktree. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: an inferred kind changed by `set-token`, a type alias through two hops, an embedded reference to a redefined inline key, a Tokens Studio stack alias in a typography, colliding Astryx text styles, a family name's spacing in DTCG and in reconcile; P1s: dependents in the destination design, incoming colours onto kinded tokens, the edit grammar, DTCG weight names).
- **Disposition:** the last round, applied without re-review in `88e35b8ec`: items 1-7, 10 and 11, each with a regression test. Items 8 and 9 (folding the import into the destination design; an incoming colour onto a token of another kind, as before stage 6) are QUEUE's "An import is checked against the design it lands in".

---

The first-round minimal repros appear fixed by inspection, but several fixes remain incomplete.

1. **P0 — Updating an implicitly typed token can change its kind.** [mod.rs:152](/tmp/rv-s6h-r2/tamber/src/import/mod.rs:152)

   Existing design: `tw.type.body = 700 16px`. Import: `@theme { --text-body:16px; }`.

   The compatibility check sees an existing `type`, but reconciliation emits a value-only `SetToken`. The resulting `tw.type.body = 16px` infers **length**, although the importer counted it as a type token. The transaction checks successfully with the wrong kind.

   **Fix:** verify the resulting token’s kind, preserving the intended kind explicitly or reporting the conflict. Test updates to tokens with inferred kinds.

2. **P0 — Tailwind’s type-alias fix still copies unrelated typography through longer chains.** [tailwind.rs:792](/tmp/rv-s6h-r2/tamber/src/import/tailwind.rs:792)

   ```css
   @theme {
     --text-base:16px;
     --text-base--font-weight:700;
     --text-mid:var(--text-base);
     --text-copy:var(--text-mid);
   }
   ```

   Only one source lookup is expanded. `tw.type.copy` becomes `tw.type.base`, inheriting weight **700**, although the source aliases only the size. This remains contrary to [RFC:2995](/tmp/rv-s6h-r2/llp/1121-tamber.rfc.md:2995).

   **Fix:** recursively resolve the source size before assembling the type. Test the folded weight and leading across multiple alias hops.

3. **P0 — Embedded references to redefined inline Tailwind keys use stale values.** [tailwind.rs:756](/tmp/rv-s6h-r2/tamber/src/import/tailwind.rs:756)

   ```css
   @theme inline { --spacing-base:4px; }
   @theme { --radius-card:calc(var(--spacing-base) + 2px); }
   :root { --spacing-base:8px; }
   ```

   The importer proposes a radius folding to **6px**; the source expression resolves to **10px**. Redefinition propagation recognizes only whole-value `var()` references, while the new conversion substitutes references inside expressions. The inline key’s override is also excluded from `redefined`.

   **Fix:** track dependencies through every unquoted `var()`, including those inside calculations and shadows. Report dependents that cannot follow the override.

4. **P0 — A Tokens Studio family alias becomes a single font inside typography.** [dtcg/kinds.rs:260](/tmp/rv-s6h-r2/tamber/src/import/dtcg/kinds.rs:260)

   ```json
   {
     "face":{"type":"fontFamilies","value":"Inter, sans-serif"},
     "body":{"type":"typography","value":{
       "fontFamily":"{face}","fontSize":"16px",
       "fontWeight":"400","lineHeight":"150%"
     }}
   }
   ```

   `ds.face` imports as a two-member stack. Inside `ds.body`, alias expansion loses the source type, and `family("fontFamily", …)` produces **`"Inter, sans-serif"`**, one font name. This is a regression from the comma-name fix.

   **Fix:** retain the referenced family’s semantics through composite conversion, or normalize family stacks before expanding aliases. Test both strict DTCG scalar names and Tokens Studio stack aliases.

5. **P0 — Astryx still combines different text styles whose sanitized names collide.** [astryx.rs:317](/tmp/rv-s6h-r2/tamber/src/import/astryx.rs:317)

   Give both `light` and `dark`:

   ```json
   {"--text-1-size":"16px","--text-s1-weight":"700"}
   ```

   Both become `astryx.type.s1`. Because their parts differ, the collision check combines them into **`700 16px`**, counting both as mapped without reporting either. These are different source styles.

   **Fix:** group parts by their original style name before sanitizing names; report collisions between those groups. The first-round test covers length collisions, not colliding text styles.

6. **P0 — DTCG font-name serialization still loses significant whitespace.** [dtcg/kinds.rs:164](/tmp/rv-s6h-r2/tamber/src/import/dtcg/kinds.rs:164)

   Input: `{"f":{"$type":"fontFamily","$value":"A  B"}}`.

   The serializer emits unquoted `A  B`. Tamber’s family fold collapses it to **`A B`**, changing the single font name. DTCG strings represent individual names. [DTCG §8.3](https://www.designtokens.org/tr/2025.10/format/#font-family)

   **Fix:** quote and escape family members whenever unquoted serialization changes their meaning. Assert the folded family value, not merely the emitted text.

7. **P0 — Reconciliation silently ignores changes inside quoted font names.** [mod.rs:189](/tmp/rv-s6h-r2/tamber/src/import/mod.rs:189)

   Existing token: `tw.family.brand: family = "A B"`. Import: `@theme { --font-brand:"A  B"; }`.

   `same()` collapses whitespace inside both strings, so the importer returns **no operation** and retains the wrong font name while counting the source as mapped. Fixing `bare()` did not fix this comparison.

   **Fix:** normalize whitespace only outside strings. Test a changed-source import against an existing design, followed by unchanged-source re-import.

8. **P1 — The synthetic fold check still misses failures in the destination design.** [kinds.rs:204](/tmp/rv-s6h-r2/tamber/src/import/kinds.rs:204)

   Existing design:

   ```tamber
   tamber app

   theme
     tw.radius.md: length = 4px
     derived: length = calc(tw.radius.md + 1px)
   ```

   Importing `@theme { --radius-md:1rem; }` passes the kind comparison and isolated probe. Accepting it fails because the existing calculation cannot fold a `rem`.

   The probe also reconstructs axes without destination attributes such as `under scheme=dark`.

   **Fix:** validate the reconciled transaction against the actual design, including existing dependents and retained axis properties. Testing only empty designs misses this.

9. **P1 — Incoming colours bypass the new incompatible-kind check.** [kinds.rs:140](/tmp/rv-s6h-r2/tamber/src/import/kinds.rs:140)

   Existing token: `shadcn.primary: length = 8px`. Import: `:root { --primary:#fff; }`.

   Colour mappings have `kind: None`, so the guard skips them. Reconciliation proposes `SetToken`, producing **`shadcn.primary: length = #fff`**, which cannot check. The new claim that existing tokens of another kind are reported is therefore incomplete. [RFC:2983](/tmp/rv-s6h-r2/llp/1121-tamber.rfc.md:2983)

   **Fix:** check colour proposals too, propagating refusals through dependencies and counts. Test both directions of a kind conflict.

10. **P1 — A value can pass the probe but fail the edit operation’s grammar.** [kinds.rs:227](/tmp/rv-s6h-r2/tamber/src/import/kinds.rs:227)

    Input: `@theme { --font-brand:Made in Heaven; }`.

    This folds as a family, so the importer proposes it. However, [edit.rs:246](/tmp/rv-s6h-r2/tamber/src/edit.rs:246) rejects any unquoted `in` word as a range clause. Even accepting into an empty design fails.

    **Fix:** validate emitted operations as well as token values; quote this family member appropriately. Add an acceptance regression.

11. **P1 — Valid DTCG weight names are incorrectly reported as unsupported.** [dtcg/kinds.rs:141](/tmp/rv-s6h-r2/tamber/src/import/dtcg/kinds.rs:141)

    `{"w":{"$type":"fontWeight","$value":"book"}}` is reported as “no number.” `extra-black` and `ultra-black` fail similarly. These are standard aliases for **400**, **950**, and **950**, respectively; typography using them is also rejected. [DTCG §8.4](https://www.designtokens.org/tr/2025.10/format/#font-weight)

    **Fix:** complete the standard alias table and test every defined alias, standalone and inside typography.

The main test gaps are folded-value assertions, longer dependency chains, colliding composite names, and acceptance against nonempty designs. The new [acceptance helper](/tmp/rv-s6h-r2/tamber/tests/it/import_kinds.rs:14) always starts from `EMPTY`.

`cargo test -p exact-tamber` failed before compilation because the read-only sandbox prevented creating its target temporary directory. I found no definite new panic, parallel-vector mismatch, arithmetic count imbalance, or unchanged-source re-import emitting operations.

WRONG
