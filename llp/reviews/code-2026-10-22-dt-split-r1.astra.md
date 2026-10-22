# Code review: the design tool's browser tests split and their waits trimmed, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** full review of HEAD `023a6999c0`, 9addb2a92e..HEAD (four commits), against the lead's rules (same 18 tests and assertions, cost only, glue lane runs the new files). The author (Claude) is not a reviewer.
- **Verdict:** no BLOCKER or SHOULD; four NITs, all taken in 66d0e2fc68.

No **BLOCKER** or **SHOULD** findings. Four **NITs**:

- **NIT — [scripts/design-tool-scrub.test.mjs:8](/tmp/rv-dtsplit/scripts/design-tool-scrub.test.mjs:8):** Imported `drive` is unused; both drives use locally declared child processes. No other unused or missing imports found.

- **NIT — [scripts/design-tool-ops.test.mjs:44](/tmp/rv-dtsplit/scripts/design-tool-ops.test.mjs:44):** The comment overstates the synchronization guarantee. `+1`, `data` does not guarantee that a late changes answer’s `follow` has run: it can remain armed for the next millisecond. **That does not recreate the dialog race**, because neither confirmation tap advances the frozen clock; the next `+1` occurs after confirmation. Describe that invariant rather than promising the read always landed beforehand.

- **NIT — [scripts/design-tool-canvas.test.mjs:69](/tmp/rv-dtsplit/scripts/design-tool-canvas.test.mjs:69), also [scripts/design-tool-scrub.test.mjs:51](/tmp/rv-dtsplit/scripts/design-tool-scrub.test.mjs:51):** Distinguish the **20-second agent wait deadline** from the **25-second server poll hold**. The current wording conflates them.

- **NIT — [llp/1122-design-tool.rfc.md:1283](/tmp/rv-dtsplit/llp/1122-design-tool.rfc.md:1283):** The coverage table still says `kindsOnTheJsLoop` runs with every invocation of `scripts/design-tool.test.mjs`. It now belongs to `scripts/design-tool-scrub.test.mjs`.

The substantive checks pass:

- Both split commits preserve all **18 test bodies verbatim**. All **321 `expect` sites** remain unchanged. Helpers moved intact, without missing or duplicate implementations.
- All four files call `toolTurns()` and appear exactly once in the parallel glue command. Port-specific staging/dist paths, unique temporary directories, storage names, local Git configuration and child cleanup remain isolated under that execution mode.
- Long `+N real` commands still perform successive clock advances based on elapsed wall time. They service `follow` and `watchCanvas`; the simulated build continues polling independently. Poll counts need not match, but no assertion requires them to.
- The lease checks retain their real-time bounds: open longer than 36 seconds, no hand-back while open, and hand-back 10–45 seconds after closing. Removed `clock data` replies contain no `resources`, so `states[]` indexes remain correct.
- No broken executable references found. The stage-11 file list still covers its matching tests; `async-inputs` needs no change.

Verification included source comparisons, Bun parsing/import checks, executing glue-command generation, and an in-memory probe of the actual clock/scheduler functions. Browser tests were not rerun because their fixtures write files. The worktree remains unchanged.
