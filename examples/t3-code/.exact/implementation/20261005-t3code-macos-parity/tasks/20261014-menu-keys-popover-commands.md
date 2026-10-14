---
name: 20261014-menu-keys-popover-commands
plan: 20261005-t3code-macos-parity
implementation: prepared
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-menu-keys-popover-commands
pr_url: null
verified_commit: null
---

# Menus use public popover commands and preserve keyboard focus

## Outcome

Complete the original `menu-keys-popover-commands` task as one application PR. Menu
triggers open at the correct row, nested Escape returns focus correctly, focus leaving
the legacy options closes it, and keyboard users can reach every Run on and Workspace
choice and the contents of long PR comments. Remove the superseded invisible menu
openers and focus doors from every caller in the same PR.

| Original row | Required result |
| --- | --- |
| KW-5 | Trigger key handlers call `showPopover(id, source=trigger)` and then focus the first or last enabled item. Remove all `KeyMenuOpen` invokers. Snooze pinning follows `toggle`. Accepted Return in the Git branch and Diff base comboboxes calls `hidePopover`, leaving no empty open popup. |
| X73 | Remove the PR Filters synthetic resting box now that Escape works after blur. Preserve first-Escape submenu dismissal and parent dismissal on the second Escape. |
| R10-1 | Closing the Filters popover returns focus to the actual Filters trigger. After the second Escape, both focus and the visible native ring are on Filters. |
| LS-3 | Legacy Sidebar options closes when focus leaves the entire popup, including when Tab leaves Visible threads. Moving between its own controls keeps it open. |
| G2-15 | The combined composer menu has one wrapping arrow order over Run on, then Workspace choices. Enter selects once, closes the menu and returns focus to Message. |
| KW-12d | Focus entering a long comment's body expands it; the remark pencil shows while its remark contains focus. Retire `KmDoor` and all redirects to it. Preserve the reference's after-body Show full comment control, as reconciled below. |

Prepared on 2026-10-14 as serial follow-up **4 of 8**, from feature head
`5e5bdb094439d155543f5eb1111eea0ccb3abc76`. Dialog #553, legacy sidebar #557 and
Connections #566 actually merged into `feat(example)/t3-code`; all three resulting
merge commits are ancestors of this prepared base. The isolated Orca checkout uses
branch `feat(example)/t3-code-menu-keys-popover-commands`. This ticket is ready for
coordinator review before behavior implementation. Verification remains unverified;
preparation does not qualify native behavior or predecessor limitations.

## Scope and exclusions

All source changes stay under `examples/t3-code/`. One complete PR includes the six
rows, both shared helpers and every affected caller/test. This is not a helper-only
PR with follow-up callers. The inventory below was recounted on the prepared base
after all three predecessors merged.

Primary ownership: `menu-keys.contract`, its caller families below,
`pages-prs.contract`, `legacy-sidebar.contract`, `composer-controls.contract`,
`r4-git.contract`, `diff.contract`, `pages-pr-edit.contract`, and the remark owners in
`pages-pr-summary.contract` and `pages-pr-threads.contract`. The summary wrappers
instantiate the collapsing `PrdRemarkBody`; code-thread comments use `collapse=false`
and cannot qualify the clipped-body expansion. Existing TypeScript command/model code may change only
where necessary to preserve selection and final focus. Use existing tests and their
`contract-action-fixture.ts` helper. Do not add test infrastructure or native hooks.

Target: the existing AppKit macOS application on Apple Silicon, macOS 14 minimum;
default 1280×840 and minimum 840×620, light and dark. This is behavioral parity work,
with a bounded input pass and targeted ring/expanded-content captures. The user's
instruction to stop repeated pixel-polish matrices remains applicable.

Excluded: framework/compiler/runner/host edits; browser hover-submenu placement and
viewport fixes; CSS popover flipping and unrelated style cleanup; global key/menu-bar
migration; timeline/window-hook retirement; reimplementation of the dialog, legacy
sidebar or connection predecessors; native-sheet conversion; rich composer/terminal
replacement. Preserve their work while migrating overlapping menu callers.

Do not send provider messages, make additional GitHub writes, switch a user's checkout, or
reconfigure the real local server to exercise these controls. Use existing isolated
fixtures and the plan's designated test data. No normal `~/.t3`, port 3773, or
`t3code` URL scheme; any fixture server uses `T3CODE_TELEMETRY_ENABLED=false`.

## Context and guidance

Portable authority is the parent plan under
`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`: `spec.md`,
`plan.md`, `reviews/20261014-first-wave-resume.md`,
`tasks/closed/20261014-adopt-main-fixes-r14.md`, and the preceding/following task records.
The original coordinator `fix-wave-plan.json` entry has rows KW-5, X73, R10-1, LS-3,
G2-15 and KW-12d; their complete scope is reproduced here so this tracked
ticket does not depend on that machine-local JSON or the planner's conversation.
Historical `daehyeon/t3-code` base names in older documents are superseded by
`feat(example)/t3-code`.

Prepared and inspected application source: `5e5bdb094439d155543f5eb1111eea0ccb3abc76`.
Adopted framework main: `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`. Exact is consumed
from this same repository, not a separate package version. Bun is pinned to 1.4.2;
the coordinator's frozen-lockfile install passed in this fresh checkout. These are
base identities, not a verified implementation commit. Reference: `https://github.com/pingdotgg/t3code` at immutable commit
`1e2ecbd9758830669684b494d4398f626b0576e0`; inspect that commit, not a moving checkout
HEAD. Relevant paths under `apps/web/src/components/`:

