---
name: 20261014-dialog-focus-return-and-traps
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-dialog-focus-return-and-traps
pr_url: null
verified_commit: null
---

# Dialog focus returns to its previous owner and stays inside busy dialogs

## Outcome

Complete the original `dialog-focus-return-and-traps` fix-wave task. A dialog takes its
intended initial focus, keeps keyboard traversal within its enabled controls, and gives
focus back to the appropriate previous control after dismissal. Pointer activation on
macOS preserves the focus that existed before opening; keyboard activation keeps the
reference's return behavior. Complete these seven rows in one application PR:

| Row | Required result |
| --- | --- |
| KD-dialog-focus-return | Cover all 10 popup families and 6 confirm callers in the owner map below. |
| KD-pointer-opened-dialog | Restore the saved focused ID for pointer-opened dialogs, rather than unconditionally focusing their opener. |
| G2-12 | AppConfirm handles Tab and Shift+Tab from both buttons; audit the other 15 keyed dialog families. |
| G2-16 | Machines focuses its first enabled Copy update command control on opening and restores its trigger on dismissal. |
| BUSY-GUARD | Guards choose the first available enabled target from an ordered list and fall back to the popup without releasing Tab. |
| ADDPROV-TAB | After provider creation, forward wrap enters the Sign in step's first enabled control instead of skipping to the X. |
| ICON-TOGGLE | Icons/Emoji/Monogram is one Tab stop; Left/Right change both selection and focus. |

Prepared base: `a218ee78fec42b1a4590b199297f30667876de67`, the refreshed remote feature
branch after all eleven first-wave PRs and main adoption #549 merged. The dedicated
Orca-managed task branch was created from that exact base. The application changes
are implemented and independently source-reviewed. Native qualification is
partial, as recorded below; task publication and full verification remain open.

## Scope and exclusions

The first implementation unit is one complete PR for all seven rows above and all 16
owners below. Use a shared lifecycle implementation in `dialog-trap.contract`, bind it
at the existing owners, fix the identified traversal cases, and update existing tests.
It is not a helper-only PR that leaves owners for later. Small commits within that PR
may separate lifecycle wiring, trap behavior, and the three specific controls.

All application paths below are relative to `examples/t3-code/`. Changes stay in that
directory. Primary ownership is `dialog-trap.contract`, `app-window.contract`,
`app-overlays.contract`, `app-settings.contract`, `shell-panels.contract`,
`settings-rest-dialogs.contract`, `settings-a-dialogs.contract`, the mapped popup/opening
components, `auto-balance.contract`, `providers-setup.contract`,
`providers-wizard.contract`, and `settings-b-icons.contract`. Existing guard callers may
need small prop/ID bindings when the shared interface changes. Existing tests and their
existing `contract-action-fixture.ts` helper may change. No new test apparatus is allowed.

Target is the existing AppKit macOS desktop app, Apple Silicon, with the app's macOS 14
minimum. Verify on the available Mac and record its actual OS/build and Keyboard
Navigation setting. Keep both appearances and the current 1280x840/default and 840x620
minimum layouts usable. This is a behavior task; no repeated pixel-polish matrix.

Excluded: framework/compiler/runner/host changes; new native focus hooks; replacement of
all authored popups with native sheets; generic DOM emulation; new debug endpoints,
scripts, fixture servers, or global input tools; menu-key/popover-command retirement;
legacy sidebar selection/rename/drop parity; network relaunch or route reordering;
browser parity; CSS cleanup; timeline/window-hook retirement; global key/menu-bar
migration; rich composer or terminal replacement. Do not send provider messages, perform
real GitHub writes, start an update, send a real SIGKILL, or delete user data to verify
focus. Use existing controlled fixtures and cancel destructive confirmations.

## Context and guidance

Repository and reference identities are portable; a local worktree path is not a task
identity. Repository-relative plan records are under
`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`:

- `spec.md` and `plan.md` define the parent scope. Current authorized base is
  `feat(example)/t3-code`; older `daehyeon/t3-code` prose is historical.
- `reviews/20261014-first-wave-resume.md` records the continuation.
- `tasks/closed/20261014-adopt-main-fixes-r14.md` records verified, merged main adoption.
- `tasks/20261014-pr-list-place-and-thread-picker.md`,
  `tasks/20261014-chat-markdown-media-and-code.md`,
  `tasks/20261014-cite-selection-anchor.md`, and
  `tasks/20261014-sidebar-hover-archive-and-glyph.md` are overlapping first-wave work.
- `tasks/20261013-r12-kept-workarounds.md`, `STATUS.md`, and `EXACT2-GAPS.md` in the
  application retain the historical gaps. Reconcile their claims with current code.

