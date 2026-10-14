---
name: 20261014-contract-keys-and-menu-bar
plan: 20261005-t3code-macos-parity
implementation: complete
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-contract-keys-and-menu-bar
pr_url: https://github.com/ccheever/exact2/pull/587
verified_commit: null
inspected_base: a218ee78fec42b1a4590b199297f30667876de67
framework_revision: 178ae7237edaf7b3b07aa419f0d32b98b7e5ee80
reference_revision: 1e2ecbd9758830669684b494d4398f626b0576e0
serial_position: 8/8
---

# Contract keys and menu bar

## Outcome

Retire the application-owned key readers and menu construction that the adopted Exact APIs can now express, preserving T3 Code's visible behavior, input ownership, menu actions, selection and focus. This is the final serial follow-up, after seven application tasks have merged. Prepared on 2026-10-15 from feature head `294db9696`.

The coordinator's 2026-10-14 fix-wave authority assigns these rows:

| Row | Required outcome |
| --- | --- |
| KW-11a / X25 | Move `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, and `T3Sidebar` key-reading responsibilities to scoped Contract capture handlers, one reader at a time. Reconstruct held modifiers from key events, releases and page focus. Preserve pointer modifiers through the press event. |
| KW-11b / X26 | Declare the reference's expressible application-menu commands with Contract buttons, `-exact-menu` and `aria-keyshortcuts`; remove supported template items through the root menu policy. Retire native menu construction only after every affected item has an equivalent working route. |
| X15 | Remove Latin chord reissue where Contract-declared shortcuts now match the physical key. Preserve terminal `WKWebView` handling and any specifically demonstrated native-menu/reader requirement remaining after KW-11a/b. |
| X292 | Move every expressible module context menu to `contextmenu`/`contextPopover` so Exact schedules it. Keep `T3MenuTurn` for the enumerated remaining native menus. |

The original triage proposed deleting whole `T3Menus.swift` and `R8KeysMenus.swift` files and leaving reissue only for the terminal. Inspection found responsibilities outside the adopted API's coverage. The coordinator explicitly requires retaining unexpressible native behavior and documenting gaps. Whole-file deletion is therefore conditional, not an acceptance shortcut. An item is finished only when it is migrated and verified or has a precise retained-native reason accepted in the task record. Do not create inert Contract menu items to make the source inventory appear complete.

## Scope and exclusions

Application-owned changes only under `examples/t3-code/`: relevant Contract components, their TypeScript actions/state, Swift module readers and menu presenters, existing regression suites, and task-owned records. Paths below are relative to that directory unless marked **repository root** or **reference**.

Start with the four named readers. Migrate their callers before deleting a bridge operation, native view registration, hatch or status field. Change other key-routing code only where required to preserve those readers' existing behavior. Inspect the base after every predecessor merge; some sites may already have moved.

Preserve:

- The composer's ordinary Exact `textarea`, source text, selection/undo history, chip accessibility projection, citation comments, queued editing and IME behavior. Do not replace the composer editor to simplify capture.
- Terminal input-context-first composition, terminal selection ownership and clipboard semantics, authentication-terminal behavior, and the vendored Ghostty code. Keep native terminal keys where the host cannot route them.
- Native application lifecycle, quit hold/double press, held-Cmd-W suppression, window zoom and native file pickers unless an actual supported replacement proves equivalent.
- Current context-menu item order, separators, checked/disabled states, destructive symbols, submenu behavior, pointer/keyboard anchors, focus return and stale-request cancellation.
- Native Services and text editing menus. App context menus must not swallow selected-text Services or expose only a generic menu.

No framework edits, new framework API, dependency upgrades, new harness/CLI, broad refactor, live provider messages, permission changes, unapproved remote effects, or production data changes. Do not reopen the permanently declared View → Toggle Developer Tools difference or implement the T3 update feed in this task. Do not delete a test binary merely because its old helper disappears: first port its meaningful behavioral assertions to the replacement's existing suite.

Keep the separate main-publication [PR #498](https://github.com/ccheever/exact2/pull/498) and Charlie's outstanding decisions unchanged. This feature-branch task does not authorize merging/rebasing that publication branch, adding async-lane checks, changing the workspace/check policy, or resolving its generated-source, hand-picked-icon, formatting or vendored-cap decisions. GitHub readback on 2026-10-14 shows #498 OPEN against `main` (`daehyeon/t3-code-main`), not merged. It is separate from the seven serial application prerequisites.

## Context and guidance

Use **repository root** `rules/RULES.md`, `rules/DEFERRED.md`, `AGENTS.md`, and `docs/testing.md`. The parent authority is `.exact/implementation/20261005-t3code-macos-parity/spec.md` and `plan.md`; the current merge/order receipt is `reviews/20261014-first-wave-resume.md`. Read the current Exact vocabulary through the supported compiler lookup when implementing; the app now uses `if`/`for`, collection methods, `.length` and `T[]` syntax from adopted main.

The inspected base is `a218ee78fec42b1a4590b199297f30667876de67`. First-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a` contains all eleven first-wave PRs. Main-adoption [PR #549](https://github.com/ccheever/exact2/pull/549) merged into `feat(example)/t3-code` at `3710af842bc54521c298124156989950b6f2e68c`; the inspected base includes it. Main is pinned to `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`. These are inspection identities, not the eventual prepared base.

Reference T3 Code is `https://github.com/pingdotgg/t3code`, commit `1e2ecbd9758830669684b494d4398f626b0576e0` (MIT; preserve `LICENSE-T3`). Read files at that revision, not an arbitrary checkout head:

- **Reference** `apps/desktop/src/window/DesktopApplicationMenu.ts` and `.test.ts`: all menu roles/items, update handling, Paste as Text's menu-click versus accelerator distinction, main-window zoom.
- **Reference** `apps/desktop/src/window/QuitHold.ts`, `DesktopWindow.ts`: 1,200 ms hold proven by auto-repeat, 500 ms double press, 600 ms release grace, repeat cadence, held close and conceal/quit behavior.
- **Reference** `apps/web/src/keybindings.ts`, `.test.ts`, `packages/shared/src/keybindings.ts`, `packages/contracts/src/keybindings.ts`: defaults, conditions, physical-key fallback, recorder normalization and the exclusion of editable/terminal focus for commands such as thread Undo.
- **Reference** `apps/web/src/shortcutModifierState.ts`, `components/RightPanelTabs.tsx`, `components/settings/KeybindingsSettings.tsx`, `components/projectScriptEditor.tsx`, `components/chat/ComposerPrimaryActions.tsx`, and composer submission logic: modifier state, launcher's capture listener, key capture and send intent.
- **Reference** `apps/desktop/src/electron/ElectronMenu.ts`, `apps/web/src/components/{Sidebar,LegacySidebar,FileBrowserPanel,ThreadTerminalDrawer,ChatMarkdown,SettingsPanels}.tsx` and file/external-link context-menu helpers: menu payloads, anchors and resulting effects.

Prior records to preserve, all under the parent plan's `tasks/`:

- `closed/20261005-thread-commands-and-keys.md`, `closed/20261009-shell-sidebar-palette-keys.md`, `closed/20261008-fix-keyboard-focus.md`, `closed/20261010-shell-context-menu.md`, `closed/20261010-realinput-1010c-native.md`: command conditions, queued edit, keyboard focus, menu opening and native-menu ownership.
- `20261014-terminal-input-and-ime.md` ([#533](https://github.com/ccheever/exact2/pull/533)): offer composing Control chords to the input context first; only declined chords commit before terminal handling. Real Hanja/cadence and middle-press qualification remains open.
- `20261014-composer-chips-accessibility.md` ([#534](https://github.com/ccheever/exact2/pull/534)): the composer is Exact's `TextArea` with enhanced accessibility, not a separate module text view. Preserve mapped source/AX ranges and chip actions. AX checks do not establish VoiceOver navigation.
- `20261014-cite-selection-anchor.md` ([#540](https://github.com/ccheever/exact2/pull/540)): plain Tab capture reaches Cite, Escape dismisses without clearing the selection, pending/stale selections cancel correctly, and comments anchor to the selected line.
- `20261014-native-helpers-and-text-menu.md` ([#544](https://github.com/ccheever/exact2/pull/544)): selected-text Services consults the actual requestor; nested menu tracking must keep native work running.

Public APIs checked in **repository root** `docs/contract-grammar.md` (Events/Keys), `llp/1021-menus.rfc.md` §5.3, `kernel/tables/schema.json`, and the production implementations:

| API | Actual coverage and consequence |
| --- | --- |
| `keycapture=action` | Capture keydown, outermost ancestor to target, before `key` target/bubble. Same optional `KeyboardEvent`. It adds no Tab stop. `preventDefault()` cancels defaults; `stopPropagation()` alone does not cancel Enter submission. Declared shortcuts run before this route, so recorder/launcher migration must resolve applicable competing shortcuts explicitly. |
| `key`, `keyup`, `KeyboardEvent` | `key`, `code`, `repeat`, `shiftKey`, `ctrlKey`, `altKey`, `metaKey`. There is no independent held-modifier resource and no public `isComposing` field. `keyup` bubbles; there is no `keyupcapture` API. Release modifiers and page focus loss must clear owned state. |
| `resource page = exactPage() as shape Page` | `hasFocus` permits a task to forget held state when focus leaves the window. Do not use it as evidence that a native child supplies all key events. |
| `press` with optional `MouseEvent` | All four modifier flags from the actual click; keyboard/assistive press has false flags. `pointerdown/up/contextmenu` have `PointerEvent` modifiers too. The native comment saying a press has no event is stale. Preserve keyboard intent separately so zero-valued programmatic Send does not overwrite its recorded Return intent. |
| Native key target | `host/apple/Sources/ExactKit/KeyEvents.swift:keyTarget` accepts the node, its owned field/textarea, supported controls and inline key views. The current composer is that owned textarea (`T3Composer.swift`), so it is eligible. Arbitrary module views and `WKWebView` keep their own keys. A replacement recorder must be an actual Contract focus target. A root capture is not automatically a window-global native monitor. |
| `-exact-menu="app\|file\|edit\|view\|go\|window\|help"` on a button | Menu presentation of the same working press action, with or without a chord. Label follows accessible label/text. Hidden/removed buttons leave the menu; disabled/inert disable it. No menu-only invisible-button contract, native responder-role selector, arbitrary separator/order policy, or window zoom/quit API is supplied by this attribute. Check layout and AX when adding declarations. |
| Root `-exact-menu-remove` | Literal IDs only: `close-all autofill show-tab-bar show-all-tabs enter-full-screen minimize zoom bring-all-to-front show-previous-tab show-next-tab move-tab-to-new-window merge-all-windows`. It does not remove app-declared commands, entire Go/Develop menus, Help's book item, Paste and Match Style, Find, Spelling and Grammar, Substitutions or Transformations. Develop remains in development builds by the framework contract. |
| `contextmenu` + `contextPopover` | A source event may prepare the menu, then the host opens its declared menu popover. `role="menu"`, item roles, `aria-checked`, `disabled`, `hr`, and nested menu `popovertarget` cover ordinary rows/submenus. The ContextMenu key reaches the focused node/ancestor; Shift-F10 and source-specific bottom-left anchors are not implied by that support. |
| Host menu scheduling | `host/apple/Sources/ExactKit/Mac/MenusMac.swift:context` uses a common-mode run-loop turn. It schedules menus it opens; it cannot schedule the application's remaining `NSMenu.popUp` calls. |

## Dependencies

The following are **merged task PR** prerequisites in this exact order. They must merge into `feat(example)/t3-code` one at a time, with the next task based on the refreshed integration branch. No dependent stack or task branch is authorized by this preparation draft.

1. `dialog-focus-return-and-traps`
2. `legacy-sidebar-parity`
3. `connections-network-settings`
4. `menu-keys-popover-commands`
5. `browser-panel-parity`
6. `css-workaround-retirements`
7. `timeline-and-window-hooks`

Satisfied on 2026-10-15. All seven merged into `feat(example)/t3-code` by squash, and each is an ancestor of prepared base `294db9696`: #553 `6a7a3cad0`, #557 `4520b0124`, #566 `fbed7fadd`, #568 `b229a2658`, #571 `70eec59fd`, #577 `6ef85eb1c`, #584 `47f3ed1bd`. The inspected base predates them; reconcile their source changes (notably #568's public popover commands and row focus, and #584's removal of R9 transcript restoration and `T3FullScreen.swift`) before selecting deletion boundaries here.

**Recorded decisions:** the parent fix-wave task assigns KW-11a/KW-11b/X15/X292, says run last/one reader at a time/nothing visible changes, and requires retaining unexpressible native menus plus terminal chord handling. The original scope text is reproduced in Outcome so the task does not depend on a temporary JSON path or chat history. First-wave and main-adoption merge prerequisites are satisfied; they do not satisfy the seven follow-ups.

Issue assessment, inspected 2026-10-14 against local adopted production source and GitHub issue readback:

| Issue/reference | Capability and target | Evidence/revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [#140 / X25](https://github.com/ccheever/exact2/issues/140) | Capture/release/modifiers on macOS | GitHub CLOSED; capture `3f283d582` and earlier `keyup/code/repeat` are adopted; actual `KeyEvents.swift` and grammar checked at main pin. Plan-local `issues/closed/20261005-x25-keyboard-keyup-code-capture.md` retains historical wording. | Nonblocking for ordinary Exact targets; blocking for an assumed window-global route over arbitrary native children. | Probe each actual responder and competing local monitor. Clear held state on page blur; retain a demonstrated uncovered path. |
| [#141 / X26](https://github.com/ccheever/exact2/issues/141) | Menu-bar declarations/removals | GitHub CLOSED; `6ef2c3f9a` adopted. Fixed menu IDs and limitations above, verified in grammar/schema/`ShortcutsMac.swift`. | Nonblocking for expressible commands; blocks unconditional deletion of all native bar code. | Make per-item disposition explicit; keep narrow role/window/development helpers until a supported replacement exists. Do not present the issue's closure as full native-menu parity. |
| [#110 / X15](https://github.com/ccheever/exact2/issues/110), [PR #168](https://github.com/ccheever/exact2/pull/168) | Non-Latin shortcut matching | Issue CLOSED; PR MERGED. `ShortcutsMac.swift` uses physical fallback for declared shortcuts/its command targets. `R10Connect.swift` still covers native standard/app-added items and readers. | Nonblocking for Contract targets; terminal-only reissue cannot be assumed sufficient. | After KW-11a/b, run the chord matrix with reissue bypassed per route. Preserve every still-required route by name. |
| [#235](https://github.com/ccheever/exact2/issues/235), main #314 | Keyboard context menu | GitHub CLOSED; ContextMenu key `d236c36d5` adopted. Draft/legacy/media keyboard anchors and Shift-F10 remain app behavior. | Nonblocking with existing handlers preserved. | Verify ContextMenu and Shift-F10 separately, including exact anchor and focus return; do not infer feature coverage from CLOSED. |
| [#292](https://github.com/ccheever/exact2/issues/292) / X292 | Work during native menu tracking | GitHub CLOSED; adopted host scheduling plus qualification `80a799d7e`. Four module scheduling call sites remain, listed below. | Nonblocking if each native call keeps `T3MenuTurn`; blocking if helper is removed while callers remain. | Host-managed menus use the host path; keep and test native scheduling for the remainder. |
| First-wave #533/#534/#540/#544 acceptance qualifications | IME, chips, Cite, Services | Merged fixes and their task records distinguish native/agent proof from attended input. | Nonblocking to source preparation; blocks a claim that final real-input acceptance is complete. | Preserve the changed behavior; run affected regressions again after migration and leave unavailable attended rows open. |
| API coverage discovered above | Native menu roles, title/order/template removal, native-child key ownership, pointer vs keyboard gesture provenance | Actual adopted source; no new upstream resolution or consumer reproduction claimed. | Blocks whole-file/terminal-only deletion where equivalence is absent. | Record the exact retained symbol/caller and capability missing. If implementation needs a new framework capability, create/update the plan-local issue through the established issue workflow, outside this app-only task. |
| [PR #498](https://github.com/ccheever/exact2/pull/498) / Charlie's publication decisions | Main delivery, check cost and source policy | GitHub OPEN against main; [decision summary](https://github.com/ccheever/exact2/pull/498#issuecomment-6146454048) lists async steps, rustfmt, source icon tables and vendored caps. | Nonblocking to this later feature-branch application task; blocks treating this work as permission to resolve or publish those decisions. | Preserve existing holds; parent reconciles final-base/main-publication scope separately. |

Checked task names in active/closed records, task-reference PR search, and exact task branch among available PR results; no existing implementation PR for `feat(example)/t3-code-contract-keys-and-menu-bar` was found. Recheck remote state during real preparation; this draft does not reserve a branch. No new issue or remote message was posted.

## Implementation notes

### Reader and caller inventory at the inspected base

| Reader | Current ownership and callers | Migration boundary and required preservation |
| --- | --- | --- |
| `modules/apple/T3ComposerIntent.swift` | `T3Module.swift` owns `intent`, merges `status.modifiers` into presentation, destroys it. `T3Module+Composer.swift` exposes `composerSendIntent`. Consumers: `client-ops-composer.ts`, `r3-composer-controls-fanout.ts`, `browser-links.ts` (two routes), `pages-pr-links.ts`; resolution in `composer-editor-intent.ts`. It observes Return/keypad Enter and left presses, excludes key repeats, stores source/modifiers/time, consumes the newest gesture once, and clears held modifiers on app deactivation. | Capture on the actual Exact textarea/root plus direct `MouseEvent` data where a press occurs. Preserve foreground/alternate/background/send-and-new behavior, custom bindings, stale-gesture rejection and read-once semantics. Inspect `T3Composer.handle`/`returnAction`: its native monitor can consume Return and invoke `send.click()` before Contract sees the key. Prove ordering or migrate that caller's required routing; do not assume all local monitors run in a fixed order. Preserve link-click modifiers as well as Send. |
| `modules/apple/T3KeyRecorder.swift` (`T3KeyRecorderView`, `T3KeyRecorder`, `T3KeyChord`) | Factory `t3-key-recorder` in `T3Module.views`; mounted by `settings-keybindings.contract` and `settings-b-actions.contract`. Takes focus on mount; Escape emits cancellation; modifiers alone do nothing; produces `mod/ctrl/alt/shift` plus normalized key; named keys, function keys, shifted punctuation and agent chords have explicit normalization. | Replace the native view with a Contract focus target and scoped capture. Disable/scope competing application `aria-keyshortcuts` while recording, because shortcuts precede capture. Preserve capture cancellation on Escape/blur and restoration of the original binding. Check both Settings and project action editor. Remove the factory/native classes only after both consumers use the new path. |
| `modules/apple/R8KeysLauncher.swift` | `T3Module.launcher`, `composer.launcher`, hatch `t3-launcher`, data `surfaceLauncherKeys`; `shell-panels.contract:SurfaceLauncher` already owns key actions and highlight. `T3Composer` consults `launcher.route` before type-to-focus. Native helper focuses after layout and reposts an eligible letter once when focus was elsewhere. | Replace window-to-launcher rerouting with capture on an ancestor covering eligible Exact focus targets. Preserve mount focus after nonzero layout, available-letter matching in both cases, no repeat, no Cmd/Ctrl/Option chords, no typing-context theft, inert/modal blocking and no leaked/reposted duplicate character. Preserve arrow wrapping/Enter behavior and focus on remount. |
| `modules/apple/T3Sidebar.swift` | `T3Module.sidebar` installs two monitors and publishes `sidebarJumpHints`/`prSpeedMode`. `T3Module+Sidebar.swift` exposes `sidebarModifiers`, `sidebarSpeedMode`, `sidebarNotify`, `sidebarMenu`. Key monitor drives exact-Cmd 200 ms jump hints and exact-Shift speed mode outside editable focus. Pointer monitor holds the click's flags until read, max two seconds. Callers include `sidebar-commands.ts`, `legacy-sidebar-links.ts`, `pages-pr-quick.ts`, `sidebar-presentation.ts`, `legacy-sidebar-commands.ts`, `r11-upstream-drafts.ts`, `r12-sidebar-keys.ts`. | Separate key state, click data, notifications and menu presentation. Direct press modifiers may retire `sidebarModifiers` only after all row/link callers carry them. Keep delayed notification operations still used by provider-pill/snooze refresh. Do not delete the class just because one key monitor moves. Preserve Cmd-click toggle, Shift-click range and flags from the click rather than a later keyboard read. |
| `modules/apple/R10Connect.swift` | Owned/destroyed by `T3Module`; `route`/`latinChord` transform non-ASCII Cmd/Ctrl chords using `physicalKeys`, preserving repeat/timestamp/window and refusing while marked text owns the event. The same file's `wake` implements `r10Wake` for `t3.notify`/`t3.pr`. | Retire reissue only for routes whose replacement proves physical matching. Preserve `wake` independently. Native menu items, queued-edit/SnapShot readers, terminal and browser children must be checked before narrowing to terminal alone. Keep the terminal's IME-first path intact. |

Existing key readers outside the four named migrations include `T3Composer.swift`, `T3ComposerQueueKey.swift`, `R9Input.swift`, `T3Menus.swift`'s quit/close guard, `T3Timeline.swift`, `RightPanelTabsInput.swift`, `T3SnapShot.swift`, terminal and browser key handlers. They are regression surfaces, not blanket deletion targets. Reconcile any changes from preceding tasks first.

### Menu-bar inventory and limits

`T3Module.menus` calls `T3Menus.install/augment` when the composer mounts. `T3Menus` owns `R8KeysMenus`, quit state/overlay, main-window zoom and native pickers. `T3Module+Window.swift:devicePresentation` supplies `confirmQuit`; that file calls `pickFolder`, and `T3Module+Shell.swift` calls `pickProjectFavicon`. Those callers make deleting the whole file unsafe even if its menu rows move.

| Reference menu | Items and current implementation | Disposition to prove |
| --- | --- | --- |
| Application | About; Check for Updates...; Settings... (Cmd+,); Services; Hide/Hide Others/Show All; Quit. `T3Menus.augmentUpdates/orderApplicationMenu`, host standard roles, declared Settings shortcut. | Declare expressible commands with `-exact-menu="app"`; retain actual Services/lifecycle roles. Keep item ordering/separators and no duplicate Settings/Updates. Current no-feed update alert is a working scoped behavior; do not turn it into a no-op or claim a live feed. Menu Quit stays immediate; keyboard quit keeps its settings-dependent hold/double-press behavior. |
| File | Close Window (Cmd+W). `R8KeysMenus` hides other host-filed app commands as usable key equivalents and routes a keystroke to Close Right Panel first. | Preserve menu-click close-window versus keyboard close-panel-first, disabled validation, and repeat suppression. Hidden buttons do not create hidden usable menu equivalents under `-exact-menu`; establish an equivalent route before removing concealment. |
| Edit | Undo/Redo; Cut/Copy/Paste; Paste as Text (Shift-Cmd-V); Delete; Select All; Speech → Start Speaking/Stop Speaking. `R8KeysMenus.undo` routes text Undo to the focused editor, and only keyboard noneditable Undo to thread Undo. `T3Menus` inserts responder-chain Paste as Text; host owns remaining roles. | Preserve native text/textarea history, thread Undo condition, single paste, editable/terminal focus and validation. Reference Paste as Text avoids a second injected paste when activated by a shortcut. Menu placement supplies a button action, not a native `pasteAsPlainText:` role. Keep the narrow role adapter if no supported action preserves it. |
| View | Reload/Force Reload; Toggle Developer Tools (declared difference); Actual Size (Cmd+0); Zoom In (Cmd+= plus hidden Cmd+Plus alternate); Zoom Out (Cmd+-); Full Screen. `R8KeysMenus` captures the development reload action; `T3Menus` changes the main window's content bounds at `1.2^level`, 0.5 steps, 25%–500%. | Keep main-window zoom even with embedded preview focus; do not substitute root-font size or native Window → Zoom. Only supported standard removals move to root policy. Preserve reload's existing availability distinction. Hidden alternate shortcut and menu item ordering require explicit proof. |
| Window / Help / development extras | Host Window roles; Help Check for Updates... and system search. Native code removes Go/Develop, tab entries, hides Help's book item and Edit template extras; Save Trace (Option-Cmd-T) remains a hidden development equivalent. | The public removal list covers tab/window IDs but not whole menus or most Edit/Help extras. Preserve the existing behavior with the smallest retained helper where required. Do not silently expose new menus or lose Save Trace. Record the framework's declared development-Develop policy separately from clone parity. |

### Context-menu and scheduling inventory

`T3MenuTurn` is declared in `modules/apple/T3ContextMenu.swift`. At the inspected base its four scheduling call sites are `T3ContextMenu.perform`, `T3MediaActions.perform` (`mediaMenu`), `T3Module+Sidebar.sidebarOps` (`sidebarMenu` only), and `T3TerminalActions.show`. Build an after table of these exact symbols with each retired/retained caller and reason.

| Presenter / callers | Menu sites and data to preserve |
| --- | --- |
| `T3ContextMenu` through `T3Module+Files.swift`, `context-menu-actions.ts`, `context-menus.ts` | Files/diff/Markdown file menus: Open, platform reveal, Open with → configured editors, copy paths, Copy mention/Add to chat and applicable media preview. Keep environment/workspace identity and failure paths, not just labels. |
| Same presenter: `settings-a-archive.ts`, `composer-controls-branch.ts`, `right-panel-tabs.ts`, `external-link-menu.ts` | Archive Unarchive/Delete with separator/trash; branch Copy branch name; tab Rename/Close/Close others/Close to the right/Close all with disabled states; external/PR links Open in integrated browser/Open in system browser/Copy Link, optional link/unlink. Keyboard tab menus use `anchor="focus"`. Refresh this inventory after earlier serial tasks. |
| `T3Sidebar` through `sidebar-commands.ts`, `legacy-sidebar-commands.ts`, `r11-upstream-drafts.ts` | Remaining native legacy/project/draft/keyboard row menus with nested sections, checked states and explicit center/bottom-left anchors. Thread-row and draft right-click Contract menus already exist in `sidebar-row.contract`; reuse their action ownership instead of adding another independent menu model. |
| `T3MediaActions` through `media-actions.ts` and media components | Copy full/relative path or Copy URL, Open in file viewer, Save image/video, Copy image. Preserve unavailable/loading/disabled conditions, immutable source occurrence, async signed URL resolution, failure messages, late dismissal/reentry handling and bottom-left keyboard anchor. Keep save/copy native operations even if menu presentation moves. |
| `T3TerminalActions` through `T3TerminalView`/terminal page messages | Selection popup: Add to chat and Copy; context menu adds Paste. Preserve `can-add-to-chat`, raw clipboard versus normalized excerpt text, 1-based lines, popup coordinates, request generations and focus return. This originates inside a `WKWebView`; retain native presentation/scheduling unless a supported bridge can preserve that ownership without duplicating the terminal selection model. |

An agent's native-menu result `shown:false`/dismissed or logged labels proves request construction only. It does not prove the AppKit menu's layout, tracking, keyboard selection or scheduling. Contract menus may be painted popovers under the agent and `NSMenu`s under real input; verify both representations at the appropriate layer.

### Bounded sequence

1. After all prerequisites merge, refresh/reconcile this inventory and record the actual prepared base/framework pin. Capture the reference and clone's current reader/menu behavior before editing.
2. Choose one reader, implement its Contract state/action path, migrate every consumer, and run its meaningful existing tests plus one fresh native before/after drive. Verify single dispatch, focus, repeat, input method and selection interactions. Only then remove the obsolete reader/registration/status path and continue to the next. Prefer root-owned modifier state adjacent to its clock/focus resource; avoid threading it through large unrelated property lists that inflate the compiled plan.
3. Migrate menu commands one menu at a time. Map each source item to a working action and test both menu click and accelerator. Apply only admitted template-removal IDs. Keep a per-item retained-native table for gaps.
4. Narrow R10 reissue after the new reader/menu routes are proven, never before. Test with the old route bypassed and actual native key dispatch. Keep remaining native consumers explicitly scoped and preserve the wake operation.
5. Migrate expressible context-menu sites, keeping shared item/action generation and owner identity. Verify pointer, ContextMenu and site-specific Shift-F10 separately. Remove each native presenter branch only when no live callers remain. Keep common-mode scheduling on every remaining native menu.
6. Run combined verification, review the deletion/caller inventory, record open qualifications and request independent review through the parent workflow. Stop a failing fix loop after three rounds and report the reproducible blocker; do not weaken a behavioral assertion into a source-string check.

## Acceptance and reproduction

All cases compare the refreshed base, changed build and pinned reference in the same state. The parent platform is macOS minimum 14; current target macOS 26.6.2 (25G83), Xcode 27.0, 1280×840 and 840×620, light and dark. Record actual runtime/tool versions rather than copying historic suite counts.

| Case | Required observations |
| --- | --- |
| K1 — held modifiers | Cmd alone shows sidebar hints after 200 ms; release, another modifier, a nonmodifier key or window blur clears correctly. Shift alone shows PR quick actions only outside editable focus; Shift+letter does not incorrectly end the held state. Releasing one of two held modifier keys reflects the remaining flag. No stuck hints after focus moves to a native child or another app. |
| K2 — send/click intent | Plain Return/keypad Enter, Shift-Return, Cmd-Return, Option-Cmd-Return, pointer Cmd-click and plain click produce the existing foreground/alternate/background/new-thread outcome under custom/default bindings; held Return sends once. Stale or reused gestures cannot affect another action. Check Send, fanout, browser links and PR links. Use a fixture with no live provider sends. |
| K3 — recorder | Both recorders focus on mount; capture named/function/punctuation and modifier combinations; modifier-only keys do nothing; Escape/blur restores original binding; existing app shortcuts do not fire while recording; no extra typed character. Test US logical characters, physical-code non-Latin fixtures and a differing Latin layout case without conflating fixtures with a real input source. |
| K4 — launcher | Available uppercase/lowercase letters open once from eligible nonediting focus, arrows wrap, Enter opens highlighted row, no repeat/typing-context theft, modal/inert launcher does nothing, remount focuses correctly, no replay reaches another target. |
| K5 — existing input/selection | Text Undo/Redo stays in the focused text editor; noneditable keyboard Undo keeps thread behavior. Composer chip labels/ranges/actions survive; queued-edit Option-Up and Return stay correct. Plain Tab to Cite then Escape preserves the selection; citation-comment Save/Cancel/send and outside-press order stay correct. |
| M1 — complete bar | Inspect every menu/item in the table, title/order/separators/enabled state/chord, both menu click and accelerator, no duplicates. Cmd-W closes panel before window while menu Close Window closes the window; repeats do not close another. Quit modes, delayed mode response, release/quiet-period guard and immediate menu Quit survive. Zoom affects the main window with browser/terminal focus. Paste as Text inserts once. Save Trace remains available in development. |
| M2 — context sites | One before/after drive for every moved site and every nested menu kind: correct items, checked/disabled state, destructive symbol, pointer anchor, keyboard anchor, Escape/selection/focus return and actual effect. No stale selection/action after dismiss, scroll, owner switch or a newer menu request. Preserve selected-text Services and text menu ownership. |
| X15 — route matrix | Every current default/server/custom chord reaches exactly one expected action under US and non-Latin fixtures; differing Latin layout keeps its logical key. Exercise composer, ordinary field, noneditable body, empty focus, modal, recorder, browser and terminal as applicable. Real 2-Set chords remain a separate attended row. Standard menu roles and retained native readers must work with the narrowed reissue. |
| X292 — tracking | While an actual host-managed context menu tracks, a bounded `native.later` cadence continues. Repeat for each remaining native scheduling family; the menu must not hold work until dismissal. Agent painted popovers alone do not establish nested AppKit tracking. Use existing native-test facilities, no new harness. |
| Terminal/IME/AX regression | Preserve consumed versus declined Control composition, Option-Return Hanja ownership, no leaked marked text on later paste, terminal Add to chat/Copy/Paste request ownership and focus. AX structure/ranges pass; real 2-Set Hanja/cadence and VoiceOver chip/menu navigation are attended qualifications. |

Use Bun 1.4.2 and the repository's pinned toolchains. From **repository root**, the existing app checks are:

```sh
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo test -p t3-code-macos --lib --tests --no-fail-fast
```

Compile the actual app Contract with the current `contract` executable and record its command, warnings, byte size and SHA-256. Do not rely on an already-built compiler from another source revision. Build the consumer with `EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle`; launch the resulting source-fresh bundle through the supported macOS agent or approved attended route. A Cargo rlib build alone does not prove the linked app.

Focused existing Bun targets include `r8-keys.test.ts`, `r10-connect.test.ts`, `thread-keys.test.ts`, `r3-settings-keys.test.ts`, `sidebar-palette-keys.test.ts`, `composer-editor-intent.test.ts`, `composer-controls-queue.test.ts`, `composer-chips-accessibility.test.ts`, `menu-keys.test.ts`, `context-menus.test.ts`, `context-menu-actions.test.ts`, `context-menu-hookup.test.ts`, `external-link-menu.test.ts`, `media-actions.test.ts`, `terminal-keys.test.ts`, `terminal-links-and-close.test.ts`, and the existing Cite/selection tests. Keep reference-derived expected behavior; do not replace assertions with “contains keycapture” or deleted-symbol greps.

Existing native suites, using the maintained recipe in `README.md` “The module AppKit/XCTest binaries”:

- `macos/tests/intent`: gesture once/repeat/modifier behavior; port assertions to the replacement before deleting the old implementation.
- `macos/tests/r8-keys`, `menus`, `real-key-chords`: launcher and actual AppKit menu key-equivalent paths, quit timing, text Undo/close priority and chord coverage. `real-key-chords` uses stand-in host pieces; it is a coverage guard, not complete proof of current Exact routing or an attended keyboard.
- `macos/tests/sidebar`, `r12-sidebar`, `r8-pointer` where present on refreshed base: hints, click modifiers and anchored row menus. Confirm exact existing directories before invoking.
- `macos/tests/contextmenu`: production flat/nested menus, secondary clicks, file menus, text/Services, tab input, reply links and ownership.
- `macos/tests/composer`, `r9-input`, `terminal`, `r6-media`: text/chip/queued-edit/IME, terminal actions and media behavior. `terminal` first builds its page with `bun examples/t3-code/terminal-host/build.mjs` as the README requires.
- `macos/tests/timeline-keyboard`: actual ExactKit key/focus regression, using the README's separate recipe and the freshly built app native archive.

The README recipe generates `ExactDataKeys.swift` through `host/apple/data-keys.mjs`, compiles the production module facade and app Swift sources, and excludes `T3Module*.swift` for `composer`, `menus` and `r5-panels` because those suites supply `exactModule`. Preserve that distinction; record each compile/run exit code. Run the repository's six required checks before landing: build, test, Clippy plus fmt, caps after staging task-owned changes, boot, and `cargo check --workspace --all-targets --keep-going` with pinned Bun, as current `rules/RULES.md` requires. Scope any additional tests to affected non-default crates as repository rules require.

Native before/after recipe:

1. Create an isolated lane using the existing app/agent workflow, fixture home/project and a development port in 16000–16999. Use the pinned reference runtime and a fixture thread with a plain field, composer chips/citation, queued draft, sidebar rows, PR list, launcher, Files tree, media and terminal. Record fixture reset and source identities; avoid the user's live home, accounts and provider sends.
2. Build/drive the refreshed base once. Capture tree/AX/focus/selection/state and actual effects for K1–M2 before removing the first reader. Use the same fixture state after each bounded change, re-resolving node IDs after rerenders.
3. With the supported agent, use `type <target> key Meta down/up`, `prefer has-focus false`, `clock +1`, repeated key duration, targeted `tap ... mouse modifiers ...`, ContextMenu and supported screenshots/layout as appropriate. Match the current `scripts/agent.mjs` syntax. Assert single action counts, owner identity, preserved text/selection and focus, not merely absence of crashes. For menu actions, distinguish the painted agent path from real `NSMenu` tracking.
4. After each moved reader/menu, collect one fresh native after drive with the same expected behavior. At the end run the combined matrix. Preserve the source SHA, plan/bundle identity, exact commands, fixture state, before/after/reference artifacts and failures.
5. In an unlocked, attended desktop session, use the approved `computer-use` skill for all menu-bar menus, real US/2-Set chords and one actual context menu per moved site. Use VoiceOver for named chip/menu-navigation rows. A locked session cannot supply these observations; keep them blocked/unverified, with the required handoff stated. Do not unlock, change input sources globally, activate VoiceOver, or send global input during this preparation task.

For exportable proof, keep artifacts in the existing evidence workflow and link immutable revisions. Transparent/stale captures, source inspection, agent synthetic keys and constructed AppKit events must not be relabeled as real-input proof.

## Progress

- Read-only preparation completed against the inspected base and pinned reference.
- First-wave and R14 adoption merge facts reconciled. The seven serial predecessors merged; prepared from `294db9696` on 2026-10-15 in the isolated checkout `t3-code-contract-keys-and-menu-bar`.
- Actual public capture/menu APIs, native routing, four readers, bridge consumers, menu bar and all four `T3MenuTurn` callers inventoried at the inspected base; refresh against the prepared base.
- 2026-10-15: implemented one reader at a time on base `0d85b6453`, every row given a final disposition (migrated and
  tested, or retained native with its exact reason); checks, the native AFTER drive (one repair, one retry) and an
  independent review are recorded in "Implementation and verification, 2026-10-15". Verification stays unverified until
  the final real-input session.

## Attempts and evidence

Preparation evidence is source/record inspection, not a runtime reproduction:

| Evidence | Result / qualification |
| --- | --- |
| Git source inspection | `a218ee78fec42b1a4590b199297f30667876de67`, clean before draft; main pin above. First-wave/R14 receipt read from `reviews/20261014-first-wave-resume.md`. |
| GitHub read-only checks, 2026-10-14 | #549 MERGED into feature at `3710af842`; #110/#140/#141/#235/#292 CLOSED, #168 MERGED. Remote closed timestamps and later adoption/qualification commits are different evidence; no claim they occurred together. Exact task-branch search found no existing task PR in returned results. Recheck before implementation. |
| Pinned reference source | `git show 1e2ecbd9758830669684b494d4398f626b0576e0:<path>` for DesktopApplicationMenu, QuitHold and keybindings; menu and input semantics above checked against those files. |
| Adopted implementation | Grammar/schema, `KeyEvents.swift`, `ShortcutsMac.swift`, `MenusMac.swift`, the app's actual readers/callers and current/prior task records. The composer is covered by Exact's textarea mapping; arbitrary native/module/WKWebView exclusions do not imply the composer is excluded. |
| Corrections to stale records | Native comment “press carries no event” is obsolete; `MouseEvent` exists. Historical X25/X26 rows saying main has no capture/menu declarations are obsolete. Framework issue closure covers the bounded APIs, not wholesale native deletion. T3Menus includes pickers/lifecycle/zoom; R10Connect includes wake; T3Sidebar includes notification/menu operations. |
| Unavailable proof | No task-specific build, tests, app drive, real 2-Set session, VoiceOver session or real menu tracking was performed. The parent reports the desktop locked; no qualification is promoted to passed. |

Store actual implementation attempts here with source/framework/tool versions, fixture, command, assertion, exit code, artifact location and limitation. Preserve failed-before and passing-after results; bound a fix loop to three rounds. The draft's assertions must be refreshed after predecessors change their callers.

## Implementation and verification, 2026-10-15

Prepared base `0d85b6453` (feature head `294db9696` plus this ticket); framework pin unchanged. App-only changes under
`examples/t3-code`. Public APIs used: `keycapture=` with `KeyboardEvent.code`, `autofocus`, plain-letter
`aria-keyshortcuts`, `press` with `MouseEvent`, `-exact-menu`, root `-exact-menu-remove`, `contextmenu` + `contextPopover`.

### Disposition: KW-11a / X25 (the four readers)

| Reader responsibility | Disposition | Where / reason |
| --- | --- | --- |
| `T3KeyRecorder` (Settings › Keybindings and the project action editor) | Migrated | `key-recorder.contract` `KeyRecorder`: a `tabindex=0` box (`id` = its test id) with `autofocus`, `keycapture=keys` (every key prevented and stopped; Escape sends `escape`; a modifier alone records nothing) and `blur=cancel`; the chord is `keybindingFromKeyboardEvent` as `fn recorderBinding` over `key`/`code` (an a–z layout letter, else the physical punctuation or digit, else the physical Latin letter for a key that types none, so Korean 2-Set's ⌘ㅠ records `mod+b` with or without R10's re-issue). Both callers use it; the native view, its factory and the `t3-key-recorder` module entry are deleted. The action editor's recording row is `aria-modal` now (its shortcuts were not scoped before), its Keybinding press moves the focus to the recorder (`focus("action-keybinding-recorder")`: the drive showed `autofocus` leaving it in the Name field), and its Escape/blur keep the old binding (the native view's `escape` was saved as the binding `escape`). |
| `R8KeysLauncher` (mount focus, letters from any non-typing focus, repost) | Migrated | `shell-panels.contract`: the launcher is `autofocus`; each available row declares `aria-keyshortcuts="<l> Shift+<L>"`, which the host presses before any key handler, never from an input context (`Shortcut.permits`), never behind a modal or while inert, once per press, by the physical key under a non-Latin layout. `R8KeysLauncher.swift` is deleted. Retained: `T3Composer.launcherLetter`, a 10-line guard so the composer's own type-to-focus monitor leaves a live launcher's letters to Exact's monitor (AppKit runs local monitors in no fixed order). |
| `T3Sidebar` click modifiers (`sidebarModifiers`, the click monitor) | Migrated | Row presses `openPress(e: MouseEvent)` (`sidebar-row.contract`, `legacy-sidebar.contract`) send `click`, `click:meta` or `click:shift`; the legacy PR glyph `prPress` sends `mod:<page>` for ⌘/Ctrl; `app.contract sidebarNewThread(e: MouseEvent)` sends `shift`. The op, monitor and `record`/`pressModifiers` are deleted. A keyboard or chord press carries no modifiers: ⇧⌘O (chat.new) now opens the palette as the reference's `chat.new` handler does (the old read of the live keyboard created at once). |
| `T3Sidebar` held ⌘ jump hints and ⇧ PR speed mode | Retained native | Exact runs `key`/`keyup` handlers only on the path of a focused node (`host/apple/Sources/ExactKit/KeyEvents.swift:225` `keyTarget` is nil for the window or a module's web view, so `keyEvent`, lines 98–141, has an empty path); the reference's window listeners hear ⌘ with nothing focused. Speed mode also needs `isEditableFocused`, and a Contract `KeyboardEvent` names no target. |
| `T3ComposerIntent` pointer gestures (Send ⌘-click, chat and pull request link clicks, fragment links) | Migrated | Each press passes its own `MouseEvent` modifiers: `composer-controls.contract sendPress` → `app.contract send(held)` → `send` op `pointer:<mods>` (`composer-editor-intent.ts pressGesture`); `app-window.contract linkOpen(kind, href, e)` sends `kind|mods`; `markdown.contract PrLinkRun open(e)` adds a fourth line (`press-modifiers.contract heldModifiers`). `browser-links.ts` and `pages-pr-links.ts` no longer ask the module. The module records no pointer gesture; a click only retires the Return gesture before it, so a stale ⌘↩ never resolves a later plain click (a plain click and a keyboard press look alike to a `MouseEvent`). |
| `T3ComposerIntent` Return gesture and held ⌘/Ctrl (the Send button's label) | Retained native | A Send chord is an `aria-keyshortcuts` press, which runs before any key handler (grammar "Shortcuts") and carries no modifiers, so no Contract handler can tell ⌘↩ from ⌥⌘↩; the plain-Return send is `T3Composer`'s own monitor, which defers to the native slash-command menu and stash state; the held ⌘ is window-level (the `keyTarget` gap above). Fan-out's Shift+Return reads the same gesture; its Shift-click was already the press's own (CO-7). |

### Disposition: KW-11b / X26 (menu bar, per item)

| Item | Disposition | Reason |
| --- | --- | --- |
| App ▸ Check for Updates... and Help ▸ Check for Updates... | Retained native | Tried as two `-exact-menu="app"`/`"help"` buttons and reverted after review: the host enables an `-exact-menu` item only while its window is key with no sheet (`host/apple/Sources/ExactKit/Mac/ShortcutsMac.swift:489–492` `validateMenuItem`), and its press went through the busy-guarded `command`; the reference's item is always enabled and reopens the main window (`ensureMain`). No window-independent menu command exists. `T3Menus` keeps its items, order and box unchanged. |
| View ▸ Show Tab Bar, Show All Tabs | Migrated | Root `-exact-menu-remove="show-tab-bar show-all-tabs"` (`app-window.contract`); `R8KeysMenus`' removal loop and its `allowsAutomaticWindowTabbing = false` are deleted (the host already disallows tabbing for a one-window app, `ExactDocuments.furnish`). |
| App ▸ Settings… | Already Contract | `ShortcutDispatch` ⌘, (unchanged). |
| About, Services, Hide, Hide Others, Show All, Cut, Copy, Paste, Delete, Select All, Speech, Enter Full Screen, the Window menu | Host | Standard roles the host owns. |
| Quit (immediate from the menu, held/double ⌘Q from the keyboard) | Retained native | `T3QuitHold` reads raw keydown/keyup/flags and auto-repeat cadence; no Contract quit API; menu Quit stays the host's `terminate:`. |
| File ▸ Close Window (⌘W closes the right panel first; menu click closes the window; held ⌘W repeats dropped) | Retained native | Needs the keystroke-versus-click distinction (`NSApp.currentEvent`) and raw repeats; `-exact-menu` gives one press for both. |
| Concealed app commands in File and Edit (hidden, still key equivalents) | Retained native | A hidden button leaves the menu with its equivalent; there is no hidden-equivalent contract. |
| Edit ▸ Undo (text history in a field, thread Undo by keystroke only) | Retained native | Responder-chain routing and keystroke detection. |
| Edit ▸ Paste as Text (⇧⌘V) | Retained native | The responder chain's `pasteAsPlainText:`; `-exact-menu` supplies a button press, not a role. |
| Edit template extras (Paste and Match Style, Find, Spelling and Grammar, Substitutions, Transformations), Help's book item | Retained native | Not in `-exact-menu-remove`'s identifiers. |
| Go and Develop removed, Save Trace kept as a hidden ⌥⌘T | Retained native | Whole menus are not removable by policy; Develop stays in development builds by the framework's contract. |
| View ▸ Reload, Force Reload | Retained native | The Develop menu's reload action. |
| View ▸ Actual Size, Zoom In (+ hidden ⌘+), Zoom Out | Retained native | No Contract API zooms the main window's content; an app chord is heard only inside a shown modal while one is up (`Shortcuts.swift` `shortcutModal`), while the reference's menu zoom works under a dialog; the host files app View items at index 0, ahead of Reload (`ShortcutsMac.swift` `sync`). |

### Disposition: X15

| Route | Disposition | Reason |
| --- | --- | --- |
| The launcher's letter re-post (`R8KeysLauncher` `NSApp.postEvent`) | Removed | Its letters are declared shortcuts, matched by the physical key (`ShortcutsMac.swift:66–74`). |
| The keybinding recorder | Removed (no reissue needed) | It binds by `KeyboardEvent.code`. |
| `R10Connect` Latin re-issue | Retained | One event-level transform. Still needed by the items AppKit matches on the typed character (Copy, Cut, Paste, Undo, Redo, Select All, Quit, Hide, Minimize, Close Window, Reload, Paste as Text, Actual Size, Zoom In, Zoom Out), the terminal's web view, the queued-edit and SnapShot readers. The host's physical fallback covers only its `ShortcutHost` items (`ShortcutsMac.swift:66–74`, `166–176`). `wake` unchanged. |
| `T3QuitHold.key` (Q by key code), `T3Menus.dropsHeldClose` (W by key code), `R9Input.chordKey` (⌘B/⌘I) | Retained | Native readers' own raw-event matching. |
| Terminal `WKWebView` | Retained | Input-context-first composition. |

### Disposition: X292 (module context menus)

| Site | Disposition | Reason |
| --- | --- | --- |
| Settings › Archived threads row (Unarchive, —, Delete with the trash glyph) | Migrated | `settings-archive.contract ArchiveMenu` context popover; `archive-pick` runs the choice (`settings-a-archive.ts`). |
| Pull request number (list row and detail header): Copy link, Open on <host> | Migrated | `pages-prs.contract PrLinkMenu` (used by `pages-pr-detail.contract`); `pullRequestLinkPick`; rows carry `linkOpenLabel`. |
| Files tree row; Files, diff and Markdown file links; chat web links | Retained native (`T3ContextMenu`) | TypeScript builds the rows at the click from per-target state (configured editors, path, preview capability, thread-link state); a context popover lists only rows the plan already holds when its `contextmenu` runs (LLP 1021 §5.1), so moving them needs per-row menu data for every tree row and link: the broad refactor this task excludes. |
| Branch name (trigger and picker rows) | Retained native | The rows sit inside the open branch picker popover and share one item model with the trigger; a host context menu over that open picker was not established. |
| Right-panel tab Shift+F10 | Retained native | Shift+F10 has no menu default on macOS (grammar); the native `anchor: focus` opens at the focused view. Pointer and ContextMenu already use `r4-tab-menu`. |
| Draft row ContextMenu/Shift+F10, media menus | Retained native | Bottom-left keyboard anchor; Exact opens a ContextMenu-key popover at the focused node's centre. Media also keeps async signed URLs and native save/copy. |
| Legacy thread and project rows | Retained native (`sidebarMenu`) | TypeScript computes them at the click (the bulk menu over the selection, legacy branch rows); no per-row menu data. |
| Terminal selection and context menus | Retained native | They come from the `WKWebView`. |
| `T3MenuTurn` | Retained | Callers: `T3ContextMenu.perform`, `T3MediaActions.perform` (`mediaMenu`), `T3Module+Sidebar` (`sidebarMenu`), `T3TerminalActions.show`, The two moved sites are scheduled by the host. |

### Tests and checks (final source, after the review and the drive repair)

| Check | Result |
| --- | --- |
| Full app Bun suite (336 tracked `.test.ts` files, explicit `./` paths) | 5,239 pass, 1 skip, 0 fail |
| Strict `tsc` on `app.ts` | exit 0 |
| `cargo run -q -p contract -- build examples/t3-code/app.contract -o target/t3-keys.plan` | exit 0; 254 warnings (colour-role lint, as before); 29,330,831 bytes; SHA-256 `55fd11a2f111664bf15c637ad84cb27c2c435aa359e0ce2775a0b7ee2f2718f8` |
| `cargo test -p t3-code-macos --lib --no-fail-fast` | 49 passed (new `contract_keys_tests`: the root's `menuRemove`; the launcher's `autofocus` and row shortcuts, none on an unavailable row; a row press's `click`/`click:meta`/`click:shift`; the recorder's capture by physical key, modifier-only and Escape) |
| AppKit binaries (README recipe) | composer 78/78, intent 4/4, sidebar 5/5, r9-input 6/6, r5-composer 3/3, contextmenu 70/70, r12-sidebar 3/3, r8-pointer 2/2, r10-connect 3/3, r5-panels 6/6, menus 47/47, r8-keys 5/5 |
| `real-key-chords` | Fixed to compile (it named `R8KeysLauncher` and the removed `R10Connect.install`); not run: it activates the app and takes the key window on an unlocked desktop, which this task leaves to the attended session |
| `timeline-keyboard` (ExactKit recipe) | Compiles; fails `Tab must leave the result region`. It builds its own `Presenter`/`NodeView` fixture and calls no app module or plan; this diff touches no ExactKit file |
| `git add -A examples/t3-code && bun scripts/caps.mjs` | all budgets within cap (`app.contract` 1,500 lines) |
| `cargo fmt --all -- --check`; `cargo check --workspace --all-targets --keep-going` | exit 0; exit 0 |
| Native bundle (`build.mjs t3-code-macos --bundle`) | exit 0, three times (first change, after the review revert, after the drive repair); final executable SHA-256 prefix `f1b9586b935ab0be` |

Test changes: `r3-settings-keys.test.ts` (recorder chord table, capture handler, both callers, action editor `capture`/`record`), `settings-escape.test.ts`, `r4-surfaces.test.ts` (row shortcuts, `autofocus`), `sidebar.test.ts`, `legacy-sidebar.test.ts`, `legacy-sidebar-full.test.ts`, `dialog-focus.test.ts` (press payloads; the AppKit `testRowPressReadsTheClickEventsModifiers` cases ported here), `composer-editor-send.test.ts`, `composer-controls.test.ts`, `browser-links.test.ts`, `pages-pr-links.test.ts`, `reply-links-reference-behavior.test.ts`, `timeline-work-rows.test.ts`, `browser-capture.test.ts` (pointer modifiers), `settings-a.test.ts`, `sidebar-hover-archive-and-glyph.test.ts`, `context-menu-actions.test.ts`, `context-menu-hookup.test.ts`, `menu-keys.test.ts`, `adopt-main-fixes-r12.test.ts` (the two popovers), `r8-keys.test.ts` (root policy); AppKit `r8-keys` (launcher test ported to the shortcut route in either monitor order), `intent` (a click retires a Return gesture), `sidebar` (no modifier op).

### Native AFTER drive

Agent mode (`scripts/agent.mjs macos --json --size 1280x840`, storage `contract-keys-a`), the bundle's executable (`EXACT_MAC_BIN`), the audit runtime `t3-0.0.46-nightly.20261005.2667` (`T3_LOCAL_RUNTIME_DIR`), port 16714, telemetry off, homes copied from the audit lane `ri1012-b10` into `target/contract-keys/lane`, external opens recorded to its `logs/opened-urls.txt`. No provider message was sent. Fixture effect: one thread archived and unarchived in the copied home. Evidence: `target/contract-keys/drive/` (`.jsonl` per segment, PNGs); segments A, B, B2, B3, C, D1–D3 ran on the first build, R and R2 on the repaired one. Early aborts were driver targeting (a page still loading, a label, an ambiguous "Back"), rerun with corrected targets. The app and its server were stopped by the driver; port 16714 is free.

| Row | Result | Evidence |
| --- | --- | --- |
| Launcher takes the focus on mount | PASS: after ⌘⌥B the focused view is `surface-chooser` (`autofocus`) | `A-launcher.jsonl` 15–16, `A02-launcher-focused.png` |
| Launcher letters: rows declare them, unavailable rows do not | PASS: `b Shift+B`, `t Shift+T`, `f Shift+F`, `d Shift+D`, `m Shift+M`; Pull request / Linked pull requests none | `A-launcher.jsonl` 16 |
| No typing-context theft | PASS: `f` with the composer focused typed `f` into it; no surface opened | `A-launcher.jsonl` 18–21 |
| A letter from another, non-typing focus | PASS: ⇧F with a sidebar row focused opened Files once (one `tab-files`) | `A-launcher.jsonl` 24–29, `A03-files-by-shift-f.png` |
| Settings recorder | PASS: focus on the recorder at mount; Shift alone records nothing; ⌘K records `mod+k` (palette stays closed, recording ends); Escape restores `mod+n` and Settings stays open; ⌃⇧↑ records `ctrl+shift+arrowup` | `B-recorder.jsonl`, `B2-recorder.jsonl`, `B01`–`B03` |
| Project action editor recorder | FAIL then PASS: the first build left the focus in the Name field (`autofocus` did not take it); repaired (`focus("action-keybinding-recorder")`), the retry shows the recorder focused, ⌘⇧T records `mod+shift+t`, and Escape keeps it (before this task Escape saved `escape`) | `B3-action-editor.jsonl` 19, 23, 28; `R-retry.jsonl` 18–22, `R01-action-editor-recorded.png` |
| Sidebar click modifiers | PASS: ⌘-click on two rows multi-selects them and opens neither; ⇧-click ranges from the anchor; a plain click clears and opens | `C-sidebar-links.jsonl` 18–32, `C01`, `C02` |
| A chat link's ⌘-click | PASS: the system browser (recorded `https://example.test`), not the preview | `opened-urls.txt`, `C03-link-meta-click.png` |
| Archive row menu (pointer) | PASS: the painted popover lists Unarchive, Delete (menu, menuitem roles; trash glyph, separator) | `D2-archive-menu.jsonl` 16–17, `D03-archive-menu-light.png` |
| Archive row menu (ContextMenu key, dark) | PASS: opens from the focused Unarchive button, focus kept there; Unarchive returns the thread to the sidebar and empties the page | `D3-archive-keyboard.jsonl` 15–26, `D04-archive-menu-keyboard-dark.png`, `D05` |
| App-menu command through the agent's path | PASS for Settings… (⌘, opens Settings, B segments); no Contract menu command was added (Check for Updates... stays native) | `B-recorder.jsonl` |
| Light and dark | PASS: launcher, recorder, editor light; archive menu dark | PNGs above |
| Launcher remount after the last tab closes | Not observed: closing the last tab closes the panel here, so no launcher mounts | `R2-launcher.jsonl` 15 |
| Held-modifier state after page blur, sidebar hints, PR speed mode | UNVERIFIED: retained native; the agent's synthetic keys bypass local monitors | — |
| Non-Latin layout chords | UNVERIFIED live (the agent sends no `KeyboardEvent.code`); covered by `r3-settings-keys.test.ts`, the Rust recorder test and the r8-keys AppKit test | — |
| Send ⌘-click / ⌘↩ intent | UNVERIFIED live (no provider sends); Bun client tests | — |
| Pull request number menu | UNVERIFIED live (the fixture has no pull request rows); Bun tests | — |
| Real menu-bar tracking, real US/2-Set chords, actual context menus by physical input, VoiceOver | UNVERIFIED: the final real-input session | — |

PR images (local): `target/contract-keys/pr-images/01-launcher-autofocus-letter-shortcuts.png`, `02-settings-recorder-captured-mod-k.png`, `03-action-editor-recorder.png`, `04-archive-context-menu-dark.png`.

### Review

A fresh reviewer read the ticket and the staged diff and reported two blocking defects, both in the first Check for Updates... move: the press went through the busy-guarded `command` (dropped while another command was out), and the host enables an `-exact-menu` item only while its window is key with no sheet, where the old item was always enabled. Both were resolved by keeping the native items (disposition above) and reverting that move; the Bun and Rust tests for it were rewritten to the root policy. A second pass over the delta (the revert and the drive's
recorder repair) found no blocking defect and confirmed the disposition tables against the staged code. Non-blocking notes taken: the recorder test's ⌘Q wording (T3Menus' monitor may see ⌘Q first, as with the native view); ⇧⌘O now matches the reference's `chat.new`. Left as noted: the r8-keys AppKit test stands in for the host's shortcut rules (the drive and the Rust plan test cover the declarations); some tests read source next to behaviour tests.

### Framework gaps (no framework edit made)

- `host/apple/Sources/ExactKit/KeyEvents.swift:225` (`keyTarget`) and `:98–141` (`keyEvent`): with no Exact node focused (the window, a module's web view) no Contract `key`/`keycapture`/`keyup` handler runs, so window-level held-modifier state (sidebar hints, PR speed mode, the Send label) stays native; a `KeyboardEvent` also names no target, so `isEditableFocused` is not expressible.
- `host/apple/Sources/ExactKit/Mac/ShortcutsMac.swift:489–492` (`validateMenuItem`): an `-exact-menu` item is enabled only while its window is key with no sheet; no window-independent menu command.
- `-exact-menu` gives a press, not a responder-chain role (Paste as Text), and no hidden-but-usable equivalent, order or separator policy; `-exact-menu-remove` has no ids for Edit's template extras, Help's book item or whole menus.
- A ContextMenu-key popover opens at the focused node's centre; source-specific bottom-left keyboard anchors (draft rows, media) are not expressible.

### Known differences

- The painted archive menu under the agent: Escape closes the menu and also reaches Settings' Back (the popover is no modal scope); with real input the host shows an `NSMenu`, which takes Escape (unverified until the real-input session).
- The keybinding recorder is a focused box: the host may draw its focus ring (the native view drew none); `outline` is refused on a box.
- A keyboard or chord press carries no click modifiers (the native read used the live keyboard): ⇧⌘O opens the palette, as the reference does.
- Shift+F10 on a right-panel tab, draft/media keyboard menus and the other retained sites still answer "dismissed" under the agent, as before.

### Real-input session realinput-1015, 2026-10-15

Bundle of `db3c4540f` (code `aa4fd19c3`) in a lane copy (`com.exact.t3code.macos.ri1015-1`) launched through LaunchServices (a normal launch, not agent mode), port 16915, isolated homes from audit lane `ri1012-b10` plus the real-GitHub lane's sandbox (read only). Real keys and pointer through `orca computer`; the input source was `com.apple.keylayout.2SetHangul` (read only, not changed). While a painted popover or dialog is open the window's accessibility tree collapses and `orca` refuses input, so those keys went as System Events key codes to the lane (front process checked first). Evidence is local in the session worktree's `target/realinput-1015/` (`notes.txt`, `ev/`). 32 steps in all: 27 passed, 2 failed, 3 not verified.

| Row | Result | Evidence (`target/realinput-1015/ev/`) |
| --- | --- | --- |
| Real menu-bar tracking (step 2) | PASS by keys (all six menus walked); pointer on the menu bar UNVERIFIED (`orca` clicks stay in the window) | `s02-g-menu-*.png` |
| 2-Set chords (steps 4, 5, 6, 7) | PASS: ⌘K palette; ⌘B reaches the composer's bold chord; ⌘⌥B launcher and ⇧F open Files; the recorder takes ⌘K as mod+k by the physical key | `s04-b-cmdk`, `s05-a-cmdb.png`, `s06-b-shiftf`, `s07-d-cmdk-window.png` |
| US chords | UNVERIFIED: the source was 2-Set and is read only | — |
| Recorder Escape after a chord (step 7) | FAIL: one Escape drops the unsaved mod+k but also closes Settings | `s07-e-esc` |
| Context menus, moved sites (steps 12, 28) | PASS: archive row NSMenu Unarchive / Delete, Escape closes only the menu (Settings stays: the agent-only Escape difference does not occur), Unarchive works; PR number NSMenu Copy link / Open on GitHub | `s12-i-small.png`, `s12-j`, `s28-b-menu.png` |
| Context menus, retained sites (steps 10, 11) | PASS: thread row NSMenu (13 items); Files tree row NSMenu | `s10-k-menu-screen.png`, `s11-g-small.png` |
| Launcher letter after a remount (steps 8, 11) | FAIL (seen): after the panel was closed and reopened, the focus is on the root and ⇧B/⇧F type into the composer or do nothing | `s08-e-shiftb`, `s11-d-f.png` |
| Held-⌘ hints, Send intent, PR speed mode, VoiceOver | UNVERIFIED: no separate modifier hold in `orca`; no provider send; VoiceOver not turned on | — |

## Next action

After realinput-1015 (2026-10-15): Keep `verification: unverified`. realinput-1015 passed menu-bar tracking by keys, the 2-Set chords and the context menus of both moved sites and two retained sites; the recorder's Escape closing Settings and the launcher letter after a remount failed (in `STATUS.md`). US chords, the pointer on the menu bar, held-⌘ hints, Send intent and VoiceOver stay open.

Merged as [#587](https://github.com/ccheever/exact2/pull/587), feature commit
`aa4fd19c3` (squash). The task stays unverified for real menu-bar tracking, US and 2-Set
chords, actual context menus by physical input, held-⌘ hints after a blur, the PR number
menu, Send intent and VoiceOver. These go to the final real-input session after the eight
serial follow-ups. Older known difference seen in the drive: the action editor hint shows
"UseBackspace" and breaks the line after it.
