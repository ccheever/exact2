---
name: 20261014-css-workaround-retirements
plan: 20261005-t3code-macos-parity
implementation: complete
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-css-workaround-retirements
pr_url: https://github.com/ccheever/exact2/pull/577
verified_commit: null
---

# CSS workaround retirements

## Outcome

Complete serial follow-up 6 of 8 in one application PR. Retire the original
`css-workaround-retirements` task's five rows, KW-1, KW-2, KW-4, KW-6 and KW-7,
using CSS capabilities already adopted from main. Preserve the pinned reference's
appearance and interaction. The task is blocked on the five preceding task PRs
actually merging and preparation against the refreshed feature branch.

| Original row | Required result |
| --- | --- |
| KW-1 / X10 | Apply `text-wrap="balance"` to the shared tooltip, popover-tip, empty-state and toast families where the reference writes `text-balance`. Author the reference's placeholder colors for each input/textarea family through `::placeholder` part blocks. Account for all 108 placeholder-bearing source lines in the inspected baseline. |
| KW-2 / X57 | Retire the four historical left-alignment overrides at the base freshness mark, filter submenu value, right-panel tab title and palette option rows. Compare a short and long value at every site before accepting removal. Preserve actual reference alignment if inspection contradicts the historical candidate list; an unresolved contradiction blocks this row. |
| KW-4 / X65 | Remove `usage-ground` and receive empty-scroll-port presses through the public scroll/root event path. Preserve the pooled Limits popover's intentional outside-press behavior and the window's outside-press accounting. |
| KW-6 / X17 | Audit all 62 popovers. Replace `position-try-fallbacks="none"` with the appropriate physical flip candidates wherever Base UI flips. Retire fixed geometry used only to fake a flip when public CSS covers it. Keep authored presentation and retain `hoverFlip` while the required hover behavior remains outside main #112's implemented slice. |
| KW-7 / #287 | Replace the two stroked dashed SVG rectangles with 1 px dashed borders on the unresolved context chip and pending review comment card. Match the reference's outer size, content inset and hit targets after borders enter layout. |

Original proof remains required: Bun source guards, Contract build, one bounded final
agent drive at 1280x840 in light and dark, with before / after / reference captures
for a long tooltip, a placeholder, each alignment site, bottom/right-edge popup
placement, the unresolved chip and pooled Limits ground dismissal. Real-input proof
must cover a bottom-edge popup and the pending card on the real-GitHub lane. Current
preparation supplies source evidence only; it does not satisfy any runtime row.

## Scope and exclusions

Target is the existing macOS AppKit application on Apple Silicon, with macOS 14 as
its declared minimum. Record the actual OS and build used for proof. Preserve the
840x620 minimum layout, but use the authorized 1280x840 proof pass rather than
restarting a pixel-polish matrix.

All implementation changes stay under `examples/t3-code/`. Ownership includes the
Contract families and exact sites below, existing Bun tests, and the task-owned
plan/gap records. Use shared styles/components for repeated color and wrapping
policy. Do not produce 108 unrelated inline color patches. Several listed lines
forward a component prop or paint a placeholder-like label and cannot take a
pseudo-element block themselves.

Excluded work is framework/compiler/runner/host changes, new native hooks, font
smoothing, unsupported anchor CSS, blanket native-popover conversion, replacement
of the hover layer, menu-key command migration, dialog-focus work, legacy-sidebar
behavior, network settings, browser behavior, timeline/window hooks and global
key/menu-bar migration. Their serial owners must land first. Preserve their public
commands, focus restoration, selection and close behavior while changing styles.
No new debug endpoint, fixture server, native driver or test apparatus is authorized.

## Context and guidance

Repository-relative plan root is
`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`.
Read `spec.md`, the current continuation and implementation order in `plan.md`,
`reviews/20261014-first-wave-resume.md`,
`tasks/20261013-r12-kept-workarounds.md`, and
`tasks/closed/20261014-adopt-main-fixes-r14.md` before implementation. Application
`EXACT2-GAPS.md`, `STATUS.md`, `README.md` and `AGENT-HANDOFF.md` retain prior
workaround and lane evidence. Historical statements that a CSS row is unsupported
must be reconciled with the adopted framework, not copied into new code.

The original coordinator entry is fully represented by the five outcome rows and
proof requirements here. Its machine-local `fix-wave-plan.json` is provenance, not
a prerequisite for using this ticket. The authorized integration branch is
`feat(example)/t3-code`; older `daehyeon/t3-code` prose is historical.

Source inspection baseline is `a218ee78fec42b1a4590b199297f30667876de67`.
It contains first-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`,
round-14 merge `3710af842bc54521c298124156989950b6f2e68c`, and main
`178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`. This is not the eventual task base
or a verified implementation. The inspection checkout was
`/Users/daehyeonmun/orca/workspaces/exact2/t3-code-fix-wave-integration`;
that path is an environment note only.

Reference repository is `https://github.com/pingdotgg/t3code`, read-only pin
`1e2ecbd9758830669684b494d4398f626b0576e0`, confirmed during preparation.
Reference paths below start at `apps/web/src/components/`:

| Concern | Reference source |
| --- | --- |
| Balance | `ui/tooltip.tsx:112`, compact `ui/popover.tsx:90`, `ui/empty.tsx:22,115`, `ui/toast.tsx:731` |
| Field colors | `ui/input.tsx:25`, `ui/command.tsx:106`, `ui/sidebar.tsx:541`, `GitActionsControl.tsx:830`, `settings/ThemeImportDialog.tsx:127`, `chat/AssistantCitationCommentEditor.tsx:54`; `ui/select.tsx:96` is a value-label state, not a native placeholder |
| Color roles | `apps/web/src/index.css:248,1085,1252,1549` maps placeholder through contrast/theme roles. Use actual reference values for the selected theme/contrast; do not assume placeholder equals text color times a fixed alpha. |
| Four alignment sites | `pullRequest/PullRequestDetailPanel.tsx` / `PullRequestBaseFreshnessWarning`, `pullRequest/PullRequestListFilters.tsx:227-233` / `MenuSubTrigger`, `RightPanelTabs.tsx:1185-1197`, `CommandPalette.tsx` plus `ui/command.tsx` |
| Border boxes | `ContextChip.tsx:25,51` and `pullRequest/PullRequestReviewAnnotation.tsx:34,66` / `PendingReviewCommentCard` |
| Popup policy | Each feature caller plus the positioner defaults in `ui/popover.tsx`, `ui/select.tsx`, `ui/menu.tsx`, `ui/tooltip.tsx` and their Base UI components. A screenshot alone cannot establish collision policy. |