Original task authority is the `dialog-focus-return-and-traps` entry in the coordinator's
`fix-wave-plan.json`. Its seven rows and complete acceptance are reproduced here so a
fresh checkout does not need that machine-local JSON or planner chat. The user authorized
continuation of the fix wave. The earlier instruction to stop repeated pixel testing
still applies. No new scope decision is needed for this task.

Reference repository: `https://github.com/pingdotgg/t3code`, read-only commit
`1e2ecbd9758830669684b494d4398f626b0576e0`. The pin was checked during preparation.
Relevant reference paths under `apps/web/src/components/` are `ui/dialog.tsx`,
`ui/alert-dialog.tsx`, `ConfirmDialogHost.tsx`, `ServerUpdateAction.tsx`,
`chat/useAutoBalanceUpdateBanner.tsx`, `chat/ExpandedImageDialog.tsx`,
`PullRequestThreadDialog.tsx`, `settings/AddProviderInstanceDialog.tsx`,
`settings/ProviderWizardAuthenticationStep.tsx`, `settings/ProjectIconPickerDialog.tsx`,
`settings/ChatGptWelcomeCoordinator.tsx`, `settings/ConnectionsSettings.tsx`, and
`onboarding/WelcomeWizard.tsx`. Read the relevant reference behavior before editing its
clone. Base UI's Dialog/AlertDialog focus behavior is the reference; do not infer it from
a screenshot or from a generic browser default alone.

Prepared-against runtime: `30be3a94a70e754340c2e6bb5fa41ccd9922661c`.
Framework pin: main `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`, merged locally through
`1d8c7d5f1350d8a55ebd1fad42a5ce435cd1ffc5`. The prepared implementation base is `a218ee78fec42b1a4590b199297f30667876de67`; its
application/runtime source is unchanged from the independently reviewed `b40d3c08d`.

Guidance is the Exact implementation library `20261005-platforms-v3`: foundations,
components/lifetime, state and data, accessibility, platforms/macOS, and testing.
The library is source-reviewed and older than this runtime. Current repository
`docs/contract-grammar.md`, `docs/recipes/keyboard-and-focus.md`, `docs/testing.md`,
`AGENTS.md`, `rules/RULES.md`, and `rules/DEFERRED.md` govern current syntax and checks.
The D1/D2 migration is already applied: use view `if`/`for`, receiver methods, `.length`,
and `T[]`. Keep task/keybinding `when` syntax where it remains a different construct.
Do not reintroduce the library examples' retired `when`/`each`/free-function spellings.

## Dependencies

| Dependency | Kind | Evidence and remaining gate |
| --- | --- | --- |
| First-wave repairs actually merged into `feat(example)/t3-code` | merged task PRs | GitHub confirmed #531, #533, #534, #539, #540, #541, #544, #545, #546, #547 and #548 merged at 03:04 UTC, publishing `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`; all are ancestors of the prepared base. |
| Main adoption round 14 actually merged into the feature branch | merged task PR | GitHub confirmed #549 merged at 03:08:53 UTC, `3710af842bc54521c298124156989950b6f2e68c`; bookkeeping followed at prepared base `a218ee78f`. Main `178ae7237` is an ancestor. |
| Bubbling focus events/X54 | resolved framework issue | `issues/closed/20261009-bubbling-focus-events.md`, upstream #283, and plan `issues/closed/20261008-x54-focus-within-subtree.md`. Fix `1bcf7436f` is an ancestor of the inspected runtime. Public API is available; this task must qualify its own AppKit behavior. |
| Existing public field selection | merged framework work and task PR | `48755525c` and first-wave commit `6ff18f2251305c8f3f30a6ace8e425eab001367f` are ancestors of the remotely merged prepared base. Preserve FieldOpenSelect and its select-all behavior. First-wave dependency #546 is confirmed merged. |
| Authorized application scope | recorded decision | The 2026-10-14 fix-wave continuation and original seven-row task authorize this work. App-only, macOS-only and no repeated pixel-polish constraints remain. |

The coordinator created the implementation checkout from `origin/feat(example)/t3-code`
after both merge gates passed. The actual base is `a218ee78fec42b1a4590b199297f30667876de67`.
No task branch, task record or PR with this slug existed at the fresh lookup. The new
branch is `feat(example)/t3-code-dialog-focus-return-and-traps`; delivery is still none.

Issue assessment, checked from the local repository and plan records on 2026-10-14:

