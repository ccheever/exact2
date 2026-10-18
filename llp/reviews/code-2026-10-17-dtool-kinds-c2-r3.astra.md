# Code review: LLP 1122, stage 6 kinds in the tool, steps 3-4, round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `7342e01d0`.
- **Method:** one brief (sha256 `3990c87597229cfdf623e1cd32f1e0a17cb2ae0ccf951b5bde51d98b516f7405`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 1 medium, 1 low).
- **Disposition:** all taken, in `025dad11f`:
  - Only this mode starting a fresh draft clears the fields' cut;
  - any cell token draft (field, text field or Only this mode) is stale once its branch's value moves (`draftFor`);
  - a dotted name stops before a digit;
  - the e2e requires the frame on the served design at every look and always checks the previewed value.

- **High — “Only this mode” revives abandoned fields.** [app.contract:606](/tmp/rv-dt-k3r3/apps/design-tool/app.contract:606), [app.contract:763](/tmp/rv-dt-k3r3/apps/design-tool/app.contract:763). Change `space.edge` from `12px` to `20px` through its amount field, change a dial without applying, then press “Only this mode.” That action resets `editValue` to `12px`, but `fieldDraft` becomes true again because `partFor` was retained. The fields show `20px`; Apply or Scrub sends `12px`. A full-value edit also leaves `rawFor` active across the dial change. **Fix:** clear both `rawFor` and `partFor` whenever another operation replaces the draft, including when “Only this mode” initializes one. The new dial-switch test misses this button path.

- **High — Full-value drafts bypass stale detection.** [app.contract:614](/tmp/rv-dt-k3r3/apps/design-tool/app.contract:614). Starting at `12px`, enter `16px` in `branch-value` and commit the field change. `cellText` sets `rawFor` but clears `partFor`. If another client writes `20px`, the refreshed branch no longer matches `rawFor`, so the typed fields show `20px`; `editValue` remains `16px`. Because `partFor` is empty, `partsStale` stays false. Once the proposal refreshes, Apply and Scrub can send `16px`. **Fix:** include raw-origin drafts in staleness detection, keeping their fields and draft together until explicitly discarded or rebased.

- **Medium — `ITEM` still accepts multiple CSS items as one word.** [parts.ts:15](/tmp/rv-dt-k3r3/apps/design-tool/parts.ts:15). Tamber accepts `red.0 1px 2px`: its CSS reader splits `red.0` into colour `red` and offset `.0`. The new identifier regex accepts the entire word, so the editor shows colour `red.0`, x `1px`, y `2px`, and empty blur. Changing colour to `blue` produces valid `blue 1px 2px`, silently changing both offsets and removing blur. **Fix:** match Tamber’s identifier boundaries—particularly dots followed by identifier starts—or refuse typed editing whenever one word contains multiple CSS items. Add this case beside `2px#000`.

- **Low — The JS-loop e2e can pass without previewing the final value.** [design-tool.test.mjs:1010](/tmp/rv-dt-k3r3/scripts/design-tool.test.mjs:1010), [design-tool.test.mjs:1050](/tmp/rv-dt-k3r3/scripts/design-tool.test.mjs:1050). If the frame receives the initial `14px` preview but misses `16px`, `look()` returns after its timeout with mismatched frame and snapshot digests. The assertions still pass: the frame changed from its original design, while the server snapshot contains `16px`. Keep’s assertions do not check digest equality either. **Fix:** fail on synchronization timeout and assert that the frame’s design equals the snapshot’s during preview and after Keep.

The unchanged-value reread, blur/spread dependency, empty unit, `normal` line-height, uppercase `INSET`, and `inlineWhy` fixes appear correct on inspection. Tests were not run because they create files.

**Verdict: Request changes — Apply and Scrub can still send values that disagree with the visible fields.**