Guidance is the Exact implementation library `20261005-platforms-v3`, covering
foundations, styling, components/lifetime, accessibility, platforms/macOS and testing.
It predates this runtime. Current `rules/RULES.md`, `rules/DEFERRED.md`, `AGENTS.md`,
`docs/contract-grammar.md` and `docs/testing.md` control current behavior and checks.
Current public CSS is documented in `kernel/tables/schema.json`, LLP 1104 and
LLP 1021 D2; macOS presentation precedence is LLP 1115.002 D7.3. Use current view
`if`/`for`, receiver methods, `.length` and `T[]`; retain task/keybinding `when`
where it is a different construct. Do not revive old library spellings.

## Dependencies

| Dependency | Kind | Evidence and gate |
| --- | --- | --- |
| First-wave repair PRs | merged task PRs | Satisfied at the inspected baseline. Current plan records all 11 merged at `1e3152209`; that checkpoint is an ancestor of `a218ee78`. Preserve the resulting application changes. |
| Round-14 main adoption | merged task PR | Satisfied. [PR #549](https://github.com/ccheever/exact2/pull/549) was checked remotely as merged into `feat(example)/t3-code` on 2026-10-14 at `3710af842`; ancestry includes main `178ae7237`. |
| Dialog → legacy → connections → menu → browser | merged task PRs | Satisfied: #553 `6a7a3cad0`, #557 `4520b0124`, #566 `fbed7fadd`, #568 `b229a2658`, #571 `70eec59fd`; all are ancestors of prepared base `233ab29bc`. Consume their merged interfaces (menu-keys public popover commands, browser Appearance placement). |
| CSS framework slices | resolved framework issues | The five fix commits in the assessment below are ancestors of the inspected baseline. Their upstream tested resolutions establish availability; this application still needs the task's runtime qualification. |
| Application-only continuation | recorded decision | The 2026-10-14 fix-wave continuation and original five KW rows authorize the bounded work. Existing no-new-apparatus and no-repeated-pixel-polish constraints apply. |
| Attended proof environment | recorded verification dependency | The desktop is unlocked with keep-awake on (2026-10-14). Attended real-input rows (bottom-edge popup, pending card on the real-GitHub lane) are batched into the one final real-input session after follow-up 8; until then they stay unverified. |

Read-only PR searches by the exact serial task names and this task name found no
matching PRs during preparation on 2026-10-14. This is time-bounded discovery,
not proof that no alternate-name implementation exists. At the actual preparation
step, search active/closed tickets, recorded branches and PRs again. Fetch/reconcile
then, verify each merge and ancestry, and only then create an isolated task checkout
from the refreshed feature branch. Record its base SHA, branch and PR state. Do not
stack this task on unmerged predecessors.

Issue assessment checked on 2026-10-14 from the plan's issue/closed-issue records,
current upstream filesystem issue records, current documentation/source and git
ancestry. GitHub transfer closure alone is not treated as framework resolution.

| Issue/reference | Capability and target | Evidence/revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X10 / #266 | Balanced text and authored placeholder colors, AppKit | `issues/closed/20261009-balanced-text-placeholder-color.md`; fix `b686fb6df` is an ancestor. Record includes native/browser balance and placeholder tests, disabled/color-reset corrections and the six-line bound. | nonblocking | Use admitted CSS and prove the app families. Font smoothing remains deferred and is excluded. |
| X57 / #291 | Overflow start alignment, Apple text | `issues/closed/20261009-apple-overflow-line-alignment.md`; `24083b9b9` is an ancestor, with the 54-test TextGeometry class recorded. Plan transfer record is `issues/closed/20261008-x57-overflowing-centred-line.md`. | nonblocking for capability; per-site parity remains unverified | Test short and long values at all four candidates. Keep true reference-authored alignment. A default-alignment mismatch blocks the affected row until resolved. |
| X65 | Empty scroll-port pointer routing, AppKit | `issues/closed/20261009-macos-empty-scroll-pointer.md`; `3998e650e` is an ancestor. Record has 48 AppKit tests including 17 scroll-routing tests and an agent reproduction. Plan record is `issues/closed/20261008-x65-scroll-empty-area-press.md`. | nonblocking | Remove only the ground workaround and prove paired down/up, wheel behavior and pooled/window dismissal. |
| X17 / #112 | Ordered physical invoker-popover flips | `issues/20261009-popover-css-flip-fallbacks.md` remains open; `38281323f` is an ancestor. Implemented physical flips, combinations, margins and painted AppKit placement have recorded tests. | nonblocking for the implemented slice; unknown for any caller needing remaining CSS | Map all callers to supported geometry before editing. Preserve `hoverFlip`. If a required invocation depends on an unimplemented inset/alignment/size behavior, record a plan-local blocker instead of dropping it. |
| #287 | Dashed border layout/paint, AppKit | `issues/closed/20261009-dotted-dashed-border-style.md`; `ff2251b0c` is an ancestor. Native and Chrome geometry proof is recorded. | nonblocking | Replace both SVG overlays and verify border-box geometry and hit targets. |
| X87 / #523, earlier held-view crashes | Press lifetime while views leave the window, AppKit | Current plan and R14 adoption include the closed main fix. This draft does not requalify the crash lane. | nonblocking | Preserve held-press semantics and stop on a reproduced crash; do not add a native workaround. |
| Five serial predecessors and refreshed base | Shared application owners | Only the first wave and R14 satisfy merge gates at this baseline. | blocking | Verify all five actual merges, then prepare from their feature head. |
| Existing pooled-limits and real-GitHub lanes; attended input | Required proof | Existing lane recipes are named below; availability, credentials and unlocked desktop were not exercised in this read-only preparation. | unknown now; blocking any unavailable acceptance row | Reconcile existing fixtures without creating apparatus. Record a missing dependency by name and retain the row as blocked. |

## Implementation notes

### KW-1: shared balance and placeholder families

Balance is inherited `text-wrap="balance"`, with the admitted six-line bound before
the first forced break. Apply it at the smallest shared container matching the
reference's `text-balance` owner. Preserve explicit newlines, code whitespace,
clamping and reference overrides. The base stylesheet need not balance all text.

| Family | Current clone owners to audit |
| --- | --- |
| Ordinary and portal tooltips | `shell-tip.contract` / `Tip`, `TimelineTip`, `TipBubble`, `TipCard`; `hover-layer.contract` / `HoverText`, `HoverScopes`, `LayerTip`; shared policy/sidebar tip owners in `settings-kit.contract`, `sidebar-row.contract`, `sidebar-overlays.contract`, `legacy-sidebar.contract` |
| Specialized tooltip adapters | `r4-polish-tip.contract`, `r4-timeline-tips.contract`, `r8-pointer-tips.contract`, `media-actions.contract`, `r10-device.contract`, `settings-a-telemetry.contract`. Verify which delegate to a shared text owner. Code/preformatted tips require the reference's wrapping policy, not a global balance rule. |
| Compact popover tips | `pages-pr-actions.contract` / `PaFreshnessCard` and other callers of reference `PopoverPopup`'s compact variant. Audit the 62-declaration map below; do not infer all popup content is a tip. |
| Empty states | `pages-hero.contract` / `EmptyThread`, `PagesLanding`; `r5-shell-prs.contract` / `PrEmptyState`; `browser-surface.contract` / `BrowserEmpty`; no-results/error/empty boxes in pages, connections and settings that correspond to reference `Empty` / `EmptyContent` |
| Toasts | `shell-toast.contract` / `ToastCard`, `ToastHeading`, `ToastDescription`, `ToastDetailRow`; retain variant-specific line clamp, actions and link alignment |

The exact placeholder inventory at `a218ee78` is 108 lines containing `placeholder=`:
79 actual declarations, comprising 67 inputs and 12 textareas, and 29 component
forwarding/display uses. The inventory is a coverage starting point, not 108 independent
pseudo-elements. It also does not cover every select or contenteditable placeholder.

| Owner/group | Baseline coverage | Implementation direction |
| --- | --- | --- |
| `providers-kit.contract` / `PvField` | 27 input declarations across Codex, Providers, Connections and pairing | Author the common reference input placeholder role once in the style. Preserve caller overrides. |
| `AeField`, `SbField`, `TkField` | 2 inputs in `settings-b-actions.contract`, 2 in `settings-b-icons.contract`, 2 inputs + 1 textarea in `settings-scheduled.contract` | Put the common role in each existing family style or a shared imported placeholder style where appropriate. |
| Inline input/textarea owners | 34 inputs and 11 textareas without an existing field class | Group by the reference control and role below. Use shared placeholder-only styles where several existing components share the same role; do not restructure event/identity ownership to share a color. |
| Forwarding and painted-label owners | 29 uses: `ApprovalComposer` 1, `RedactedText` 4, `CodexSignedIn` 2, `CodexPathRow` 3, `PrdWords` 4, `UsagePriceInput` 3, `R7DevField` 6, `CoreField` 1, `ScopedTextField` 1, `ScopedTextArea` 2, `UsageRedacted` 1, `PoolEmail` 1 | Follow each prop to its real field or text owner; one shared change covers all callers. A component call cannot take an input pseudo-element unless that component exposes it. |

Reference roles are distinct: generic Input/Command uses `text-placeholder`;
SidebarInput uses `text-sidebar-muted-foreground`; the Git action input uses
`text-muted-foreground/60`; theme JSON and citation comment textareas use
`text-muted-foreground`. `ui/select.tsx` colors its empty value using
`data-placeholder:text-placeholder`, so color the clone's empty value text
conditionally. The rich composer's placeholder and redacted displays may be text
nodes; preserve their existing contenteditable/native ownership and reference
role instead of adding an illegal pseudo-element to them. Reuse the current theme
role mapping, including custom theme and contrast behavior, rather than freezing a
new color that disagrees with the appearance task.

Current Contract syntax allows a literal part in a `style`, or a computed part under
an `input`/`textarea` before children. Only `color` is admitted; it is non-inherited,
its alpha is its own, and the field's block wins over the class's. Other tags and
`placeholder-color=` are not supported. Example syntax only, with a deliberately
illustrative color rather than an approved T3 color:

```text
style ExamplePlaceholder
  ::placeholder
    color="#777777"

input placeholder="Search" class=ExamplePlaceholder
```

Inline field source files to reconcile against those groups:
`browser-surface.contract`, `citation-comment.contract`, `diff-comments.contract`,
`pages-pr-compose.contract`, `pages-pr-edit.contract`, `pages-pr-meta.contract`,
`pages-pr-threads.contract`, `pages-prs.contract`, `pages-usage-prices.contract`,
`pages-welcome.contract`, `providers-wizard.contract`, `r4-git-publish.contract`,
`r4-surfaces-files.contract`, `r7-device.contract`, `r9-connect.contract`,
`settings-a-bitbucket.contract`, `settings-a-fonts.contract`, `settings-a-hosts.contract`,
`settings-appearance-editor.contract`, `settings-appearance-import.contract`,
`settings-b-actions.contract`, `settings-b-icons.contract`, `settings-core.contract`,
`settings-keybindings.contract`, `settings-licenses.contract`,
`settings-prompt-preview.contract`, `settings-rows.contract`,
`settings-scheduled.contract`, `settings-source-control.contract`,
`sidebar-overlays.contract`, `sidebar.contract`, `timeline-plan.contract`.
Record the disposition of each family and any reference-specific exception in the
implementation evidence. Recount after predecessor merges.

### KW-2: four exact candidates

| Clone anchor at inspection baseline | Required comparison |
| --- | --- |
| `pages-pr-actions.contract:378`, `PaFreshnessMark`, `text base` | Short base name and overflowing branch name; preserve icon spacing, ellipsis and the leading characters. |
| `pages-prs.contract:588`, `PrSubTrigger`, `text value` | Short filter value and a value exceeding `max-width="8rem"`; keep the neighboring label's intentional alignment. |
| `r4-surfaces.contract:222`, `R4TabChip`, `text tab.title` | Short tab title and long browser title; do not alter adjacent launcher/menu labels. |
| `palette.contract:292`, `PaletteRowView`, normal `role="option"` button | Short option and a long project path/description; leave the code-search line's explicit left alignment and the line number/timestamp alignment intact. |

Delete only the four historical overrides after source and runtime comparison.
The absence of a reference `text-left` class does not establish its computed
alignment: the reference element type, inherited styles and reset also matter.
The main fix changes overflowing lines; short lines retain their normal alignment.
If any candidate still requires authored alignment to match the reference, record
the conflict with both values and leave KW-2 incomplete pending resolution. Merely
renaming `left` to `start` does not prove a workaround was retired. Preserve other
reference-authored `text-left` sites such as PR list rows, launcher/menu rows and
copyable text. Do not apply a global replacement.

### KW-4: pooled Limits ground dismissal

`pages-usage.contract:245-300` already attaches `pointerdown=pressDown` and
`pointerup=pressUp` to `usage-page`; `usage-ground` at line 289 only expands the
scroll content's hit area. Remove the extra column and keep the centered
`usage-content` width, padding and normal flow. Prefer the existing root listener
when empty-port events bubble there; move to the scroll owner only if the actual
public event path requires it. Do not attach duplicate down/up handlers.

The down must be primary and outside the held card, segment and legend. The up
must not finish inside the card. Close before another segment's own activation so
that activation can pin the new segment. Preserve same-trigger toggle, hover-open
state clearing, Escape focus return and scroll/wheel handling. Preserve the global
`outsidePresses` path through `app-window.contract`, `PagesCover`, sidebar,
transcript and appearance editor. `usage-pooled.test.ts:455-528` currently requires
the obsolete `usage-ground` shape and separately asserts this wider outside-press
wiring; update the former and retain the latter's behavioral coverage.

### KW-6: authored presentation and collision placement

An authored `position-try-fallbacks` row, including `none`, opts a non-menu AppKit
popover into Exact's painted layer. Removing that row can select `NSPopover` and
change surface, focus and placement. Keep an authored row where the reference
requires its painted surface. Native menus and confirms retain their existing
platform handling. The menu-key predecessor owns opening/closing APIs; preserve
its `showPopover`/`hidePopover`/`toggle=` wiring and source IDs.

For each declaration below, record the reference caller, initial physical area,
main-axis and cross-axis collision behavior, spacing/margins, fallback order,
painted/native intent, and any fixed tail/offset being removed. Use `flip-block`
for vertical opposite-side fallback, `flip-inline` for horizontal, and an explicit
combined candidate when both flips are needed. For example,
`"flip-block, flip-inline, flip-block flip-inline"` tries the two single flips
and then both. `"flip-block, flip-inline"` alone cannot solve a corner where
neither single flip fits. The order must match the caller's reference policy;
this example is not a blanket replacement for every popup.

Main's first slice supports physical areas, ordered flip combinations, margin-box
fit and exchanged margins. The current issue retains `flip-start`, logical/other
area spellings, area-dependent sizes, nonzero insets, self-alignment and full CSS
scroll-offset bookkeeping as remaining work. Inspect layout before removing a
fixed placement. Padding used as a transparent tail is not automatically the same
as a flippable margin. Preserve geometry needed for genuine reference alignment.
If the reference requires an unsupported behavior, retain the existing behavior
with a named blocker and do not mark the affected retirement complete.

Keep `hover-layer.contract:119` / `hoverFlip` and its callers. It implements a
best-fit choice when neither side has enough room; that differs from assuming
CSS's ordered fitting candidates cover hover behavior. The open issue's invoker
slice does not establish parity for every hover-opened layer. A future main #112
resolution must be tested for that capability before retiring this helper.

Current inventory, 62 declarations in 39 files. Entries name the owning component
and source line; repeated instances retain separate alignment/role decisions:

| File under `examples/t3-code/` | Popover owners at baseline |
| --- | --- |
| `auto-balance.contract` | `NoticeMachines:37` |
| `browser-stage.contract` | `BrowserPresetSelect:279` |
| `browser-surface.contract` | `BrowserMore:226`, `BrowserAppearanceItem:312`, `BrowserAddItem:515`, `BrowserLauncherProfiles:576` |
| `diff.contract` | `DiffBasePicker:356` |
| `legacy-sidebar.contract` | `LegacyOptions:196` |
| `markdown.contract` | `TerminalContextChip:772` |
| `pages-hero.contract` | `HeroPicker:129` |
| `pages-pr-actions.contract` | `PrdActionsMenu:230`, `PaFreshnessMark:381` |
| `pages-pr-code.contract` | `PrdCodeToolbar:210` |
| `pages-pr-compose.contract` | `PrdComposer:124` |
| `pages-pr-handoffs.contract` | `PrdCheckoutMenu:63` |
| `pages-pr-meta.contract` | `PrdReactionBar:82`, `PrdCandidatePicker:188` |
| `pages-pr-quick.contract` | `PrRowChecks:122`, `PrRowStack:171` |
| `pages-pr-stack.contract` | `PrdStackControls:95` |
| `pages-prs.contract` | `PrSortMenu:365`, `PrHostMenu:399`, `PrFiltersMenu:538` |
| `pages-usage-prices.contract` | `UsagePricesApply:153` |
| `pages-usage.contract` | `UsageBreadcrumb:356`, `UsageSelect:486` |
| `providers-upkeep.contract` | `UpkeepSelect:194`, `ProviderVersionAdvisory:252` |
| `r4-git.contract` | `R4BranchPicker:177`, `R4GitMenu:235`, `R4EnvRow:449` |
| `r4-surfaces.contract` | `R4HeaderBar:135`, `R4AddSurface:268` |
| `r6-pr.contract` | `R6ChecksPopover:82` |
| `settings-a-fonts.contract` | `FontFamilyPicker:32` |
| `settings-a-hosts.contract` | `HostRowView:172` |
| `settings-appearance-import.contract` | `AddThemeDialog:75` |
| `settings-b-accent.contract` | `ProviderAccentPicker:154` |
| `settings-b-actions.contract` | `ProjectActionEditor:104` |
| `settings-b-kit.contract` | `CnMenu:70` |
| `settings-keybindings.contract` | `WhenEditor:288` |
| `settings-kit.contract` | `SkPopup:149` |
| `settings-mobile-beta.contract` | `MobileBetaControl:65`, `MobileBetaControl:80` |
| `settings-rows.contract` | `ScopeMenu:120`, `CoreRowView:227`, `CoreMenu:377`, `TraitsMenu:414` |
| `settings-scheduled.contract` | `ScheduledEditor:421` |
| `settings-source-control.contract` | `Inheritance:159`, `DeviceVersions:517` |
| `shell-details.contract` | `WorkspaceMenu:364` |
| `sidebar-row.contract` | `ThreadRow:179`, `ThreadMenu:528`, `ThreadMenu:546`, `DraftMenu:565`, `DraftMenu:583` |
| `snapshot.contract` | `SnapshotPanel:201`, `SnapshotTile:246` |
| `snooze-calendar.contract` | `SnoozeDateField:37` |
| `theme-color-picker.contract` | `ThemeColorSwatch:259` |
| `timeline-plan.contract` | `PlanCard:50` |

`adopt-main-fixes-r12.test.ts` currently asserts every popup has the exact string
`popover="auto" position-try-fallbacks="none"`. Replace that obsolete assertion
with coverage of authored presentation intent and the reviewed placement policy;
do not simply delete the protection. Preserve its vertical-list and unrelated
border regressions. Recheck the map after menu/browser merges change declarations.

### KW-7: the two dashed boxes

- `markdown.contract:740-748`, `ResolvedChip`, unresolved branch,
  `testId=context-chip-${run.id}`. Current outer row has a 1.0625rem border-box
  height, 0.375rem horizontal padding and a 0.375rem radius; a positioned SVG adds
  no layout width. Replace the overlay with a 1 px dashed border using its existing
  color role. Check the 14 px icon, baseline, text clipping and maximum width against
  `ContextChip`'s border-inclusive reference metrics.
- `pages-pr-threads.contract:195-217`, `PrdPendingCard`,
  `testId=pull-request-pending-${id}`. The overlay is pointer-transparent because it
  previously intercepted the discard control. Use a real 1 px dashed border with
  `ptBorder()` and the reference CARD_CLASS geometry. Adding a border affects an
  auto-sized card's content and outer height even with border-box. Preserve body
  padding, radius, margins, text wraps and the discard button's clickable area.

Measure the reference before choosing a 1 px compensation. Do not blindly subtract
padding or enlarge every box. Remove the obsolete solid-only comments and only the
two workaround rectangles; other SVG artwork and dashed code drawings remain.

## Acceptance and reproduction

Preparation runs no builds, application launches, native tests, global input or
remote writes. The following is the required implementation/verification recipe.
Use the existing task lane and existing public agent driver. Read `docs/testing.md`
before changing tests. Stop after the repository's three unsuccessful fix rounds;
record evidence and the unmet condition rather than running an unbounded loop.

1. After all five predecessor PRs merge, record the refreshed feature SHA, framework
   ancestry, isolated checkout/branch and reference pin. Save a clean before build
   of that source and fixture state. Re-inventory all placeholder owners, popovers
   and four alignment anchors against that base before edits.
2. Reconcile the existing pooled-limits pages lane using application
   `AGENT-HANDOFF.md` and the `usage-and-pr-pages` / pooled-usage records under the
   parent plan. Their historical `target/t3-ui-parity` artifacts are ignored and
   are not promised to exist in a fresh clone. If unavailable, record the missing
   fixture/handoff; do not create new apparatus or omit Limits proof.
3. For the pending card, use the committed recipe
   `examples/t3-code/tools/github-lane/README.md` and its existing disposable
   sandbox, with lane-only primary/secondary credentials checked by `lane.mjs
   whoami`. Use an existing reviewable PR, create a local pending comment, capture
   and discard it. Submitting a review or reseeding/probing remote data is not
   required to prove the border or hit target. Missing credentials are recorded
   by name only; do not use the user's ordinary GitHub configuration.
4. Build and launch the before and after app serially with `EXACT_APP_DIR`, a unique
   `T3_LOCAL_HOME`, a reserved lane `T3_LOCAL_PORT` outside 3773, and
   `T3CODE_TELEMETRY_ENABLED=false`. Use the same fixture, data, fonts, contrast,
   theme and viewport for clone/reference. Do not share `~/.t3` or a target directory.
   Record process IDs and stop only those started for this lane.
5. Run the bounded drive below once at 1280x840 in light and dark after focused
   tests pass. Save named before / after / reference screenshots and `tree`,
   `layout`, `state` or event assertions that prove the behavior. Check 840x620 for
   clipping only if new border or popup geometry creates an unresolved concern.
6. Capture the two real-input rows while the desktop is unlocked and the expected
   app is frontmost, using the existing approved computer-use workflow. Driver
   `tap` is not attended input. Keep a blocked row when the desktop is unavailable.

| Proof row | Setup and actions | Assertion and evidence |
| --- | --- | --- |
| KW-1 balance | Open a long ordinary tooltip/compact tip that wraps; inspect an empty state and long toast through existing fixture actions. Preserve explicit newline/code examples. | Breaks and width match the reference family. Capture the long tooltip trio; source guards cover all shared owners. |
| KW-1 placeholders | Clear a generic input and textarea, then inspect sidebar, command/select, Git action, theme JSON and citation roles using existing fields. Type then clear once. | Correct role/alpha in both appearances; typed text and disabled styling are unchanged. Capture a representative placeholder trio and record family/role coverage. |
| KW-2 four sites | At each exact owner, show one short value and one value that exceeds the available width. Use existing fixture data or local editable titles without remote writes. | Short alignment matches reference; long values retain their start and trailing ellipsis/clamp. Eight short/long comparisons, with per-site before/after/reference evidence. |
| KW-4 empty port | In pooled Limits, pin a segment or its legend, click unused viewport space below/beside content, then repeat with another segment. Test down outside/up inside card, non-primary press and wheel. | Ground closes on the intended up; a press ending inside does not close; segment activation pins the new segment; wheel still scrolls. Save popover state before/after and captures. |
| KW-4 window path | Pin again and press an existing sidebar, transcript or theme-editor control that contributes to `outsidePresses`; test Escape and return focus. | The shared outside counter closes the held popup without duplicate processing or swallowing the destination's action. Preserve the existing regression assertions. |
| KW-6 collision | Open a reviewed vertical popup near bottom, a horizontal/end-aligned popup near right, and a corner requiring both axes. Compare a fitting center case, then resize/move its invoker while open and reopen. | Reference side, alignment, gaps and surface are retained; no button-covering clamp, stale placement, focus loss or dismiss regression. Record layout bounds and before/after/reference captures. |
| KW-6 attended | With the app frontmost, physically open a popup near the window bottom and use/dismiss its contents. | Actual pointer-opened placement and hit targets pass. Record input method and screen state; an agent open alone does not satisfy it. |
| KW-7 chip | Display a message containing an unresolved context reference, including a long label. Hover it and use adjacent controls. | Border, outer height, radius, baseline and clipping match reference; tooltip and neighbors still receive input. Capture the trio. |
| KW-7 pending card, attended | On the existing real-GitHub lane, queue a local review comment with wrapping body text. Inspect the card, then physically click Discard. | Dashed border and box geometry match reference; discard removes the local pending card and does not submit a review. Capture before/after/reference plus resulting state. |

Run practical verification on the final implementation, recording results rather
than copying historical counts:

```sh
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/fix-wave/css-workaround-retirements.plan
```

Update the existing relevant source guards, including `adopt-main-fixes-r12.test.ts`
and `usage-pooled.test.ts`. Guards verify complete adoption and preserve behavior
contracts; they do not replace the runtime drive. Add no new test apparatus.
Use `EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos
--bundle --run` only after lane environment and native ownership are reconciled.
A public drive starts with `EXACT_APP_DIR="$PWD/examples/t3-code" bun
scripts/agent.mjs macos --size 1280x840 tree state`; resolve current test IDs from
that tree and the anchors above rather than hard-coding stale fixture-generated IDs.
Use `clock settle` where applicable and capture `layout` for changed boxes.

Run required repository gates before delivery: `cargo build --all-targets
--keep-going`, `cargo test --lib --bins --tests --no-fail-fast`, `cargo clippy
--all-targets --keep-going -- -D warnings`, `cargo fmt --all -- --check`,
`bun scripts/caps.mjs`, and `bun scripts/boot.mjs`. Stage only the authorized task
paths in its isolated checkout before caps. Follow current repository instructions
if their gate scope changes. Record native build/launch and the exact source commit;
a successful Contract build alone does not prove the app works.

Acceptance also requires updated KW cells in `EXACT2-GAPS.md` and the parent task
records: identify each retired workaround and any still-open #112 capability.
Preserve source pins, commands, fixtures, images, logs and unresolved rows under a
portable task evidence path. Large/ignored lane outputs remain local-only until an
authorized durable handoff exists. Independent review must assess the exact final
source and proof before verification/closure. Do not mark all KW rows retired while
an original required row or attended proof is blocked.

## Progress

- [x] Read the original five-row task, retained-workaround record, parent plan/spec,
  prepare workflow, current CSS documentation and issue resolutions.
- [x] Confirm the inspected source/reference pins and ancestry of the five CSS fixes,
  first-wave checkpoint, round-14 merge and adopted main revision.
- [x] Inventory 62 popover declarations, 108 placeholder-bearing lines and the four
  exact alignment candidates; distinguish actual fields from forwarding/display uses.
- [x] Preserve every original acceptance row and record unsupported assumptions.
- [x] Verify all five serial predecessor PRs merged and refresh the feature base (prepared base `b98eee019`).
- [x] Reconcile existing lanes: the real-GitHub lane works; the pooled-limits fixture is missing (below).
- [x] Create isolated implementation checkout, record base/branch, mark prepared.
- [x] Implement all five KW rows (2026-10-14).
- [x] Focused and full checks, one native AFTER drive, independent review.
- [ ] Attended real-input rows, the KW-4 pooled dismissal and the unresolved chip: unverified.

## Attempts and evidence

| Attempt | Source and action | Actual result | Limitation |
| --- | --- | --- | --- |
| 2026-10-14 read-only preparation | `a218ee78fec42b1a4590b199297f30667876de67`; original coordinator scope, parent records, current Contract/source/issue review; reference `1e2ecbd9758830669684b494d4398f626b0576e0` | Produced this portable blocked draft, ownership maps and reproduction plan. `git merge-base --is-ancestor` confirmed all named CSS fixes, first wave, R14 and main. | Source inspection only; no behavioral reproduction, screenshots, compile or native run. |
| Same preparation, remote reads | `gh pr view 549`; exact task-name searches through `gh pr list --state all` | R14 merged into feature at `3710af842`; no matching PRs found for the serial names or this task at the time checked. | No fetch/ref mutation, branch, remote write or promise about future state. Refresh before implementation. |
| Same preparation, file checks | Verified nine required sections, all five KW identifiers, source references and unchanged tracked status after writing the draft | Local artifact only at `target/fix-wave/css-workaround-retirements-task.md`. | Ignored output is not published or a tracked portable task until the coordinator promotes it. |

There have been zero implementation or repair attempts for this task in this
preparation. No current runtime verification is claimed. Historical issue tests
establish capability availability, not this application's acceptance.

## Implementation and verification, 2026-10-14

Base `b98eee019` (feature head `233ab29bc` plus this ticket), branch
`feat(example)/t3-code-css-workaround-retirements`. macOS 26.6.2, Apple Silicon, development bundle from
`EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle`. Reference pin
`1e2ecbd975` (read only). Base UI's package is not installed in the reference checkout, so "Base UI flips side and
alignment near an edge" comes from this ticket and the app's records, not from reading Base UI source.

### Per-row changes

| Row | Change | Files |
| --- | --- | --- |
| KW-1 balance | `text-wrap="balance"` on 16 shared owners: `TipCard` (all `Tip`/`TimelineTip`/`TipBubble` users), `HoverText`, `HoverCode`, `HoverScopes`, `MediaTip`, `CnTooltip`, the PR links error tip, the Git publish hint, `StatInfo`, the context-window details and the Usage unpriced popup (tooltipStyle), and the empty states `PagesLanding`, `PrEmptyState`, `BrowserNothingYet`, `environments-empty` and the source-control scan card. Not balanced, as the reference: `EmptyThread` (DraftHeroHeadline has no `text-balance`), the freshness card (not tooltipStyle), the stacked toasts (only the anchored toast balances; the clone's anchored "Copied!" toast cannot wrap), `nowrap`/`pre` text. | `shell-tip`, `hover-layer`, `media-actions`, `settings-b-kit`, `pages-pr-links`, `r4-git-publish`, `settings-diagnostics`, `composer-controls`, `pages-usage`, `pages-hero`, `r5-shell-prs`, `browser-surface`, `connections`, `settings-source-control` |
| KW-1 placeholders | Recount: 108 lines = 79 fields (67 `input`, 12 `textarea`) + 29 forwarding/display uses. `::placeholder` in `PvField` (27), `SbField` (2), `TkField` (3); `AeField` cannot hold a part (`lower-attr-tag`: it also styles a row and a button), so its 2 inputs hold their own. New placeholder-only style `FieldPlaceholder` (`field-placeholder.contract`, 27 fields). Field parts: PR pages and the device field (7, each file's muted ink), themed settings (6, `pal.muted`), theme JSON (`pal.muted`), citation (muted), Git publish path (muted /60). Forwarded: `CodexPathRow` 3, `PrdWords` 4, `UsagePriceInput` 3, `R7DevField` 6, `CoreField` 1, `ScopedTextField` 1, `ScopedTextArea` 2 (their owning field covers all callers). Painted text kept: `ApprovalComposer` 1, `RedactedText` 4, `CodexSignedIn` 2, `UsageRedacted` 1, `PoolEmail` 1, and the sidebar inputs' painted labels (2). `SettingsSelect` takes `empty` and colours an empty value muted, as `ui/select.tsx`'s `data-placeholder`. | 40+ contracts, `field-placeholder.contract` |
| KW-2 | The four overrides removed. Freshness `text base` and the tab title are sized by their words. The filter value is the reference's `min-w-0 max-w-32 truncate` (nowrap, ellipsis, shrinkable) and its label keeps its words' width (the reference's `flex-1`, min-width auto; repair round 1). The palette row button lost `text-align="left"`; its match line is no longer 100% wide, so every text in the row is word-sized in a start-aligned column. The code-search line's own left stays. | `pages-pr-actions`, `pages-prs`, `r4-surfaces`, `palette` |
| KW-4 | `usage-ground` removed; `usage-content` is the scroll's own child (same width, padding, centring). The page root keeps its one `pointerdown`/`pointerup` pair; the window's `outsidePresses` path is unchanged. | `pages-usage.contract` |
| KW-6 | All 62 popovers keep `popover="auto"` and an authored row (painted layer). 59 now name the reference caller's area (`bottom span-right`/`bottom span-left`/`top span-right`/`top span-left`/`bottom`/`top`/`right span-bottom`) with `flip-block, flip-inline, flip-block flip-inline` (edge-aligned), `flip-block` (centred) or `flip-inline` (submenus, as #571's Appearance submenu). Transparent padding tails became the 0.25rem margin on the invoker side; fake geometry removed: `-16rem`, `-12.5rem`, `-11rem`, `-18.75rem`/`-18.5rem`, `(narrow ? -362 : -299)`, `endAlign - width`, `tool.offset`, `top="100%" right=0`, `left/top=0` insets, the profiles submenu's `-1.75rem`/`11.75rem`, and the colour picker's own `above` flip (with its `vh` prop). `CnMenu` and `UpkeepSelect` take an `area` prop per caller. Kept `none`: the native thread, draft and tab context menus, item-aligned Selects (3 ACP selects, `providers-setup`), the sign-in method menu. `hoverFlip` unchanged. Menu-keys sources and Appearance placement unchanged. | 39 contracts, `pages-prs.ts`, `settings-a-integrations.ts`, `app-main`, `app-window`, `legacy-sidebar` |
| KW-7 | The unresolved chip and the pending card have `border-width=1 border-style="dashed"` in the border-box (the chip `light-dark(#e4e4e7, #ffffff12)`, the card `ptBorder()`); both SVG overlays and the chip's `position="relative"` are gone. Outer size follows the other chips and the thread card (the reference's border is in layout too). | `markdown.contract`, `pages-pr-threads.contract` |

Tests: the obsolete AD-1 "every popover says `none`" became a reviewed per-site table (area and fallbacks, with
the reference caller), checks that the fake geometry is gone and that `hoverFlip` stays
(`adopt-main-fixes-r12.test.ts`); a KW-1 block there checks every placeholder field's colour source, the role
colours and the balance owners. Updated into real assertions: `usage-pooled` (no ground, one handler pair),
`pages-prs` (word-sized texts need no alignment; the two KW-2 lines and the label width), `browser-surface`,
`sidebar-palette-keys`, `realinput-1010f-followups` (RF-4: a border, no overlay), `usage-pr-pages`,
`adopt-main-fixes-r11`, `menu-keys`, `settings-a`, `view-depth`, `scheduled-tasks-followups`.

### Checks (final source)

| Check | Result |
| --- | --- |
| Full app Bun suite, 335 tracked files (explicit `./` paths) | 5,201 pass, 1 skip, 0 fail (base 5,175 / 1 / 0). One run under a concurrent `cargo check` timed out the 2 subprocess tests in `tools/github-lane/lane.test.ts` (5 s limit); that file alone then passed 15/15, and the final full run passed. |
| Strict TypeScript (`app.ts`) | exit 0 |
| `contract build examples/t3-code/app.contract` | exit 0; 254 warnings, the same set as the base (no new warning) |
| `cargo test -p t3-code-macos --lib --no-fail-fast` | 44 passed, 0 failed |
| `git add -A examples/t3-code && bun scripts/caps.mjs` | all budgets within cap; no file over 1,500 lines |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo check --workspace --all-targets --keep-going` | exit 0 |
| Native bundle | built twice (before and after repair round 1), exit 0 |

### Native AFTER drive

Agent mode (`scripts/agent.mjs macos --size 1280x840 --storage css-retire-a`), light then dark, on the existing
real-GitHub lane (primary server pid 42586 on 16520, `lane.mjs` start/project/pair/stop; isolated homes; telemetry
off; only reads from GitHub, the pending comment stays local). Evidence is local under `target/css-retire/drive/after/`
(segment logs `seg-a.log`, `seg-a2.log`, `seg-c.log`, `seg-d.log`). Segment A first stopped twice on setup taps
(a toast that does not always show, then a collapsed file); segment C stopped once on a navigation tap; segment D
was the one retry after repair round 1.

| Row | Result | Evidence |
| --- | --- | --- |
| KW-1 long tooltip balance | PASS (agent): Git fetch interval tip, 320×74, four even lines, light and dark | `C05-long-tooltip-light.png`, `C06-long-tooltip-dark.png`; layout `git-fetch-policy-tooltip` 311.61,404 320×74 |
| KW-1 placeholders | PASS (agent) for generic Input (welcome pairing link, PR search), palette search, composer painted text unchanged, light and dark | `A01-welcome-placeholder-light.png`, `A02-prs-search-placeholder-light.png`, `D04-palette-dark.png`, `C01-thread-light.png` |
| KW-2 filter submenu value | PASS after repair 1: short "All"; long "Changes requested" keeps its start and ends in an ellipsis; the label is not overdrawn (before repair the label was drawn under the value) | `A04-filters-short-light.png`, `C02-filter-long-value-light.png` (fault), `D01-filter-long-value-light.png` |
| KW-2 freshness mark | PASS short ("main", 47.67×16); long value UNVERIFIED (no seeded long base) | `A06-pr113-freshness-light.png` |
| KW-2 palette option rows | PASS short rows start-aligned; long project path UNVERIFIED (not searched) | `D04-palette-dark.png` |
| KW-2 right panel tab title | UNVERIFIED (no tab was opened in the drive) | `D02-right-panel-light.png` |
| KW-6 right edge | PASS: provider menu (`bottom span-right`) flips inline: invoker 1148.27–1210, popover 1050–1210 at y 113 (4 pt gap); Filters and More end-aligned (`pr-filters-layer` right 1140.27 = invoker right; `pr-more-layer` 884–1264 = invoker right 1264) | `A03-provider-right-edge-light.png`, `A04`, `A08`, `A10-more-right-edge-dark.png` |
| KW-6 bottom edge | PASS (agent): #112 row checks popover flips above: invoker y 800–814, popover 718–796 (4 pt gap); #148 fits below (760–838) and stays | `B01-checks-bottom-edge-flip-light.png`, `A05-checks-bottom-edge-light.png` |
| KW-6 freshness popover | PASS: below the mark, start-aligned, 657,174 | `A07-freshness-popover-light.png` |
| KW-7 pending card | PASS (agent): 1 px dashed border, body wraps, Discard 24×24 at 1231,348, Discard removes the card, light and dark | `B02-pending-card-light.png`, `B03-pending-card-dark.png`, `B04-pending-discarded-dark.png` |
| KW-7 unresolved chip | UNVERIFIED: no thread holds a message with an unresolved reference without sending a provider message | source and tests only |
| KW-4 pooled Limits dismissal | UNVERIFIED: the pooled fixture is missing (fix-hover-cards' uncommitted `usage-lane` is gone; the lane's Codex reads no limits). A press on the empty Limits port was delivered (`tap usage-scroll at 600 600`) without error | `C08-limits-dark.png`, `seg-d.log` |
| Attended: bottom-edge popup by physical input; pending card Discard by real click | UNVERIFIED: batched for the final real-input session | — |

PR images (local): `target/css-retire/pr-images/01-popup-flips-right-and-bottom-edge.png`,
`02-pending-card-dashed-border-light-dark.png`, `03-filter-value-short-long.png`,
`04-long-tooltip-balance-light-dark.png`.

### Gaps (framework, recorded; no caller dropped)

- `browser-surface.contract` profiles submenu (`r4-browser-profiles`): a flipped submenu meets the chevron's start,
  not the row's; it needs an anchor other than the invoker (`position-anchor` is not admitted, #112).
- Item-aligned Select placement (`alignItemWithTrigger`) has no `position-area`: the three ACP selects in
  `providers-upkeep.contract` and `providers-setup.contract` keep the invoker's bottom-left and no flip.
- `max-w-(--available-width)` (area-dependent size) is not admitted; popovers keep their fixed widths.
- `AeField` (`settings-b-actions.contract`) cannot hold a `::placeholder` part because the style also applies to
  non-field elements (`lower-attr-tag`); its fields hold their own parts.

### Known differences

- The filters menu (184 pt wide) cuts "All projects" to "All proje…" now that the label keeps its words' width, as
  the reference's flex rules do; the reference menu's width was not measured.
- Textareas use the muted placeholder role; the reference `Textarea` writes no placeholder colour (its effective
  colour comes from its reset, not inspected here).
- Themed fields use `pal.muted` at full strength: `CorePalette` carries no custom-theme placeholder role, and the
  prompt preview has no 75% token.
- With an authored area there is no window clamp (CSS): a popover that fits no candidate runs past the window
  edge. Large ones (When editor 34rem, `task-branch-menu`, the checkout menu) were not checked at 840x620.
- The `Pages` hero heading wraps its "?" alone beside an open right panel (seen in `D02`); `EmptyThread` is not
  balanced, as its reference.

### Independent review

A fresh general-purpose reviewer read this ticket and the full diff on 2026-10-14: **no blocking defects**. It
checked about 50 popover callers' areas against the reference, the #568 and #571 interfaces, the balance owners,
the placeholder roles, KW-4's single handler pair and both KW-7 borders, and reran the suite (5,201 / 0 / 1), `tsc`
and the Contract build. Non-blocking notes and handling: the alignment guard's exemption was narrowed to exactly the
two KW-2 lines (`pages-prs.test.ts`); the textarea colour, the missing clamp and the prompt-preview alpha are
recorded above as known differences; the runtime rows stay as the drive table says.

### Real-input session realinput-1015, 2026-10-15

Bundle of `db3c4540f` (code `aa4fd19c3`) in a lane copy (`com.exact.t3code.macos.ri1015-1`) launched through LaunchServices (a normal launch, not agent mode), port 16915, isolated homes from audit lane `ri1012-b10` plus the real-GitHub lane's sandbox (read only). Real keys and pointer through `orca computer`; the input source was `com.apple.keylayout.2SetHangul` (read only, not changed). While a painted popover or dialog is open the window's accessibility tree collapses and `orca` refuses input, so those keys went as System Events key codes to the lane (front process checked first). Evidence is local in the session worktree's `target/realinput-1015/` (`notes.txt`, `ev/`). 32 steps in all: 27 passed, 2 failed, 3 not verified.

| Row | Result | Evidence (`target/realinput-1015/ev/`) |
| --- | --- | --- |
| KW-6 bottom-edge popup, physical (step 27) | PASS: a real click on #112's checks icon (the list's bottom row) opens the checks popup above the row | `s27-e-crop.png`, `s27-f-crop.png` |
| KW-7 pending card Discard by a real click (step 29) | PASS: dashed card "Pending — sent when you submit the review" with wrapped text; a real click on the trash removes it; `gh-calls.tsv` holds no review or other write | `s29-o-crop.png`, `s29-q`, `gh-calls.tsv` |
| KW-4 Limits dismissal, unresolved chip, large popovers at 840×620 | UNVERIFIED: fixtures gone or not set up; not driven | — |

## Next action

After realinput-1015 (2026-10-15): Keep `verification: unverified`. realinput-1015 passed both attended rows (bottom-edge popup and the pending card's Discard by a real click). KW-4's Limits fixture, the unresolved chip and the 840×620 large popovers stay open.

Merged as [#577](https://github.com/ccheever/exact2/pull/577), feature commit
`6ef85eb1c` (squash). The task stays unverified for the bottom-edge popup by physical
input, the pending card's Discard by a real click, the KW-4 ground dismissal (the pooled-limits
fixture no longer exists), the unresolved chip and large popovers at 840×620. Known
differences: the filter menu cuts "All projects" to "All proje…"; plain textareas use the muted
placeholder colour; themed fields use `pal.muted` at full strength; a popover that fits on no
side runs past the window edge. Follow-up 7, `timeline-and-window-hooks`, starts from this
feature head.