| Issue/reference | Capability/target | Evidence | Impact | Next action |
| --- | --- | --- | --- | --- |
| #283 / X54 | Bubbling focus on AppKit | Closed record and `1bcf7436f` ancestry; target and relatedTarget documented | nonblocking | Use the public event and verify opening/removal/nested transitions in this app. |
| #282 / X53 and #327 adoption | GUI modal commands and focus | Closed `issues/closed/20261009-gui-dialog-action-commands.md`; current authored role dialogs still use app guards | nonblocking | Keep authored presentation and repair its public-API focus behavior. Do not broaden into native-sheet conversion. |
| #490 / X79 | Tab from a focused non-Tab-stop box on macOS | Closed `issues/closed/20261010-macos-tab-from-a-focused-box.md`; Keyboard Navigation off intentionally excludes ordinary buttons | nonblocking | Implement explicit modal traversal/fallback and record the setting in native evidence. Do not claim a popup fallback alone traps Tab. |
| #275 / existing range selection | Plain-field selection, AppKit | Closed `issues/closed/20261009-field-selection-beforeinput-range-edit.md`; main adoption includes it | nonblocking | Preserve FieldOpenSelect; no rich composer or chip retirement. |
| Remote first-wave/main-adoption delivery | Usable serial task base | GitHub merge readbacks and feature head `a218ee78f` recorded above | resolved | Implement only in the prepared task worktree. |

The read-only preparation performed no remote mutation. The coordinator subsequently
certified the actual prerequisite merges above. No unresolved framework capability was
discovered from source inspection. A
runtime contradiction must be recorded with reproducible evidence, not worked around by
inventing a DOM API or changing the framework.

## Implementation notes

### Public capabilities and ownership

- `focusin=action` and `focusout=action` accept an optional `FocusEvent`. Its `target` is
  the focused element's own string ID, or `""`. `relatedTarget` is `some(id)` or `none`.
  Events bubble; the wrapper gains no Tab stop. `stopPropagation()` stops that event's
  ancestor delivery. `focusout` can fire between two descendants of the same wrapper.
- `focus(id)` is the public focus command. There is no public `activeElement`,
  `closest()`, DOM element object, or generic `:enabled` selector. `testId` is not `id`.
- `querySelectorAll()` supports declared `[data-word~="value"]` selectors. Results are
  mounted, displayed, non-inert own IDs. Disabled controls are not automatically
  filtered. Candidate eligibility must use the authored disabled/rendering state, or
  an explicit data marker derived from exactly that state.
- Popovers support `toggle=changed` with `ToggleEvent.oldState/newState`, and public
  `showPopover`, `hidePopover`, `togglePopover` with an explicit optional source. Do not
  rely on a resize observer to prove each native popover reopening.
- Keep focus state and handlers in `T3Window`, which is 804 lines at the runtime pin.
  `app.contract` is 1,498 lines against the 1,500-line cap. Use its existing `provide`
  boundary rather than adding root state or wrapping hot `chatLocal` call paths.
- `modal` includes Settings being open. It is not an "actual dialog is open" predicate.
  Track the appropriate focus boundary. Focus moving inside a popup must not overwrite
  the outside return target, and child dialogs must retain their parent's return path.
- Snapshot once per opening, before initial focus can contaminate the saved target.
  Keep the snapshot through edits, async refreshes and layout changes. Restore only
  after the closing instance releases focus. Empty saved focus is a valid state.
  Handle deleted/hidden/inert targets and dialog-to-dialog handoffs without stealing
  focus from a newer dialog or a deliberate navigation destination.
- Add IDs without adding needless `focus`, `key`, or `tabindex` behavior to openers.
  On macOS those attributes can change click focus. Preserve keyboard-open behavior and
  the real pointer focus that existed before opening; do not infer the saved target from
  the button that was clicked.
- Existing `DialogOpenFocus` and `FieldOpenSelect` remain the initial-focus owners.
  Existing `DialogFinalFocus` fixed-target callers, including Add Host and route removal,
  need pointer-return qualification. Avoid duplicate final-focus commands, especially
  ImagePreviewDialog's first-wave `returnFocus` path.
- Compose with Cite's root capture/outside-press behavior, sidebar hover state, and
  Appearance's `data-paint`/`data-pick` attributes. Keep typed drafts, selection, menu
  dismissal, and live appearance updates working. No new global keyboard handler may
  swallow keys before the existing Cite behavior runs.

### Complete 16-owner map

"Add ID" below names an existing test ID that currently lacks its own element ID.
All 16 rows must have an implemented lifecycle and a meaningful acceptance case, or a
precise existing-flow explanation where there is no live opening control.

