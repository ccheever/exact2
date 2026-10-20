# Code review: LLP 1121 D3/D7, the design server's write-time row check, narrow re-check after round 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `65a83bf6d0`.
- **Method:** one brief (sha256 `47d90e15705f17002ea05d860d1de64d4215dca620aa4143a1f25d92b9700ace`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX, 1 NIT).
- **Disposition:** the SVG claim corrected: an `svg` subtree is left whole to the build, stated and QUEUE'd with its route; the NIT taken.

I'll re-check HEAD against the round 3 findings only: read those notes, the amend diff, and the "Round 3" commit message, then verify each claimed fix.Round 3 named four Astra findings and one Grok finding. I'll read the amended check, the scan, the tests, and the QUEUE entry against those claims.**RIGHT WITH CHANGES**

1. **SHOULD-FIX** — The `svg` skip drops style rows the build still kind-checks. `scan.rs:137` marks the `svg` element and every descendant and records no rows. Lowering renames only a few of those attributes (`fePointLight`'s `x` to `lightX`, `scale` to `feScale`, SVG text's `x` to `textX`). `fill`, `stroke`, and a `rect`'s `x` stay CSS rows (`style_names.rs:40`). Round 3's probe still holds: `rect x="pos"` accepts `8px` and refuses a colour with `tamber-kind`. This amend accepts that write. `QUEUE.md:15` calls every attribute in the subtree a prop. `rows_the_build_judges_otherwise_are_left_to_it` (`tamber_rows_check.rs:223`) asserts `pos` → `#123456` is accepted and does not compile the file, so the "never a style row" claim is untested and false. HTML under `foreignObject` is skipped on the same flag.

2. **NIT** — `written` (`rows.rs:137`) still says a `let` is checked where it is bound, and not as the row's literal. QUEUE is the accurate line: `padding={let c = "gap" in c}` is judged through the binding, as the row.

What checks out: (a) the before set is `Place.row`, so one already-failing arm suppresses that authored row. (b) A computed arm makes `written` return false, and an attribute-level `let` is not a row. (c) The group key is `(name, plain, literal)`; a plain place is refused at the attribute and an arm at its literal. `a_plain_row_and_an_arm_alike_keep_their_places` compares the server's full list to the build. (e) The budget asserts the minimum of ten runs, so one slow run cannot fail it.
