# Code review: the design tool's Gallery view, G4, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `9811c48335708167da4769d76031e49bff6a80957b50853bb7c67d7a15365d46`), shared by both reviewers, each blind to the other's review; HEAD `41786fcae1`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX); round 1's others closed.
- **Disposition:**
  Nothing blocks from either reviewer. All taken post-round, without another round (noted in SIGNOFFS):
  - Astra 1, Grok NIT 2 (framing): the gallery page's header must equal the loop's FRAME_ANCESTORS exactly (the tool's origin and its own), on the real loop. The browser's refusal of a frame from another origin rests on that header, as the canvas's own tests do.
  - Astra 2, Grok NIT 1 (the param case): the join test gives a gallery cell a param row. It stays the app's param rung, with no template and so no detach, beside the token row's token rung. The contract test asserts no tokenize or detach on the token row, and no tokenize on the literal one.

No BLOCKER found. Two round 1 findings remain partially addressed:

1. **SHOULD-FIX — [host/web-js/dev-gallery.test.mjs:95](/tmp/rv-dt-g4r2/host/web-js/dev-gallery.test.mjs:95): Foreign-origin framing refusal is still untested (A2).** The new assertion checks a CSP prefix; it never embeds the gallery from an unauthorized origin. It would also accept that prefix followed by `*`. Add the required browser framing refusal case.

2. **SHOULD-FIX — [apps/design-tool/app.test.contract:2304](/tmp/rv-dt-g4r2/apps/design-tool/app.test.contract:2304): The inspector coverage still omits the param case (A3).** HEAD adds a real selection join and client control assertions, but both fixtures contain only a token row and literal opacity. Checking Detach’s absence on opacity does not exercise its param-with-template branch. Add a joined param row, verify its edit remains available without a template/Detach control, and verify token and param edits target the app design. Explicitly assert Tokenize’s absence too.

The other round 1 findings are closed:

- **A1/B1:** [grid.ts:159](/tmp/rv-dt-g4r2/apps/design-tool/grid.ts:159) clears page-specific navigation state before assigning frame URLs. [app.test.contract:2279](/tmp/rv-dt-g4r2/apps/design-tool/app.test.contract:2279) covers switching to Gallery after fallback reload.
- **A4/B2:** [tamber-stub.ts:451](/tmp/rv-dt-g4r2/apps/design-tool/stub/tamber-stub.ts:451) clears `galleryCopies` on restart.

No additional implementation defect found: origin validation and paired mode remain intact; gallery builds skip navigation leadership and navigate/reload commands; pins, holds and give-backs remain available; generated-source rungs suppress recipe writes; build comparisons and the Live chip do not count gallery designs.

No files changed; no builds or test suites run.