| # | Owner/opening state | Opening and closing actions | Initial focus; return wiring |
| --- | --- | --- | --- |
| 1 | `app-overlays.contract` ProjectPicker, `projectsOpen` | Root `newThread()` when `data.projectId == ""`; closes through `closeModal`, selection or management | `project-dialog-add`, else `project-dialog-close`. Observe the lifecycle outside the mounted picker. The old test's six-openers description is stale: `openProjects` is passed to Sidebar but unused there. Preserve the focused control before the shortcut, without creating a new opener. |
| 2 | `app-overlays.contract` / `timeline-attachments.contract` ImagePreviewDialog, `data.imagePreviewId` | `image-open` / `image-md-open`; `closePreview` sends `image-close` | `image-preview-close`. First-wave Markdown path already uses `returnFocus` and `data.imagePreviewReturn`; integrate rather than issue two final-focus commands. Add IDs to `attachment-image-${image.id}`, `play-attachment-${id}`, and context-chip openers. Existing Markdown `nodeId` stays. |
| 3 | `app-overlays.contract` / `r4-composer.contract` VideoPreviewDialog, `data.composerVideoPreview` | `editorlocal:r4c-video-play`; `editorlocal:r4c-video-close` | `video-preview-close`. Add ID `play-video-${video.id}`; final-focus state outlives the preview's `for`. |
| 4 | `app-overlays.contract` / `timeline-mermaid.contract` DiagramPreviewDialog, `data.diagramPreview` | `diagram-open`; `diagram-close` | `diagram-preview-close`. Add ID `mermaid-expand-${block.id}` in `markdown.contract`. |
| 5 | `app-overlays.contract` / `r9-connect.contract` R9CheckoutDialog, `shell.prCheckout.open` | Both branch pickers send `chatlocal:git-pr-open`; close is `git-pr-close` | Keep FieldOpenSelect on `pr-checkout-input`. Add IDs `branch-checkout-pr` and `details-branch-checkout-pr`. Search entry points already have `branch-search` and `details-branch-search`; a menu may be hidden when the dialog closes, so qualify its return path. |
| 6 | `app-overlays.contract` / `r4-surfaces.contract` R4DeviceSetup, `shell.panel.deviceSetup` | Device selected through `panelUi("choose", surface.id)`; `surface-setup-close` or successful `shell:surface-device-complete` | `device-setup-step-0`, else enabled Close. `surface-chooser` and `add-surface-${surface.id}` have IDs. Add IDs to the actual `surface-${surface.id}` button and `panel-add-surface`; do not return to a hidden menu item. |
| 7 | `legacy-sidebar.contract` LegacyDialogFrame, rename-project/grouping | `legacy-project-menu`; close `legacy-dialog-close`; save `legacy-rename-project` / `legacy-grouping-save` | `legacy-project-title` / `legacy-grouping-cancel`. One family, two bodies. Add ID `legacy-project-toggle-${p.key}` to the actual button. |
| 8 | `app-overlays.contract` WelcomeLayer / `pages-welcome.contract`, `welcome.show` | Launch/setup state; `welcomeFinish` sends `pageslocal:welcome-finish` | Preserve the existing step-dependent initial target. No fixed opener at launch. Escape and outside press intentionally do not dismiss this wizard. Empty saved focus is valid. |
| 9 | `app-settings.contract` / `codex-setup.contract` ChatGptPlanDialog, `data.chatgptPlan` | An unacknowledged connected profile appears; `setup:chatgpt-plan-ack` closes/advances | `chatgpt-plan-usage-button`. No invented opener or real sign-in in tests. Keep the original snapshot if one notice advances to the next. |
| 10 | `connections-network-dialogs.contract` NetworkDialogs / PairingRevealDialog, `net.reveal.open` | Failed `pairing-copy`; `pairing-reveal-close` | `pairing-reveal-value`. Add IDs to copy controls through NetXs in `connections-network.contract`. Copy-link options repeat the existing row test ID, so new IDs must distinguish the option as well. |
| 11 | `legacy-sidebar.contract` SettingsConfirm `legacy-remove-project` | Native `legacy-project-menu`, or toast `legacy-remove-anyway`; `legacy-dialog-close` / `legacy-dialog-confirm` | `legacy-remove-project-cancel`. Preserve the real prior focus; the native context menu is not a Contract opener node. |
| 12 | Same file, SettingsConfirm `legacy-archive-many` | `legacy-thread-menu`, selected rows' Archive; `legacy-dialog-close` / `legacy-dialog-confirm` | `legacy-archive-many-cancel`. Add ID `thread-${t.id}` to the actual button; existing `legacy-row-${t.id}` names the container. |
| 13 | `settings-rest-dialogs.contract` SettingsConfirm `archive-delete`, `restEditor == "archive-delete"` | No current literal UI opening caller; closes through `restClose` or `raw("delete-archived-thread", ...)` | `archive-delete-cancel`. Cover the existing shared owner directly; do not invent a UI route. The active archived-row flow is row 15. |
| 14 | `settings-a-dialogs.contract` SettingsConfirm `signal-kill`, `core.killConfirm.pid` | Diagnostics `diag-signal` with SIGKILL; `rest:diag-kill-cancel` / `rest:diag-kill` | `signal-kill-cancel`. Add ID `signal-kill-${row.id}` in `settings-a-telemetry.contract`. Native verification cancels without signaling a process. |
| 15 | Same file, SettingsConfirm `archive-context-delete`, `core.archiveConfirm.id` | `rest:archive-menu` then Delete; `rest:archive-cancel` / `rest:archive-delete` | `archive-context-delete-cancel`. Owner is ArchivedRow in `settings-archive.contract`. Preserve focus for pointer context menus; do not manufacture an extra Tab stop on the row. |
| 16 | `app-overlays.contract` AppConfirm `server-update-confirm`, `data.serverUpdate.confirmTitle` | `su:update` or held automatic-update batch; `su:cancel` / `su:confirm` | `server-update-confirm-cancel`. Add IDs to notice action buttons in `composer-controls.contract`, especially `notice-action-${notice.id}`. Snapshot before the async result mounts the confirm. |

