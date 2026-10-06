---
name: 20261005-composer-fidelity
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Composer, queue editing and small chat details match the reference

## Outcome

The composer behaves like the reference desktop composer in the places where the 2026-10-05
audit found differences: the Claude "ultrathink" flow and frame, the implicit Fast-mode default,
the "More composer controls" overflow menu, editing a queued message together with its
attachments, the subagent hover card with its account, and three small details (approval
warning tooltip, editor brand icons, the "No active thread" page).

## Scope and exclusions

| Part | Reference behavior (all states) | Clone today (mc-orch tree, 2026-10-05) |
| --- | --- | --- |
| G9 ultrathink | A prompt containing "ultrathink" on a model whose primary select descriptor has `promptInjectedValues` draws a 2 px static spectrum ring around the composer (`ultrathink-frame`) and filters the model icon (`ultrathink-chroma`). Choosing the injected effort in the traits menu rewrites the draft to start with `Ultrathink:\n` and stores no option; a draft that already contains "ultrathink" disables the row with "Your prompt contains "ultrathink" in the text. Remove it to change this option." (the row stays visible, its radios do not change the value) Send adds the prefix once; slash commands keep none. | `applyOptionChoice` stores any listed option (`composer-controls-commands.ts:20-27`); no frame; no prefix; `recallablePrompt` already strips the prefix (`composer-editor-menu.ts:330`) |
| G9 Fast | If the model has a boolean `fastMode` descriptor and the user chose nothing, dispatch `{id:"fastMode",value:false}`; an explicit choice wins. | `modelSelection` sends stored options only (`protocol.ts:119-122`) |
| G11 overflow | When resting controls do not fit: first icon-only labels, then trailing blocks (Mode, then Traits) move into a `…` button (`aria-label` "More composer controls"); then the model picker shrinks; below its minimum the cluster hides. Menu: traits content, "Mode" radio (Chat, Plan) when Plan UI is on and Mode is hidden, "Access" radio. Promotion back needs 1 px of slack. Open menu closes when its trigger hides. | `footerLayout` has icon steps only (`composer-controls-view.ts:34-60`) |
| G12a queue edit | Editing a queued message loads its existing attachments into the composer; remove or add; limit 100 ("A message can have at most 100 attachments."); save sends `queued-run.edit{text, attachments, context}`; empty text with attachments sends the attachment-only prompt; failures: "Could not save the edited queued message." Rows show 16 px thumbnails and a clock "Saving queued message". Lost run: kept edit toast "Your unsaved edit was kept in the composer." or discarded. | text only, count only (`composer-controls-queue.ts:11,31,90-114`) |
| A15 subagent card | Hover card: provider glyph (grey), "<model> · <account>" only when several accounts share the provider, status, elapsed, Branch/Folder rows, preview. | none; subagent bar already has the badge (`composer-controls.contract:1007-1008`, `presentation.ts providerBadge`) |
| G15 approval | An option with `warning` shows the triangle and a tooltip with the warning text above it; `aria-description` carries it. | icon only (`requests.contract:143-153`) |
| G15 editors | Brand icons for 20 editors: Cursor, Trae, Kiro, VS Code, Insiders, VSCodium, Zed, Antigravity, 12 JetBrains IDEs, plus Finder. | 4 icons and a fallback (`shell-icons.contract:34-60`) |
| G15 no thread | Header "No active thread"; title "Pick a thread to continue"; text "Select an existing thread or create a new one to get started." | absent |

Excluded: queue row steer and reorder (done), terminal chips, any new send path, usage
price/reset work (`20261005-usage-reset-and-feedback`), inline `$skill` chips
(`20261005-upstream-timeline-and-markdown`). The multi-environment usage refresh (G15; gap CN4:
`server.refreshUsageRates`, then a refetch per selected connected environment, merged totals, abort for
an environment that disconnects, limits refresh deduplicated for 5 minutes) moved to its single owner,
`20261005-usage-pooled-view`; its reference code is `packages/client-runtime/src/state/usage.ts:85-125`
and its tests are `usage.test.ts` and `UsagePage.refresh.test.tsx`.

## Context and guidance

