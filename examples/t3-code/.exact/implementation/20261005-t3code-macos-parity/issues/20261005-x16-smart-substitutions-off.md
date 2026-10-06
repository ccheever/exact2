---
name: 20261005-x16-smart-substitutions-off
plan: 20261005-t3code-macos-parity
status: adopted
kind: framework-gap
blocks: [20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-pr-writing-and-metadata]
upstream_url: https://github.com/ccheever/exact2/issues/111
reproduced_on: null
---

# X16: `autocorrect="off"` on a macOS textarea turns off spelling correction only, so AppKit still rewrites quotes, dashes and text

## Summary

In T3 Code every text field keeps exactly the bytes the user types. On exact2's macOS host a `textarea` with
`autocorrect="off"` stops spelling correction but leaves AppKit's smart quotes, smart dashes, text replacement
and link detection on, so typing `"0.1.0"` can be stored as `“0.1.0”`. The clone switches these off per text
view with a hook on two textareas and inside the composer; other textareas have no switch-off. Needed:
`autocorrect="off"` means "no automatic text changes" on the macOS host.

## Why this issue arose

### The T3 Code behavior
- Text typed in the composer, the Files editor, the commit message, the scheduled-task prompt, the theme JSON
  box and settings fields is stored exactly as typed. Per the clone's header comment
  (`T3PanelsNative.swift:6-8`), a browser text field "never substitutes"; this was not re-tested in the oracle.
- The reference sets `spellCheck={false}` on the composer and chips
  (`apps/web/src/components/ComposerPromptEditorTiptap.tsx:236,397`) and `autoCorrect="off"` plus
  `autoCapitalize="none"` on token fields (`.../auth/PairingRouteSurface.tsx:104-111`,
  `.../onboarding/WelcomeWizard.tsx:575-578`). The Files editor is the `@pierre/diffs/editor` component
  (`.../files/FilePreviewPanel.tsx:16,599`), also hosted in the browser.
- Where bytes matter: the Files editor saves to disk; the commit message becomes git history; the theme JSON
  box needs straight quotes to parse; a prompt carries code and shell snippets (`--`, `-->`, `'`).

### What exact2 does today
- GAPS summary row X16 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked
  against `main` `d2cb661eb`): "`autocorrect="off"` also turns off smart quotes, dashes, text replacement" /
  "Exact bytes typed in composer and Files editor" / workaround "`t3-plain-text` hook". GAPS has no detail
  section for X16; the mechanism comes from the clone's own header comment below.
- Bundled library: not covered: unknown.
- Observed in the clone tree (mc-orch, 2026-10-05): `modules/apple/T3PanelsNative.swift:1-8`: "A Contract
  textarea maps `autocorrect="off"` to spelling only, so AppKit's smart quotes, smart dashes, text replacement
  and link detection stay on and `"0.1.0"` would be saved as `“0.1.0”`." README.md:278-282 says the same.
  The round-11 check "type `"`, `'`, `--` and `-->`; the sent prompt and the bytes on disk must be exactly what
  was typed" (AGENT-HANDOFF.md:431) covered the composer and the Files editor only.

### Where the clone hits it
- Hook `t3-plain-text` (`modules/apple/T3PanelsNative.swift:52-55` turns off quote, dash, text-replacement and
  spelling substitution): attached to the Files editor (`r4-surfaces-files.contract:206`) and the script
  command box (`settings-b-actions.contract:121`). The composer's text view gets the same switch-off when it
  attaches (`modules/apple/T3ComposerEditor.swift`).
- No hook on: the commit message (`r4-git.contract:327`), the theme JSON box
  (`settings-appearance-import.contract:91`), the scheduled-task prompt (`settings-scheduled.contract:315`),
  scoped settings text values (`settings-source-control.contract:213`). If the same mapping applies there
  (to confirm in a drive), a quote typed in the theme JSON box becomes a curly quote and the JSON no longer
  parses, and a commit message gets curly quotes. Single-line `input` fields were not checked.
