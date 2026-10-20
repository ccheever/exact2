# Code review: Tamber importers, the rest of rt1's QUEUE entry (budget calibrated to the isolate, CSS importers linear and bounded, marks), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6ad67867b9`.
- **Method:** one brief (sha256 `dc66edc08eac9c91aa15058492605bab53f564028c244d22b8c6a574254d39ca`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (the CLI's baseline fingerprint export outside the budget), two should-fix (shadcn kind chain at 256; colour walks not stopping at 256, memo per block), two nits (§10 ratios; a joined Tailwind value past 64 KiB)
- **Disposition:** all taken in d840c4c880: export::own_digest inside REIMPORT; walks cut at 256 and nothing on a cut walk remembered; shadcn allows 256 references and past that none; the join held to 64 KiB; §10 gives each limit's share

## Blocking

**Baseline re-import builds the export outside the budget** — `contract/cli/src/tamber_import.rs:271`

`plan` fingerprints the design with `export::dtcg` before `reimport_against`. `budget::spend` (`budget.rs:59`) returns success when `DEPTH == 0`, so the new per-context `copy` at `export.rs:394` does not run on that call. Every wire re-import sends `baseline` (`scripts/tamber-reimport.test.mjs`). For 3,000 tokens × 64 contexts that call builds every context tree and the pretty-printed document, and the isolate kills the child at 90% of 1 GiB (`scripts/tamber-isolate.mjs:143`) with "the source is too costly to read". `.ok()` at `tamber_import.rs:273` would also turn a budget error into a missing digest, so a baseline that is not already recognised comes back as "pull first". `scale.rs` calls `reimport` directly, under `Work::begin(REIMPORT)`, and stays green.

Fix: drop the fingerprint `dtcg`. `reimport_against` already builds `fresh` under `Work(REIMPORT)` (`export.rs:734`). Compare the baseline digest there and return that diagnostic. Keep a single `Work` around that one export. A second `begin` left live across `reimport_against` shares `LEFT` and charges the same trees twice.

## Should-fix

**A shadcn chain of 256 hops becomes a name kind** — `tamber/src/import/shadcn.rs:445`

`through` returns `None` when `seen.len() >= 256`, before reading the node those hops reached. DTCG allows that hop (`dtcg.rs:957`, `visiting.len() > 256`), and so does colour resolution (`engine.rs:290`, `links > LINKS`). The `None` then hits `.or_else(named)` on the way back (`shadcn.rs:450`). `--shadow-sm: var(--a)` through 256 links to a length comes out `box-shadow`; `--font-*` comes out `family`. A name with no kind comes out none, including a chain of exactly 256 links that should still resolve.

Fix: after `seen.insert`, fail only when `seen.len() > 256`, and return that miss so callers skip `or_else(named)`. A chain of 256 links to a literal still resolves. A longer chain stays none.

**Colour walks do not stop at 256, and the memo is per requesting block** — `tamber/src/import/engine.rs:209`

`seen` walks the whole chain, then marks `Fate::Deep` if `links > 256`. The memo key is `(Target, var)` (`engine.rs:197`), and `chain` falls through to a shared root. K theme blocks that each reference one root chain of length M re-walk all M links. A sheet under 1 MiB can make both K and M thousands, which is quadratic, and the cap never cuts the walk. Within one block the memo is linear and matches the old `colour_of` plus `visiting` list for chains of 256 links or fewer, auxiliary variables included.

Fix: stop the forward walk once the next link would pass `LINKS` and store that as deep, so each block costs at most 256 steps. Keying remembered colours by the winning declaration the walk actually used would share one root chain across blocks.

## Nit

**§10 calls both limits about three quarters** — `llp/1121-tamber.rfc.md:4835`

`IMPORT` × 12 bytes is about 604 MB, 59% of 1 GiB. `REIMPORT` × 30 bytes is about 755 MB, 74%. The isolate kills at 0.9 GiB, which §10 does not say. State the two ratios. The constants themselves clear 75% and the 90% kill if those bytes-per-unit figures hold.

**A joined Tailwind type value skips the 64 KiB check** — `tamber/src/import/tailwind.rs:950`

`written_out` caps size, weight, and leading at 64 KiB each (`tailwind.rs:737`). The join at 950–959 is unchecked, so one type token can be about 192 KiB. The same error text blames `var()`s when the oversized value is a long literal. Check `out.len()` against `LONGEST` after the join.

## Equivalence and the budget

Checked against the previous code, for input the caps still accept:

- `Read::seen` matches recursive `colour_of`: `on` is the visiting set, an auxiliary variable writes the resolved colour, a named variable writes `names[r]`, and a cycle fails closed.
- `cyclic` is the nodes that reach themselves: components of two or more, plus a self-loop. Names outside the graph are dropped, as the old `reaches` walk dropped them.
- The failed-propagation worklist sorts by `(Target, var)` and the first target wins, same as `BTreeMap` `or_insert` over `sides`.
- shadcn `seen_as` matches the old `own()` scans under the cap, including an empty uniform kind. `Kinds::of` keeps the last declaration.
- `marks` is exact, then ASCII case-insensitive, first index. The impact map keeps the first name and still emits `cur.tokens` order.
- `nested` under 1 KiB and at most 1,024 selectors has the same set and order. The length formulas match replacing `&` and joining with a space.
- `Work::begin` is only `dtcg::import_with` (`IMPORT`) and `reimport_against` (`REIMPORT`). Re-import calls `export::dtcg`, so the two limits do not nest. A nested `begin` would share the outer budget.

An unread nested rule is reported on the line of `{`. Comment newlines are counted once through the stripped map, which is what the scale_css case (line 4) expects. The refusing context is allocated and then charged, so the budget overshoots by one context. On the measured 3,000 × 64 re-import that is inside the 470 MB the text already quotes.

Tests do not drive `plan` with a baseline, so the blocking path is untested. There is no RSS assertion, no fresh 3,000 × 64 refusal, no cross-block long colour chain, and no check that a 256-link `--shadow-*` or `--font-*` chain is none. The 8 MiB copy cap does not check the reported line. The §10 timings were not re-run.