- `ui/menu.tsx`, `ui/combobox.tsx`, `pullRequest/PullRequestListFilters.tsx`,
  `LegacySidebar.tsx` (ProjectSortMenu).
- `BranchToolbar.tsx` (MobileRunContextSelector),
  `BranchToolbarEnvironmentSelector.tsx`, `BranchToolbarEnvModeSelector.tsx`,
  `BranchPicker.tsx`, `WorktreeBaseBranchPicker.tsx`, `DiffPanel.tsx`.
- `chat/composerEventScope.ts`, `pullRequest/PullRequestCommentBody.tsx`, and
  `pullRequest/PullRequestEditButton.tsx`.

**Reconciled KW-12d source correction:** the
original summary says to remove the Show full comment Tab stop. At the reference pin,
`PullRequestCommentBody.tsx` has `onFocusCapture={() => setExpanded(true)}` on the body
wrapper (line 32), then an ordinary Button after that wrapper (lines 45–57), with no
Tab-stop exclusion. The current `PrdWords` likewise puts `PrdShowFull` after the body
(`pages-pr-edit.contract`, around line 193); the missing behavior is body focus
expansion. Coordinator direction on 2026-10-14 is to preserve that reference control
and remove an extra before-body focus stand-in only if one exists. Do not delete or
make the genuine after-body button unreachable to satisfy the stale summary. Preserve
Show less and its scroll-into-view behavior too.

Guidance snapshot: installed Exact implementation library `20261005-platforms-v3`,
especially foundations, component lifetime, state/data, accessibility, macOS and
testing. Its metadata is source-reviewed at `fd1c879...`/`e45434f...`, with no certified
runtime revision. The user chose the checked-out repository as current framework
authority (spec U1). Use `docs/contract-grammar.md` focus/popover sections,
`docs/recipes/keyboard-and-focus.md`, `docs/testing.md`, `rules/RULES.md`,
`rules/DEFERRED.md`, and current source for the newer APIs. R14 already migrated view
syntax to `if`/`for`, receiver methods, `.length`, and `T[]`; do not reintroduce stale
library/source-test spellings.

## Dependencies

| Dependency | Evidence at preparation | Gate / next action |
| --- | --- | --- |
| Eleven first-wave task PRs | #531, #533, #534, #539, #540, #541, #544, #545, #546, #547 and #548 are MERGED into the intended feature branch; every actual merge commit passed ancestry at the prepared base. | Satisfied; preserve their behavior. |
| Main adoption R14 | #549 MERGED at `3710af842bc54521c298124156989950b6f2e68c`; that merge and framework main `178ae7237` are ancestors. | Satisfied; no independent framework upgrade. |
| Dialog focus and traps | #553 MERGED at `6a7a3cad0cb5f66fb7b5d677d21f8e130356e5d7`, present in the prepared base. | Satisfied; preserve menu/dialog owner bubbling and invoker lineage. |
| Legacy sidebar parity | #557 MERGED at `4520b0124b36cf24481a973f59a414ae8753dab1`, present in the prepared base. | Satisfied; preserve row focus, rename, selection and file-drop ownership. |
| Connections/network settings | #566 MERGED at `fbed7fadd8673ed58eaa98c585bccaf9f0bc5d16`, present in the prepared base. Its 28 source hashes match the tracked final manifest, aggregate `af34fbd9753f1d09c0c618ef90623dd64295ccbd8d5fd98b4f74872e06184ef8`. | Satisfied for implementation. Keep its recorded normal-relaunch, OS pointer, spoken VoiceOver and live-tailnet limitations separate. |
| Complete task scope / comment correction | Original six-row task plus coordinator's 2026-10-14 reference correction. | Reconciled here; retain the real after-body Show full control. |
| Native fixtures / input | Existing local/G2/Diff material; newly authorized real summary comment; one coordinated input slot. | Source work can proceed after review; native eligibility/height and G2 variants still require qualification. |
| Browser and CSS follow-ups | Original task explicitly precedes `browser-panel-parity` and `css-workaround-retirements`. | Wait for this PR's actual merge. |

The serial order remains dialog → legacy sidebar → connections → **this task** →
browser → CSS → timeline/window hooks → contract keys/menu bar. The coordinator
created the isolated checkout after verifying actual predecessor merges. Remote
feature readback was `5e5bdb094439d155543f5eb1111eea0ccb3abc76` at preparation.
Approved, closed-unmerged or merely locally integrated PRs do not satisfy these gates.

Issue assessment, source and read-only remote checks on 2026-10-14:

