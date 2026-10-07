# Desktop comparison, 2026-10-07

This audit compares the running Electron desktop with the running Exact macOS example.
It adds tracking records only; it does not implement fixes or accept existing tasks.
Result: seven new tasks, one local framework issue, and observations added to two existing
tasks. All newly observed cosmetic differences share one task.

## Baseline and method

- Audit worktree: `t3code-desktop-audit`, branch `daehyeon/t3code-desktop-audit`.
- Exact source and native build: `fbce02624d2e33449ee2cde34497083d6fd47457`.
- Requested reference checkout: `/Users/daehyeonmun/Documents/work/3.open-source/t3code`,
  revision `1e2ecbd9758830669684b494d4398f626b0576e0`. The desktop was built from an
  installed dependency copy of that revision: all 24,112 tracked files compared equal,
  with no missing files or byte mismatches. This is Electron, not a browser-only substitute.
- Both clients use one disposable reference backend and a disposable Git project under
  `target/desktop-audit/project`. No existing user project was imported.
- Electron has an isolated profile. Exact uses isolated agent storage and volatile
  credentials. Its native window runs at 1280 × 840 logical points. Initial Electron
  captures used its actual 1226 × 840 window; later comparisons normalize its content
  viewport. Initial captures do not establish pixel-level spacing differences.
- Reference reports a development build (`0.0.45`), Exact a nightly identity (`0.0.46`).
  Version banners and development/nightly sidebar artwork are fixture differences.
- Controls were activated through the actual Electron renderer and the native Exact
  input driver, with screenshots and accessibility/tree snapshots after actions.
  Native asynchronous changes were followed by explicit clock advances. A hidden node
  in a tree alone does not prove that a menu or panel was displayed.
- Existing tracking was searched before filing: 65 task records, including 39 in
  `tasks/closed`, and 46 issue records. A merged task with unverified or residual rows
  remains relevant tracking. The newer status in `plan.md` overrides older handoffs.

Local evidence lives under `target/desktop-audit/` and is intentionally not committed,
following this plan's evidence policy. `reference-launch.json`, `native/launch-record.json`,
`reference-actions.ndjson`, `native-actions.ndjson`, screenshot/tree pairs, and the
font probe record the session. Pairing tokens and account identifiers must not be copied
into tickets. The new records include reproduction steps that do not depend on retaining
those local files.

## Coverage and findings

Every Settings page was visited, including the conditional Project page, Diagnostics and
Licenses. The table distinguishes opening/inspecting a control from verifying a completed
operation. It does not claim every mutation, account state or backend failure was exercised.

