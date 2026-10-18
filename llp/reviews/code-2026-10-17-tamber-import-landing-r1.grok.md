# Code review: An import is checked where it lands (LLP 1121 D14, H's follow-up), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** HEAD (`225aa2718`), the landing check.
- **Method:** one brief (sha256 `5c2b4b6fff447cf74342df30adb60c34202b961ebae689cf3694ed0eb91d0871`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P0s: the role-less vocabulary, owned when lines, kept when lines; P1s: refused colours still named, every mentioned token charged, a fixed pass count; P2s: repeated folds, docs and tests).
- **Disposition:** all taken in the first-round commit `837bbbc46`, each with a test in `tamber/tests/it/import_kinds.rs` (`the_landing_check_charges_what_breaks_and_only_that`) or `landing.rs`.

---

1. **P0 — Tailwind and DTCG skip the landing fold whenever the design has a role.** `tamber/src/import/landing.rs:102` and `tamber/src/import/landing.rs:125`. `tailwind.rs:647` and `dtcg.rs:1419` call `landing` with `Vocab::default()`. `check_design` then fails `tamber-unknown-role` on any `-exact-…` token, `before` is `None`, and the function returns after the written-kind scan. The CLI already has the real vocabulary (`contract/cli/src/tamber_import.rs:123`) and does not pass it.

   Failing input: a design that folds under the app vocabulary

   ```
   tamber app

   theme
     -exact-background = #ffffff
     tw.radius.md: length = 4px
     derived: length = calc(tw.radius.md + 1px)
     tw.brand = 8px
   ```

   with `@theme { --radius-md: 1rem; --color-brand: #ff0000; }`. `--radius-md` is proposed; accept then refuses the whole transaction because `calc(1rem + 1px)` does not fold. `--color-brand` is proposed onto the untyped length and accept writes `tw.brand = #ff0000`, so the length becomes a colour. A written `: number` is still caught; a kind taken from the value is not.

   Fix: pass the caller’s `Vocab` through `tailwind::import` and `dtcg::import`. If the design already fails to fold, diff diagnostics against that failure instead of returning. Keep the inferred-kind check on that path.

2. **P0 — `laid` keeps `when` lines `reconcile` deletes, so a bad edit looks like it folds.** `tamber/src/import/landing.rs:57`. `reconcile` (`tamber/src/import/mod.rs:161`) emits `unset-token-when` for a `when` on an owned axis the source no longer has. shadcn owns `theme`; DTCG owns every axis it imports. `laid` only overwrites or appends.

   Failing input:

   ```
   tamber app

   axes
     theme = default | ocean

   theme
     shadcn.primary: length = 1rem
       when theme=default: 4px
       when theme=ocean: 8px
     derived: length = calc(shadcn.primary + 1px)
   ```

   with `:root { --primary: 1rem; }`. The whens cover every tuple, so `1rem` is never folded and the design folds. The source drops both whens. `laid` keeps them, the probe still folds, and the proposal is two `unset-token-when`s. Accept then sees `calc(1rem + 1px)` and refuses the transaction.

   Fix: give `laid` the same `owned` list and delete those `when` lines before `check_design`.

3. **P0 — An error on a `when` the import did not rewrite is not charged to the token it set.** `tamber/src/import/landing.rs:144`. Preserved whens keep their source line. `tamber-under-light` is reported on `when.line` (`tamber/src/check.rs:1139`). That line is below `LAID`, so the import-token arm does not see it. The design-token arm only blames names that appear in the old text, and a token’s own `when` does not name itself. `if !refused { return why }` at line 166 then treats the probe as clean.

   Failing input:

   ```
   tamber app

   axes
     dark-style = black | dim under scheme=dark

   theme
     tw.brand = #ffffff
       when dark-style=dim: light-dark(#ffffff, #222222)
   ```

   with `@theme { --color-brand: #000000; }`. The new default’s light side is `#000000` and the `when`’s light side stays `#ffffff`, so the landed design does not fold. `tw.brand` is still proposed. Accept refuses the whole import, including any other token in that sheet.

   Fix: stamp every line of a token the import sets, preserved whens included, with `LAID + index`. If a diagnostic still matches a design token whose name is in the import, blame that import token. If the probe still fails and nothing new was blamed, refuse the tokens that were laid in.

4. **P1 — A colour refused for kind is still named by other proposed tokens and by role offers.** `tamber/src/import/landing.rs:131` returns on the first successful fold. `engine.rs:168` writes a `var(--x)` as the token name. After `shadcn.primary` is removed, `shadcn.ring = shadcn.primary` still folds, because the design’s `shadcn.primary` is a length and the new token inherits that kind.

   Failing input: `shadcn.primary: length = 8px` and `:root { --primary: #ffffff; --ring: var(--primary); --background: var(--primary); }`. `--primary` is reported. `ops` still contain `shadcn.ring = shadcn.primary` and `shadcn.background = shadcn.primary`. `offers` (`shadcn.rs:618`) still offers `-exact-background` bound to `shadcn.background`.

   Fix: once a colour is refused, also refuse every still-proposed token whose value names it, and do that before `offers`.

5. **P1 — One failing design token refuses every import token its text mentions.** `tamber/src/import/landing.rs:156`. `words` keeps every identifier, and each of them is refused in the same pass.

   Failing input:

   ```
   theme
     tw.radius.a: length = 1px
     tw.radius.b: length = 1px
     derived: length = calc(tw.radius.a + tw.radius.b)
   ```

   with `@theme { --radius-a: 1rem; --radius-b: 1px; }`. Only `1rem` breaks `calc`. `--radius-b` is unchanged and is reported anyway.

   Fix: blame the import token whose removal makes that diagnostic go away, one candidate at a time, and leave the others mapped.

6. **P1 — The probe gives up after 8 passes and proposes whatever is left.** `tamber/src/import/landing.rs:129`. A reference to a token that failed to fold is a quiet error (`tamber/src/check.rs:1250`), so each pass reports only the current root. A chain `t1 = t2` … `t9 = not-a-colour` refuses `t9`…`t2` and leaves `t1` in `ops` with `why` still `None`.

   Fix: keep going until a pass reports nothing new, and if the last probe still fails, refuse every token that was laid in on that pass.

7. **P2 — The same fold runs two or three times on a full theme.** `kinds.rs:134` calls `landing`, then `unfolded` calls it again at `kinds.rs:163`. Tailwind then calls it once more from `colours_refused` (`tailwind.rs:647`). Each call lays in every token with a linear name search (`landing.rs:57`) and folds the theme. The default sheet is about 360 tokens, so this is several full folds of that sheet on every import.

   Fix: fold once and share that `Vec<Option<String>>`. Index tokens by name inside `laid`.

8. **P2 — The docs and the new test describe a stricter check than the code.** `docs/tamber-protocol.md:538` says every token, a colour too, is checked where it lands. `llp/1121-tamber.rfc.md:2987` says each token is laid in as `reconcile` writes it. The test `an_import_holds_where_it_lands_or_says_why` (`tamber/tests/it/import_kinds.rs:497`) uses designs with no roles, no `when`, and no `var()` to a refused colour, which is why it stays green.

   Fix: cover the four inputs above, and make the protocol sentence match the vocabulary and the owned-`when` behaviour.

Verdict: WRONG
