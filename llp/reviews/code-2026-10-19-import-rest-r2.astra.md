# Code review: Tamber importers, the rest of rt1's QUEUE entry (budget calibrated to the isolate, CSS importers linear and bounded, marks), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d7594a29a5`.
- **Method:** one brief (sha256 `00a3d1c41ab0e188785ea773541ca207f24784020be9c46a959920da70be390c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two blocking (a long literal copied by every alias; long error messages copied by every Tailwind key), three should-fix (context cloning per sibling; no-op resets quadratic; source grouping and themes scans)
- **Disposition:** all taken in 5375ce5e85: a custom property's name past 256 bytes, its value or a prelude past 4 KiB reported, not copied; the context borrowed; resets by lookup, kept ranges passed over; sources and themes by name

Two blockers remain; the “linear and bounded” claim is premature.

1. **Blocking — colour memoization retains unbounded literal copies.** [engine.rs:252](/tmp/rv-imp2/tamber/src/import/engine.rs:252), [engine.rs:298](/tmp/rv-imp2/tamber/src/import/engine.rs:298). A valid ~319 KB stylesheet containing one `rgb()` literal with 200,000 zero digits and 6,000 one-hop aliases requires **2.4 GB of resolved strings** across the memo and `sides`. Neither the 256-link limit nor `COPIED` catches it. **Fix:** share resolved literals and charge expanded strings before copying, including auxiliary-variable expansion.

2. **Blocking — Tailwind’s budget excludes diagnostics.** [tailwind.rs:755](/tmp/rv-imp2/tamber/src/import/tailwind.rs:755), [tailwind.rs:1014](/tmp/rv-imp2/tamber/src/import/tailwind.rs:1014). Set `--text-base` to a missing variable whose name is 200,000 characters, then have 6,000 type keys reference it. The ~379 KB input produces **1.2 GB of copied error messages while `WRITTEN` remains zero**. Errors also bypass `LONGEST`. **Fix:** budget diagnostic and memo-error copies before allocation; stop with a bounded refusal when exhausted.

3. **Should-fix — context cloning bypasses the copy cap.** [mod.rs:601](/tmp/rv-imp2/tamber/src/import/mod.rs:601). A 500 KB outer prelude containing 20,000 empty `.x{}` siblings copies that prelude **10 GB cumulatively**. The input is ~580 KB, and empty rules never spend `COPIED`. **Fix:** traverse with borrowed preludes or a shared push/pop context; clone only when retaining an accepted block.

4. **Should-fix — no-op resets remain quadratic.** [tailwind.rs:421](/tmp/rv-imp2/tamber/src/import/tailwind.rs:421). With 18,000 `--text-xN` keys and 12,000 `--text-x:initial` declarations, every exact-name reset scans all prefix matches: **216 million candidate visits** from ~535 KB. Namespace resets likewise repeatedly scan excluded namespaces. **Fix:** use direct lookup for exact resets and skip excluded prefix ranges for namespace resets.

5. **Should-fix — wide-import scans remain.** [shadcn.rs:373](/tmp/rv-imp2/tamber/src/import/shadcn.rs:373), [tailwind.rs:705](/tmp/rv-imp2/tamber/src/import/tailwind.rs:705). Source grouping still searches the growing vector per colour declaration: 30,000 declarations (~409 KB) cause **449,985,000 name comparisons**. Shadcn’s theme collection also retains quadratic `contains` scans at [shadcn.rs:201](/tmp/rv-imp2/tamber/src/import/shadcn.rs:201). **Fix:** retain ordered vectors with name-to-index maps or membership sets.

The round-1 mark storage, brace matching, label charging, inline propagation, and prelude-line fixes are present. Within the new limits, independent in-memory comparisons found no unintended differences in alias resolution, Tarjan cycles, failure ordering, kind summaries, Tailwind ordering, or selector deduplication. Parser models matched all 12 checked-in CSS files; the claimed **162-file full-output byte identity was not independently reproduced**.

Budget nesting preserves the outer allowance. The documented ratios imply 576/720 MiB; RSS calibration was not remeasured. Add regressions for the cases above and qualify [§10:4843](/tmp/rv-imp2/llp/1121-tamber.rfc.md:4843) and QUEUE’s CSS closure.

No files modified or builds run.
