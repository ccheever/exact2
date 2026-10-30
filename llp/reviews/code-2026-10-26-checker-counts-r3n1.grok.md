# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), narrow re-check 1 after round 3, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `e3a0b39139`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**CLEAN.** Commit `e3a0b39139` matches the five claims. Nothing in these fixes yields a wrong fold, a wrong range, or a stale name index.

**1. `fold_with` cache.** `fold_with` (`contract/cli/src/tamber_params.rs:199`) calls `check_design_cached`. Describe and resolve go through `design_with` (`tamber_serve.rs:858`). Direct checks stay on `check_design`: param writes (`tamber_ops.rs:262`), plan/gen (`tamber_gen.rs:581`), and the range probe (`contract/lower/src/params.rs:629`). Re-import never calls `fold_with`. The cache key is the whole laid design, the vocab, and `trace::on()`; a hit returns a clone of that answer. `fold_with_answers_an_unchanged_design_from_the_last` checks that a repeat of the same 800-dial design returns the same token count and a faster answer. Scrub's `write_checks` (`tamber_scrub.rs:323`, `:390`) also reaches the cache because it calls `fold_with`; the answer is the same one `check_design` would return.

**2. `dials()` budget.** `dials` returns the partial `Dialed` when `budget::spent()` is set, both at the start of each dial (`tamber/src/export_dials.rs:236`) and before the joint `apply_all` (`:296`). `spent()` is set only inside a `Work` that has already gone over the budget. The only production caller (`tamber/src/export.rs:1308`) is inside `reimport_against`'s `Work` and refuses the import on the next line, so those partial ops are dropped.

**3. Probe stem.** The stem is the first `exact.probe{n}` that is not equal to, and is not a prefix of, any token name, param-line name, or dial name (`params.rs:583–596`). Readers are added only when the probe substitutes more than one line (`:609–625`). `a_batchs_dial_readers_take_no_authored_name` declares `exact.probe.dial0` and `exact.probe0.dial0`, so the stem moves off `exact.probe0` and `p1` stays the authored `2`. An icon of the same name fails `icon::check`, the probe returns `None`, and that value is folded alone.

**4. Batching is sound for range results.** A ranged line is folded alone when any of its values (the default or a `when` value) has a word that `names_a_param` accepts — token name, full name, short name, ambiguous short name — or that is a param-line name, or that is the written name of a design token reached by the BFS (`params.rs:656–708`). The BFS starts from param words in token values and `when` values and walks the word-to-token index. `when` conditions are axis values, not token reads. `many_ranged_params_naming_a_token_build_in_moments` is the 5000-line case (`half` / `most` / `two`, `p7 = two`) and expects `tamber-param-range` on `p7`.

A batched value cannot successfully read a param line that same probe rewrites. The probe rewrites only batched param lines, and it adds dial readers. Every spelling that folds to a number, duration, or px length is one the classifier sees:

- Role spellings (`secondary-label`, `-exact-secondary-label`, any case) resolve only for a role override. A non-colour role is refused in `kinds` (`tamber/src/check.rs:736–750`), so `check_design` fails and the probe falls back to folding each value alone. A colour role folds to a colour; `folded_in` range-checks only numbers, durations, and px lengths (`params.rs:503–511`). `number()` and `length()` both reject a colour or a role.
- Kind inference follows the same idents. It does not create a second name.
- `calc` / `min` / `max` / `clamp` and `oklch(from …)` contribute their idents as words. Channel letters (`l`, `c`, `h`) can only pull extra lines into the alone set. A `#` glued onto a name is one word here and two CSS items there, so the value does not fold and the probe falls back. A `/` splits in both.
- Icon aliases rewrite icon sources only. Named lengths and bases (`calc(space.md)`) are keywords the folder matches as keywords; a design token that takes that name is referenced by that same written name.

**5. `shown_index` does not go stale.** `AppParams::shown` fills the `OnceLock` once from `self.tokens` (`tamber_params.rs:49–52`). `params_of` builds a new `AppParams` with an empty lock (`:117`). The server keeps it in an `Rc` and replaces that `Rc` when the sources key changes (`tamber_walk.rs:307–316`). Nothing in this tree assigns `tokens` after that. `relocate` (`:138`), the pinned-lost list (`:536`), and the serve reply (`tamber_serve.rs:872`) all call `app.shown` on that same value.