| Surface | Live actions and observed state | Finding or limit |
| --- | --- | --- |
| Welcome | Expanded Add a computer; submitted malformed input; opened help; paired to the disposable backend; inspected Agents and skipped import | Invalid URL error and already-ready Codex state differ; tracked below |
| General | Opened model/effort, permissions, workspace, submodules, project order, notifications, timestamps, streaming, diff state/layout, send/follow-up, background profile, quit behavior and update track menus; scope and inheritance controls | New model-picker task. Other menus inspected; not every preference was saved |
| Appearance | System/light/dark, advanced typography, family/size menus, font search; Add theme and invalid JSON; Create theme, Advanced, color popup, preset selection, minimize/restore and cancel | Installed fonts, color interaction and preview rendering tracked below; no downloaded/saved theme |
| Project | Selected fixture scope; opened icon editor, Icons/Emoji/Monogram, model/workspace, Add action and Remove project confirmation; canceled | No project removed or action executed |
| Keybindings | Search/filter, add row, command selector, shortcut recorder, When conditions/groups; restored edited native expression | Selector alignment in the visual task; no binding saved |
| SnapShots | Off baseline, setup wizard, permission state and gated Shortcut step; exited setup | No OS permission grant, screenshot capture or completed setup; agent-mode limits apply |
| Providers | Runtime/model controls, custom-model and hub dialogs; all seven Add provider drivers and ACP registry; manual Identity/Config path | No provider installation, sign-in, credential edit or saved instance. Bulk enable/disable was not conclusively exercised |
| Integrations | Reference browser profile/viewport/zoom/appearance/recording/link menus; device hub status; Add host and SSH options, canceled | Exact Browser disabled: existing X1. Hub absent in both; no installation or remote connection |
| Scheduled Tasks | Empty page, New task, environment/project/workspace/branch/model controls, time/interval modes, cancel; native empty submission rejected | Selector alignment in visual task. No schedule saved or executed |
| Source Control | Merge method, branch naming, writing style, Git details, Bitbucket credentials modes | No credentials, rescan, Git write or preference save from this page |
| Storage | Inspected six switches; native inactive-worktree/browser-artifact/log retention switches temporarily enabled, 8-day controls inspected, then all restored off | No cleanup invoked |
| Connections | Machine menu/icon, Add environment Remote link/SSH; native Routes and Add route; canceled | Primary-owner Electron and paired native client have different permissions; no network/access change |
| Archive | Empty archive during Settings pass | Populated archive behavior requires a fixture; not accepted from this empty state |
| Settings search | `font` returned five Appearance results; nonsense query returned no results; cleared | No new discrepancy |
| Diagnostics | Opened CPU details, history 5m/15m, process refresh, traces/failures/log tables | Shared backend lacks resource-monitor binary; not an Exact-specific failure. No process killed |
| Licenses | Loaded 519 notices; searched zero/one result; expanded MIT text | External links not opened |
| Files | Expanded tree; search/no-results; Markdown source/render, CSV table/source, HTML source/render; tab context actions and editor chooser; inline file edit then restore | Editing works. Existing focus/editing tasks retained; overflow-tab input limitation below |
| Diff | Branch changes, file expansion, stacked/split, wrap, whitespace, tree and scope controls | Shared fixture has inconsistent branch-diff data; no parity claim about changed-file count |
| Terminal | Opened panel and drawer, safe echo, horizontal/vertical splits, close confirmations; native echo output visibly returned | No new terminal defect. The banner already existed before terminal interaction |
| Panel shell | Opened launcher, available surfaces, maximize/restore, tabs/context menu; device setup then cancel | Browser/PR/linked surfaces depend on existing gaps or absent fixture prerequisites |
| Browser, reference | Loaded local HTML; history, preview menu, zoom, appearance menu, responsive/device presets, rotate, float/dock, annotation mode/cancel and screenshot notification | Native surface disabled under existing X1. No external browser, DevTools, cache clearing or annotation submission |
| Conversation | Shared synthetic assistant Markdown, code, table, two tools and checkpoints; expanded turn/work group; composer menus below | Clipped table rendering in the visual task; checkpoint diffs empty because fixture does not modify files |
| Composer | Provider/model picker, search empty state, favorites add/remove, four permission modes, slash commands, file mention insertion and empty skills | Functional composer picker is present; General's flat picker is a separate gap. Drafts were cleared without sending |
| Thread actions/sidebar | Title actions and Copy/Auto-settle/Snooze submenus; rename/edit cancellation; search/no-results and project filter; scratch pin/unpin and settle/unsettle; custom snooze canceled | Title Custom action tracked below. Native sidebar context-menu activation remained inconclusive; destructive actions excluded |
| Navigation and draft workspace | Command palette, Go to file, project content search, New thread; workspace/worktree choices and branch search | Native searches returned fixture results. No actual worktree/branch or provider turn created from the draft; one backend cannot exercise multi-backend selection |
| Usage | Limits/Cost/Tokens, all four periods, populated charts/totals, Model/Day breakdown and model detail; reference custom-price draft opened and discarded | Display inspected, not independently reconciled accounting or saved price/mapping verification |
| Pull Requests | Empty list, search, Sort, State/Involvement/Author/Labels/Draft/Review/Checks/Project and provider menus | No remote/PR fixture; populated details, comments/reviews and merge remain unverified |
| Plans | Synthetic Markdown plan and actions menu, including Copy, Download Markdown and Save to workspace | No implementation, plan download or workspace write started |
| Approval and structured question | Approval actions and extra-options menu; plan question choices, freeform composer, collapse/reopen | Approval left pending. Reference choice submitted the mock answer immediately; a fresh equivalent pending question was used for the native capture and left unanswered |
| Attachments | Deferred image opened in expanded preview; text opened in attachment panel; wrap roundtrip and copy controls | Exact bytes/MIME verified through official asset URLs. No save, message release or provider turn |

