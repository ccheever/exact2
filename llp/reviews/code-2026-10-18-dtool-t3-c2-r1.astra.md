# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e7c0a682f`.
- **Method:** one brief (sha256 `f7cc3c2ac9b04484059e9d6aeed2b3c331d3f4222621e52a5e16d99047c87fe3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 5 medium).
- **Disposition:** all taken in `31290cd65` (and `212c41b73` for params written by their short name): an icon param shows its sources, read only, and no longer throws; a duration's slider and stepper write ms, and kinds the dials have no control for take a text field; a panel edit clears the branch editor's draft and mode scope, so it is never a stale draft, and a scrub on another branch of the same param is cancelled; Only this mode on a param with no line writes its line first (and has no scrub); a token option is matched by its name, a literal by its meaning; the panel is shown on the app's design only, its native mark by hello's design file; the real-server setups that copy the stub's design now copy its app too. Tests: the panel's other kinds and a panel edit beside an open param branch. Kept: a colour param whose value names a token shows 'Follows … / Pin' in the branch editor, as a token's does; pinning a literal there is D14's pinned param.

I would block this commit. I found these correctness issues:

1. **[P1] An icon param prevents the entire design from loading.** [components.ts:44](/tmp/rv-dt-t3s2/apps/design-tool/components.ts:44) calls `numberOf(folded)` for every kind. Icon params return objects such as `{sf, material, svg}` in `values`, so this throws `TypeError: v?.trim is not a function`. `loadDesign` catches it and returns `noDesign`. Reproduced with the server’s icon-value shape.

2. **[P1] Opening one param disables edits to other params in the panel.** [app.contract:647](/tmp/rv-dt-t3s2/apps/design-tool/app.contract:647) treats every `set-param` draft as belonging to the open param branch. Open `Card.fill` through Inspect, then move the panel’s `Card.pad` slider: `draftStale` becomes true because their targets differ, disabling Apply and Scrub. The same happens when the panel edits the unconditional value while that param’s conditional branch is open. Panel drafts need distinct ownership from branch-editor drafts.

3. **[P2] “Only this mode” fails for a param without an existing override.** [app.contract:835](/tmp/rv-dt-t3s2/apps/design-tool/app.contract:835) produces a single conditional `set-param`. For untouched `Card.fill`, the server explicitly refuses this because no base `params` line exists ([params.rs:27](/tmp/rv-dt-t3s2/tamber/src/edit/params.rs:27)). The reviewed transaction needs to establish the original default and add the conditional override together.

4. **[P2] Duration sliders emit invalid values.** [components.ts:28](/tmp/rv-dt-t3s2/apps/design-tool/components.ts:28) and the reused dial control support only `px` or no unit. A valid duration param with default `150ms`, range `0..1000`, and `control=slider` starts at **0**, and moving it to 200 writes `"200"` instead of `"200ms"`. The real server rejects that nonzero unitless duration. Reproduced directly.

5. **[P2] With another design file open, the panel reads a different file from the one it edits.** [app.ts:782](/tmp/rv-dt-t3s2/apps/design-tool/app.ts:782) builds components from `describe(file)`, while `set-param` deliberately writes the app design. If `app.tamber` sets padding to 12px and `brand.tamber` has no override, opening `brand.tamber` shows the 8px default and hides Reset. Applying 16px changes `app.tamber`, but refreshing still shows 8px. Pinned state has the same problem. `filesAt` **does move**; rereading the wrong file cannot fix this. Passing the selected filename also incorrectly removes the native marker from `app.tamber#MapCard`.

6. **[P2] Switching branches of the same param leaves its old scrub open.** The generalized cancellation guards compare operation and name, but omit the branch ([app.contract:721](/tmp/rv-dt-t3s2/apps/design-tool/app.contract:721)). Start a scrub on `Card.pad when density=compact`, then use its panel slider, which clears `editWhen`. No cancellation occurs; `scrubFrame` reports “another edit,” and release refuses it. Confirmed through the client’s exported resource functions. The guard needs to recognize this branch change.

7. **[P2] Token select options are compared as colour literals.** [dials.ts:133](/tmp/rv-dt-t3s2/apps/design-tool/dials.ts:133) still applies `folded()` to token identifiers. With colour-token options `red|blue` and current literal `#f00`, it selects token `red`, even if that token resolves to blue. Likewise, `brand.A` matches `brand.a`. Token references need exact identity matching; literal colour equivalence should apply only to literals. I found no corresponding regression for ordinary dials.

The fixtures and tests also need attention:

- **The shared fixture now breaks existing integration-test setups.** [stub/dials.tamber:65](/tmp/rv-dt-t3s2/apps/design-tool/stub/dials.tamber:65) adds `Card.pad` and `Card.tone` overrides, but [scripts/design-tool.test.mjs:157](/tmp/rv-dt-t3s2/scripts/design-tool.test.mjs:157) and several other setups still create a trivial app with no `Card`. Those designs contain unknown param overrides; the real-server reads and golden comparison cannot succeed.
- **The new tests prove less than their names claim.** The stub classifies rebuilds solely by name and accepts transactions without applying them. Thus the Reset test checks stack counts, not byte restoration or refreshed `set`/pinned state. The param-rung test supplies no conditional param branch and never checks `editWhen`, so it does not exercise the winning-branch or “Only this mode” requirements.

The basic `opOf`/`label`/`opText`, Apply, and inverse-stack plumbing looks correct. The real server supports `set-param` scrubs, operation identity separates same-name dials from params, and ordinary token↔param selection clears the field draft.

Validation used read-only Bun probes and source tracing. The checkout remains unchanged; I did not run suites that create files.
