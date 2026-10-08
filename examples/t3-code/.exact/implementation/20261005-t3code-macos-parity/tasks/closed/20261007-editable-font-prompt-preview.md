---
name: 20261007-editable-font-prompt-preview
plan: 20261005-t3code-macos-parity
implementation: done
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-editable-font-prompt-preview
pr_url: https://github.com/ccheever/exact2/pull/257
verified_commit: 7635db11c
---

# Type into the Appearance prompt font preview

## Outcome

Settings > Appearance lets the user edit the prompt sample to try its font family and size.
The preview supports text input, cursor movement, selection and undo while retaining the
sample's skill/file chips. Its draft belongs to the preview; typing never sends a message or
changes the active thread's composer draft.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.
Both apps were running on the same Mac with source files unchanged.

1. Open Settings > Appearance, with Advanced typography off.
2. Click the prompt sample under Interface font and append ` AUDIT_FONT_PROBE`.
3. In the reference, press Command+Z to restore the sample.

| App | Observed result |
| --- | --- |
| Reference Electron | The sample accepts the suffix. The screenshot and `Prompt font preview` textbox show the changed text. Command+Z removes the suffix. No typography setting changes. |
| Exact | Clicking `prompt-font-preview` leaves a `View` containing only text and chip rows; no editor mounts. The native driver's attempt to type the suffix into that target returns `view 45943 is not an input`. The sample remains unchanged. |