The initial settings passes saved 128 native screenshot/tree pairs and 121 reference
screenshots with 123 accessibility snapshots, followed by targeted captures. The action logs and named captures establish
the controls above. Files with a transient dialog or stale route are not acceptance evidence;
use the settled captures cited by the individual tasks.

## New tracking

| Record | Confirmed difference | Disposition |
| --- | --- | --- |
| [Invalid pairing URL](../tasks/closed/20261007-invalid-pairing-url-validation.md) | Reference validates malformed Welcome input inline; Exact shows a generic connection failure both inline and globally | New app behavior task |
| [Installed font picker](../tasks/20261007-installed-font-picker.md) | Reference enumerates installed families and rejects proportional Code fonts; Exact offers a fixed generic catalog | New app task, blocked by X48 for arbitrary family application |
| [Settings model picker](../tasks/20261007-settings-model-picker.md) | General's model menu lacks the reference's search, provider navigation, favorites and legacy grouping | New app behavior task |
| [Theme color picker](../tasks/20261007-theme-color-picker.md) | Exact swatch popup offers fixed presets; reference offers arbitrary hue/saturation/brightness and RGB input | New app behavior task |
| [Editable prompt preview](../tasks/20261007-editable-font-prompt-preview.md) | Reference Appearance sample accepts typing and undo; Exact's clicked sample remains static text/chips | New app behavior task, separate from preview cosmetics |
| [Title-menu Custom snooze](../tasks/20261007-title-custom-snooze.md) | Reference title action opens the dialog; Exact closes the menu without it, while its sidebar route works | New app behavior task |
| [Desktop visual parity](../tasks/20261007-desktop-visual-parity.md) | Typography preview icons/highlighting, selector alignment, and clipped Markdown table text | One grouped cosmetic task, as requested |
| [X48: runtime font family](../issues/20261007-x48-runtime-font-family.md) | Contract rejects state/data-bound font families; literal and finite literal-choice controls compile | Reproduced local framework draft; no external publication |

Each task includes baseline revisions, live reproduction, local evidence paths, source
guidance, deduplication and acceptance criteria. Their unverified status concerns a future
fix; the discovery evidence does not verify an implementation.

## Existing tracking retained

- [Managed Codex](../tasks/20261005-managed-codex-chatgpt.md) now records Welcome showing
  sign-in choices while the same backend's Electron Agents page already reports Ready.
  [Sign-in terminals](../tasks/closed/20261005-sign-in-terminals.md) is related existing scope.
  The sign-in hold stays in place; no duplicate task was created.
- [Abandoned-answer banners](../tasks/closed/20261007-let-go-banner.md) now records the
  observed `native.watch outside an answer` banner and a repeatable canceled-refresh probe.
  The first capture is after Changes → Uncommitted; the terminal merely retained that banner.
  The probe demonstrates a swallowed aborted `environments` read followed by unguarded
  `readLocalBackend`/`native.watch`. It does not prove the exact operation canceled in the
  original UI occurrence. Existing implementation/delivery status is unchanged and acceptance
  remains unverified.
- Browser/X1, app developer tools/X2, T3 Connect/X38, telemetry/X39 and updates/X40 already
  have explicit tasks/issues. Disabled surfaces or unavailable integrations were not filed again.
- The previous [minor UI task](../tasks/closed/20261007-fix-minor-ui-issues.md) retains its
  residual traits, mixed-option, menu-height, compaction and focus rows. The new visual task
  names only different controls or new rendering findings.
- File editing is implemented and was exercised. Existing
  [round-12 F2](../tasks/closed/20261005-round12-wrapup.md) and
  [input adoption](../tasks/closed/20261007-adopt-main-fixes-input.md) own its activation/focus
  history. A source preview before pressing the editor is not missing editing support.