### Traps, Machines, provider and icon details

AppConfirm and SettingsConfirm must consume plain Tab from either button, with or
without Shift, and focus the other enabled button. The same two-control weakness exists
in `usage-bars.contract` ResetCreditDialog and `codex-setup.contract` ChatGptPlanDialog.
Audit all 16 families in `dialog-trap.test.ts`'s KEYED map. NetworkAccessDialog,
TailscaleDisableDialog, LocalEnvironmentDialog and PrActionDialog already handle both
directions. Preserve intermediate controls in multi-control dialogs; an audit is not
permission to collapse every dialog into a two-button loop.

FocusGuard's candidates must reflect enabled and rendered controls in traversal order.
Use a real popup ID with `tabindex=-1` as the fallback, and consume/retry both Tab
directions from that fallback. Focusing the popup alone does not trap the next key.
Keep busy refusals on actions and Escape. Required cases include:

- Snapshot setup: Advanced if the error section exists, otherwise the popup when all
  controls are unavailable. The Close button is not drawn while busy.
- Welcome Agents: `welcome-step-1` remains an alternative unless importing disables it.
- Usage hub: URL/label inputs remain editable when footer/X are disabled.
- Device host: `device-host-options` remains enabled.
- Route/environment confirmations and other all-disabled confirms: popup fallback.
- PR checkout and Device setup: use their available Close control before the popup.

G2-16's Machines component is **NoticeMachines in `auto-balance.contract`**, not the
separate ProviderAdvisory in `providers-upkeep.contract`. Add IDs
`notice-machines-trigger-${notice.id}` and `notice-machine-copy-${machine.id}`. Observe
popover opening/closing with its public toggle event. Focus the first enabled copy
control on every open, fall back appropriately when no machine has a copy action, and
qualify Escape, trigger-toggle dismissal and outside dismissal without stealing a
deliberate new focus. Keep the associated server-update confirm's saved return target.

Add Provider's post-created end guard currently targets `close-add-provider-x`. Its
replacement must follow the actual Sign in content in `providers-setup.contract` and
`providers-wizard.contract`: email toggle, method picker, Open Browser, Copy Link, docs,
Cancel, Sign In, Sign Out, then credentials/callback fields as rendered. Derive an
ordered eligible list from account state rather than hard-coding Sign In. Discovery,
active auth, signed-in state and no account have different available controls. Add
stable IDs through SetupButton and relevant shared controls, preserving the `wizard-`
prefix and avoiding duplicate credential IDs. Use enabled footer/X controls before the
popup. Preserve the nested sign-out confirm's return into the wizard.

SbToggle already owns `project-icon-mode-Icons`, `project-icon-mode-Emoji`, and
`project-icon-mode-Monogram` in `settings-b-icons.contract`. AppKit projects these radio
members into a native segmented control and hides their authored NodeViews. The
implementation therefore gives the visible `project-icon-mode` group the one
`tabindex=0`, initial focus, key handler, and guard target. Left/Right change the selected
radio in reference order while retaining actual focus on that visible group. Member
selection semantics and accessible names remain intact; no visual restyling was made.

## Acceptance and reproduction

Do not execute this section while merge dependencies remain pending. Use the actual
prepared base and existing isolated T3 fixtures. Never use the normal T3 home, port
3773, or `t3code` URL scheme. Existing fixture server runs keep telemetry disabled.
Record the fixture/reset commands actually used; a machine-local cache path is not the
only reproduction instruction. If an existing fixture is unavailable, record the
missing prerequisite rather than creating new apparatus.