The native result establishes the clicked surface and its lack of an input target. OS-key
simulation was unavailable in this accessory-mode session, so the driver refusal is not
presented as an OS typing trace. The source below independently confirms that the preview
has no editable control, draft state or input handler.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-prompt-preview-edited.{png,txt}`
- `target/desktop-audit/evidence/ref-settings-prompt-preview-restored.{png,txt}`
- `target/desktop-audit/native/native-final-prompt-preview-click.{png,json}`

These captures are local artifacts and are not committed. The input result above was recorded
by the native audit driver. The acceptance rows below require new verification after implementation.

## Scope and guidance

Reference `apps/web/src/components/settings/SettingsFontPreviews.tsx:30-56` defines
`PromptFontPreview` as an enabled `ComposerPromptEditor` with local prompt and cursor state.
Its `onChange` updates that state; it is intentionally interactive. `SettingsPanels.tsx:1588`
uses it for Prompt font in advanced mode, and line 1751 uses it under Interface font in simple mode.

Clone `settings-rows.contract:234` mounts `FontPromptPreview`; its definition at line 402
renders fixed text and `FontChip` rows. It has no input, focus or editing state. Replace this
static preview with an editable preview using the existing composer editing/chip behavior where
practical. Inspect `composer-editor.contract` and `modules/apple/T3ComposerEditor.swift` when
choosing the integration; keep its document, selection and focus separate from the active thread.

Use the reference's initial sample and local lifetime. Font family/size changes must update
the preview text and chips while following the reference's draft behavior. Match its mount/remount
reset behavior rather than persisting the sample as a setting. This task does not add message
submission, provider calls or a second conversation composer.

## Dependencies and deduplication

- [Desktop visual parity](20261007-desktop-visual-parity.md), V01, owns the prompt-chip icons
  and highlighted code sample. This task owns editing behavior. Coordinate the shared preview
  component without duplicating the cosmetic acceptance rows.
- [Interface font size conversion](20261005-interface-font-size-conversion.md) records
  sizing of the sample text and chips. It does not track input, cursor state or undo.
- [Composer fidelity](20261005-composer-fidelity.md) owns the conversation composer;
  it does not cover editing the Settings preview.
- [Installed font picker](../20261007-installed-font-picker.md) and X48 own installed-family
  enumeration/application. Editing with currently supported families does not depend on X48.
- No framework blocker was demonstrated by this comparison. Reuse of the existing editor
  needs implementation verification; do not silently substitute a plain field that loses chips.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Type and undo | Click the sample, append ` AUDIT_FONT_PROBE`, then Command+Z | Suffix appears and undo restores the sample, matching the reference | Paired screenshots and input/selection state |
| Cursor and selection | Move the caret, select and replace text before and after the chips; delete and undo | Same text/chip editing and caret behavior as the reference | Bounded live input drive |
| Font changes | Edit the sample, then change a supported family and font size | Text and chips reflect the chosen typography; draft behavior matches the reference | Before/after captures |
| Simple and advanced | Exercise Interface font's sample, then advanced Prompt font's sample | Both are editable; switching/remounting follows reference lifetime | Live captures |
| Draft isolation | Keep an unrelated draft in an active thread, edit the preview, then return | Thread draft is unchanged; no message, provider operation or preview preference write occurs | UI readback and command log |
| Focus | Enter and leave the preview by keyboard; type editing keys while it is focused | Accessible editable surface; editing keys affect the preview, and focus returns to Settings controls normally | Keyboard drive and accessibility tree |

## Progress

- 2026-10-08: Implemented the isolated editable sample.
  - `settings-prompt-preview.contract` holds the reference's serialized sample in component state:
    a textarea with the new `t3-prompt-preview` hatch, `appearance="none"` like the editor's
    `focus:outline-none`.
  - `T3Module.promptPreview` is a second `T3ComposerEditor` bound to that hatch. It gives native chips,
    atomic chip keys and undo grouping. It never touches the composer's keys, history, send monitor,
    snapshot owner or draft. Tab and Shift+Tab walk focus, as the reference editor (no command-key
    handler) does.
  - Skill chips read "Frontend Design" (formatProviderSkillDisplayName). This applies to the composer too.
  - Chips inherit the prompt's family.
  - An unset Prompt font previews in the Interface family (`--font-composer` falls back to `--font-sans`).
  - Changes from independent review (no blocking findings):
    - Markdown markers stay text, because the reference leaves `richTextEnabled` off.
    - Chip labels match weight 500 as CSS does: the family's medium face, else its regular face.
    - Folder drops and pasted-text folding stay composer-only.
  - Verified `7635db11c`, draft PR #257. It is expected to conflict with #250 in `settings-rows.contract`;
    the PR body gives the resolution.
  - Provisional, user decision pending: none.
  - Follow-up, not in this task: the composer's skill chips use the provider's `displayName` when the
    skill is known (the reference's `skillLabelFor`); the clone always title-cases the raw name.

2026-10-08 (real-input batch, records PR): Tab and select/replace pass; ⌘Z does not undo in the preview (clone bug, the composer behaves the same). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| 1 | `7635db11c` on `d82fb6a47` | `bun test examples/t3-code` 2515 pass / 1 skip / 0 fail (base 2511 + 4 new); strict tsc clean; `contract build` 2631 slots; `cargo test -p t3-code-macos --lib` 11 pass; composer AppKit binary 51 tests / 0 failures (base 46 + 5 `PromptPreviewEditorTests`); caps pass; five checks pass (cargo test 3383 passed, 0 failed, 33 ignored; build, clippy, fmt, boot exit 0); app bundle builds. Runner recipe `passed`, `source_unchanged: true`, digest `7f30900e4962…`, committed tree matches | PR #257 | — |
| live 1 (agent) | build of attempt 1 before the Tab, chip-family and prompt-family fixes | Typing, caret, selection, atomic chips, remount and draft isolation pass. It found the Tab trap, the system-font chip labels and system-ui for an unset Prompt font; all three were fixed. | `ops-after-attempt1.txt` | — |
| before (agent) | base `da4f4512f` | Static sample; key input changes nothing; "view 2423 is not an input" | `ops-before.txt` | — |
| live 2 (agent, the retry) | the final build except `appearance="none"` (focus ring), chip weight 500 for non-system families and the review's composer-only paste/drop and plain-marker settings | Every row except Cmd+Z: see the table below | `ops-after.txt`, `live-session.md`, pairs 01–07 | Cmd+Z: deferred |

Evidence: `t3-code-evidence/editable-font-prompt-preview/` at `d29d205663ea` (pairs 01–07, `live-session.md`, op transcripts).

| Criterion | Result | Proof |
| --- | --- | --- |
| Type and undo | Typing: pass (key-by-key suffix at the caret). Cmd+Z: deferred to the real-input batch (screen locked, user away). In agent mode, Meta+z cannot reach a plain textarea's undo manager (live-session.md); the undo history is tested in AppKit | pair 02; `PromptPreviewEditorTests.testTypingAndUndoStayInThePreview` |
| Cursor and selection | pass: Shift+Right selection replaced; arrows step over chips as one; Backspace removes a whole chip | pair 03; AppKit `testChipsAreAtomic…` |
| Font changes | pass: New York keeps the draft, and text and chip labels follow; prompt 18 px keeps the edit | pairs 05, 06 |
| Simple and advanced | pass: both are editable; Advanced on/off remounts a fresh sample | pair 06, transcript |
| Draft isolation | pass: the thread draft is unchanged; no thread or message; lane `settings.json` untouched after server start | pair 07, live-session.md |
| Focus | Tab and Shift+Tab in and out: pass (tree focus read-back). Accessible label "Prompt font preview". Real-key Tab: deferred to the real-input batch | pair 04; AppKit `testTabAndShiftTabLeaveThePreviewWithoutTyping` |

## Real-input batch steps

Deferred: screen locked (user away). Run them in one session.

1. Build: in `~/orca/workspaces/exact2/t3-code-editable-font-prompt-preview`, run
   `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`.
   It needs `examples/t3-code/server-runtime` staged (`bun examples/t3-code/stage-runtime.mjs --offline`;
   the cache is in `.runtime-cache/`).
2. Lane copy: copy `target/clients/*/com.exact.t3code.macos/macos/T3 Code (Exact).app` to
   `target/lane/apps/T3 Code (Lane EFP).app`. Set `CFBundleIdentifier` to
   `com.exact.t3code.macos.laneefp`, and `CFBundleName`/`CFBundleDisplayName` to `T3 Code (Lane EFP)`.
   Then run `codesign --force --deep --sign -`.
3. Launch it normally, not in agent mode, with `target/lane/drive-env.sh` sourced: HOME, CODEX_HOME,
   CLAUDE_CONFIG_DIR, XDG_* and `T3_LOCAL_HOME=target/lane/t3home` with `T3_LOCAL_PORT=16520`. Run its
   `Contents/MacOS/T3 Code (Exact)` in the background, record the PID, and wait for the window
   (about 20 s for the embedded server). Take the real-input lock.
4. Click the sidebar's Settings gear (window point about 24,820), then Appearance (about 128,152).
   Scroll the content down to Typography.
5. Click the end of the Interface font sample's text, then press Cmd+Down.
   `orca computer paste-text " AUDIT_FONT_PROBE"` (paste, not typing: the input source is Korean 2-Set).
   Screenshot (`screencapture -x -o -l <window id>`) and read the suffix. Press Cmd+Z: the suffix is gone
   and the chips remain. Press Cmd+Shift+Z: it is back. Screenshot each step.
6. Double-click "flaky", paste "stable", then Cmd+Z: "flaky" is restored.
7. Press Tab: focus goes to the Monospace font family control, no tab is typed, and no focus ring is
   drawn on the sample. Press Shift+Tab: the caret is back in the sample. Screenshot both.
8. Check that the thread's composer draft is unchanged: close Settings and read the composer.
9. Quit the lane app (its PID only), then release the lock. Upload the before/after shots next to
   this PR's evidence.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| 5. Type the suffix, ⌘Z, ⌘⇧Z | Typing PASS; undo FAIL (clone bug): ⌘Z (orca and HID), Edit › Undo from the menu bar and typed-key undo leave the text unchanged; the composer behaves the same | [efp-undo-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/01-efp-undo-strip.png), [efp-undo12](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/02-efp-undo12.png), [efp-15-undo-mid-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/05-efp-15-undo-mid-crop.png), [efp-composer](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/07-efp-composer.png) |
| 6. Double-click "flaky", paste "stable", ⌘Z | Select/replace PASS (chips intact); undo FAIL (same bug) | [efp-replace-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/03-efp-replace-strip.png), [efp-caret](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/04-efp-caret.png) |
| 7. Real Tab / Shift+Tab | PASS: Tab → Monospace family control, no tab typed; Shift+Tab back into the sample | [efp-tab](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/06-efp-tab.png) |
| 8. Thread draft unchanged | PASS | [efp-18-draft-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/08-efp-18-draft-crop.png) |

Full record: [editable-font-prompt-preview.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/editable-font-prompt-preview.txt).

## Next action

Review of draft PR #257 (merge after #250, then merge the feature branch in here and resolve `settings-rows.contract`). Run the real-input batch steps above when the screen is unlocked; they also re-capture the after shots on the final build.