Parent specification: [spec](../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/components/chat/composerProviderState.tsx:50-160`, `.../TraitsPicker.tsx:340-360`,
`apps/web/src/components/ChatView.tsx:782-792,8920-8935`, `apps/web/src/index.css:2075-2118`,
`packages/shared/src/model.ts:302,480-540`; `apps/web/src/components/composerFooterLayout.ts:184-224`,
`.../chat/CompactComposerControlsMenu.tsx`, `ChatComposer.tsx:5331-5420,5500-5540`;
`.../chat/queuedMessageEdit.ts`, `ChatView.tsx:4518-4690,8514-8640`, `QueuedRunsControl.tsx:134-160,360-410`,
`packages/client-runtime/src/operations/commands.ts:281,1000-1022`; `.../chat/SubagentTooltipContent.tsx:28-90`
(commit `daa1d0ed94`); `.../chat/ComposerPendingApprovalActions.tsx:54-97`; `.../chat/OpenInPicker.tsx:83-183`,
`components/JetBrainsIcons.tsx`; `.../NoActiveThreadState.tsx`.
Logic reuse: port `getComposerProviderState`, `withImplicitFastModeDefault`,
`getComposerPromptInjectionState`, `isClaudeUltrathinkPrompt`, `applyClaudePromptEffortPrefix`,
`resolvePromptInjectedEffort`, `resolveRestingComposerControlsLayout`, `prepareQueuedEditAttachments`,
`recoverQueuedMessageEdit` as new files with a header naming
source, license (`LICENSE-T3`) and each change (no React, `now` argument for time).
Library revision: `20261005-platforms-v3`. Selected topics: components (props, child state),
layout-and-interaction (bounded layout; flex children need `min-width=0`; assert geometry),
accessibility (icon button `aria-label`, tooltips are not the only label, reduced motion),
design (all states), motion (menu open/close, short and interruptible), testing-and-debugging.
Unknown in the library: the native composer text view, attachments and menus (`T3Composer*.swift`);
the clone's runtime evidence is the basis. Do not grow `app.contract` (X9).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23); "oracle"
below is `target/t3-ui-parity/electron-oracle.mjs`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Merged | pending |
| scheduling preference | 20261005-main-fix-adoption | pending | Merged first (tooltip and popover Contract) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| decision | U2: fixture for ultrathink and Cursor Fast mode | none | The lane fixture provider may lack these descriptors. Choose: unit tests on HEAD shapes plus an attended session with the user's own account, or approve the optional catalog-injecting proxy listed in U2. | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X20](../issues/20261005-x20-rich-text-editing.md) | Rich-text editing (prompt rewrite keeps chips and caret) | `EXACT2-GAPS.md` X20 | nonblocking (workaround: native `NSTextView` composer) | Rewrite through the existing text-view bridge |
| [X22](../issues/20261005-x22-reactive-layout-facts.md) | Reactive layout facts (host width for G11) | X22 | nonblocking (workaround: `t3-frame` hook; `r5-composer-measure.ts`) | Reuse the measured widths |
| [X11](../issues/20261005-x11-shadow-blur-parity.md) | Faint shadow under the tooltip and menu | X11 | nonblocking (declared visible difference) | Declare in `EXACT2-GAPS.md` |
| [X12](../issues/20261005-x12-textarea-field-sizing.md), [X16](../issues/20261005-x16-smart-substitutions-off.md) | Composer height and exact bytes after a prefix rewrite | X12 (the textarea sizes from its plain string, not from the drawn chips; no workaround), X16 (`t3-plain-text` hook in the composer) | nonblocking for this ticket; parity waits for both issues | Re-check after the prefix rewrite; record the height difference in X12 |
| [X17](../issues/20261005-x17-popover-position-try.md) | Menu flips near edges | X17 | nonblocking (workaround: fixed placement; differs near edges) | Declare |
| [X30](../issues/20261005-x30-ts-announce-readback-picker.md) | Attachment bytes in queue edit | X30 | nonblocking (workaround: native modules, `T3ComposerAttach.swift`) | Reuse `uploadAttachment` (`T3Transport.swift:470`) |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap | Child components for new views |
| [X18](../issues/20261005-x18-svg-path-animation.md) | Morphing icons (the Send / Stop icon swap) | X18 | nonblocking (workaround: cross-fade; a visible difference until X18 is adopted) | add the morph when X18 is adopted |

## Implementation notes

- Prompt rewrite and frame state belong with the composer data source; the frame is a Contract
  border ring (two stacked boxes, `border-radius` inherited) with the six-stop spectrum; check
  the ring in light and dark, and with `prefers-reduced-transparency` (no change expected).
- Overflow menu: feed `resolveRestingComposerControlsLayout` with the measured block widths of
  `footerLayout`; keep the `previous` state per client for the 1 px slack.
- Queue edit: extend `beginQueuedEdit` with the run's attachments; reuse
  `composer-controls-attach.ts` (limit 100) and `uploadAttachment`; send `attachments` and
  `context` in `saveQueuedEdit` (`queued-run.edit` accepts both, `orchestrationV2.ts:2810-2822`).
- Editor icons: vendor the SVG paths from `Icons.tsx` and `JetBrainsIcons.tsx` with a header
  (source, `LICENSE-T3`); brand-mark terms are an open question for the user at `prepare`.

## Acceptance and reproduction

Every row, attended or not, runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773; see
`20261005-embedded-server-runtime`).

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ultrathink | Claude-like descriptor (see decision) | Pick the effort; type "ultrathink"; send | Draft starts `Ultrathink:\n`; ring and chroma show; row disabled with the wording above; sent text has one prefix; no option stored | macOS 1280×840 and 840×620, light and dark | pixel pair vs oracle; trace `message.dispatch` text |
| Ultrathink logic | — | `bun test` ports of `composerProviderState.test.tsx` ("adds ultrathink class names when the prompt triggers a promptInjectedValues descriptor" and the following cases), `model.test.ts` ultrathink cases, `TraitsPicker.test.ts:195` | Pass | host machine | log |
| Fast default | Model with `fastMode` descriptor | Send without choosing | Options contain `fastMode:false`; explicit Fast keeps `true` | macOS | trace; ports of the three `fastMode` cases and `withImplicitFastModeDefault` |
| Overflow menu | Draft, Claude-like model with traits; 840×620 is the window minimum, so the composer is narrowed inside the window | At 1280×840 open the right panel (the chat column narrows) and close it again; at 840×620 do the same with the sidebar open and closed; read the composer's host width with `layout` at each step; open `…` | Steps in the order above at the same thresholds as the oracle's, both when narrowing and when widening; menu content per hidden blocks; the open menu closes when its trigger hides | macOS 1280×840 and 840×620, light and dark | `layout` + screenshots; pixel pair; ports of `composerFooterLayout.test.ts:224-300` and `restingComposerControlsMeasurement.test.ts` |
| Overflow menu keyboard and motion | Same | Tab to `…`; Enter or Space opens; arrows move; Escape closes; set prefers-reduced-motion | Visible focus; `aria-label` "More composer controls"; Escape closes and focus returns to `…`; the menu fades with no movement under reduced motion | macOS | `tree --ax`; film (`over 300 every 30`) in both modes; `(attended session)` for real keys |
| Queue edit with attachments | Running thread; queue a message with an image and a file | Edit it; remove the image; add another; save | Chips load; limit message at 101; trace shows `queued-run.edit` with the full attachment list; row thumbnails update | macOS | trace; server state; ports of `queuedMessageEdit.test.ts` (6) and the `commands.test.ts` editQueuedRun case; agent checks that mirror `QueuedRunsControl.test.tsx` "renders an attachment thumbnail on the queued row" and "keeps the original queued message visible while editing" |
| Queue edit failure and loss | Same | Refuse the write; start the run from another client | Error text above; kept/discarded toast wording | macOS | screenshots |
| Subagent card | Unit shape (the fixture cannot produce subagents) | `bun test` | Account suffix only with several accounts | host machine | log `(unit only)` |
| Approval tooltip | Fixture approval with a warning option | Hover (attended session) and focus by keyboard; press Escape (read the oracle's result first) | Tooltip text equals `warning`; `aria-description` set | macOS | session notes; `tree --ax` |
| Editor icons | `availableEditors` with all 20 ids | Open the Open-in menu with the mouse and with the keyboard; press Escape | Each row has its brand mark; Escape closes the menu and focus returns to the trigger | macOS, light and dark | pixel pair; `tree --ax` |
| No active thread | Route to a missing thread | Open it | Header and text as above | macOS both sizes | pixel pair |
| Clone checks | `git add -A` | Usual list, `bun scripts/caps.mjs`, five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States covered: loading ("Saving queued message"), empty (no queued rows),
error (save refused, fetch failed), disabled (effort row, `…` items), hover and keyboard focus on
the tooltip and `…`, reduced motion (menu fades with no movement).
Task-owned source paths: new `composer-provider-state.ts`, `composer-resting-layout.ts`,
`queued-edit-attachments.ts` (+ tests), `composer-controls*.ts|contract`,
`requests.contract`, `shell-icons.contract`, the no-thread view.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build, one lane backend.

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

After dependencies merge: `prepare` (settle the fixture decision and the brand-icon terms), then `implement`.
