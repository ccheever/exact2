# Code review: The app's value of a param (LLP 1121 stage 7 B), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `55186696d..7cb7f1cbc` as reviewed (rebased since onto stage 7 A's re-check fixes), B and its first round.
- **Method:** one brief (sha256 `8be31b9be3867f50ed07f30817fdb0897564b706a470a7c7d9239031198b773f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0: a rename changing a literal param value; P1s: icon SVG on write, token-valued dominated ranges, entry errors only the compiler's, conditional pins, icon explain/uses, motion uses per instance; P2: an emptied block with a comment).
- **Disposition:** the last round, applied without re-review in `b2dfdb9f6`, with tests (`edit_params.rs`, `tamber_set_param.rs`). 1: a rename is proved folded with the params (`step` takes them). 2: an icon param's SVG must be the app's file on write; the per-target check stays the build's. 3: each written value folded on its own at its line (`written_ranges`). 4: `params::entry_errors` shared by compiler and server, reads and writes; unset by the line's spelling repairs. 5: pinned per branch. 8: the emptied block goes whole. Declined, to QUEUE: 6, an icon's explain/uses (an icon is read by describe and resolve, as stage 6 built); 7, motion uses by instance (rows written before expansion; Grok agrees the decline holds) — both in one QUEUE entry.

---

Read-only source review; builds and tests were not run.

1. **P0 — Renaming a token can change a literal param value.** [tamber/src/edit.rs:827](/tmp/rv-s7b-r2/tamber/src/edit.rs:827). Declare `App.face: family = "serif, sans-serif"` and render it through `font-family="param.face"`. Given:
   ```text
   theme
     serif = 1px
   params
     App.face = serif, sans-serif
   ```
   `rename-token serif monospace` changes the font stack to `monospace, sans-serif`. `rename_words` rewrites the literal family name, while `step`’s equivalence check folds only the design without params, so it approves the change. **Fix:** rename resolved references and include params in the equivalence check. Add a regression covering literal family names.

2. **P1 — Icon-param writes can commit an unbuildable design.** [contract/cli/src/tamber_ops.rs:189](/tmp/rv-s7b-r2/contract/cli/src/tamber_ops.rs:189). Starting with a valid rendered `Card.glyph`, submit `set-param Card.glyph 'icon(svg "missing.svg")'` where that asset does not exist. The transaction passes the Tamber fold and commits; compilation subsequently fails with `tamber-icon-svg`. Platform source requirements are likewise unchecked. **Fix:** apply the compiler’s icon asset and target validation to proposed overrides before committing. Test acceptance followed by compilation, beyond classification alone.

3. **P1 — The range fix still skips token-valued dominated branches and misattributes failures.** [contract/lower/src/params.rs:352](/tmp/rv-s7b-r2/contract/lower/src/params.rs:352). For `Card.radius` ranged `0..40`, `radius.big = 80px`, and axes `density=regular|compact`, `size=small|large`:
   ```text
   params
     Card.radius = 8px
       when density=compact: radius.big
       when density=compact size=small: 8px
       when density=compact size=large: 8px
   ```
   This passes: `written_ranges` recognizes only simple literals, and `folded` checks only winning tuple values. Removing the two specific branches detects the violation but attributes it to `Card.radius = 8px`. **Fix:** range-check each written expression after folding in its applicable tuples, retaining that expression’s span. Cover dominated token references, calculated values, and token-valued error locations.

4. **P1 — Shared layout still discards errors that only the compiler reports.** [tamber/src/params.rs:124](/tmp/rv-s7b-r2/tamber/src/params.rs:124). With both `Card.radius = 8px` and `card.contract#Card.radius = 16px`, compilation refuses `tamber-param-twice`, but `resolve` returns `8px` and server validation accepts the fold. An ambiguous handwritten short-name entry is similarly ignored by server reads while compilation refuses it. **Fix:** share entry validation alongside layout across compilation, reads, and planning; propagate duplicate and ambiguity errors while retaining unknown entries as warnings.

5. **P1 — Conditional literal overrides remain unmarked as pinned.** [contract/cli/src/tamber_params.rs:174](/tmp/rv-s7b-r2/contract/cli/src/tamber_params.rs:174). With default `radius.md`, this produces `pinned: []`:
   ```text
   params
     Card.radius = radius.md
       when density=compact: 8px
   ```
   The union of all branch dependencies contains `radius.md`, hiding that the compact value stopped following it. Thus a scrub of that `when` still misses D14’s mark. **Fix:** determine lost dependencies per effective branch and expose the conditional pin. Use parsed dependencies; the current word splitter also mistakes literal words for references.

6. **P1 — Icon-param `explain` and `uses` remain unimplemented.** [contract/cli/src/tamber_serve.rs:1453](/tmp/rv-s7b-r2/contract/cli/src/tamber_serve.rs:1453). For a rendered `Card.glyph` defaulting to `icon(sf "star")`, both requests still return an error. The round 1 change removes the hash from that error but does not provide the requested explanation or instance sites. **Fix:** handle icons in these reads, preserve their alias dependencies, and enumerate their image-use sites. The new test currently asserts the error.

7. **P1 — Declining per-instance motion uses leaves the explicit requirement unmet.** [contract/cli/src/tamber_serve.rs:980](/tmp/rv-s7b-r2/contract/cli/src/tamber_serve.rs:980). In the supplied fixture containing two `PostButton()` calls, `uses PostButton.fade` returns one declaration-level transition with `site: null`. **Fix:** preserve motion-param provenance through substitution and component expansion, then join each resulting row to its instance site. Test both direct and aliased motion params. The substitution order explains the limitation but does not justify dropping the required coverage.

8. **P2 — An indented comment prevents removal of an emptied block.** [tamber/src/edit/params.rs:119](/tmp/rv-s7b-r2/tamber/src/edit/params.rs:119). Unsetting the sole entry in:
   ```text
   params
     // radius
     Card.radius = 8px
   ```
   leaves the `params` header because the comment counts as nonempty content. **Fix:** determine emptiness from remaining entries, preserve comments deliberately, and remove the header. Cover CRLF and a following block, including byte-for-byte undo.

The unused-motion rebuild decline is reasonable: it conservatively follows existing duration/easing classification.

WRONG
