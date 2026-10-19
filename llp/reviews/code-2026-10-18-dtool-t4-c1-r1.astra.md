# Code review: LLP 1122 T4 step 1, the Hold control, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `335242300`.
- **Method:** one brief (sha256 `51bd3a4ba32cf55cfba001b79611f2a1592c0268e349aab9374887dd752d5ee5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 6 medium).
- **Disposition:** taken in `ab3c809bed` (the reviewed `52c780fd1`, `335242300` are `1f9f9766a`, `493b45ac2` after a rebase): a build's `held` report updates its hold (no listing needed) and confirms only the hold it answers, since a changed hold drops the listing's earlier report; a let-go with no reason keeps the last word and Let go from here says 'Let go.'; the notes are cleared with a new server run; the held look's rows show only while the hold on this copy is held, each at its part's place (a held row's source is a place), and the page is asked once a hold to report the selection again; a hold's end is said even when the selection is stale (only the controls go); a held hold on another node says Held; the press's node is named when the build names it (stage 9 E's `held.site`), which until E lands it cannot. The test drives the wire's own flow (a `held` event alone, the join's held rows, Let go). `335242300`: confirmed a straight move.

**I would block S1 on the issues below.** `335242300` preserves behavior.

1. **[P1] Successful `held` reports are ignored** — [app.ts:1069](/tmp/rv-dt-t4s1/apps/design-tool/app.ts:1069). The real server emits only a `held` event when the host acknowledges a hold; this handler does nothing for a non-null state. After pressing **Pressed**, the inspector therefore stays “Holding” until an unrelated listing/status arrives. The test masks this by injecting an updated `builds` listing before the acknowledgement at [app.test.contract:1197](/tmp/rv-dt-t4s1/apps/design-tool/app.test.contract:1197).

2. **[P2] A previous acknowledgement confirms a replacement prematurely** — [holds.ts:42](/tmp/rv-dt-t4s1/apps/design-tool/holds.ts:42), [app.ts:422](/tmp/rv-dt-t4s1/apps/design-tool/app.ts:422). The server’s listing retains its **last** `held` report when another hold is requested. Starting with acknowledged `active`, another client requesting `hover` produces `hold.state = "hover"` alongside `held.state = "active"`. The inspector immediately says **“Held hovered.”** I reproduced this before the hover acknowledgement. A changed hold must invalidate the previous acknowledgement; any truthy `held.state` is insufficient. The stub masks this difference by resetting `held` on every request.

3. **[P2] Held rows survive release and state changes** — [inspector.ts:79](/tmp/rv-dt-t4s1/apps/design-tool/inspector.ts:79). `heldRows` comes unconditionally from the saved selection, independently of the current build hold. After receiving active rows and then a real-input release, I got the release message **and the old active rows still displayed**. Switching states likewise retains the previous look. The hold flow also requests no fresh selection, so starting a hold need not fetch its rows. Invalidate the old snapshot and refresh it for the current hold.

4. **[P2] Real held-row sources render as `[object Object]`** — [inspector.ts:68](/tmp/rv-dt-t4s1/apps/design-tool/inspector.ts:68). Held rows’ `source` is a location object, whereas base rows use strings such as `"authored"`. Reusing the base-row formatter loses the file and span. Passing an actual `tamber-inspect.join()` result through `selectionView()` reproduces this. The test instead supplies `"source": "authored"` and a fabricated editable rung, so it does not exercise the shipped payload.

5. **[P2] A new-plan clearing diagnostic is hidden precisely when needed** — [inspector.ts:76](/tmp/rv-dt-t4s1/apps/design-tool/inspector.ts:76), [app.contract:1165](/tmp/rv-dt-t4s1/apps/design-tool/app.contract:1165). When no site corresponds in a new plan, the server sends `hold-cleared`, but the plan change also makes the selection stale. `noHold` discards the status, and the Contract hides the entire block. The designer sees the generic stale-selection message instead of why the hold disappeared. Keep the diagnostic visible independently of whether hold controls are usable.

6. **[P2] Reasonless clear acknowledgements overwrite useful diagnostics** — [app.ts:1069](/tmp/rv-dt-t4s1/apps/design-tool/app.ts:1069). After that `hold-cleared`, the host legitimately acknowledges the clear with `{state: null}` and no reason. This replaces the correspondence diagnostic with **“The hold was let go: .”** Explicit **Let go** can produce the same malformed message. Preserve the meaningful reason and handle explicit clears separately.

7. **[P2] The ancestor press target is never identified** — [holds.ts:42](/tmp/rv-dt-t4s1/apps/design-tool/holds.ts:42). Selecting a label inside a pressable ancestor always produces “this one or the nearest above with a press.” It never names which node actually holds the press. This fails review2’s explicit S1 condition. The current `Holding` model contains no resolved target identity to display.

Smaller points:

- [holdNotes](/tmp/rv-dt-t4s1/apps/design-tool/app.ts:398) is cleared only by this client’s `sendHold`, not on server restart or a new external hold. I reproduced an old unsupported warning appearing for the same build ID in a fresh server run.
- [holds.ts:41](/tmp/rv-dt-t4s1/apps/design-tool/holds.ts:41) always says “Holding” for another node, even after acknowledgement. The test currently asserts that inaccurate wording.
- The test checks replacement wording but never sends a second hold, and never taps **Let go**.

For `335242300`, the three helpers and `whenText` are unchanged byte-for-byte, and the re-exports resolve to the imported functions.

I inspected both commits with `git show` and ran in-memory Bun reproductions using the actual client, build channel, and selection join. No files were changed; the Contract/browser test was not run.
