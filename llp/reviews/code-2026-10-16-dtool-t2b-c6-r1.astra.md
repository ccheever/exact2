# Code review: LLP 1122 T2b step 6 (the inspector panel), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `10d7f5f4d`.
- **Method:** one brief (sha256 `83bc54370096448d2b731170f626b17642dc5ec4d0c14d3f0d0770c6e4185106`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high, 4 medium, 1 low).
- **Disposition:** taken, but Astra 5 in part. The selection keeps what it was joined against: its build (the build's key), the design's axes, and the scheme it is shown in (the node's own `color-scheme`, else its build's). It is marked stale, with no Edit or Open, when its build goes or re-registers, the axes change, the page sends `selected: null`, or the scheme is not known. Its column is found against those axes, and the modes line is that column's label. Edit and Open open the app's design (`file = ""`). The toggle follows the builds the server said it told (`inspect`'s `builds`), and drops a build on its `selected: null`, when it leaves, or on a new epoch. Impact is shown only when `uses.token` is the token asked for; slot and derive vias are labelled. Outline lists the builds that were told, says when there are none, and the stub tells only builds that can inspect. Rows use the author's CSS names. The chain shows full paths. Tests: a light copy on the light column, an unknown scheme, a build gone, `selected: null`; the wasm drive asserts the exact branch label `text.primary · default (Light)` and the matching row. In part: another client's `inspect` for the same build is not seen (the wire has no inspect-state event), so the toggle shows only what this client did.

---

1. **High — Inspector actions can edit the wrong design file.** [app.contract:607](/tmp/rv-dt-t2b6/apps/design-tool/app.contract:607)  
   `editFromSelection` sets the token and column but leaves `file` unchanged. If another `.tamber` file is open and contains the same token and column, “Edit surface.card” opens that file’s token; subsequent proposals and writes target it. Impact meanwhile describes the running app’s token. **Fix:** switch to the app’s design and resolve the selection’s column against that design before opening its editor.

2. **High — Scheme selection can choose a branch that did not paint the node.** [app.ts:545](/tmp/rv-dt-t2b6/apps/design-tool/app.ts:545)  
   The scheme comes exclusively from the build’s root status. In the existing theme fixture, selecting `dim-preview` in a light build reports `dark-style=dim`, but that subtree explicitly uses `color-scheme="dark"`. This code discards Dim and opens the Light/default column, so an edit changes the wrong branch. Missing status also silently becomes Light. **Fix:** use the selected node’s computed scheme, falling back to a known build scheme; withhold editing when the effective scheme is unknown.

3. **High — Invalidated selections remain current and editable.** [app.ts:1144](/tmp/rv-dt-t2b6/apps/design-tool/app.ts:1144)  
   The fold replaces selections only on `selection`. It ignores `selected: null`, retains selections when builds disappear or re-register, and treats an `unsupported inspect` report about a vanished node only as a build diagnostic. Missed events likewise preserve the selection. Closing or reloading a selected build therefore leaves its old source, values and Edit buttons presented as current. The comment promises a stale label that is never produced. **Fix:** clear or explicitly mark the selection stale and disable its actions on these invalidations; require a fresh joined selection. The explicit clear on an observed server-epoch change is correct.

4. **High — Selection columns and the grid can describe different axis revisions.** [app.ts:547](/tmp/rv-dt-t2b6/apps/design-tool/app.ts:547), [app.ts:812](/tmp/rv-dt-t2b6/apps/design-tool/app.ts:812)  
   `selectionView` converts modes into a positional column ID using the independently cached `designAxes`. Updating the design does not recompute the published selection until another poll returns; even an overtaken design read updates this cache. For example, swapping the order of `dark-style` and `density` changes `2-dark` from Dim/Regular to Black/Compact. A selection converted with the old order can open the latter cell in the refreshed grid. **Fix:** retain named modes and resolve them against the same design revision used by `loadBranch`; reject missing or incompatible axes instead of guessing defaults.

5. **Medium — The inspect toggle tracks clicks, not build state.** [app.contract:595](/tmp/rv-dt-t2b6/apps/design-tool/app.contract:595)  
   `inspecting` flips before the request succeeds and is never reconciled. Escape in the page, another client stopping inspection, an expired connection, or a refused request leaves “Stop inspecting” and Outline enabled while inspection is off. Reloading the tool can produce the reverse disagreement. **Fix:** expose authoritative per-build inspect state and reconcile the control, including failures and reconnects. If the wire remains unchanged, present explicit start/stop commands without claiming synchronized toggle state.

6. **Medium — Impact temporarily attributes another token’s uses to the selected token.** [app.contract:914](/tmp/rv-dt-t2b6/apps/design-tool/app.contract:914)  
   Resources retain their previous answer while fetching. After opening `surface.card`, opening `text.primary` immediately changes `impactToken`, while the count and sites still belong to `surface.card`. The returned `uses.token` is never checked; the initial placeholder also claims zero uses before any answer arrives. **Fix:** display results only when their token matches the requested token, show loading otherwise, and associate the retained Outline result with its token too.

7. **Medium — Outline reports success without establishing any build received it.** [app.ts:1412](/tmp/rv-dt-t2b6/apps/design-tool/app.ts:1412)  
   The result ignores `outline.ok.builds`. The server can resolve several source sites while returning `builds: []`, so closing the last build and pressing Outline still produces “Outlined N site(s).” Builds outside inspect mode also ignore the command. **Fix:** report the returned recipient builds and distinguish a request being sent from outlines actually being shown; report zero recipients explicitly.

8. **Medium — The new tests do not establish their claimed branch and Outline guarantees.** [design-tool.test.mjs:942](/tmp/rv-dt-t2b6/scripts/design-tool.test.mjs:942), [tamber-stub.ts:109](/tmp/rv-dt-t2b6/apps/design-tool/stub/tamber-stub.ts:109)  
   The real-server branch assertion accepts any label beginning `text.primary · `, including the wrong branch. Its impact assertion accepts the placeholder’s zero count. The stub ignores inspect’s `on` value and returns all builds for Outline, including unsupported builds; the test checks only the resulting message. **Fix:** select an explicitly nondefault branch, assert the exact branch and resulting edit scope, wait for the matching impact answer, and verify actual Outline recipients. Add the lifecycle and revision cases above.

9. **Low — Inspector labels expose internal row names instead of CSS names.** [app.contract:900](/tmp/rv-dt-t2b6/apps/design-tool/app.contract:900)  
   Users see `background_color` and `text_color` where their source says `background-color` and `color`. This conflicts with AGENTS.md’s author-facing CSS vocabulary rule. **Fix:** retain wire identifiers internally and display canonical author-facing names from the existing vocabulary.

Static review only; no files changed or builds/tests run.

**Verdict: Request changes—selection validity and edit targeting are not reliably fail-closed.**
