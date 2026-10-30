# Code review: the Mac window bar's read set, round 3, narrow re-check (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ea4879b5e3`.
- **Method:** one brief (sha256 `c195446ed5f6258d7d722945ca03a3ef688a9a70e57694b913e7088c95ad482b`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Both round-3 majors are fixed in `ea4879b5e3`. The skip check stays on the read set, and the shadow can no longer re-hide a projected node without a mismatch.

`BarInputs.gates` stores the host's `isHidden` (bit 1) and `routeInert` (bit 2) for every id in `reads.nodes`, and only the non-zero pairs. `recordReads` already puts headers, carriers, invocation targets (external `alertdialog` and popover targets included), labelled-by nodes, and both ancestor chains into that set. `NavigationHost.gate` writes those two flags on the route, so a second root hiding the alert's route changes `gates` even though `concerns` is false: `alerts.plan` fails and the header paints. The bar's own projection hide is left out of `gates`.

`BarInputs.projected` stores each projected node's actual `isHidden` (the owner when `projected`, and each beside part). `gate` clears `isHidden` on the selected route on every batch, including an unrelated text edit. `visible()` ignores that projection hide, so the old route and carrier words stay the same. `projected` is what presents, and the bar hides the node again. The debug shadow's first line is that same `isHidden` for the owner, the header, the beside header, and the beside parts. `shadowCheck` still restores the record after `sync()`, but a re-hide is a before/after mismatch (`hidden owner 82 false … → … true`), so the repair cannot pass unseen.

The two new `BarReadsMacTests` are those sequences, both under `unguarded` (shadow off). On the parent, neither `concerns` nor the old inputs move, so the alert button stays an item and the projected route stays unhidden. After this commit they follow `inputsChanged`.

The skip comparison is `O(read set)`, not `O(page)`. `concerns` walks the batch. `currentInputs` walks `reads.nodes`, the projected parts, and the already-indexed routes and carriers. `carrying` is an index lookup. The new inline-run scan and the ancestor early-stop run only while recording a present, and both are cheaper than the walks they replace. Navigation gating the bar actually reads is covered: `visible()` and `inert` walk the same two chains now recorded, and `opensFromTheBar` fails on ancestor `routeInert`. `node.window == nil` is still only inside `visible()` for the header route and carriers, which is not what `gate` writes.

VERDICT: APPROVE

No findings.