| Issue/reference | Capability / target | Evidence | Impact | Next action |
| --- | --- | --- | --- | --- |
| X66 / #319 | Public popover commands on macOS | `issues/closed/20261009-popover-action-commands.md`, plan-local closed X66 record, upstream #319 CLOSED; fix `62469206a129eafb227b805823cf10607f408b64` is in the base. | nonblocking | Use explicit source plus ordered show/focus and hide; qualify the app's native behavior. Remaining UIKit navigation-bar lifecycle limitations are outside this macOS task. |
| X54 / #283 | Bubbling focus events on macOS | `issues/closed/20261009-bubbling-focus-events.md`, plan-local closed X54 record, upstream #283 CLOSED; fix `1bcf7436f54ed8e58349386f44ee7de17b059576` is in the base. | nonblocking | Follow current nested/removal semantics and verify boundary membership. Older partial-resolution prose is superseded by the record's latest resolution. |
| X73 / #446 | Popover Escape after blur | `issues/closed/20261010-macos-popover-escape-after-blur.md`, plan-local closed X73 record; PR #446 MERGED into main at `ccc9afd52a703ed7b068f9e4d90ad313378c292c`, present in the base. | nonblocking | Delete the obsolete Filters resting box and prove two Escapes in this application. #401 published the issue record; it is not the runtime fix. |
| PR-list / public field selection | Selection and reopen focus | First-wave `6ff18f2251305c8f3f30a6ace8e425eab001367f` is present. FieldOpenSelect owns focus and selection; the obsolete R9 caret observer is gone. | nonblocking | Preserve checkout/rename select-all and PR-list/thread-picker focus. |
| Cite / IME repairs | Root key capture and native composition | `706f3d3f44a718d19f90996e82258718ec6e3e49` and `ce8cc79bb16f0caa3d7372a7818ffa51b6f031f9` are present. | nonblocking | Keep Cite capture/save/dismiss and IME behavior intact; menu keys must not cause global Send. |
| Three serial predecessor PRs | Safe application implementation base | #553/#557/#566 MERGED into the feature branch; their actual merge commits are ancestors of prepared `5e5bdb094`. | nonblocking | Preserve their source and qualified behavior, retaining separate unverified cases. |

No required unresolved framework capability was found by source inspection. That is
not a native runtime pass. A contradictory runtime result must become a precise
plan-local issue assessment; do not invent a DOM API or edit the framework. Relevant
local issue bodies, `EXACT2-GAPS.md`, and kept-workaround records retain historical
"not adopted" claims; current resolution, code and ancestry take precedence.

