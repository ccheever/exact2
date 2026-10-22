# Code review: the Inspector's folded initial rows and the base rows across the grid, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `94f542c91277e34d6a3e516891802d667d3a598cf5812ff02a3c7325e439bea2`), shared by both reviewers, each blind to the other's review; HEAD `cf1ce29292`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 1 SHOULD-FIX, 2 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken post-round, in their own commit:
  - Astra 1 (the refold task could be cancelled: A shown, then B and A again within its 1 ms, or B and A in one poll): the task is gone. Each selection event counts a visit when it names another view than the one before it (inspector.ts `visitOf`, called where app.ts takes the event, on a line it already had). The fold opens for one visit (`inspected.visit`), so a round trip however close is a new visit, and a refresh of the same view keeps it. Tested: the A, B, A round trip folds again; a refresh of the same view keeps it shown. A `visitOf` that never moves fails the test.
  - Astra 2 (same-key refreshes untested): added, as above.
  - Astra 3 (layout coverage): non-overlap holds; narrow, expanded, package and override geometry remain untested and share the markup.
  - Grok 1 (a review-history note in the scale test's comment): its wording is kept as the layout's reason in the test; the code comment was reworded in round 1.
  Suites: contract 67 of 67; design-tool-scale and design-tool.test 18 of 19 with T2b timing out under load (its known flake), then T2b passing alone.

Nothing blocks. Round 1’s refold finding is only partially closed.

1. **SHOULD-FIX — [app.contract:956](/tmp/rv-dt-rows2-astra/apps/design-tool/app.contract:956): reset can be cancelled.** Expand A, receive B, then receive A before `after(1)` fires: the gate becomes false, cancelling the reset, and A remains expanded. B→A events consumed in one poll also escape detection. Track selection transitions explicitly. The added test advances past this window.

2. **NIT — [app.test.contract:2359](/tmp/rv-dt-rows2-astra/apps/design-tool/app.test.contract:2359): useful regression test, incomplete coverage.** It checks the delayed round trip, but never verifies expansion survives same-key refreshes or held-state reports. The impossible editable-initial fixture remains a defensive test, as acknowledged; it does not establish real edit-flow coverage.

3. **NIT — [design-tool-scale.test.mjs:75](/tmp/rv-dt-rows2-astra/scripts/design-tool-scale.test.mjs:75): layout finding partly closed.** Adjacent cells and the following control now have sound nonoverlap assertions. Narrow scrolling, expanded platforms, package/override geometry and stretch provenance remain untested.

Same-key resource refreshes retain the previous value while pending and **do not trigger refolding**. Same-key held reports likewise preserve expansion. A reload or pin that changes the selection key does reset it.

The derives share the correct predicate and edit-address comparisons. Real held rows have source locations, so they remain noninitial and visible. `editTarget` alone still does not prove an active edit—cancellation retains it—but this causes no demonstrated production folding defect.

The screenshot finding is closed: both recaptured pairs differ and visibly show before/after. The Inspector comment finding is closed. Read-only review; no builds or tests run.
astra exit 0
