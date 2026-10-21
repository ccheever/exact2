# Code review: LLP 1121 stage 2, commit 7, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `14e4e5c142db6204b1d1058940593ff7319e626bd1109146364cfb47be8bade4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: three low.
- **Disposition:** all fixed in the round-2 commit

---

## Findings

**Low — `alpha` is a channel keyword the folder accepts and no-tells rejects.** `scripts/no-tells.mjs:162`

`KEYWORDS` lists each function's components and leaves out `alpha`. `Channels::get` treats `alpha` as a channel keyword for every relative colour (`tamber/src/color_fn.rs:54`), so `rgb(from brand calc(alpha) g b)` and `oklch(from brand l c calc(alpha))` fold and name no literal colour. `relativeTo` still reports them: the letter-runs are `calc` and `alpha`, and neither is in the set (`scripts/no-tells.mjs:184`). `rgb(from brand r g b / alpha)` slips through only because the scan stops at `/` (`scripts/no-tells.mjs:178`).

Add `alpha` to every function's keyword set.

**Low — A relative colour whose origin is itself a function is reported as a literal.** `scripts/no-tells.mjs:169`

The `from` capture is `[^\s,()]+`, so it stops at `(`. `rgb(from color-mix(in oklab, surface.card 40%, accent.fill) r g b)` and `rgb(from oklch(from surface.card l c h) r g b)` therefore fail `relativeTo`, and `colourIn` reports the outer function. Both fold: the origin is parsed as a colour item, and a function is one (`tamber/src/fold.rs:219`, `tamber/src/color_fn.rs:499`). Neither expression contains a hex or a named colour. The file's own note says an expression the build folds is not a tell (`scripts/no-tells.mjs:31`).

When the origin is a call, exempt it only if that call itself has no tell.

**Low — The summary usage lines still describe one flag order, and a stray flag is rejected only after the design is checked.** `scripts/help.mjs:276`, `contract/cli/src/main.rs:31`, `contract/cli/src/tamber.rs:63`

`bun exact.mjs tamber --help` and `contract --help` print `check | eval | ir <path> [--mode …] [-o …]`. That line puts both flags on every verb and does not say a flag may precede the path. The either-order note exists only on `contract tamber --help` (`contract/cli/src/tamber.rs:14`). The parser does accept either order, and `eval --mode` is tested both ways (`contract/cli/tests/it/tamber_cli.rs:50`).

`exact_tamber::check` runs before the verb is matched against its flags (`contract/cli/src/tamber.rs:63`, match at `contract/cli/src/tamber.rs:72`). `contract tamber check app --mode dark-style=dim`, `eval` with `-o`, or `ir` with `--mode` all check first. A design that has errors exits 1 with those diagnostics, and the stray flag is never mentioned. A clean design then hits the fallback arm and exits 2 (`contract/cli/src/tamber.rs:100`).

Match the verb and its flags before checking, and put the same either-order note on the `help.mjs` tamber usage and the `contract` one-liner. Keep `--mode` on `eval` and `-o` on `ir` in those summaries.

## Checked, and not findings

The start-here line names a colour. Stage 2 has colour tokens only, which is the round-1 wording. Section 6's "colour or size" line is the finished required read. D2's `fmt`, `gen`, and `export` are not what this commit's docs claim; those claim `check`, `eval`, and `ir`. D7's boundary rows are the eight inherited colour rows already recorded for commit 3; this commit only stops treating every unwritten row as implied.

Against `origin/main`, the RFC diff drops the commit-6 "as built" section because that section is `cc08e3c50`, which is on `origin/main` and not in this branch. Relative to the merge-base, `fb843046b` does not edit `llp/1121-tamber.rfc.md`. There is no commit-7 section in section 10; that matches the earlier commits, whose as-built notes landed in a follow-up.

A literal alpha after `/` (`rgb(from brand r g b / 0.5)`) stays exempt on purpose, and the new test locks that in. `color-mix` of two token names, with no hex and no named colour, is exempt. `clamp`/`calc` that mention a channel keyword are exempt by the rule this commit wrote.

`recorded` and `row_token`'s `set_tuple` have no drop guard. A panic inside the inspection parse would leave the thread-local record and the tuple unrestored. Nothing in `set_plan_style` on text that has already parsed shows that panic, so this stays a residual.

No test separates a compiler-added row (`written: false`, `implied: false`) from a boundary row (`implied: true`). `ir -o` on either side of the path is the same parser as the tested `--mode` orders and is not given its own case.