- Workaround difference: none visible where the hook is attached; unfixed elsewhere. It also costs a hook
  named in `app.json`, a Swift routine, and the discipline to attach it to every new textarea.

## Why it must be resolved

The goal is a complete clone; a declared difference is not an end state. Silent rewriting of typed text is a
data-integrity problem, not a style one: a commit message, a JSON theme or a shell command is wrong without
any visible sign. Today safety depends on every ticket remembering the hook; new text boxes are planned
(PR comment and review editors in `20261005-pr-writing-and-metadata`, diff comments in
`20261005-diff-review-engine`, automations), and each would need it. `20261005-composer-fidelity` rewrites the
composer prompt (the ultrathink prefix) and relies on exact bytes. Cost: one more native hook per box.

## Requested support

The web way: HTML `autocorrect="off"` (WebKit-defined, in the HTML Living Standard) says the user agent
must not change typed text automatically; browsers other than WebKit never substitute. On the macOS host:
- **A (recommended).** For `textarea` and text `input`, `autocorrect="off"` also sets the text view's
  `isAutomaticQuoteSubstitutionEnabled`, `isAutomaticDashSubstitutionEnabled`,
  `isAutomaticTextReplacementEnabled`, `isAutomaticLinkDetectionEnabled` and data detection to false
  (and spelling correction as now). `spellcheck="false"` keeps controlling spell checking.
- **B.** Make "no automatic substitution" the default for web-style fields and add an opt-in
  (`autocorrect="on"`) for the AppKit behavior. Closest to Chrome, but changes the default for every app.
Other hosts: iOS has the same notion (smart punctuation); web and Linux need nothing.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Minimal app: `textarea autocorrect="off" spellcheck="false" testId="t"`. Type with a real keyboard (an
   attended session; the agent's `type` may bypass substitution) `"0.1.0"`, `'a'`, `a -- b`, `-->`, `...`,
   and a URL. Expected (browser): bytes as typed. Actual (per the clone's note): curly quotes and an en/em
   dash. Read the value with the agent `state`.
2. Clone: a textarea without the hook (the theme JSON box) with the same input; compare with the Files editor.
3. Reference: the same text in the desktop oracle (`target/t3-ui-parity/electron-oracle.mjs`).

## Acceptance for the fix
- AppKit test binary: with `autocorrect="off"`, `insertText` of each probe string and a simulated typing
  sequence leaves the value byte-equal; with `autocorrect="on"` AppKit's default behavior remains.
- Real keyboard in an attended session (`T3_LOCAL_HOME` and `T3_LOCAL_PORT` set for the lane build): the minimal
  app and the clone's theme JSON, commit message, scheduled prompt and Files editor all keep their bytes.
- `input` fields: a case for single-line fields, or a recorded result saying they are unaffected.

## App adoption after resolution
- Remove the `t3-plain-text` hook from `app.json`, `r4-surfaces-files.contract:206` and
  `settings-b-actions.contract:121`, the routine in `T3PanelsNative.swift`, and the composer attach switch-off;
  put `autocorrect="off"` on all textareas.
- Re-run the typing check of AGENT-HANDOFF.md (round-11 item 3) on every text box and add the new ones.
  `issue-close` verifies bytes with the AppKit case and one attended pass.

## Status and next action

Filed as #111; fixed by main #160. Adopted by task `20261007-adopt-main-fixes-input`: the `t3-plain-text` switch-off is removed (Files editor, diff comment, script command and composer), the Files editor's hook is renamed `t3-file-editor` (it still takes the focus its press began), and `autocorrect="off"` is on every editable textarea (commit message, theme JSON, scheduled prompt and scoped setting added); `text-entry.test.ts` checks it. Link and data detection, text completion and smart insert/delete are no longer forced off: they are AppKit's defaults again, as #160 leaves them. The attended real-keyboard pass was not run.