### Behavioral tests

Extend `dialog-trap.test.ts`, `dialog-focus.test.ts`, and `dialog-focus.test.contract`.
Use the existing migrated `contract-action-fixture.ts` for actual action/derive behavior
where applicable, and existing domain tests for asynchronous opening/closing state.
Keep the exhaustive source inventory as wiring coverage; source strings alone cannot
establish native focus behavior. Existing fixture helpers already understand migrated
view syntax; do not add a second interpreter or obsolete method shims.

| Acceptance | Required observations |
| --- | --- |
| All 16 owners | Each mapped open state takes the existing intended initial focus. Keyboard and pointer paths restore the saved appropriate ID after supported dismissal. Automatic/no-live-opener cases use their real lifecycle; Welcome rejects Escape and returns only after completion. |
| Snapshot lifetime | No resnapshot after field edit, data refresh, resize or scheme change. A second opening captures a new target. Empty, removed, hidden and inert saved targets do not focus an unrelated node. |
| Nested/handoff | Sign-out confirm returns into Add Provider; the wizard later returns outside. Replacing a dialog with another does not run a competing final-focus command behind the new dialog. Markdown image close runs one effective return. |
| G2-12 | From Cancel and Confirm, Tab and Shift+Tab each prevent default and land on the other eligible control. No background control is reached. Busy disabled buttons are never invoked. |
| BUSY-GUARD | First candidate disabled, last candidate disabled, candidate unmounted, one survivor, and all-disabled cases. Popup fallback keeps both Tab directions and returns to newly enabled controls when busy ends. |
| ADDPROV-TAB | Post-created discovery, signed-out, active-auth, signed-in, docs-only and no-account states. Forward wrap reaches the first enabled rendered control; reverse wrap reaches the correct last control. Nested sign-out is retained. |
| ICON-TOGGLE | Exactly one visible mode-group Tab stop. Left/Right change selected mode in both directions while keeping actual focus in the group; next Tab leaves the group once. |
| G2-16 | Open, Escape, reopen, trigger close, outside dismissal, no-copy-action and disabled-copy cases. Opening focuses the actual first eligible copy control. |
| First-wave regressions | Checkout select-all and device rename remain intact; Markdown image keyboard return, Cite Tab/Escape, hover dismissal, terminal confirm return, and live appearance changes still behave as before. |

Correct the old native test assumption that pointer tapping Add Provider, then Escape,
must focus Add Provider so Enter reopens it. First establish and record the focused
control. Keyboard-open and pointer-open cases have distinct expected return IDs.

Focused development commands, from the repository root, after preparation:

