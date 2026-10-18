# Code review: An import is checked where it lands (LLP 1121 D14, H's follow-up), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD (`225aa2718`), the landing check.
- **Method:** one brief (sha256 `5c2b4b6fff447cf74342df30adb60c34202b961ebae689cf3694ed0eb91d0871`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: aliases of a refused colour, a role-bearing design disabling the check; P1s: owned when lines, indirect dependents, kept when lines, a fixed pass count, branch attribution, unattributed refusals).
- **Disposition:** all taken in the first-round commit `837bbbc46`, each with a test in `tamber/tests/it/import_kinds.rs` (`the_landing_check_charges_what_breaks_and_only_that`) or `landing.rs`.

---

Read-only source review; no files changed or tests run.

1. **P0 — A refused colour leaves aliases mapped to the destination’s wrong kind.** [landing.rs:131](/tmp/rv-land-r1/tamber/src/import/landing.rs:131)  
   Design: `shadcn.base: length = 8px`. Import:
   ```css
   :root { --base: #fff; --primary: var(--base); }
   ```
   `base` is refused, but `primary = shadcn.base` survives: without a written kind, it successfully folds as a **length**. The importer also offers `AccentColor = shadcn.primary`. The count passes as `2 = 1 + 1`, despite the wrong mapping.  
   **Fix:** propagate refusals through imported references, or replace references with their resolved source values. Validate surviving tokens’ resulting kinds and generate offers afterward. Test the accepted value and kind, not just counts.

2. **P0 — Ordinary platform vocabulary disables both destination checks.** [landing.rs:125](/tmp/rv-land-r1/tamber/src/import/landing.rs:125)  
   Design:
   ```tamber
   tamber app
   theme
     Canvas = #fff
     tw.brand = 8px
   ```
   Import `@theme { --color-brand: #f00; }`. Tailwind passes `Vocab::default()`, so `Canvas` makes `before` fail. The fallback finds no *written* kind for `tw.brand`, and the importer proposes changing this length into a colour. Adding `Canvas` to the commit’s radius/`derived` example likewise bypasses its dependency check. DTCG has the same problem.  
   **Fix:** pass the destination’s real vocabulary into both importers. An unrelated unknown role must not silently disable validation. Add role-bearing and inferred-kind regressions; the protocol documentation’s “Every token” claim currently overstates coverage.

3. **P1 — `laid` preserves `when` lines that reconciliation removes.** [landing.rs:61](/tmp/rv-land-r1/tamber/src/import/landing.rs:61)  
   Import `:root { --gap: 1rem; }` into:
   ```tamber
   tamber app
   axes
     theme = default | blue
   theme
     shadcn.gap: length = 4px
       when theme=blue: 8px
     derived: length = 0px
       when theme=blue: calc(shadcn.gap + 1px)
   ```
   The simulation retains the `8px` override and passes. `reconcile` removes that importer-owned override, leaving `derived` calculating over `1rem`; acceptance then fails.  
   **Fix:** carry `owned` into the simulation and apply the same operations reconciliation produces. Test by applying the returned operations to this destination.

4. **P1 — Indirect design dependencies escape attribution.** [landing.rs:156](/tmp/rv-land-r1/tamber/src/import/landing.rs:156)  
   Design tokens:
   ```tamber
   tw.radius.md: length = 4px
   bridge: length = tw.radius.md
   derived: length = calc(bridge + 1px)
   ```
   Import `@theme { --radius-md: 1rem; }`. Folding fails at `derived`, whose text names only `bridge`. The direct word comparison finds no imported token, and `landing` returns no refusal. The invalid update remains proposed.  
   **Fix:** trace dependencies transitively back to changed imports, using the checker’s dependency information. Add a regression with at least one intermediate design token.

5. **P1 — Errors on retained literal `when` lines lose their changed owner.** [landing.rs:145](/tmp/rv-land-r1/tamber/src/import/landing.rs:145)  
   Import `@theme { --color-brand: #eeeeee; }` into:
   ```tamber
   tamber app
   axes
     dark-style = normal | dim under scheme=dark
   theme
     tw.brand = light-dark(#ffffff, #000000)
       when dark-style=dim: light-dark(#ffffff, #111111)
   ```
   The retained `when` now has the wrong light side. Its diagnostic retains the original line number, and its literal values contain no `tw.brand` reference. Consequently, the importer reports nothing and proposes an update acceptance rejects.  
   **Fix:** retain ownership information for existing `when` lines, including their dependence on a changed default. Attribute this error to `tw.brand`.

6. **P1 — Eight rounds can leave a broken alias chain proposed.** [landing.rs:129](/tmp/rv-land-r1/tamber/src/import/landing.rs:129)  
   Start with `shadcn.a0: length = 8px`. In `:root`, declare `--a0: #fff` and nine aliases: `--a1: var(--a0)` through `--a9: var(--a8)`. In `.dark`, set `--a1` through `--a9` to `#000`.  
   After refusing `a0`, each round exposes the next alias failure; downstream errors are suppressed by the checker. Eight rounds remove `a1`–`a8`, but leave `a9 = light-dark(shadcn.a8, #000)` proposed without its dependency.  
   **Fix:** continue until validation succeeds or no progress is possible, bounded by the number of candidates rather than eight. Never return unchecked survivors. Test a chain longer than eight.

7. **P1 — One failing branch falsely refuses imports used only by another branch.** [landing.rs:152](/tmp/rv-land-r1/tamber/src/import/landing.rs:152)  
   With axis `mode = normal | compact`, use:
   ```tamber
   tw.radius.bad: length = 4px
   tw.radius.good: length = 8px
   derived: length = calc(tw.radius.bad + 1px)
     when mode=compact: calc(tw.radius.good + 1px)
   ```
   Import `--radius-bad: 1rem` and `--radius-good: 12px`. Only the default expression fails, but attribution unions references from **all** branches and reports both declarations. The valid `good` update disappears.  
   **Fix:** attribute expression errors through the particular failing value and its dependencies. Test that the unrelated valid operation survives.

8. **P1 — Non-token diagnostics are silently treated as success.** [landing.rs:149](/tmp/rv-land-r1/tamber/src/import/landing.rs:149)  
   Design:
   ```tamber
   tamber app
   dials
     brand: color = #fff
   theme
     tw.ink = brand
   ```
   Import `@theme { --color-ink: #000; }`. This removes the dial’s only use. Folding returns `tamber-dial-inert` on the **dial** line, which the token-only lookup ignores; the invalid update remains proposed.  
   **Fix:** attribute lost dial uses to the responsible operations, and return a refusal when remaining diagnostics cannot be attributed. Add an acceptance regression for this case.

WRONG
