---
name: 20261014-timeline-and-window-hooks
plan: 20261005-t3code-macos-parity
implementation: complete
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-timeline-and-window-hooks
pr_url: https://github.com/ccheever/exact2/pull/584
verified_commit: null
---

# Timeline and window hooks

## Outcome

Retire the app's native turn-visibility reader, transcript position restoration,
and full-screen fact after their public Exact replacements preserve the current
macOS behavior. A reader returns to the same place in each thread; the minimap
marks and jumps to the correct turn; full-screen title rows reserve the correct
space. This is serial follow-up **7 of 8**, followed by
`contract-keys-and-menu-bar`.

This is a portable preparation draft, not an implementation checkout. Reviewed
application base: `a218ee78fec42b1a4590b199297f30667876de67`. Consuming main pin:
`178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`. Wait for all six serial predecessors
and refresh from the resulting feature head before creating a branch. No newer
main adoption is authorized or needed by this ticket.

## Scope and exclusions

The authorized `fix-wave-plan.json` entry names exactly these rows. Its scope is
reproduced here so implementation does not require the planner's scratch file.

| Row | Required change | Boundary |
| --- | --- | --- |
| KW-12a | Put `intersect=` on user-turn rows and keep visible-turn state in the app. Remove native visibility measurement and the user-turn hatch once its remaining reader has migrated. | Keep `t3-frame` and `t3-anchor`. The shared `t3-turn` hatch still serves assistant citation targets unless those callers are also migrated and verified. |
| KW-12b | Save first-visible key/offset per thread from `ScrollEvent`; restore through keyed `scrollIntoView` after the selected transcript is ready. Replace the native minimap jump with a public command and preserve the 24-point lead. Remove R9's transcript restoration. | Keep R9 composer focus, marked-text handling and chord helpers. Preserve citation jumps and their relayout correction; they are separate consumers of the same native registry. |
| KW-12c | Use `exactViewport().displayMode == "fullscreen"` for title-row layout and remove `T3FullScreen.swift`. | Keep `T3WindowChrome.swift`'s 52-point toolbar, traffic-light placement, appearance and locale responsibilities. Titlebar geometry/configuration remains under #267. |

The original instruction that the minimap must jump instantly until #277 lands
is superseded by round 14: AppKit smooth element jumps are available, and keyed
list smooth jumps already existed. Request smooth movement for ordinary minimap
navigation; respect reduced motion. Frozen agent output cannot establish animation.

Target: macOS desktop only, the app's declared minimum macOS 14, 1280×840 and a
bounded 840×620 check. Use light/dark and interface-font changes where they affect
measured rows or title insets. No framework edits, new platform support, provider
messages, real-user T3 home, port 3773, remote Git actions, key/menu rework, or new
verification apparatus. KW-12d belongs to `menu-keys-popover-commands`. Do not
remove the unrelated `T3Timeline` composer-rest/end/pull observer.

## Context and guidance

All source paths below are relative to the repository root. Parent records live
under `examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`:
`plan.md`, `spec.md`, `tasks/20261013-r12-kept-workarounds.md`,
`tasks/closed/20261014-adopt-main-fixes-r14.md`, and
`reviews/20261014-first-wave-resume.md`. The plan's 2026-10-06 feature-branch rule
supersedes its older branch names; the cancelled downloadable-app work stays out.

Reference: T3 Code commit `1e2ecbd9758830669684b494d4398f626b0576e0`, MIT,
`LICENSE-T3`. The inspected reference checkout matched this pin. Relevant files:

- `apps/web/src/components/chat/MessagesTimeline.tsx`: identity/remembered state
  around 563, restoration and cancellation around 805–925, scroll capture/minimap
  around 1029–1100, minimap selection around 1388.
- `timelineScrollAnchoring.ts` in that directory: `RememberedTimelinePosition`,
  scoped-thread session cache with a 100-entry bound, around 145–174.
- `MessagesTimeline.logic.ts`: `resolveTimelineMinimapCurrentIndex`, around 342.
  The pinned reference calculates minimap visibility from list geometry on scroll;
  `intersect=` is the chosen Exact adaptation, not a claim that this reference
  function uses IntersectionObserver.
- `AssistantCitationSource.tsx`: citation lead `min(120, viewportHeight / 3)`,
  measured-position correction, cancellation, and size/position subscriptions.
- `apps/web/src/components/AppSidebarLayout.tsx`, around 246–282, and
  `apps/desktop/src/window/DesktopWindow.ts`: full-screen state and title controls.

