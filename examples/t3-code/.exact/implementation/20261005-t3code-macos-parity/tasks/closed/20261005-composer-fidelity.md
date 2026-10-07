---
name: 20261005-composer-fidelity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-composer-fidelity
pr_url: https://github.com/ccheever/exact2/pull/144
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

Parent specification: [spec](../../spec.md). Reference (T3 Code `1e2ecbd975`):
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
| [X20](../../issues/20261005-x20-rich-text-editing.md) | Rich-text editing (prompt rewrite keeps chips and caret) | `EXACT2-GAPS.md` X20 | nonblocking (workaround: native `NSTextView` composer) | Rewrite through the existing text-view bridge |
| [X22](../../issues/20261005-x22-reactive-layout-facts.md) | Reactive layout facts (host width for G11) | X22 | nonblocking (workaround: `t3-frame` hook; `r5-composer-measure.ts`) | Reuse the measured widths |
| [X11](../../issues/20261005-x11-shadow-blur-parity.md) | Faint shadow under the tooltip and menu | X11 | nonblocking (declared visible difference) | Declare in `EXACT2-GAPS.md` |
| [X12](../../issues/20261005-x12-textarea-field-sizing.md), [X16](../../issues/closed/20261005-x16-smart-substitutions-off.md) | Composer height and exact bytes after a prefix rewrite | X12 (the textarea sizes from its plain string, not from the drawn chips; no workaround), X16 (`t3-plain-text` hook in the composer) | nonblocking for this ticket; parity waits for both issues | Re-check after the prefix rewrite; record the height difference in X12 2026-10-07: X16 adopted (#111, main #160): the composer keeps its bytes through `autocorrect="off"`; the hook's switch-off is gone (adopt-main-fixes-input). |
| [X17](../../issues/20261005-x17-popover-position-try.md) | Menu flips near edges | X17 | nonblocking (workaround: fixed placement; differs near edges) | Declare |
| [X30](../../issues/20261005-x30-ts-announce-readback-picker.md) | Attachment bytes in queue edit | X30 | nonblocking (workaround: native modules, `T3ComposerAttach.swift`) | Reuse `uploadAttachment` (`T3Transport.swift:470`) |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap | Child components for new views |
| [X18](../../issues/20261005-x18-svg-path-animation.md) | Morphing icons (the Send / Stop icon swap) | X18 | nonblocking (workaround: cross-fade; a visible difference until X18 is adopted) | add the morph when X18 is adopted |

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

Implemented on `feat(example)/t3-code-composer-fidelity` (base `d78ac86ff`), 2026-10-06. Verification: unverified.

- G9 ultrathink (`composer-provider-state.ts`, `composer-ultrathink.ts`): ports of getComposerProviderState,
  withImplicitFastModeDefault, applyClaudePromptEffortPrefix, resolvePromptInjectedEffort and the TraitsPicker
  flow. Choosing an injected effort rewrites the draft to start with `Ultrathink:\n` and stores no option;
  "ultrathink" in the body shows the reference note and disables the primary effort rows; another effort strips the
  prefix and is stored; the trigger reads "Ultrathink"; sends prefix once (slash commands none). A client-side
  rewrite reaches the window through `requestKey` (`#rewrite:<n>`), so `app.contract` gains no action. Ring: an SVG
  gradient stroke over the card's edge (`composer-ultrathink-frame`); chroma: `filter=saturate(1.2)` on the model mark.
- G9 Fast: every send path (thread, launch, multi-model, compact, implement) dispatches
  `modelOptionsForDispatch` (explicit choices plus `fastMode:false` when the model offers Fast and nothing was chosen);
  the traits trigger shows Normal in that case.
- G11 (`composer-resting-layout.ts`, `composer-overflow.ts`): resolveRestingComposerControlsLayout fed from the
  footer's block geometry: labels, then mode, then traits into an icon-only "More composer controls" trigger, then
  the picker shrinks, below its minimum the cluster hides; promotion needs 1pt of slack (previous step per client).
  The menu holds traits content, Mode (Chat/Plan, when Plan UI is on) and Access; it goes when its trigger hides.
- G12a (`queued-edit-attachments.ts`): edit loads the message's attachments as removable chips (images signed
  through the existing attachment URL resource, extended to queued messages), stashes the thread's text and SnapShot
  images, saves `queued-run.edit` with the full list and merged context, 100-attachment limit, attachment-only
  prompt, "Could not save the edited queued message.", lost-run kept/discarded toasts with the reference wording.
  Rows show 16pt thumbnails; a queued send still in flight shows the "Saving queued message" clock row.
- A15 (`subagent-card.ts`): the card's content with the account rule. Unit only; no view (fixture cannot produce subagents).
- G15: approval warnings as tooltip + `aria-description` (row and More menu); 17 more editor marks in
  `editor-icons.contract` (20 editors + Finder); "No active thread" landing for a route to a missing thread.

Shared-file edits (own commits `4d6f79e4f`, `a588ba5be`, `ad0082876`): `client.ts` lines 22 (import), 880
(queued-edit uploader), 885 (promptForSend), 912 and 925 (dispatchSelection), 1017 (missing thread opens the empty
state), 1376 (ultrathinkChoice); `app.contract` line 75 (`pageCover` includes "no-thread").

Not done / differences: the trigger hides at once with no menu fade (no motion added; reduced-motion row not
filmed); an open "More" menu that loses its trigger stays logically open (renders nothing) until the next menu
action; the remote removal of the open thread still lands on a draft (only a route to a missing thread shows the
empty state); the subagent card has no view; the ring has no saturate/brightness filter at runtime (baked into the
stops) and its gradient spans the card's box, not a 220% box; menu flips (X17), faint shadows (X11) and textarea
height after a rewrite (X12) as declared in those issues; brand-mark terms are an open question for the user.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | `ad0082876` on `d78ac86ff` | `bun test examples/t3-code` 1274 pass / 0 fail (base 1200; new: composer-provider-state 24, composer-ultrathink 5, composer-resting-layout 16, composer-overflow 5, queued-edit-attachments 14, subagent-card 8, no-active-thread 2); strict tsc clean; `contract build` 2171 slots, 42 resources, 51069 nodes; `cargo test -p t3-code-macos --lib` 10/0; no Swift changed (no AppKit binary touched); macOS bundle built (`build.mjs t3-code-macos --bundle`); five checks pass (build, test 2927/0, clippy, fmt, caps, boot) | Live drive (one script session, `target/cf/drive.mjs`, isolated server on 16100 with a version-only `claude` shim and editor shims): paired through the wizard; landing `providerDriver: "claudeAgent"`, `traitsLabel: "Medium · 1M"`. The bootstrapped project was not present after "Do not import projects", so the composer stayed inert under the "What should we work on?" landing; no task flow ran. Retry used (attempt 1 failed on a wizard target before mount). | Every live row: unverified (one-drive rule spent). Real-input rows (hover tooltip, keyboard menu, Escape): unverified (attended). Queue edit and approvals need a running provider turn: unit tests only. |

## Next action

Verify: one live drive with a lane project imported in the wizard (import the bootstrapped project instead of "Do not import projects"), then the ultrathink, overflow and editor-mark screenshots; settle the brand-mark terms with the user.
