# Code review: LLP 1122 T1a commit 7 (import review), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `2f7aee4ad` (commit `ed457d779` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `1cdd18f8cb8d0b06bc89bbf25d811e9de43ec9818a91e1a9a2e2eed64a2d8594`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 2 low).
- **Disposition:** taken but 1 and 3 in part: an edit now holds again only what the card has chosen (the tokens not left out, the overrides ticked), with the edit, its review renumbered to those items and its overrides on, so an unticked override is never checked with an edit it has no part in and the card's diff is what Accept writes; a reference is followed in the same mode through what the import adds or sets, and a design token's residual through its fallback; several files' diagnostics are named by their message alone, no place invented; the report of an import with nothing to propose names each value's block; the stub renumbers a recomputed import's review as the server does; tests untick the override on the edited card through to the bytes, edit and accept a Figma mode's value, and check a reference painted as its token in that mode. 1 in part: an edited value that makes an offered role vary by an axis is not caught before accepting (the build refuses it, `tamber-role-axis`). 3 in part: a reference whose target has modes it has not is not given rows for them. Round 3 was the last.

---

Read-only review at `2f7aee4ad`. Tests were inspected, not run because the harness writes files.

1. **Medium — Editing preserves role offers that are no longer valid.** [app.ts:1079](/tmp/rv-dt-7r3/apps/design-tool/app.ts:1079)  
   The replacement copies `review.offers` without checking them against the edited values. On `theme-fixture`, import shadcn-neutral, change `shadcn.primary` to `accent.soft`, tick AccentColor, and accept. `accent.soft` varies by `dark-style`, but AccentColor remains offered without a warning. Acceptance writes a design whose next build fails with `tamber-role-axis`; that restriction is enforced during lowering, not by the transaction’s Tamber check. **Fix:** recompute offer eligibility after edits and replace invalid offers with note-only rows.

2. **Medium — Unchecked overrides can prevent valid token edits.** [app.ts:1071](/tmp/rv-dt-7r3/apps/design-tool/app.ts:1071)  
   Editing submits every held operation, including unchecked overrides. On the same fixture, leave AccentColor unchecked and change `shadcn.primary` to `AccentColor`. The proposed operations still include `AccentColor = shadcn.primary`, producing a cycle and refusing the edit. The chosen import, without that override, is valid. **Fix:** validate edited tokens independently of unselected offers, then validate each optional offer separately while preserving the selections.

3. **Medium — Reference swatches still omit modes and can show obsolete colours.** [app.ts:487](/tmp/rv-dt-7r3/apps/design-tool/app.ts:487)  
   Resolution follows only an added token’s default value, ignores proposed `set-token` changes, and otherwise reads tuple zero from the latest design snapshot. Mode rows also come only from explicit `when` operations. In the Figma fixture, `dtcg.surface.surface-brand` references `dtcg.color.brand.watermelon`: its review shows the default `#ffbe16` but omits Modern Theme’s `#7db8a9`. References to tokens updated during re-import can instead show their old colours. **Fix:** resolve the proposed design through Tamber for every relevant mode and scheme, retaining the authored expression separately for editing.

4. **Medium — The tests still leave claimed behaviours unproved.** [design-tool.test.mjs:635](/tmp/rv-dt-7r3/scripts/design-tool.test.mjs:635), [tamber-stub.ts:89](/tmp/rv-dt-7r3/apps/design-tool/stub/tamber-stub.ts:89)  
   The Figma assertions check nonempty strings; deleting non-default-mode editing or returning an arbitrary nonempty swatch would still pass. The recomputed-import test leaves AccentColor checked and never exercises unticking it after editing. The stub also still renumbers operations without remapping `review.tokens` or `offers[].items`, so partial-import reviews differ from the real server. **Fix:** edit a non-default mode through acceptance and undo, assert exact rendered swatches and checkbox states, exercise the unticked override case, and remap the stub’s review metadata.

5. **Low — Multi-file diagnostics are attributed to the first file.** [tamber_import.rs:179](/tmp/rv-dt-7r3/contract/cli/src/tamber_import.rs:179)  
   Every diagnostic receives `files.first()` as its filename. Picking valid `a.json` and malformed `b.json` therefore displays an `a.json:1` location alongside an explanation naming `b.json`. **Fix:** preserve the actual source filename and position in importer diagnostics; omit an unavailable location instead of inventing one.

6. **Low — Imports with no proposal still lose unmapped source identities.** [app.contract:834](/tmp/rv-dt-7r3/apps/design-tool/app.contract:834)  
   The held-card path now displays `u.block`, but the no-proposal report does not. Re-importing unchanged Figma files therefore lists repeated spacing/radius names without their source file or mode. **Fix:** display the retained block here using the same formatting as the held review.

Verdict: request changes — 4 Medium and 2 Low findings.