- Actual scheduled execution, PR mutations, account usage and provider authentication remain
  in their existing tasks. Empty/read-only inspection here does not close their acceptance rows.

## Fixture effects and excluded claims

The rich transcript and approval request came from unchanged upstream mock peers launched
without inherited credentials, through public RPC operations. Their content is synthetic,
and their tools did not execute workspace changes. This provides populated rendering and
approval states, not real provider integration acceptance.

One reference-panel action had an unintended side effect: the primary Commit button
immediately invoked real Codex commit-message generation and committed the disposable fixture.
The reference audit restored that fixture to its original commit and dirty file contents with
a mixed reset; it did not push. The dropdown Commit action is the dialog entry. Thus this
session must not be described as having made no real provider calls. No existing user project
was involved. The reference Fork button also created a local fixture thread immediately.

The reference question-choice probe immediately answered its mock request. An identical pending
question was recreated under a separate isolated mock instance for the native inspection,
without restarting the backend. Native collapse/reopen was captured and the replacement remains
unanswered. A later unsent reference composer draft was explicitly discarded. No plan was
implemented and no real provider call occurred during this fixture extension.

Both clients showed the same branch-diff data problem: a large outer Exact checkout diff,
while the disposable project had only its intended modified/untracked files and Uncommitted
showed no changes. This shared backend/fixture result cannot establish a clone-only defect.

Direct native selection of overflowed file tabs was inconclusive. Native target/mouse attempts
and OS accessibility fallback did not establish an ordinary user click at the clipped tab;
the tab context menu's Close others operation did work. No new tab-selection defect is filed
without a normal-input reproduction. Source Control's All environments visibility also needs
an equal-role comparison before claiming a discrepancy.

Version-dependent sidebar artwork, development update banners, device/hub absence,
resource-monitor absence, masked account text and unequal initial capture widths are excluded
from cosmetic findings. The matched-width rich-thread captures support the table-clipping finding.

## Interpretation limits

Agent mode substitutes native OS grants/export behavior, uses volatile credentials,
and advances a deterministic clock. It does not prove normal-launch Keychain behavior,
OS permission prompts, real-time scheduling, or persistence across a normal app relaunch.
The embedded local server is disabled in this native audit session; both apps instead
connect to the isolated backend. That setup does not establish a local-server regression.

Provider sign-in, real account usage and external pull-request writes are not accepted by
this audit. Their existing tasks remain on the recorded sign-in hold. A disabled or empty
surface caused by missing credentials, repository remotes or device prerequisites is
reported as such, rather than treated as a new missing feature.

This audit does not accept normal app relaunch/update, all menu-bar/window commands,
destructive archive/delete flows, real PR detail/review/merge flows, OS screenshot import,
PDF/audio/video playback, populated multi-environment routing, or scheduled execution.
The local fixtures exercise supported controls without providing those prerequisites.

## Validation of this tracking change

- The native macOS app built successfully from the audit worktree and was launched and
  driven. The reference ran as real Electron from the byte-identical source copy described above.
- The X48 CLI probe ran on this revision: literal and literal-choice controls compiled;
  the runtime-selected family failed with the recorded `lower-font-family-literal` diagnostic.
- The canceled-refresh probe was rerun successfully and distinguishes a swallowed diff abort
  from the environment-refresh path that produces `native.watch outside an answer`.
- New record metadata, unique task-index entries, relative links and referenced evidence files
  were checked. Staged `git diff --check` and `bun scripts/caps.mjs` passed.
- Only `.exact` Markdown records changed. No product implementation or regression-test suite
  was changed; the full Cargo/boot gate and future task acceptance matrices were not run.

## Publication

The user authorized publication and merge of these tracking records on 2026-10-07.
[PR #241](https://github.com/ccheever/exact2/pull/241) targets `feat(example)/t3-code`.
The reviewed audit content is commit `7438bce85bdb6633999c1c404ec8bb1763b9a225`.
Independent read-only review found no blockers and validated the 13-file documentation scope,
seven unique indexed tasks, 46 added relative links and absence of credential-bearing additions.
This publication entry is bookkeeping only. Publishing the records does not change the future
tasks' implementation, verification or delivery status.
