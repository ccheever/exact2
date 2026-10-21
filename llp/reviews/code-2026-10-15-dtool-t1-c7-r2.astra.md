# Code review: LLP 1122 T1 commit 7 (T1a, import review), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `905b0a69b` (commit `cd635dade` and its round-1 fixes).
- **Method:** one brief (sha256 `b71fbea7eb6d2b12adf97adcb474552bdf326727df60a053708cccf2bd662cbe`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 6 medium, 1 low).
- **Disposition:** taken but 4 in part: an accept draws its baseline from the same mark the card does (a recomputed import, or an edit of one: `kept`); the checkboxes are off while a write or an edit is in flight; an edited import names the one it replaced (`from`) and shows the notice and its diff; the role-overrides heading says when they are as ticked; the server refuses several stylesheets (400) and the tool says so; an importer's diagnostics are in the wire's shape, and a string one is shown whole; an unmapped value names its block, keyed by position; an import that maps nothing says so; tests edit the recomputed card through to the bytes, and Figma's references and other modes. 4 in part: a reference is followed to a token the import adds or the design has, and painted; one to anything else stays unpainted.

---

Reviewed HEAD read-only; tests were inspected, not run because the harness writes files.

1. **High — Editing a recomputed import reverses override selection semantics.** [app.ts:1013](/tmp/rv-dt-7r2/apps/design-tool/app.ts:1013)  
   `editImport` preserves `review.kept`, and `reviewOf` uses it to keep overrides checked. `acceptImport` checks only `replaces`, which the edited proposal loses. Import shadcn, tick AccentColor, accept partially, edit a value, then untick AccentColor: Accept includes that unticked override and can write immediately. Leaving it checked instead excludes it. **Fix:** use the same selection baseline in rendering and acceptance, including `review.kept`; test both checkbox states after editing a recomputed proposal.

2. **Medium — Checkbox changes made during an edit are lost.** [app.ts:1042](/tmp/rv-dt-7r2/apps/design-tool/app.ts:1042), [app.contract:764](/tmp/rv-dt-7r2/apps/design-tool/app.contract:764)  
   The carried selections come from the snapshot captured when editing starts. Checkboxes remain enabled while the asynchronous proposal replacement runs. Press Enter on a value, then untick another token before the response: the replacement includes that token again. **Fix:** transfer the current selections when replacement completes, or disable selection controls while replacing the proposal.

3. **Medium — Editing can silently rebase an import and remove its review diff.** [app.ts:1036](/tmp/rv-dt-7r2/apps/design-tool/app.ts:1036), [app.contract:758](/tmp/rv-dt-7r2/apps/design-tool/app.contract:758)  
   Editing submits a fresh proposal without the previous bases or replacement identity. Its diff is hidden because the card displays it only when `replaces` exists. For a Tailwind re-import, another client can change a token, then editing a different imported value replans everything against those new bytes. Accept can overwrite the other client’s value without showing the recomputed diff required by D10. Editing an already recomputed card also hides its diff. **Fix:** preserve replacement provenance and baseline checks, and display the replacement’s returned diff before acceptance.

4. **Medium — Alias tokens still lack the required swatches.** [app.ts:486](/tmp/rv-dt-7r2/apps/design-tool/app.ts:486)  
   The round‑1 fix suppresses invalid CSS instead of resolving proposed colours. Figma aliases such as `dtcg.color.gray.s900`, including aliases inside `light-dark()`, consequently have no swatches. D11 explicitly requires those previews before writing. **Fix:** evaluate the proposed design through Tamber for each mode and scheme, while retaining the authored expression in the editable field.

5. **Medium — The protocol still silently discards additional CSS files.** [tamber_import.rs:112](/tmp/rv-dt-7r2/contract/cli/src/tamber_import.rs:112)  
   The picker and client now reject multiple stylesheets, but `import-plan` still accepts a `sources` array for shadcn/Tailwind, sorts it, and imports only `files.first()`. A protocol client sending two sheets receives a successful review and count that omit the second entirely. **Fix:** reject multiple sources for these importers at the server boundary and test that refusal.

6. **Medium — Importer diagnostics disappear from the displayed error.** [tamber_import.rs:170](/tmp/rv-dt-7r2/contract/cli/src/tamber_import.rs:170), [app.ts:30](/tmp/rv-dt-7r2/apps/design-tool/app.ts:30)  
   Import failures serialize diagnostics as strings, while the client reads them as `{file, line, id, message}` objects. Picking malformed DTCG JSON therefore displays “the source is refused” followed by `:`, losing the filename and parser explanation. **Fix:** return the standard structured diagnostic shape and assert the displayed error using an invalid picked file.

7. **Medium — The revised tests still do not establish several claimed behaviours.** [design-tool.test.mjs:625](/tmp/rv-dt-7r2/scripts/design-tool.test.mjs:625), [tamber-stub.ts:81](/tmp/rv-dt-7r2/apps/design-tool/stub/tamber-stub.ts:81)  
   The Figma assertion checks only that some second mode contains text. Removing non-default-mode editing or swatch rendering would still pass. The shadcn sequence edits only before recomputation, missing finding 1. The stub also renumbers operations without remapping `review.tokens` and offer indices. **Fix:** assert rendered checkbox states and swatches, edit a non-default mode through acceptance and undo, exercise editing after recomputation, and make the stub remap review metadata like the real server.

8. **Low — Unmapped entries lose their source identity.** [app.ts:514](/tmp/rv-dt-7r2/apps/design-tool/app.ts:514), [app.contract:785](/tmp/rv-dt-7r2/apps/design-tool/app.contract:785)  
   `reviewOf` discards `block`, which identifies the source file or CSS block. Figma’s repeated spacing/radius names consequently appear without their mode/file, and share keys and test IDs. A person cannot identify which source declaration needs correction. **Fix:** retain and display the source identity and give each report entry a unique key.

The other round‑1 fixes are consistent on inspection: operation-kind handling, individual `when` editing, zero-operation reports, retained note-only offers, `tamber-role-axis` wording, derived/source-count labels, narrowed picker grant, and server-bound edit/drop calls. The real-server test now meaningfully asserts edited bytes, exclusion of a token, no write on partial acceptance, and exact restoration by one undo.

Verdict: request changes; acceptance can still apply an override the user explicitly unticked.