Guidance snapshot: `implement/best-practices`, revision `20261005-platforms-v3`;
foundations, state/data, layout/interaction, motion/reduced motion, macOS and
verification. Metadata records source review at `fd1c879072aa` or `e45434ff217f`,
with no runtime-tested revision. Apply ownership/verification guidance, but use
current repo vocabulary. This app has already migrated to `if`, `for`, receiver
methods and `T[]`; do not copy the library's old `when`/`each`/`list<T>` examples.

Current authority: `docs/contract-grammar.md` ScrollEvent and host commands,
`docs/reference.md` Intersection visibility, `llp/1070.000-scroll-into-view.rfc.md`
§§13–14, `llp/1039-viewport-facts.rfc.md`,
and the three live issue files below. The grammar's intersection paragraph still
says native delivery is owed; the newer reference text, live #127 record and
shared sampler/Apple tests show the implemented native slice. Do not infer a
missing API from that stale sentence. Read `docs/testing.md` before changing tests.

## Dependencies

Checked read-only on 2026-10-14 with GitHub PR metadata, `git ls-remote`, local
ancestry, local issue records, and the pinned reference. GitHub #127/#267/#277 are
closed copies transferred to repository `issues/`; closure is not resolution of
their remaining capabilities.

| Dependency or issue | Kind / capability | Evidence at reviewed base | Impact and next action |
| --- | --- | --- | --- |
| Eleven first-wave repairs | Merged task PRs | #531, #533, #534, #539, #540, #541, #544–#548 all remotely merged into `feat(example)/t3-code`; checkpoint `1e3152209` is an ancestor | Nonblocking; preserve their changes, especially Cite, Markdown and native input. |
| Round 14, [#549](https://github.com/ccheever/exact2/pull/549) | Merged task PR | Merged 2026-10-14 03:08:53 UTC as `3710af842bc54521c298124156989950b6f2e68c`; main `178ae7237` and merge commit are ancestors of `a218ee78` | Nonblocking; keep this framework pin. |
| `dialog-focus-return-and-traps` → `legacy-sidebar-parity` → `connections-network-settings` → `menu-keys-popover-commands` → `browser-panel-parity` → `css-workaround-retirements` | Six merged task PR prerequisites, in order | Satisfied: #553 `6a7a3cad0`, #557 `4520b0124`, #566 `fbed7fadd`, #568 `b229a2658`, #571 `70eec59fd`, #577 `6ef85eb1c`; all are ancestors of prepared base `97f3fbb23`. | Nonblocking; consume their merged interfaces (menu-keys popover commands, browser placement, KW-6 position areas). |
| Timeline implementation duplication | Task/branch reconciliation | Rechecked 2026-10-15: no PR for the exact head or task name | Nonblocking. |
| [#127](https://github.com/ccheever/exact2/issues/127), `issues/20261009-intersection-visibility-event-design.md` | Resolved bounded framework capability; broader issue open | AppKit shared sampler, mount/enter/leave, generation checks and presented-scroll ordering; sticky/rotated qualification adopted in r13 (`c016978a6`) | Nonblocking for ordinary user-row boxes. Consumer recycling/edge behavior must pass. Animated presentation geometry, SVG clips, inline runs and remaining sticky cases do not authorize deletion of geometry hooks. |
| [#277](https://github.com/ccheever/exact2/issues/277), `issues/20261009-scroll-padding-settlement-restore.md` | Resolved bounded framework capability; remainder open | `c099b17b7` key/offset, `c5d3b3f8e` spacing/AppKit smooth, and `46f300937` scrollend are ancestors. App-owned restore has compiler, runner, browser and bordered-list AppKit evidence. | Nonblocking for implementing macOS key/offset restore. Real-clock animation and trackpad settlement remain required qualification. Agent motion is frozen. Other hosts are outside this app. |
| [#267](https://github.com/ccheever/exact2/issues/267), `issues/20261009-titlebar-env-display-mode.md` | Partly resolved framework issue | `11483d6fd` public `displayMode` is an ancestor; `DisplayModeMacTests` supplies state/notifications. Env geometry and manifest positioning remain open. | Nonblocking for removing the duplicate full-screen fact. **Blocking for retiring window geometry**, which is excluded here. Real full-screen entry/exit remains unverified. |
| X19, plan issue `issues/closed/20261005-x19-data-source-timers.md` | Recorded deferred capability | `timelineSleep` remains the declared bounded native wait used by rollback and citation flows | Nonblocking; preserve the helper and router. Do not replace it with unsupported data-module timers. |
| Consumer collection lifecycle | Required application verification | Current transcript is explicitly `list appearance="none" virtualized=true`, one direct `for message in data.messages key=message.id`, `app-main.contract:168` | No conversion from a plain scroller is needed. Verify initial/restored events and recycled-row cleanup on the fresh base. The earlier nonvirtualized description was an assumption, corrected by source inspection. |
| Desktop availability | Verification environment | The desktop is unlocked with keep-awake on (2026-10-14). | Attended rows (real full-screen entry/exit, IME, real-clock motion and trackpad settlement) are batched into the one final real-input session after follow-up 8; until then they stay unverified. |

Remote feature HEAD matched `a218ee78` at the first check. At preparation on
2026-10-15 the six predecessors had merged and the prepared base is `97f3fbb23`.
Remote main was not adopted into this task.

## Implementation notes

### Ownership and current callers

| Owner | Current responsibility | Migration rule |
| --- | --- | --- |
| `app-main.contract` ChatColumn | Keyed virtual transcript, `scroll=citeScrolled`, end-follow, `foldHold`, forced end jumps, composer overlay | Compose the existing Cite scroll callback with the new full ScrollEvent handler. Preserve all end/fold/history/composer behavior. |
| `timeline.contract` | User `t3-turn` at 122; assistant `cite:` target at 314 | Observe the actual user-turn box, not work/assistant rows. Keep assistant registration while citations use it. |
| `timeline-minimap.ts`, `timeline-presentation.ts`, `timeline-minimap.contract` | Native in-view/above status, current strip, `timelineJump` routing | Move the migrated state and minimap command to public Contract/data paths. Remove obsolete native status consumption, rather than run two writers. |
| `T3TimelineTurns.swift` | Visibility measurement, row registry, minimap/citation jumps, estimated placement and two-second relayout hold, `sleep` | Remove only migrated responsibilities. The whole class, shared hatch, jump route and sleep cannot disappear while citation/rollback callers remain. |
| `R9Input.swift` | Position cache/settling/restore plus composer responder observation, marked-text commit and physical chord helpers | Remove transcript save/restore and its observers only after new behavior passes. Keep composer installation, monitor and helpers used by `T3Composer.swift`. |
| `T3Module.swift`, `T3Module+Timeline.swift`, `app.json` | Status merge, hook install/remove/destroy, native routes and declared hatches/data | Clean up retired wiring only after checking every caller and test. `t3-transcript` still serves `T3Timeline`. |
| `T3WindowChrome.swift`, `T3FullScreen.swift`, `presentation.ts`, `shapes.contract` | Native full-screen fact, locale, controls inset and sidebar minimum | Add public `displayMode` to the viewport shape and thread one source of truth to every consumer; remove the duplicate fact, retain locale/appearance/geometry. |

**Visibility.** Keep state scoped to the selected environment/thread and current
row incarnation. Duplicate entries must be no-ops; reordered data must not reorder
current selection by callback arrival. The first visible user turn wins; when a
long assistant/tool row fills the port, use the last preceding user turn from
ordered rows and the first-visible list key. No user turn means no current index.
Do not depend on a mounted preceding user view being present.

`intersect=` reports mount and threshold-zero enter/leave, not continuous ratio
updates. Removal sends no leave. Filter/reset state on owner and row changes and
handle virtual-window recycling explicitly; an ID still present in `data.messages`
is not proof that its old observation remains mounted. Cover a large jump that
removes a previously visible row before a leave callback. Use existing public
mounted-ID/action geometry reads if needed, with declared IDs/data words; no new
Swift visibility calculator or per-frame polling. Edge contact can report
`isIntersecting=true` with zero ratio; the reference's strict positive overlap
must not leave an edge-only strip falsely active. Resolve and test this with the
actual row rather than assuming `ratio > 0` gets another callback automatically.
If public lifecycle support proves insufficient, reproduce and record the gap
before removing the old behavior. Do not silently weaken the minimap requirement.

**Position and jumps.** Save `{key, offset, atEnd}` and any needed fallback position
in bounded session state, at most 100 scoped thread identities. Reuse the existing
scoped-thread-key conventions, not a bare thread ID, and distinguish an old
resource generation from the new owner. A route-origin change must not mix two
environments. `ScrollEvent.firstVisibleKey` is `option<string>`; absent keys,
empty/padding-only lists and stale rows must not overwrite a valid saved anchor.
The offset is signed CSS pixels into the keyed **row wrapper**, not into its
nested `t3-turn` view. New anchors can name assistant/work rows too.

Restore once the intended owner's rows and list are ready, including when the
list stays mounted across a thread switch. Do not rely only on a resize callback
or set `scroll-restoration="auto"` and assume it creates a top-level cache.
Use `scrollIntoView("transcript", key, offset=savedOffset, behavior="instant")`;
the runner's bounded keyed correction handles measurement. End-follow, initial
`scrollTop=1000000+jumpRequest`, reference citation positioning and restore must
not compete. Suppress saving old-owner/default-end/intermediate restoration
reports; resume after settlement or user takeover. Preserve the last valid
position when switching during a gesture's settling tail. A wheel, scroll key,
press or new explicit navigation cancels pending restoration. Missing/deleted
keys use the reference fallback, never a different row selected by stale index.

The minimap's 24-point lead is a visual destination, not `offset=24`: the public
offset is port-start minus row-start. At the same row origin, 24 points below the
port is `offset=-24`. Alternatively use alignment with `scroll-padding-top=24`
and no explicit offset. Explicit offsets override padding/alignment. Account for
the row's history control, leading padding and inner bubble origin before
claiming parity. Test middle and boundary rows, including an initially unmounted
target. Request smooth behavior normally and instant under reduced motion.

Citation remains a regression requirement: `r5-composer-citation.ts` calls
`timelineJump` with `cite:<id>`, `lead:"citation"`, `inset:2`, and uses
`timelineSleep`; rollback uses the same sleep. Keep their scroll placement,
thread switching, changed-quote warning, and relayout hold until those complete
call paths have replacements. Never leave a native jump/restore and Contract
jump/restore independently repositioning the same transcript.

**Full screen.** Add `displayMode: string` to `Viewport`. Replace
`client.presentation.fullScreen` at the shared layout boundary, rather than
patching only ChatHeader. Cover new/legacy sidebar, Home, PRs, Usage, Settings,
sidebar minimum width and interface font size. Keep the existing 90-point
windowed inset and `0.75 × interface font size` fullscreen inset. Delete the
`T3FullScreen` instance, its callbacks/status field and its dedicated test only
with equivalent public-fact consumer coverage. Preserve `systemLocale`, theme
appearance, 52-point toolbar and x=16 traffic-light positioning.

Re-read predecessor changes before editing shared contracts. First-wave Cite
owns `citeScrolled`; Markdown owns row/media lifecycle; dialog/menu/browser/CSS
predecessors touch the same window, popovers and root callbacks. Root
`app.contract` is already 1,498 lines at the inspected base. Keep state near its
owner and stay under the 1,500-line cap without broad unrelated extraction.

## Acceptance and reproduction

Do not execute this section until prerequisites are satisfied. This preparation
ran no tests/builds/apps. Source inspection and inherited reports are not a new
runtime pass.

| Case | Required evidence |
| --- | --- |
| KW-12a visibility | Initial mount, enter/leave, duplicate callback, multiple visible turns, long assistant-only viewport, top/bottom edge contact, row removal, fast distant jump/recycling, prepend/reorder, resize and thread/environment change. Compare current strip and all visible strips; stale IDs never survive. |
| KW-12b restore | Two threads at different offsets; same thread ID in two environments; key plus signed offset; prepend/changed height; empty/loading and missing key; at-end reopening; switch while momentum settles; old-owner events rejected; wheel/key/press interrupts. Restored port differs by at most 1 CSS px from its saved anchor after settlement. |
| Minimap/citation | First/middle/last and unmounted turn, both chevrons, 24-point lead with clamping, changed row size, reduced motion. Existing citation source lands at its own lead and stays aligned after relayout; Cite action still dismisses on scroll. |
| KW-12c | Public display mode drives every title row and sidebar minimum at interface sizes 12/16/20; windowed → fullscreen → windowed restores the original inset. Locale/appearance/geometry still work. |
| Preserved input | R9 click-focus bridge, marked text then Command-B/Command-I, outside press committing composition, Korean physical chords, reopened field select-all. Do not delete these tests with the restoration tests. |
| Native ownership | No native visibility/status or Swift restore remains for migrated paths; retained citation/sleep and geometry callers remain reachable. One effective writer per navigation. |

Extend existing behavioral tests: `timeline-minimap.test.ts`,
`r5-composer-citation.test.ts`, `timeline.test.ts`, `r4-composer-overlay.test.ts`,
`desktop-shell-details.test.ts`, `r12-sidebar.test.ts`, and affected Contract action
coverage through `contract-action-fixture.ts`. `r9-input.test.ts` concerns panel
keys/sidebar hover, not transcript restoration; preserve it but do not call it
restore proof. Port R9's actual restoration cases before removing their native
owner. Source-string guards are cleanup inventory, not behavioral acceptance.

Native tests to retain/adapt: `macos/tests/r9-input/main.swift` contains the
momentum/owner/key-offset cases and independent composer/IME cases;
`macos/tests/r5-composer/main.swift` verifies citation lead and minimap exclusion;
`macos/tests/menus/main.swift` has the old full-screen notification test plus
unrelated menus; `macos/tests/composer` and `timeline-keyboard` cover input/scroll
regressions. Use the existing README recipes under "The module AppKit/XCTest
binaries" and "timeline-keyboard". Preserve suite counts and unrelated assertions;
do not delete an entire test binary because one native reader retires.

From the prepared checkout, Bun 1.4.2 first on PATH:

```sh
bun install --frozen-lockfile
bun test ./examples/t3-code/timeline-minimap.test.ts ./examples/t3-code/r5-composer-citation.test.ts ./examples/t3-code/timeline.test.ts ./examples/t3-code/r4-composer-overlay.test.ts ./examples/t3-code/desktop-shell-details.test.ts ./examples/t3-code/r12-sidebar.test.ts --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/timeline-window.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Final checks include the full app Bun suite, app Rust tests, and the repository's
current required checks with keep-going/no-fail-fast flags, including the workspace
check added by the pinned main. Use the current AGENTS.md/RULES.md, not an old
five-check count. Reuse coordinator evidence only when it covers the final inputs.

For one bounded native pass, reuse `scripts/agent.mjs`, the existing isolated
lane/fixture and README test commands. Stage the official embedded runtime with
`bun examples/t3-code/stage-runtime.mjs` and build the terminal assets if absent;
never substitute a placeholder archive or a runtime sentinel. Use a named scratch
store, an isolated `T3_LOCAL_HOME`, a free non-3773 lane port, and telemetry off.
Record actual fixture setup and binary/source identities in the implementation
attempt. No new fixture generator is authorized by this draft.

The existing integration recipe is `target/fix-wave/native-smoke.md` and its
corresponding driver. It illustrates isolation and bundle resolution; its seeded
empty thread is insufficient for this task. Obtain an existing controlled
transcript with at least three user turns, tall assistant/work rows, two threads,
a citation and a history/relayout case. Record the missing fixture as blocked if
none exists; do not send a provider prompt to manufacture history.

1. On the baseline and final builds with the same fixture, capture transcript
   `layout`, tree, relevant state and a screenshot. Scroll partway through a row,
   switch threads and return. Repeat after prepend/size change and at end.
2. Exercise a distant minimap jump and both chevrons; check the target's measured
   lead, selected strip and visible-set cleanup. Open a citation, force the
   existing fixture's relayout, and verify placement and scroll dismissal.
3. Use `prefer display-mode fullscreen` and `standalone` for consumer wiring
   only; read `state.device.displayMode` and title layout. This does not prove a
   real OS fullscreen transition.
4. In the coordinator's unlocked desktop slot, record real full-screen entry/exit,
   real-time smooth movement/settlement and gesture interruption, and the scoped
   R9 Korean composition regression. The frozen agent and synthetic notification
   tests cannot substitute for these. Keep unavailable cases unverified/blocked.

The completed round-14 integration evidence is a startup/navigation baseline,
not timeline proof. Its durable packet is linked from the r14 task. The prior
branch-navigation PNGs are transparent and are not a visual oracle. Inspect new
images before making a visual claim. No repeated full pixel matrix is required.

## Progress

| Stage | State |
| --- | --- |
| Scope and current callers | Reconciled read-only at `a218ee78`; actual keyed virtual list confirmed. |
| Framework capabilities | Checked against pinned main source, live issue files and recorded upstream tests. |
| First wave / r14 delivery | Actual remote merges and local ancestry verified. |
| Six serial predecessors / fresh base | Merged; prepared from `97f3fbb23` on 2026-10-15. |
| Implementation branch/worktree | `feat(example)/t3-code-timeline-and-window-hooks` in `t3-code-timeline-and-window-hooks`. |
| KW-12a, KW-12b, KW-12c | Implemented 2026-10-15 (below); no framework gap blocked a row. |
| Tests, builds, checks | All pass (below). |
| Native AFTER drive | Run once (agent mode) plus one supplementary 840×620 segment; agent rows PASS, real-input rows UNVERIFIED. |
| Independent review | Done before commit (below). |
| Verification | Unverified: the final real-input session after follow-up 8 owns full-screen entry/exit, real-clock motion and trackpad settlement, IME. |

## Attempts and evidence

Preparation date: 2026-10-14. Checked parent plan/specification, r12/r13/r14 records,
plan-local X22/X23/X27/X19 records, live main issues #127/#277/#267, existing tests,
public syntax/source and the pinned reference. Read-only GitHub checks found the
twelve prerequisite merges, no exact timeline task PR, and only unrelated iOS PR
#216 open against the feature branch. Local ancestry confirmed `178ae7237`,
`3710af842`, `1e3152209`, `c099b17b7`, `c5d3b3f8e`, `11483d6fd` and `46f300937`.

No implementation attempt, tracked-file edit, build/test execution, branch creation,
app launch, native/global input or remote mutation occurred. The completed parent
smoke was not rerun. Future attempts must record source/framework/binary identities,
fixture/reset commands, assertions, outputs, limitations and accessible evidence.
Preserve failures and the repository's three-round fix-loop limit.

## Implementation and verification, 2026-10-15

Base `1225616f0` (feature head `97f3fbb23` + ticket). App-only: every change is under `examples/t3-code`.

### Per row

- **KW-12a (visible turns).** `timeline.contract` UserRow: `intersect=seen(message.id) intersect-margin="-1px 0px"` on the
  user-turn box; the user-turn `hatch="t3-turn"` is removed (the assistant `cite:` hatch stays). The -1px root margin makes
  edge contact (threshold-0 `isIntersecting` with ratio 0) count as off screen, and an overlapping box report on entry, so
  no second ratio callback is needed. ChatColumn (`app-main.contract`) keeps `turnsSeen`/`turnsOwner` and the list's
  `firstKey`; `timeline-minimap.contract` `turnRunStart`/`turnRunEnd`/`turnBefore` derive the strips on screen (one
  consecutive run from the first turn at or after the first visible row, so rows recycled without a leave never survive)
  and the current turn (the first on screen, else the last turn before the first visible row, else none). Owner changes
  reset the set. `timeline-minimap.ts` keeps only the strip items (now with `row`); `nativeTurns`, `minimapCurrent`,
  `jumpToTurn`, the `jump` op, `data.minimapCurrent` and the `turnsInView`/`turnAbove` status merge in `T3Module.swift`
  are gone. `T3TimelineTurns.swift` no longer measures or observes bounds.
- **KW-12b (position and jumps).** `timeline-anchor.contract` (shape `TimelineAnchor`, `anchorOf`, `anchorKept` ≤ 100,
  `anchorKey`). ChatColumn `transcriptScrolled(left, top, e)` calls `citeScrolled` first, then keeps
  `{key, offset, top, end}` only while the open owner's restoration has run and no command (a thread switch's end jump)
  is pending; absent or stale keys never overwrite. A keyed zero-size marker beside the list
  (`for owner in [composerOwner] key=owner`, `intersect=transcriptOpened(owner)`) reports when each owner's transcript is
  laid out, including when the list stays mounted; tasks are root-only and `app.contract` is at 1,500 lines, so this
  component-local mount report replaces a root task. `transcriptOpened` files the left owner's anchor and restores this
  owner's with `scrollIntoView("transcript", key, offset=…, behavior="instant")`; a gone row falls back to its saved offset
  through `scrollTop` until the next end jump; a thread left at its end, and a citation in flight (`data.citing`, set by
  `chip-citation` in `timeline-presentation.ts`), restore nothing. `turnJump` gives strips and chevrons
  `scrollIntoView(…, offset=-24, behavior="smooth")`, `"instant"` under reduced motion. `R9Input.swift` keeps composer
  focus, marked-text commit and chord helpers; its transcript cache, samplers, settling and restore are deleted (with
  its `agent` flag and `remove`). Citation jumps, `timelineSleep` and the two-second relayout hold stay native.
- **KW-12c (full screen).** `Viewport` gains `displayMode`; `app.contract` passes `viewport.displayMode` to `snapshot`, and
  `presentation.ts` derives `controlsLeft`/`sidebarMinWidth` from it (one source for chat, sidebar, legacy sidebar, Home,
  PRs, Usage and Settings title rows). `T3FullScreen.swift` is deleted; `T3WindowChrome.swift` keeps the 52-point toolbar,
  x=16 traffic lights, appearance and `systemLocale`. `T3Timeline`'s composer-rest observer is untouched.

### Tests and checks

| Check | Result |
| --- | --- |
| Focused Bun (minimap 11, anchor 11, desktop-shell-details 34, citation 5, cite-selection-anchor 24, timeline, r4-composer-overlay, r12-sidebar) | all pass |
| Full app Bun suite, 336 tracked `.test.ts` files (explicit `./` paths) | 5,222 pass, 1 skip, 0 fail (base 5,201 / 1 / 0 at css-workaround-retirements) |
| Strict `tsc` on `app.ts` | exit 0 |
| `contract build app.contract` | exit 0 (warnings only, as before) |
| `cargo test -p t3-code-macos --lib --no-fail-fast` | 45 passed (new `timeline_hooks_tests`: the snapshot request follows `displayMode` standalone → fullscreen → standalone) |
| AppKit binaries (README recipe) | r9-input 6/6 (composer and IME cases kept; its 7 restoration cases ported to `timeline-anchor.test.ts`), r5-composer 3/3 (citation lead, the relayout hold, and no native writer after `cancel()`), menus 47/47 (full-screen fact test replaced by a window-chrome status/title-row test), composer 78/78. `real-key-chords` does not compile at the base either (`R10Connect.install`, pre-existing, untouched) |
| `git add -A examples/t3-code && bun scripts/caps.mjs` | all budgets within cap; `app.contract` 1,500, `app-main.contract` 451 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo check --workspace --all-targets --keep-going` | exit 0 |
| Native bundle `build.mjs t3-code-macos --bundle` | exit 0, twice (before and after the review repair); final executable SHA-256 prefix `6d8f83ae897e1075` |

Test changes: `timeline-minimap.test.ts` (rewritten: ChatColumn's actions/derives through `contract-action-fixture.ts`),
`timeline-anchor.test.ts` (new, the R9 restoration cases), `desktop-shell-details.test.ts`, `r5-composer-citation.test.ts`,
`cite-selection-anchor.test.ts` (Cite still hides on the composed scroll handler); `contract-fn-fixture.ts` now holds the
`match`-aware `contractFns` helper the cite test had (deduplicated); `contract-action-fixture.ts` `state()` takes a scope.
All checks above were rerun after the review repair with the results shown.

### Native AFTER drive

Agent mode (`scripts/agent.mjs` `open`, storage `timeline-hooks-a`), the bundle's executable (`EXACT_MAC_BIN`) with its own
embedded 0.0.46-nightly runtime staged by `stage-runtime.mjs --offline`, port 16650, telemetry off, isolated homes copied
from the existing audit lane `ri1012-b10` into `target/timeline-hooks/lane` (a 4-turn and a 2-turn thread plus the
verification thread). Fixture setup: the copied home's `q2` user message text was given a `[Assistant quote](t3-citation://…)`
link to thread B's first answer (one SQL update in the copy; no provider message, no new generator). Evidence:
`target/timeline-hooks/drive/after/` (1280×840, light then dark), `drive/after-840/` (a supplementary 840×620 segment for
the unclamped 24-point lead) and `drive/retry/` (the one retry, on the bundle after the review repair: every row below
reproduced with identical numbers). The app and its server were stopped by the driver each time; port 16650 is free.

| Row | Result | Evidence |
| --- | --- | --- |
| Scroll thread A partway, switch to B and back: same place | PASS: A saved row a1 +411.9 (top 526.65); after B and back, the same anchor and identical user-turn tops [-502.65, 321.5, 501, 680.5]; `scrollIntoView` done | `A02`, `A05` readouts and PNGs |
| Each thread keeps its own | PASS: B saved a1 +116.9; restored identically after A | `A04`, `A06` |
| Minimap marks visible turns while scrolling | PASS: at the end and scrolled, u2–u4 bright, u1 dim; after a jump to the top only u1 (stale reports dropped); dark too | `pr-images/02`, `A01`, `A02`, `A07-*` |
| Minimap click with the 24-point lead | PASS: strip 0 → first turn at y=24; at 840×620 strip 1 → its turn at y=24 (1280×840 clamps middle turns at the end, as expected); chevrons previous/next jump at 1280×840 | `A07-strip-0`, `C02-strip-1`, `A07-previous`, `A07-next` |
| Citation jump | PASS (agent): View source opened B with the cited answer at its lead (clamped at the top: row start 114.75 < 118), held for 2.6 s; no remembered position overrode it | `A08`, `pr-images/04` |
| Title row windowed / `prefer display-mode fullscreen` / windowed | PASS (consumer wiring): `controlsLeft` 90 → 12 → 90, toggle x 91 → 13 → 91, `sidebarMinWidth` 208 | `title-*` log rows, `pr-images/03` |
| 840×620 and dark | PASS: restored thread and strips render | `B01`–`B03`, `C04` |
| Minimap jump right after View source (one writer) | Inconclusive live: the strip press landed on thread A before the cited thread opened (`A08b`). Covered by the r5-composer AppKit test (`cancel()` leaves the port alone) and `timeline-minimap.test.ts` (`turnJump` sends `timeline-release` first) | `drive/retry/A08b-*` |
| Real full-screen entry/exit, real-clock smooth motion and trackpad settlement, reduced-motion animation, IME/Korean chords | UNVERIFIED: final real-input session | — |

PR images (local): `target/timeline-hooks/pr-images/01-each-thread-keeps-its-place.png`,
`02-minimap-visible-turns-and-jump.png`, `03-title-row-windowed-fullscreen.png`, `04-citation-lands.png`.

### Gaps and known differences

- Framework gaps: none blocked a row. Tasks are root-only and the root is at its line cap, so restoration is triggered by a
  keyed marker's `intersect=` mount report rather than a task.
- Known differences: a press or scroll key does not cancel a pending restoration (the runner cancels on reader travel; the
  instant request settles within a few reports). The reference's whole-row in-view test is the user-turn box here, so a
  turn showing only its 16pt gap is off screen. Under `prefer display-mode fullscreen` the window is not really in full
  screen, so the real traffic lights still draw over the 12pt inset (emulation only). At 840×620 with the sidebar open the
  minimap is hidden and its chevrons inert, as in the reference (gutter < 48).

### Independent review

A fresh reviewer read the ticket and the staged diff and reported two blocking defects, both fixed and retested:

1. A recycled row next to the new run stayed lit (no leave is sent). Fix: UserRow's box has
   ``id=`turn-${message.id}` ``, and `transcriptScrolled` drops reported turns whose `frame()` is `unavailable` (scroll reports
   precede the intersect reports of the same pass). Test: the adjacent-stale case in `timeline-minimap.test.ts`.
2. A minimap jump no longer ended the citation's native hold (two writers). Fix: `turnJump` first sends
   `chatLocal("timeline-release")` → `timelineRelease` → `T3TimelineTurns.cancel()`, which releases the hold and bumps a
   placement number that the pending retries and the animation completion check.

Non-blocking notes taken: a stale `fallbackJump` is now cleared whenever another owner opens; the X22 rows of
`EXACT2-GAPS.md` say the minimap uses `intersect=`. Left as noted: the marker's own enter/leave reports would rerun
restoration with the current anchor (the marker sits at a fixed place, so none were seen); `data.citing` arrives with the
snapshot that switches the thread (observed in the drive: no override).

### Real-input session realinput-1015, 2026-10-15

Bundle of `db3c4540f` (code `aa4fd19c3`) in a lane copy (`com.exact.t3code.macos.ri1015-1`) launched through LaunchServices (a normal launch, not agent mode), port 16915, isolated homes from audit lane `ri1012-b10` plus the real-GitHub lane's sandbox (read only). Real keys and pointer through `orca computer`; the input source was `com.apple.keylayout.2SetHangul` (read only, not changed). While a painted popover or dialog is open the window's accessibility tree collapses and `orca` refuses input, so those keys went as System Events key codes to the lane (front process checked first). Evidence is local in the session worktree's `target/realinput-1015/` (`notes.txt`, `ev/`). 32 steps in all: 27 passed, 2 failed, 3 not verified.

| Row | Result | Evidence (`target/realinput-1015/ev/`) |
| --- | --- | --- |
| Real full-screen entry and exit (step 32) | PASS: View › Enter Full Screen (by Help search and Return) and Exit Full Screen; the window ends windowed at 1280×840. Difference: in full screen an empty 52-pt titlebar band covers the title row | `s32-titlerow-windowed-vs-fullscreen.png`, `s32-f-small.png` |
| Real-clock smooth motion (step 30) | PASS: a minimap jump moves through three intermediate positions in about 350 ms and then holds the first turn at the top | `s30-burst-sheet.png` |
| Each thread keeps its place (step 31) | PASS: switch to another thread and back restores the same place | `s31-restore-pair.png` |
| Trackpad/wheel settling (step 31) | UNVERIFIED: two `orca` wheel scrolls moved nothing; tool and app cannot be told apart | `s31-g*.jpg` |
| Reduced-motion animation, IME/Korean composition | UNVERIFIED: no system-setting change; input source read only | — |

## Next action

After realinput-1015 (2026-10-15): Keep `verification: unverified`. realinput-1015 passed real full-screen entry/exit (with a covered title row, now in `STATUS.md`) and real-clock minimap motion; wheel settling, reduced motion and IME stay open.

Merged as [#584](https://github.com/ccheever/exact2/pull/584), feature commit
`47f3ed1bd` (squash). The task stays unverified for real full-screen entry and exit,
real-time smooth motion, trackpad settling, reduced-motion animation, IME and Korean chords.
Known differences: a click or scroll key does not cancel a pending restore; the visible-turn
test uses the user-turn box, not the whole row; in emulated full screen the traffic lights still
draw. Follow-up 8, `contract-keys-and-menu-bar`, starts from this feature head.
