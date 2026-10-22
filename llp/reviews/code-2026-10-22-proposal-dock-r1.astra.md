# Code review: the design tool's proposal dock, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `519223b03d`. The author (Claude) is not a reviewer.
- **Verdict:** two BLOCKERs (delayed focus, the row under the dock) and three SHOULD-FIXes, all taken in 4fb2fcb3bb.

1. **BLOCKER — [app.contract:984](/tmp/rv-dt-propanel/apps/design-tool/app.contract:984): Delayed focus can steal typing.** `focusApply` survives typing, cancellation, selection changes, and manual focus moves. Press Reset, move to another field while the proposal loads, and its eventual readiness steals focus. The task key follows the current edit; it does not bind the request to the original press. Cancel the request when the user moves on.

2. **BLOCKER — [app.contract:2141](/tmp/rv-dt-propanel/apps/design-tool/app.contract:2141): The edited row can be covered.** A bottom-sticky overlay capped at `40vh` still covers a field already near the viewport bottom. Nothing reserves space or scrolls that row above the dock. Preserve row visibility without changing keyboard focus.

3. **SHOULD-FIX — [app.contract:921](/tmp/rv-dt-propanel/apps/design-tool/app.contract:921): Grid edits can inherit docking.** `dockedFor` is never cleared. Open a param’s Back to the package proposal, cancel, then invoke the grid’s equivalent for the same token: `remove-token|name` matches again. Clear docking ownership on cancellation and non-panel edits.

4. **SHOULD-FIX — [app.contract:2147](/tmp/rv-dt-propanel/apps/design-tool/app.contract:2147): The announcement repeats and can be misleading.** Each proposal refresh clears and restores the live text, including background poll refreshes of an unchanged draft. It also announces “Apply or Cancel” while Apply is disabled by stale guards, or scrubbing replaces those buttons. Announce once per newly ready draft, accounting for the available controls.

5. **SHOULD-FIX — [design-tool-dock.test.mjs:41](/tmp/rv-dt-propanel/scripts/design-tool-dock.test.mjs:41): The viewport assertion permits an offscreen dock.** `field.y + 2000` exceeds the harness’s 900px viewport. Assert against `layout.viewport.h`, and require the edited row itself to remain visible; non-overlap alone also passes when the row is offscreen.

**Findings 1–2 block approval.** The containing block spans Dials and Components; the named `tabindex=-1` region is appropriate. No builds or tests run.