Read-only duplicate searches on 2026-10-14 found no active/closed ticket with this
task name and no PR with head `feat(example)/t3-code-menu-keys-popover-commands`.
A title/body search returned only historical merged framework/adoption records
(#236, #314, #328), not an earlier implementation. Reuse this checkout and ticket.

## Implementation notes

### Public API and focus ownership

`showPopover(id, source=triggerId)`, `hidePopover(id)` and `togglePopover(...)` are
public action commands. Source is explicit, not inferred from the callback. A trigger
needs its own `id`; `testId` alone is not an invoker ID. Show/hide are idempotent, and
commands run in action order. Open and then select the first/last real enabled row in
the same action. AppKit native menu tracking can honor that opening focus request; do
not depend on changing its highlight later while tracking is already running. Invalid
target/source commands are refused in the journal, so inspect logs during proof.

`toggle=changed` supplies `ToggleEvent.oldState/newState` (`open` or `closed`). It is a
coalesced logical visibility transition, not an input-modality signal. Use it for the
snooze open pin and Filters final focus. Remove visibility guesses based on descendant
focus; moving from one menu row to another must not momentarily release a snooze row.

`focusin`/`focusout` bubble and add no Tab stop. `FocusEvent.target` is the focused
element's own ID or `""`; `relatedTarget` is `some(otherId)` or `none`. Moving between
two descendants still emits focusout/focusin. On removal a detached wrapper can hear
its own focusout, but disconnected former ancestors do not. Blur occurs before
post-commit/autofocus transitions. Do not close a popup on every focusout, and do not
clear focus-within while the next target remains in the same live boundary.

Use stable owned IDs, an explicit eligible-ID sequence, or the documented restricted
`querySelectorAll('[data-word~="value"]')` mechanism to test boundary membership.
There is no DOM `contains`, `closest`, `activeElement`, or generic `:enabled` API.
Selector results are mounted/displayed/non-inert own IDs and do not automatically
exclude disabled controls. Give previously unnamed relevant controls stable IDs;
handle absent/empty related targets deliberately. Preserve a genuine new outside
focus instead of always restoring the opener on an outside-focus dismissal.

### Shared migration inventory

At prepared `5e5bdb094`, there are **38 `KeyMenuOpen` call sites in 28 files**, **38 KeyMenu
calls plus one KeyMenuWatched**, and **47 direct `focus(...-km)` redirects in 31
files**. Two loops in the shared helper instantiate the `KmDoor` component for the
menus' item sequences. Earlier counts of 69 textual references and 48 redirects
included a different counting surface; use executable call inventory below. The
post-merge recount matches the following per-file baseline.

Every file name below is relative to `examples/t3-code/`; numbers are call sites, not
runtime instances. The opener table covers all 38 calls:

| Family | `KeyMenuOpen` callers |
| --- | --- |
| Browser (5) | `browser-defaults.contract` (1), `browser-profiles.contract` (2), `browser-surface.contract` (2) |
| Connection / legacy (3) | `connections.contract` (2), `legacy-sidebar.contract` (1) |
| PR (8) | `pages-prs.contract` (3), `pages-pr-actions.contract`, `pages-pr-code.contract`, `pages-pr-handoffs.contract`, `pages-pr-quick.contract`, `pages-pr-stack.contract` (1 each) |
| Settings (12) | `settings-a-hosts.contract`, `settings-b-kit.contract`, `settings-keybindings.contract`, `settings-kit.contract` (1 each); `settings-projects.contract` (2), `settings-rows.contract` (4), `settings-scheduled.contract` (2) |
| Composer / other (10) | `pages-hero.contract`, `pages-usage.contract`, `pages-usage-prices.contract`, `providers-setup.contract`, `r4-git.contract`, `r4-surfaces.contract`, `r7-device.contract`, `sidebar-row.contract`, `snapshot.contract`, `timeline-plan.contract` (1 each) |

Retiring KmDoor also reaches owners that do not use KeyMenuOpen. Its complete direct
redirect inventory is:

| Family | Existing `focus(...-km)` callers |
| --- | --- |
| Browser (5) | `browser-defaults.contract` (1), `browser-surface.contract` (4) |
| Connection / legacy (3) | `connections-routes.contract`, `connections.contract`, `legacy-sidebar.contract` (1 each) |
| PR (14) | `pages-prs.contract` (5), `pages-pr-actions.contract` (3), `pages-pr-code.contract` (2), `pages-pr-handoffs.contract`, `pages-pr-links.contract`, `pages-pr-quick.contract`, `pages-pr-stack.contract` (1 each) |
| Settings (7) | `settings-b-kit.contract`, `settings-kit.contract`, `settings-projects.contract`, `settings-scheduled.contract` (1 each), `settings-rows.contract` (3) |
| Other (18) | `pages-usage.contract` (2), `r4-surfaces-files.contract` (2), `r4-surfaces.contract` (3); `diff.contract`, `markdown.contract`, `pages-hero.contract`, `r4-git.contract`, `r6-device.contract`, `r6-polish.contract`, `requests.contract`, `shell-details.contract`, `sidebar-row.contract`, `snapshot.contract`, `timeline-plan.contract` (1 each) |

Replace hover-to-door redirects with focus of the actual row and public bubbling focus
observation where the parent needs the current row. Keep one highlight; arrows after
pointer hover must continue from the actually focused row. Preserve wrapping,
Home/End, label search, disabled-row skipping and modifier guards. Delete the old
helper declarations and all callers together. Do not rename the workaround and retain
its hidden controls. Keep legitimate first/last opening behavior for state-owned menus
if its public focus targets remain necessary; distinguish that behavior from per-row
KmDoor and over-trigger KeyMenuOpen nodes.

Preserve non-popover menus owned by application/data state: chat title/submenu,
approval choices, Details editors/scripts, Diff scope/turns, Device text/rotate/more,
Files crumbs/editors, linked PR actions and table-copy menus. They still need their
existing opening, modal scope, initial focus and dismissal policy; do not blindly call
show/hidePopover on a state-owned ID. Browser appearance hover opening belongs to the
later browser task; this PR only migrates its existing shared-menu behavior.

### Migration constraints at the prepared base

Thirteen real triggers lack own IDs. Existing test IDs locate them: browser-more and
surface-browser-profiles (browser-surface); empty-choose-project (pages-hero);
pull-request-checkout (pages-pr-handoffs); `${popId}-trigger` (pages-pr-quick);
pull-requests-sort/provider/filters (pages-prs); settings-scope-project/environment
and `setting-${row.id}-effort` (settings-rows); `snooze-${t.id}` (sidebar-row); and
snapshot-sound (snapshot). Give each a stable unique own ID. Snooze's test ID already
identifies its popup, so use a distinct button ID such as `snooze-trigger-${t.id}`.
Preserve existing environment-qualified scheduled-task IDs and section identity.

Real triggers handle unmodified ArrowDown/Up, Enter and Space, prevent default and
propagation, and guard repeat. Preserve pointer/keyboard preparation exactly once:
profile loading, Filters reset, quick-stack/Details Git loading, Legacy placement
and Snooze tooltip dismissal. Retain pointer popovertarget. Reuse the popup's eligible
KmItem sequence; no second ordering/registry. With no eligible item, omit an invalid
empty-ID focus. Delete old imports, armed state and callbacks with their helper.

KeyMenu focusin updates current only for its own member row ID, without redirecting
focus or stopping propagation. Ignore nonmember root events so nested opening focus
is not overwritten. Preserve state-owned root focus and kmOpenTarget/kmEndKeyed
wrappers. Public popover owners can use keyed=0 once the trigger owns initial focus.
Action reads use the starting snapshot: compute a target locally rather than reread
just-written keyed. AppKit initial focus must name an enabled AccessibleMenuItem
before tracking starts; a first/last forwarding box or delayed toggle cannot replace
that request (MenusMac.swift opening-focus and deferred-tracking paths).

Snooze becomes KeyMenu; actual popup toggle pins from newState=open, combined with
existing thread hover. Remove parent pin callbacks driven by descendant focus/pointer
transitions, retain row highlight, and delete KeyMenuWatched once unused. Every close
releases the pin. For Legacy clamp, prefer preserving number-input identity with
explicit draft reconciliation, or qualify removal/replacement without transient exit.

Preserve #553 CnMenu/SkPopup, both Details Git owner handlers, composer branch-only
menuEntered and the admitted dialog-focus/focus-stop/dialog-return data words.
dialogMenuEntered intentionally stops at that owner before the window clears lineage;
do not move this stop into KeyMenu. Keep Connections' actual route grip button,
stable ID, tabindex, key/blur handlers, empty title, appearance=none and transparent
styling. Root-owned route transactions and Change.routeReorder/Change.relaunch remain
intact; only RouteMenuItem's existing focus-door redirect changes there.

The collapsing PR owners are PrdCommentCard and PrdCollapsedComment in
pages-pr-summary.contract. PrdDescription and code-thread bodies use collapse=false;
neither qualifies body expansion. Anonymous Markdown links may have no own ID;
do not infer remark membership solely from an ID prefix.

### Specific owners

**PR Filters:** `PrFiltersMenu.closeSub` currently focuses `pr-filters-rest` when
`subLeft` is true, otherwise the submenu trigger. Replace the former branch with
`blur()` and delete the resting box; keep return to `pr-filter-${sub}` where the
reference keeps the pointer/keyboard highlight. Preserve `subEscape` and search
Escape consuming the first Escape without closing the parent, ArrowLeft return, and
the next ArrowDown continuing from the submenu trigger. Add own ID
`pull-requests-filters` to the real trigger, and focus it from the popover's closed
toggle event. Qualification includes submenu pointer exit, search focus, keyboard
opening, first Escape, second Escape and native ring visibility.

**Legacy options:** the five sort rows are already `menuitemradio`; do not claim this
as missing. Visible threads and its number-field controls sit inside the popup but
outside KeyMenu. Observe the whole popup boundary, including those controls. Moving
among rows/field/buttons must keep it open; a genuine move outside hides it without
stealing the new focus. Clamping the value to 1–15 recreates its keyed `LegacyCount`;
test this removal/replacement path so it does not count as leaving the popup. The
historical `legacy-sidebar-full` ticket uses LS-3 for file drop; this task's LS-3 is
specifically options focus dismissal, not that predecessor's LSF-3/file-drop work.

**Composer:** the workspace branch of `ComposerMenus` currently counts only 2/3
Workspace choices and gives Run on buttons no IDs in its arrow order. Build one eligible `KmItem[]` sequence in
rendered order: Run on choices, then current checkout/worktree, new worktree, and
previous worktree when present. Skip disabled current checkout under forceWorktree;
handle gitless/environment-only, no environments, previous worktree and auto-balance
choices. Enter sends the existing command once and returns to `composer` (Message)
after the selection closes. Follow `composerEventScope.ts`'s final-focus exception
when a deliberate outside focus already exists. Preserve wide/narrow Select/menu
semantics; do not flatten unrelated runtime/effort/context/branch menus into this list.

**Combobox Return:** `R4BranchPicker` currently hides its body after `git.branch.picked`
while the native popover stays open; its search submit action must close the actual
`details-branch-menu` when a Return pick is accepted. `DiffBasePicker` similarly uses
`pickedAt/session` to remove its body; call `hidePopover` on its actual popup for a
`pick` or `close` outcome from `dbReturn`; only `pick` dispatches a changed base.
The existing `stay` outcome, Automatic under a query, remains open. A missing valid
row, including a zero-match query, closes without dispatch. This preserves the
accepted Diff parity task's recorded reference observations at lines 154–156.
Git branch Return retains its separate empty-`enterOp` guard. Keep the
existing command, chosen value, query/session reset, trigger return and pointer-row
selection. The composer's branch picker is state-owned; preserve its existing close
path rather than applying a command to an unrelated popup ID. IME composition must
not become an accidental Return selection.

**PR comments/remarks:** put expansion on the clipped body wrapper's `focusin`, not
on an extra focusable proxy and not on the after-body toggle. Tab reaching an actual
Markdown link expands the complete body before the focused content is hidden. Use
remark-boundary focusin/out to keep the pencil visible across its descendants and
hide it once both pointer and focus leave. Preserve the existing editing Escape hold,
Write/Preview controls, save refusal and Cmd+Enter keyup cleanup. A bubbling change
must not leak a stale `pr-escape-hold`, submit a composer message, or interfere with
Cite's root capture. No actual PR comment write is needed to verify these behaviors.

Preserve PR #553's menu-owner `dialogMenuEntered` handlers and `data-dialog-return`
markers when adopting bubbling focus events. A row focus event must reach the owner
so a later menu command can return its dialog to the actual invoker. Keep this
boundary as checked against the prepared post-connections source. Preserve
Connections' root-provided `RouteKeyboard` and its participation in Settings'
`escapeOwned`, plus the Remove-route dialog's `aria-modal=true`. Registered native
shortcuts run before the focused key handler, so a component-only Escape test cannot
prove Settings stayed open. Include the registered shortcut owner in the actual-plan
assertions and qualify the full native Escape sequence.

## Acceptance and reproduction

After coordinator review, use the repository-pinned Bun (1.4.2), current compiler,
current bundle and an existing isolated fixture. Essential fixture constraints are
included below; no machine-local scratchpad or draft is a required dependency.
Record actual OS/build, Keyboard Navigation setting, source/framework SHA,
fixture/reset commands and environment names. Do not record credential values.
Several test bundles share `com.exact.t3code.macos`. During Connections qualification,
Orca app activation launched an older registered checkout even when the input targeted
a recorded PID and the current app had been opened by absolute path. PID selection
alone did not prevent this. The controlled agent standalone launch supported attended
input successfully; use that established isolated path and verify the actual executable
before input. Preserve the normal-launch activation failure as a tooling qualification
limit. Do not repeat the failed normal-launch sequence or treat it as an app failure.

Extend existing behavior coverage; source guards are wiring checks, not native proof.
`menu-keys.test.ts` currently mandates the hidden invokers, doors and Filters resting
box. Replace those assertions with real command-order and focus-boundary assertions;
do not merely delete the coverage. Reuse `contract-action-fixture.ts` to execute real
Contract actions/derives where applicable; do not create another interpreter. Preserve
the exhaustive row-ID/owner checks while updating the implementation they protect.

| Acceptance | Existing test targets and required observations |
| --- | --- |
| KW-5 / shared menus | `menu-keys.test.ts`: ArrowDown/Up opens first/last enabled row with explicit source, repeat open is safe, Enter/Space behavior retained, modifiers excluded, hover→arrow follows actual row, wrap/Home/End/typeahead, no KeyMenuOpen/KmDoor callers. Snooze pin remains through descendant focus changes and releases on close. |
| X73 / R10-1 | `menu-keys.test.ts`, `pages-prs.test.ts`, `pages-pr-logic.test.ts`: submenu/search first Escape closes only child; pointer-left branch blurs; second Escape closes parent and focuses real Filters; ArrowLeft and subsequent row movement retained. |
| LS-3 | `legacy-sidebar.test.ts`, `legacy-sidebar-full.test.ts`: internal relatedTarget keeps open; outside/none closes under the qualified host policy; number-field clamp/remount does not spuriously dismiss; Tab exit preserves destination focus. |
| G2-15 | `composer-controls.test.ts`, `composer-controls-branch.test.ts`, `composer-workspace-snapshots.test.ts`, `r4-composer-overlay.test.ts`: combined order, wrap both ends, forceWorktree skip, gitless/previous/auto-balance variants; one selection/close/final-focus action. |
| Return dismissal | `r4-git.test.ts`, `diff-panel-parity.test.ts`: Git accepted Return hides; Diff `pick` hides and dispatches once, `close` hides without dispatch, and `stay` remains open. No empty overlay; reopen shows rows and correct query/selection state. |
| KW-12d | `pages-pr-writes.test.ts`, `pages-pr-writes-logic.test.ts`, `usage-pr-pages.test.ts`: focus on a body descendant expands without writing; after-body toggle remains; pencil holds across descendants; Escape/save behavior preserved. |
| Preserved first-wave behavior | `pr-list-place-and-thread-picker.test.ts`, `cite-selection-anchor.test.ts`, `diff-cite-popover.test.ts`, `dialog-trap.test.ts`: selection on reopen, thread-picker focus, Cite Tab/Escape/save and inherited dialog focus behavior. |

### Portable fixture prerequisites

Use existing procedures in the dialog/legacy/Connections task records and the closed
Diff/usage tasks. Obtain a coordinator handoff or copy needed disposable data into
this task's private area. Resolve current runtime/bundle paths and unused ports;
do not run historical scripts with fixed homes, ports, PID files or binaries. Do
not copy old identities, credentials, logs or databases wholesale. No new server implementation,
harness or replacement fixture infrastructure is authorized.

| Case | Prerequisite and readiness |
| --- | --- |
| PR Sort/Filters, Snooze, state-owned menu | Existing local smoke flow creates/registers a disposable Git project with main/feature and a local thread without a provider turn, then completes Welcome. PR controls render with no PR rows: open PR overview → Filters → Author or State. Use current thread IDs for Snooze and Details editor or Diff scope for state-owned behavior. Historical trees establish reachability only. |
| Legacy options | Same project/thread; Settings → General → Legacy features → Sidebar (legacy), then legacy-options and legacy-preview-count. Prefer the predecessor's final fixture handoff. Exercise all number controls and real Tab exit. |
| Combined Run on/Workspace | Reuse three independent existing project/server copies plus isolated primary project. All need the same canonical origin and connected/synchronized generations. Use a new unstarted draft and narrow composer; inspect actual envOptions first. Auto-balance needs offered state and at least two eligible environments. Reapply previously restored primary-origin changes only to a task-owned copy when needed. |
| G2 variants | forceWorktree requires valid two-model selection. Previous worktree needs an eligible nonarchived same-project thread with a different nonempty worktree path and valid updatedAt. Native prerequisites remain unestablished; use existing No project UI for gitless where available. Do not fabricate records or start agents. Keep domain/native coverage distinct. |
| Git Return | Select in the disposable project or already configured new-worktree draft; use real details-branch-search, accept and reopen. |
| Diff Return | Copy established work repository and bare origin as a pair, retaining main, feature/audit and origin/main, origin/feature/audit, origin/release/2026-10 refs; retarget origin paths only in copies and register the copied root through existing setup. Require branch-range selection, nonempty baseRef, diffCompare=true and mounted diff-base. A dirty file alone is insufficient. The coordinator has prepared and registered this copied pair and an unstarted feature/audit thread on the same isolated primary backend; use that handoff instead of duplicating setup. |
| Long summary remark/pencil | User explicitly authorized one fixture. Coordinator created and GET-verified [PR159 summary comment](https://github.com/daehyeonmun2021/playground/pull/159#issuecomment-6166089478): primary-owned, 1,034 characters, 25 newlines, 18 ordered-list items and two Markdown links. Use the approved primary lane; verify rendered height exceeds 240 points, real link focus and canEdit in the app. Opening/canceling edit is allowed; no additional posts/edits. Native eligibility/height remain unverified. |

Agent-native credentials, preferences and route catalogs are process-local even with
named module storage. Use fresh one-use pairing and keep related sequences within
one app lifetime. Keep secrets private. The user unlocked the screen and authorized
temporary keep-awake through all work; the coordinator owns that process and one
native-input slot. Coordinate launch/input and verify the actual executable. The
coordinator prepared the isolated primary GitHub lane with the existing lane tool;
its server, wrapper resolution and project setup passed. Obtain its owned-process
receipt before use and do not start an overlapping server. Mint fresh pairing through
that same lane environment without printing the URL; cleanup remains its owner's
responsibility until explicitly handed off.
Normal preferences-writing launches require the recorded snapshot/restore procedure
and no competing writers. Stop only owned PIDs.

EXACT_AGENT=1 paints menu popovers instead of using normal AppKit NSMenu tracking
(MenusMac.swift). Physical keyboard input to that process qualifies only its agent
presentation. Keep normal NSMenu opening/highlight qualification distinct; coordinate
an independently verified activation path before claiming it passed. Do not repeat
the failed normal activation sequence. Source-equivalent predecessor binaries may
support BEFORE observations when the coordinator establishes identity; AFTER needs
the implementation build.

The existing Contract action helper does not execute every match/named-host-argument
form. Extend it narrowly only where needed; source assertions remain wiring checks.
Include connections-routes.test.ts and network-routes-followups.test.ts alongside the
focused tests below to preserve the real grip, root transaction, registered Settings
Back Escape owner, Remove modal boundary and return focus.

### Development and gate commands

Focused development commands from the repository root, after the dependencies pass:

```sh
bun install --frozen-lockfile
bun test ./examples/t3-code/menu-keys.test.ts ./examples/t3-code/pages-prs.test.ts ./examples/t3-code/pages-pr-logic.test.ts ./examples/t3-code/legacy-sidebar.test.ts ./examples/t3-code/legacy-sidebar-full.test.ts --timeout 60000
bun test ./examples/t3-code/composer-controls.test.ts ./examples/t3-code/composer-controls-branch.test.ts ./examples/t3-code/composer-workspace-snapshots.test.ts ./examples/t3-code/r4-composer-overlay.test.ts ./examples/t3-code/r4-git.test.ts ./examples/t3-code/diff-panel-parity.test.ts --timeout 60000
bun test ./examples/t3-code/pages-pr-writes.test.ts ./examples/t3-code/pages-pr-writes-logic.test.ts ./examples/t3-code/usage-pr-pages.test.ts ./examples/t3-code/pr-list-place-and-thread-picker.test.ts ./examples/t3-code/cite-selection-anchor.test.ts ./examples/t3-code/diff-cite-popover.test.ts ./examples/t3-code/dialog-trap.test.ts --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/t3-menu-keys.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Use explicit `./examples/t3-code/menu-keys.test.ts`-style paths so Bun does not also match copied baseline
tests under target. Run the full application Bun suite once before handoff. If native
hooks are unexpectedly touched, run their existing affected Swift tests (including
`macos/tests/r9-input/main.swift` and `macos/tests/r10-connect/main.swift` when relevant)
through the existing application test recipe; new hooks are outside this scope.
Current binding repository gates include build, test, lint (both clippy and fmt),
caps, boot, and workspace. Read current AGENTS/RULES for exact flags; the current
commands are:

```sh
cargo build --all-targets --keep-going
cargo test --lib --bins --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings
cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs
cargo check --workspace --all-targets --keep-going
```

Stage only this task's complete app changes in its isolated checkout before caps.
The coordinator may provide unchanged integrated gate evidence when source identity
covers the PR; do not multiply broad runs after a pass without new changes or a
remaining concern. App build and native use remain necessary even with green Cargo.

### Bounded before/after native recipe

Use one existing agent drive per menu kind before/after and one coordinated real-input
session for the cases the driver cannot prove. Build the actual source being tested;
do not bypass stale-artifact refusal. Use existing `scripts/agent.mjs macos` with
`EXACT_APP_DIR`, an isolated `--storage` value and `--size 1280x840`; read `tree`,
`tree --ax`, `state` and `logs` at each boundary and settle through observed state or
`clock settle`. Record the exact commands, mounted IDs and actual focus sequence.

The user unlocked the screen on 2026-10-14 and authorized temporary keep-awake
through the end of this work. The coordinator owns that process and schedules one
OS-input/native slot at a time. Use that slot for physical Tab destinations, visible
focus rings and VoiceOver delivery; they remain unverified until captured. A normal
preferences-writing launch requires the existing complete snapshot/restoration recipe
and exclusion of concurrent app writers. A named agent store preserves module files,
but this app's native credentials, preferences and saved-route catalog stay in process
memory in agent mode. Pair fresh one-use credentials and complete each related menu
sequence in one app lifetime; do not reuse consumed pairing links or claim normal
cross-process persistence from named storage. The authorized real summary comment
is available; prove its native height and edit eligibility before claiming KW-12d.

1. Keyboard-open one ordinary popover (PR Sort or Settings select), one nested menu
   (Filters), one snooze menu and one state-owned menu (Details editors or Diff scope).
   Check ArrowDown/Up first/last, hover then arrows, wrap, Enter and Escape. Snooze
   actions remain visible while open and release after actual dismissal.
2. Filters: open a submenu by keyboard, focus its search where applicable, move the
   pointer out, press Escape twice. After first Escape record child closed/parent
   open and the appropriate row/blur state; after second record parent closed and
   `pull-requests-filters` focused. With the actual key window and physical keyboard
   path, capture the visible ring on Filters. Repeat the keyboard row-return branch
   without pointer exit. No filter write beyond isolated local UI state is needed.
3. Legacy options: move among its radio rows and Visible threads field/buttons;
   change/clamp the fixture value, then Tab and Shift+Tab across its outside boundary.
   Internal transitions remain open; an outside focus closes and stays at its target.
   Real Tab from the number field is required for LS-3.
4. Combined Run on/Workspace: use the existing multi-environment draft fixture and
   a narrow composer. Arrows visit every eligible Run on and Workspace row in order
   and wrap; Enter picks once, closes and focuses Message. Cover forceWorktree and
   one gitless/wide variant using existing fixture state. Do not send a message.
5. Git branch and Diff base: using a disposable fixture repository, accept a valid
   Return selection, observe popup closed (not an empty rectangle), then reopen and
   select again. In Diff, Automatic under a query stays open; a zero-match query
   closes without changing the base. Preserve the Git picker's own no-op rule and search
   composition. Record any unavailable fixture instead of using a personal checkout.
6. Long PR remark: Tab onto a link within its clipped body, observe expansion and
   pencil visibility; move between its descendants, then outside. Verify the real
   after-body Show full/Show less control, including its pointer behavior. Open and
   cancel edit to check Escape/save-chord isolation without posting anything.
7. Spot-check one minimum-window case and dark appearance for focus/expanded content.
   Recheck checkout or rename select-all and Cite Tab/Escape after shared focus changes.
   Capture only the controls needed to prove these behaviors; no repeated pixel matrix.

Do not manufacture a pass by sending `type <focusable-trigger> key Escape`: the agent
can focus that target before sending the key and thereby conceal broken return focus.
Read focus before the key; route it through the actual keyboard focus, or the existing
nonfocusable app root where the driver supports that, and record the path. Logical
focus in a non-key agent window does not establish a visible native focus ring.
Native input, first responder, Keyboard Navigation and ring evidence remain distinct
from Bun action tests. Record any unexecuted case as unverified.

## Progress

| Stage | State |
| --- | --- |
| Scope and callers | Reconciled on prepared `5e5bdb094`; six rows, 38 openers and 47 redirects retained. |
| Public APIs / issues | Source checked; X54/X66/X73 fixes present; remote issue/PR states refreshed. |
| Serial dependencies | #553/#557/#566 actually merged into intended base; actual merge ancestry and final Connections source hashes pass. |
| Prepared checkout / ticket | Isolated Orca branch at `5e5bdb094`; this portable tracked-path ticket ready for coordinator review. |
| Fixture / native readiness | Local, core G2 and paired Diff data available; real authorized summary comment available, native height/edit eligibility unmeasured; G2 variant prerequisites still need qualification. |
| Implementation, tests, build, app drive, real input | Not performed by this preparation. |
| Verification / publication | Unverified / none. |

## Attempts and evidence

Preparation on 2026-10-14 reconciled the six rows, parent spec/plan/continuation,
current helpers/callers, Filters, legacy options, composer choices, branch/base
pickers, summary remark owners, existing tests and retained failure records.
Reference reads used immutable `git show 1e2ecbd9758830669684b494d4398f626b0576e0:<path>`.
The accepted Diff behavior is in `tasks/closed/20261009-diff-panel-parity.md:136–163`,
especially 154–156: queried Automatic stays, a lit ref picks, no selected row closes.
Pinned DiffPanel/combobox wiring corroborate that setup; behavior is historically
recorded, not newly run. Installed guidance metadata and current grammar/issue
resolutions were checked; repository API authority follows U1.

The initial draft/preflights inspected `a218ee78f`, `1025adf78` and `a2492229a`.
Promotion rechecked clean prepared base `5e5bdb094`, inventory and issue status.
Read-only GitHub GraphQL returned all eleven first-wave PRs, #549 and #553/#557/#566
MERGED into `feat(example)/t3-code`, #446 MERGED into main, and #319/#283 CLOSED.
Every resulting merge commit and these framework/application fixes passed
`git merge-base --is-ancestor <sha> HEAD`: `178ae7237`, `62469206a`, `1bcf7436f`,
`ccc9afd52`, `6ff18f225`, `706f3d3f4`, `ce8cc79bb`. All 28 files match the tracked
`evidence/20261014-connections-network-settings/source.json`, aggregate
`af34fbd9753f1d09c0c618ef90623dd64295ccbd8d5fd98b4f74872e06184ef8`.
Exact-head PR search returned none; title/body matches were historical records only.

The coordinator's read-only approved-sandbox search found no tall candidate among
all five issue comments and 19 reviews across 17 seeded PRs. The user then explicitly
authorized creating one fixture; the coordinator posted and independently read back
the single real PR159 comment linked above. Body/author matched; native height and
edit eligibility remain unverified. This supersedes the earlier missing-data gap.

Preparation wrote only this ticket and checked structure, references, whitespace,
complete scope and source/branch identity. It did not change behavior, run tests or
builds, launch apps, supply native input, create fixtures, message providers, write
GitHub state, commit or push. Coordinator fixture work is attributed above. Source
checks establish availability and identity, not behavior. Future attempts must record
source/framework SHA, fixture/reset, command, assertion, actual result and artifact
paths. Preserve failures. The repository permits three repair rounds; changing worker
or test variant does not reset that limit.

## Next action

Coordinator reviews this prepared ticket, then authorizes the complete six-row
implementation in this branch. Settle the shared real-trigger/real-row API first,
migrate every opener and door caller preserving dialog owners, then finish the six
specific behaviors and existing tests. Use the coordinated native slot and fixture
prerequisites for the bounded app pass; record unavailable cases as unverified.
Publication/merge remain coordinator-owned. Browser and CSS follow-ups wait for
this task's actual PR merge.