```sh
bun install --frozen-lockfile
bun test ./examples/t3-code/dialog-trap.test.ts ./examples/t3-code/dialog-focus.test.ts ./examples/t3-code/pr-list-place-and-thread-picker.test.ts ./examples/t3-code/chat-markdown-media-and-code.test.ts --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/t3-dialog-focus.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Use explicit `./examples/...test.ts` paths for focused Bun runs so they cannot match
copied baseline tests under target. Run affected existing provider/connection/icon tests
as their files change. Final verification includes the app Bun suite, the required six
repository checks and a current native app launch; the coordinator may supply unchanged
integration checks when their source identity covers this PR. Do not broaden repeated
testing after a pass without a new change or unresolved concern. Read the repository's
current `AGENTS.md` for the exact flags and staging rules.

### Native recipe

Use existing launch/drive tools and controlled data after obtaining the coordinator's
Mac input slot. The preparation phase does not launch an app or send global input. The
implementation must build its own current bundle before driving; do not bypass stale
artifact refusal. Record `tree --ax`, `state.focus` and the mapped element IDs before
opening, immediately after initial layout, after traversal, and after dismissal. Use
`clock settle` or observed state completion rather than adding fixed sleeps.

1. G2-12: with Keyboard Navigation off, focus an existing field, open the controlled
   server-update confirm, then press real Tab and Shift+Tab from each button. Cancel
   with Escape. Repeat keyboard-open and pointer-open return cases. Do not update.
2. AH-1: focus Settings search, pointer-open Add device host, then Escape. Compare the
   saved and final focus. Separately open Add Host from keyboard and check its return.
3. G2-16: open Machines, verify Copy update command holds focus, Escape to its trigger,
   then reopen. Test the existing no-copy/busy fixture without copying real secrets.
4. Open one legacy remove/archive confirm from its keyboard context-menu path and its
   pointer path. Cycle both Tab directions and cancel; do not remove or archive data.
5. Drive one existing all-disabled busy dialog and one busy dialog with a surviving
   enabled control. Both Tab directions stay inside; after completion traversal resumes.
6. In the existing provider fixture's created Sign in step, cycle through its enabled
   controls and nested sign-out confirm, then cancel. Do not perform a real sign-in.
7. Choose project icon: open each initial mode, use Left/Right, verify selected/focused
   IDs and one Tab stop, then cancel. Spot-check minimum window size and dark appearance.
8. Check one existing checkout select-all path, Markdown image open/close, and Cite
   Tab/Escape path after the root focus handler is present. Record any unverified native
   path explicitly rather than converting source coverage into a runtime pass.

Keep one bounded native pass. Capture screenshots only where a focus ring or visible
control needs inspection; this ticket does not require another complete pixel matrix.
Native Keyboard Navigation semantics and first responder are part of the evidence, not
inferences from the Bun model. No iOS, web or Linux parity is claimed.

## Progress

| Stage | State |
| --- | --- |
| Original scope and owner map | Reconciled read-only with runtime `30be3a94a` and the pinned reference. |
| Current public API and syntax | Source-checked against main `178ae7237`; D1/D2 and existing focus helpers are present locally. |
| Prepared ticket | Promoted into the portable task record from the ignored preparation draft. |
| Actual remote first-wave/main merges | Confirmed merged; prepared feature base `a218ee78f`. |
| Implementation checkout/branch | Created through Orca from the exact prepared base; branch recorded in frontmatter. |
| Code/test changes | Shared dialog lifetime and enabled-target guards wired across the 16 mapped owners; menu return ownership, confirm traversal, provider wrapping, icon-group focus, and Machines trigger return implemented. |
| Source/model checks | Final source aggregate `46868b6a9d63983cccaafb92fd704050630c92d094601507ae56ef1e3620fb15`: 270 focused tests pass; full app 5,053 pass, one skip, zero failures; strict app TypeScript, direct Contract compile, and staged caps pass. |
| Repository gates | Final workspace check passes on `46868…` (212.34 seconds). Build/test/clippy/fmt/boot retain passing receipts with unchanged framework inputs. Prior Contract sweep passed 94 roots/195 imports; direct final app compilation covers the changed owner. |
| Native checks | Four authored cases pass on aggregate `033965…`; OS provider/icon/server-confirm observations retained on that source. Final `46868…` build proves Machines Copy focus, Escape/trigger return, deliberate outside focus, and no-copy popup fallback. Full native acceptance remains incomplete. |
| Independent implementation review | All 70 runtime/test file hashes independently checked. Full/menu-owner source and both later owner-specific increments approved; final source review found no remaining confirmed source issue. |
| Delivery | Local implementation only. Coordinator owns evidence publication, PR, and merge. Frontmatter remains `in-progress` / `unverified` / `none`. |

## Attempts and evidence

Preparation on 2026-10-14 inspected the complete 16-owner map, source opening/closing
actions, existing guard/initial/final helpers, KEYED classification, first-wave overlap,
current grammar, local issue records, parent plan/specification, and round-14 adoption
record. Existing local reference checkout HEAD matched
`1e2ecbd9758830669684b494d4398f626b0576e0`.

Ancestry checks at `30be3a94a` confirmed main `178ae7237`, bubbling-focus fix `1bcf7436f`,
field-selection fix `48755525c`, PR-list/helper commit
`6ff18f2251305c8f3f30a6ace8e425eab001367f`, Cite commit
`706f3d3f44a718d19f90996e82258718ec6e3e49`, Markdown commit
`c8b10717e5e6efe115b15b79bf3f80fedfd625bd`, and sidebar-hover commit
`a6fa91916dad1a27b34fc9ad42f5a91c383e81a8`. These are local inclusion facts, not remote
delivery evidence. Existing code retains root 1,498 lines, T3Window 804 lines, and the
first-wave FieldOpenSelect component.

Implementation used the existing action fixture and audit driver, without framework
changes, new fixture-server code, native focus hooks, or debug endpoints. The native
repair loop reached its third round: explicit menu-owner returns, the visible AppKit
icon group, then the Machines invoker. No further source repair round was started.

The prepared baseline failed the final typed-search assertion for pointer-opened Add
provider, New task, and Add host; Model prices already passed. The same four authored
cases pass after the shared lifecycle repair. Native OS input on macOS 26.6.2 (25G83),
Apple Silicon, with Keyboard Navigation off (`AppleKeyboardUIMode=0`) established:

| Row | Observed native result | Remaining qualification |
| --- | --- | --- |
| KD-dialog-focus-return / KD-pointer-opened-dialog | Add provider restores the previously focused Settings search after pointer opening and Escape; an OS `x` edits that search. Authored New task/Add host returns and Model prices menu-owner fallback pass. Server-update confirmation cancellation restores the composer. | All 16 owners, automatic openings, and every nested/handoff variant have not received a complete native pass. |
| G2-12 | Server-update AppConfirm passes Tab and Shift+Tab from both Cancel and Confirm, then Escape returns to the composer. The actual NodeView first responder was read after every key. No update was confirmed. | Every distinct confirm caller and native all-disabled confirmation remain open. |
| G2-16 | Final source: Copy receives focus; Escape and trigger-toggle close return to the trigger; reopening focuses Copy again; an outside composer click keeps composer focus. With the existing Laptop fixture changed temporarily to desktop-managed/no desktop-update capability, no Copy is rendered, the popup itself receives focus, and Escape returns to the trigger. | Disabled-copy/busy native state remains unverified. |
| BUSY-GUARD | Ordered candidate eligibility, empty-list popup fallback, both Tab directions, and renewed eligibility pass source/action tests. | Native all-disabled transitions are unverified. |
| ADDPROV-TAB | Existing isolated ACP stand-in reaches the created signed-out step: Account → Tab → Sign in → Shift+Tab → Close → Tab → Sign in. Escape restores Settings search. Temporary provider settings were restored byte-for-byte. | Active auth, signed-in/nested sign-out, and remaining alternate account states are model-tested only. |
| ICON-TOGGLE | Group initially focused; Right cycles Emoji/Monogram/Icons, Left reverses to Monogram; Tab enters Letters and Shift+Tab returns to the group. Escape returns to Settings search. Dark 840×620 layout inspected. | Separate saved Emoji/Monogram openings and the full first-wave/appearance regression matrix were not repeated. |

OS input was synthetic through Orca, with independent presenter first-responder and
accessibility readbacks where exposed. No human typing or VoiceOver qualification is
claimed. AppConfirm's accessibility snapshot omitted its modal subtree; its presenter
state and inspected screenshot supplied the recorded evidence.

Evidence is prepared under ignored `target/dialog-focus/publish/`: a README, compact
focus records, source manifest, check provenance, independent reviews, and selected
screenshots. The coordinator publishes that sanitized packet; raw state, pairing
links, private paths, and personal machine labels are excluded. Retained icon/provider,
server-confirm, and authored-after observations belong to aggregate
`033965250143898b664d45f6fdc581d19e7544edba7fcc127db0047ee7a572d0`.
The final increment changes only `auto-balance.contract` and `dialog-focus.test.ts`;
the final Machines proof and native build belong to `46868…`.

Preserved local failures include `native-before-authored-final.log`, the earlier icon
hidden-member observation, `g2-os-snapshot-51.json` (Machines Escape incorrectly returned
to the composer on `033965…`), and the focused `machines-return-red.log`. The first
no-copy opening attempts did not establish an open popup and are not counted; retry
snapshots `g2-final-snapshot-23.json` and `-24.json` prove open/focus and Escape/close.
Carrier/setup failures are retained separately from product outcomes.

The existing three G2 fixtures were copied into isolated homes and paired through the
normal Connections UI. Their matching `example.invalid` Git origin was temporarily
added to the isolated audit project, then removed with exact config-byte restoration.
The temporary stub capability was restored. All owned app/server/proxy processes were
stopped. Stub counters confirm zero update RPCs and zero prepared desktop updates.
No provider message, real authentication, GitHub write, update, force-kill, or user-data
deletion was used.

## Next action

Coordinator readback, 2026-10-14: implementation commit
`a70632fdea2a67a4265bb3d3d9b8502694e5907c` exactly matches the reviewed 70-file
aggregate `46868b6a9d63983cccaafb92fd704050630c92d094601507ae56ef1e3620fb15`.
[Published checks, focus observations, reviews and screenshots](https://github.com/ccheever/exact2/blob/110276e2b07f74d021132dd588176d9c1616f95e/dialog-focus-return-and-traps/20261014/README.md)
are pinned to evidence commit `110276e2b07f74d021132dd588176d9c1616f95e`.
The coordinator reviewed all four selected screenshots and the final Machines focus
records. Source checks and the bounded native cases pass; full task verification
remains open for the qualifications above.

The coordinator prepares the application PR. Keep this ticket unverified
until the remaining native states above are qualified or explicitly resolved in the
parent plan. The third repair round is complete; do not restart a new worker to evade
the fix-loop limit. Publication and merge remain coordinator-owned under the user's
existing authorization.

Preparation completion, 2026-10-14: the coordinator verified remote first-wave and R14
merge evidence, refreshed the feature head, searched for duplicate work, created the
Orca checkout and this task record. No application source changed during preparation.
