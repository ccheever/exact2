---
name: 20261014-contract-keys-and-menu-bar
plan: 20261005-t3code-macos-parity
implementation: prepared
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-contract-keys-and-menu-bar
pr_url: null
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
- Implementation, compilation, native launch and verification have not run yet.

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

## Next action

Prepared on 2026-10-15 from feature head `294db9696` in the isolated checkout
`t3-code-contract-keys-and-menu-bar` (branch `feat(example)/t3-code-contract-keys-and-menu-bar`).
Implement this one-PR unit one reader at a time. Completion requires the per-reader and per-menu
disposition table (migrated and tested, or retained native with an exact reason), meaningful tests
and an independent review; a count of deleted Swift files is not completion evidence.

Verification budget (overrides the per-reader drive recipe above): one native agent AFTER drive at
the end, at most one retry after a repair. No before drive unless a comparison image is needed.
Attended rows (real menu-bar tracking, US/2-Set chords, actual context menus, VoiceOver) go to the
one final real-input session after this task; until then they stay unverified. Evidence stays local
in `target/`; PR images are GitHub attachments.
