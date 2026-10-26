# Code review: the Inspector's folded initial rows and the base rows across the grid, round 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `94f542c91277e34d6a3e516891802d667d3a598cf5812ff02a3c7325e439bea2`), shared by both reviewers, each blind to the other's review; HEAD `cf1ce29292`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; 1 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken post-round, in their own commit:
  - Astra 1 (the refold task could be cancelled: A shown, then B and A again within its 1 ms, or B and A in one poll): the task is gone. Each selection event counts a visit when it names another view than the one before it (inspector.ts `visitOf`, called where app.ts takes the event, on a line it already had). The fold opens for one visit (`inspected.visit`), so a round trip however close is a new visit, and a refresh of the same view keeps it. Tested: the A, B, A round trip folds again; a refresh of the same view keeps it shown. A `visitOf` that never moves fails the test.
  - Astra 2 (same-key refreshes untested): added, as above.
  - Astra 3 (layout coverage): non-overlap holds; narrow, expanded, package and override geometry remain untested and share the markup.
  - Grok 1 (a review-history note in the scale test's comment): its wording is kept as the layout's reason in the test; the code comment was reworded in round 1.
  Suites: contract 67 of 67; design-tool-scale and design-tool.test 18 of 19 with T2b timing out under load (its known flake), then T2b passing alone.

1. **NIT** — `scripts/design-tool-scale.test.mjs:74`. The overlap check’s comment still cites “finding #3 of the clone’s pass,” the same review-history note round 1 asked to drop from `inspector.ts`.

The refold task does not fold while the person is still on that view. The gate is `initialFor != "" and initialFor != inspected.key` (`apps/design-tool/app.contract:956`). A refresh, a held-state report, or `loadSelection` answering again leaves `inspected.key` as `[build, node, site, instance, buildKey]` (`inspector.ts:103`). While that string is unchanged the gate stays false, and a gated `after` is not restarted when its key is unchanged (`runner/src/runner/gates.rs:52`). Pending and failed reads keep the previous `inspected` value, so the key does not flicker to `""`. It fails to fold only if `inspected.key` is the expanded key again before that 1 ms deadline: a false gate drops the timer. That is the commit the other view is shown, plus 1 ms. A later return is folded. A person does not get back inside that window; the test clocks it (`app.test.contract:2361`).

The in-play clause is the same on both derives (`app.contract:953`): keep a row whose `edit` or `state` is `editTarget`. Held rows are built by the same `row()`, so a `set-row` or `set-state` edit matches. It does not change what a real held list shows. A held row’s `source` is a place, not `"initial"` (`scripts/tamber-inspect.mjs:454`), so `initial` is false (`inspector.ts:91`) and `foldedHeld` stays empty. The clause never decides a held row.

The tests assert what they claim. On node 8 the hidden rows follow the derive alone. The return to node 7 requires the task: if `initialFor` were still that key, `selrow-initial` would be absent and `selrow-stroke-linecap` would be shown (`app.test.contract:2369`). The “2 more” count requires the padding row to stay out of the fold. They never re-post the same key after expanding, so they do not lock the same-view case above.
grok exit 0
