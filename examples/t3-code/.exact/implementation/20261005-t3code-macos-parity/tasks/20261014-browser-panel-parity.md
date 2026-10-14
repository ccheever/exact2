---
name: 20261014-browser-panel-parity
plan: 20261005-t3code-macos-parity
implementation: complete
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-panel-parity
pr_url: https://github.com/ccheever/exact2/pull/571
verified_commit: null
---

# Browser panel toggle, Appearance submenu and viewport size

## Outcome

Complete serial follow-up 5 of 8, `browser-panel-parity`. Preserve all four audit rows,
with G4-18 and G5-A3 resolved by the user on 2026-10-14: match the pinned reference.
The user selected “Match the reference; fix same-tab return, submenu behavior and
viewport sizing.” No further scope approval is needed for that choice.

| Row | Required result |
| --- | --- |
| G4-18 | Shift+Command+J hides and shows the Browser panel with the reference’s floating preview on hide; reopening restores the same selected tab and session, without opening a new tab. |
| G4-19 | More > Appearance opens on hover beside its parent menu. Moving into the submenu keeps it usable; moving to a sibling closes it. Zoom minus stays visible and usable. |
| G4-16, viewport half | A fixed or responsive page reports `innerWidth` and `innerHeight` equal to its requested size fields, including at fractional zoom and fitted presentation sizes. |
| G5-A3 | Preserve the reference’s omitted Safari cookie counts. The user declined the intentional difference; retain this audit row as a resolved premise, not a request to add counts. |

This is a portable preparation draft, not implementation readiness or runtime verification.
The dialog, legacy-sidebar, connections, and menu-command predecessors must merge first.
Refresh the feature base afterward. The user has resolved both reference conflicts:
retain float-on-hide and omitted Safari counts, and repair same-tab return, submenu
behavior and viewport sizing.

## Scope and exclusions

Target the existing Apple Silicon macOS application, with macOS 14 as its supported minimum.
Use the available Mac for verification and record its actual OS/build. Keep the existing
1280x840 default and 840x620 minimum layouts usable. One bounded native acceptance sequence
is required after implementation; repeated pixel-polish matrices are not.

Application paths in this document are relative to `examples/t3-code/` unless marked as
repository or reference paths. Expected owners are:

- `browser-navigation.ts`, `browser-surface.ts`, `r4-surfaces-panel.ts`,
  `keyboard-dispatch.ts`, and `settings-shortcuts.contract` for the shortcut and tab state.
  Inspect the existing `app.contract` `panelUi` binding, but keep new browser behavior in
  its existing owner. `app.contract` is already 1,498 lines at the inspection base.
- `browser-surface.contract` for More/Appearance. Consume the preceding task's final
  `menu-keys.contract` interface; do not restore its retired invisible invokers.
- `browser-stage.contract`, `browser-viewport.ts`, and the app-owned Swift files
  `modules/apple/T3BrowserView.swift`, `T3BrowserSessions.swift`, and
  `T3BrowserAutomation.swift` for size ownership and initial/parked views.
- `browser-import-sources.ts`, `browser-import-service.ts`, `browser-import-safari.ts`,
  `browser-profiles-settings.ts`, and existing import/profile tests for regression coverage of
  the retained Safari behavior. `browser-defaults.contract` and `browser-profiles.contract` are
  consumers to preserve; they do not need a new count format or new settings.

Keep changes within the application. Framework/compiler/runner/host changes, Safari
private APIs, new browser engines, global menu replacement, recording/download changes,
whole-page mute, new scripts or fixture servers, and general CSS cleanup are excluded.
The cursor-image half of G4-16 belongs to main PR #527 and must not be implemented here.
Do not change explicit Float preview, separate-window behavior, or another panel's close
semantics to fix this shortcut. Preserve PR #445's recording queues, download ownership,
session lifetime, and the distinction between agent requests and user panel actions.

Preparation permits only this ignored draft. It authorizes no task branch, tracked edits,
build, test execution, native launch, global input, remote write, Safari access, or access
to a person's browser home. Future acceptance uses controlled fixtures only.

## Context and guidance

Repository: `https://github.com/ccheever/exact2`. Parent records are
`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/{plan.md,spec.md}`.
The current integration branch is `feat(example)/t3-code`; older `daehyeon/t3-code` prose
is historical. The original coordinator task had rows G4-18, G4-19, G4-16 and G5-A3, with
Bun tests, Contract compilation, and one before/after/reference native drive. Those
requirements are reproduced here so the machine-local `fix-wave-plan.json` is unnecessary.

Source inspection base: `a218ee78fec42b1a4590b199297f30667876de67`.
Adopted framework: main `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`.
Reference repository: `https://github.com/pingdotgg/t3code`, read-only pin
`1e2ecbd9758830669684b494d4398f626b0576e0`. Do not move that pin to make a discrepancy vanish.

Read these relevant reference paths before editing their clone:

| Behavior | Reference paths and important locations |
| --- | --- |
| Shortcut and panel | `packages/shared/src/keybindings.ts:33`; `apps/web/src/routes/_chat.tsx:140`; `components/ChatView.tsx:5628-5665`; `rightPanelStore.ts:214,643`; `previewMiniPlayerStore.ts` under `apps/web/src/` |
| Appearance menu | `apps/web/src/components/preview/PreviewMoreMenu.tsx:126-145`; `components/ui/menu.tsx:301-324` uses inline-end, start alignment, and default align offset -5 |
| Page dimensions | `apps/web/src/browser/{HostedBrowserWebview.tsx,browserViewportLayout.ts,useBrowserViewportResize.ts}` and `apps/desktop/src/preview/Manager.ts` |
| Safari discovery/counts | `apps/desktop/src/preview/BrowserImport/{Sources.ts:370-503,SafariCookies.ts,SafariPermission.ts}`; `apps/web/src/components/settings/BrowserImportWizard.tsx:232,389` |

Historical observations are in application `STATUS.md:269-270,324,340`, from
`realinput-1011c` on `1f813789b`. They are historical evidence, not a new reproduction on
this base. The prior browser records are under the parent plan's `tasks/closed/`:
`20261005-browser-surface-navigation.md`, `20261005-browser-surface-profiles.md`, and
`20261011-browser-recording-and-downloads.md`. The last is merged PR
[#445](https://github.com/ccheever/exact2/pull/445), not work to reopen.

Controlled reference addendum (2026-10-14): `target/fix-wave/browser-reference-20261014/README.md`
records a completed two-tab CDP drive against the existing pinned audit app. Hiding floats
the selected first tab; reopening preserves that tab. Screenshots and panel state agree.
The earlier no-float oracle is disproven for this controlled case. The coarse after-reopen
`floating` Boolean matched a regular toolbar control; the mini-player-specific control is
absent and the screenshot shows the panel only. This is CDP keyboard evidence, not physical
input. Both created tabs were closed and the original route restored. Keep the requested
row with the user’s 2026-10-14 resolution: preserve reference float-on-hide.

The historical reference conflicts and their accepted resolutions remain visible:

1. G4-18's recorded oracle hides/shows without a floating preview. The pinned source maps
   the chord to `preview.toggle`, dispatches `toggle-panel`, and calls
   `closePreviewPanel`, which can open the mini-player for `kind: "preview"` with a
   resource ID. `rightPanelStore.browserSurface` also uses `kind: "preview"`.
   No kind-based explanation resolves this discrepancy. The controlled reference drive
   confirmed float-on-hide. The user explicitly chose matching that behavior on
   2026-10-14; the old no-float target is superseded.
2. G5-A3's "as the reference" premise is not supported by this source pin.
   `Sources.ts:listSafariProfiles` returns directory/name only, and its Safari branch
   returns before `withCookieCounts`. The clone does the same in
   `browser-import-sources.ts:156-185,208`. Both wizard renderers hide an undefined count.
   The user explicitly chose matching that behavior on 2026-10-14. Preserve omitted
   counts; no Safari-count implementation or private-store read is authorized by this row.

Current repository guidance is authoritative: `AGENTS.md`, `rules/RULES.md`,
`rules/DEFERRED.md`, `docs/testing.md`, `docs/contract-grammar.md:1628`, and LLP 1021 D2's
popover command and physical placement amendments. The parent library revision is
`20261005-platforms-v3`; its older examples do not override the adopted D1/D2 syntax.
Use view `if`/`for`, receiver methods and `T[]`. Task/keybinding `when` remains valid.

## Dependencies

| Kind | Dependency | Evidence at inspection | Required next condition |
| --- | --- | --- | --- |
| Merged task PRs | Eleven first-wave repairs | Parent review `reviews/20261014-first-wave-resume.md`; checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a` is an ancestor | Preserve their combined behavior when shared files change |
| Merged task PR | Main adoption round 14, [#549](https://github.com/ccheever/exact2/pull/549) | GitHub reports merged 2026-10-14 03:08:53 UTC as `3710af842bc54521c298124156989950b6f2e68c`; main and merge are ancestors | Satisfied at this base |
| Merged task PRs | Browser navigation #352, profiles #354, recording/downloads #445 | GitHub reports all merged into the feature branch; commits `ab220bfdb`, `a6e31eed7`, `443380976` are ancestors | Reuse, do not duplicate |
| Merged task PRs | `dialog-focus-return-and-traps` -> `legacy-sidebar-parity` -> `connections-network-settings` -> `menu-keys-popover-commands` | All four merged: #553 `6a7a3cad0`, #557 `4520b0124`, #566 `fbed7fadd`, #568 `b229a2658`; all are ancestors of prepared base `07f9de1e3` | Satisfied; consume `menu-keys.contract` as merged in #568 |
| Recorded decisions | G4-18 and G5-A3 source/oracle conflicts | User selected reference behavior on 2026-10-14 after the controlled reference evidence: float-on-hide and omitted Safari counts | Satisfied; apply the revised acceptance above without asking again |
| Environment | Pinned Bun and native tools | Repository pins Bun 1.4.2, Rust 1.97.0; this inspection shell resolves Bun 1.3.14 | Select the pinned Bun and record actual tool versions before checks; do not substitute a lower version |

### Issue assessment

Checked plan `issues/` and `issues/closed/`, application `EXACT2-GAPS.md`, prior browser
tasks, current repository issue records, public API source, and GitHub readbacks on
2026-10-14. GitHub closure alone is not proof that every requested capability landed.

| Issue/reference | Capability and target | Evidence/revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X66 / [#319](https://github.com/ccheever/exact2/issues/319) | Show/hide/toggle a macOS popover from hover and observe its lifetime | `issues/closed/20261009-popover-action-commands.md`; public commands at `docs/contract-grammar.md:1628`; implementation `62469206a` is in adopted main | Nonblocking for macOS; remaining UIKit navigation-bar work is outside this task | Use explicit `source="browser-more-appearance"`; consume the predecessor's menu door |
| X17 / [#112](https://github.com/ccheever/exact2/issues/112) | Side placement and collision flips | GitHub closed after transfer, but local `issues/20261009-popover-css-flip-fallbacks.md` remains open. Its first slice, `38281323f`, supports physical left/right spans and ordered block/inline flips on painted Apple layers | Nonblocking for this physical left/right submenu; not blanket support for all anchor CSS | Use only the admitted subset and verify actual frame placement. If it cannot meet this bounded case, record a framework issue rather than adding per-site geometry arithmetic |
| [#527](https://github.com/ccheever/exact2/pull/527) | CSS resize cursor images on macOS | Merged `c46ed7431`; ancestor of this base; `issues/closed/20261013-macos-css-resize-cursors.md` | Nonblocking, separate from page sizing | Preserve adopted cursor mapping; do not count it as proof of G4-16 viewport equality |
| X1 path B | WKWebView browser engine instead of Chromium | Parent specification and `issues/closed/20261005-x01-chromium-cdp-browser-surface.md`; app-owned native view already controls bounds and `pageZoom` | Nonblocking for investigating frame rounding; engine differences remain declared | Repair the app sizing path with public AppKit/WebKit APIs; report a bounded engine limitation if exact sizes cannot be obtained |
| G4-18 reference evidence | Recorded no-float oracle versus pinned callback source | Exact paths in Context; coordinator independently confirmed the callback; controlled two-tab reference run in the addendum confirms float-on-hide and same-tab return | Resolved by user decision | Preserve float-on-hide and fix same-tab return |
| G5-A3 decision | Safari count semantics and reference claim | Pinned reference and clone both omit counts; existing parsers and count label can support an intentional addition | Resolved by user decision | Preserve omitted counts and retain this row as a corrected audit premise |

No new framework defect was proven by this read-only inspection. No issue was published
and no tracked plan-local issue record was added. A future public-API failure must follow
the parent issue workflow; this task does not authorize a framework patch.

## Implementation notes

### G4-18: preserve reference floating and restore the selected tab

The current chain is `previewToggleRow` at `browser-navigation.ts:268-272` -> dispatch kind
`panel-close` -> `settings-shortcuts.contract:65` `panel("close", "")` -> `app.contract`
`panelUi` -> `shelllocal:surface-hide`. `r4-surfaces-panel.ts:243-251` then floats the live
Browser tab. Preserve this reference behavior. Repair the two-tab selection defect
without adding a separate no-float shortcut path.

There are also two active identities. `surfaceLocal("activate")` at
`r4-surfaces-panel.ts:213-221` changes `PanelState.active`; `showPreview` at
`browser-navigation.ts:166-175` reads `PreviewStateStore.activeTabId`. The latter can still
name the newer empty tab after the user selected another tab in the panel. Reconcile
these owners or choose the preserved panel selection deliberately. Test two tabs, not
only the existing single-tab dispatch assertion at `browser-navigation.test.ts:235-253`.
Hide/show must not create or close a browser session. Preserve the reference mini-player
transition when hiding a loaded Browser tab.
Preserve thread/environment scoping, native session identity, saved panel restoration,
and the user's panel revision semantics from #445.

### G4-19: use the adopted popover lifetime

`BrowserAppearanceItem` at `browser-surface.contract:293-320` currently only focuses
`browser-more-appearance-km` on hover. Its popup uses negative margins and no declared
side, so it is not a side-anchored submenu. After the menu predecessor merges, use its
final opening/closing and focus conventions. The public call is
`showPopover("browser-appearance-menu", source="browser-more-appearance")`; no invoker
is inferred from a hover handler. Repeated show/hide is idempotent. Keep the submenu in
the parent's nested auto-popover lifetime and close it on sibling entry without closing
the parent or flickering while the pointer travels into its popup.

Use physical `position-area="left span-bottom"` or `"right span-bottom"` and an authored
`flip-inline` fallback appropriate to the reference's preferred side. Remove the child's
conflicting negative-position margins. Preserve any deliberate gap using supported
margins, measured against the pinned reference rather than copied from guessed geometry.
Logical `inline-end`, `flip-start`, general shifting, nonzero insets and arbitrary anchor
CSS are not all admitted. Native NSMenu placement remains platform-owned; preserve the
app's current authored presentation choices. Do not retire unrelated menu-placement
workarounds, which belong to serial follow-up 6.

### G4-16: keep layout size, zoom and presentation scale distinct

`BrowserStage` computes `vw/vh = requested CSS size * zoom * fit scale` and passes only
`fit-scale` to `t3-browser` at `browser-stage.contract:121`. `T3BrowserView.Host.applyScale`
at `T3BrowserView.swift:48-55` divides the laid-out frame by that scale and gives the
resulting bounds to WKWebView. Initial offscreen views instead get `width * zoom` in
`T3BrowserSessions.sync:135-140`; parked automation views use
`T3BrowserViewport.hold` at `T3BrowserAutomation.swift:731-736`. Repair the same size
contract at every applicable entry point so a remount or first navigation cannot restore
the one-pixel loss. Do not round the requested CSS size based on a fitted display frame,
and do not assume rounding AppKit points alone is correct at every zoom.

The current AppKit test explicitly permits `389x844` or `390x844` for requested 390x844
at 125% in `macos/tests/browser/navigation.swift:77-87`. That allowance is evidence of
the existing difference, not a sufficient regression check. Add exact cases at fractional
zoom and non-integral fitted dimensions, including first layout and remount. The generic
readiness helpers in `browser-viewport.ts:207-213` and `T3BrowserViewport.matches` allow
one pixel for the ported protocol. Do not weaken or use that tolerance as visual equality;
keep any protocol-compatibility decision separate from exact page-size assertions.

### G5-A3: preserve the reference’s omitted counts

`BrowserImportSourceProfile.cookieCount` is already optional. SQLite browsers use
`withCookieCounts` in `browser-import-sources.ts:136-153`; Safari returns earlier from
`listSafariProfiles`. `browser-profiles-settings.ts:409` already forwards the value through
`cookieCountLabel` in `browser-import.ts:121-125`, producing empty text for unknown,
"no cookies", "1 cookie", or a grouped plural. No new UI formatting is needed.

The user chose reference behavior, so leave optional Safari counts undefined. Preserve
metadata ordering, deleted-profile exclusion, UUID/path validation, permission handling
and existing Chromium count behavior. Existing synthetic discovery tests provide the
regression check; this task requires no new Safari reader, enrichment or private-data access.

## Acceptance and reproduction

All current findings below are source-reproduced or historical; no runtime check ran in
preparation. The implementation must add meaningful failures to the existing tests,
then show those failures pass on its committed behavior revision.

| Row | Existing test/fixture owners | Required assertions |
| --- | --- | --- |
| G4-18 | `browser-navigation.test.ts`, `browser-surface.test.ts`, `browser-state.test.ts`, `r4-surfaces.test.ts`; existing `contract-action-fixture.ts` for the authored routing | Two tabs, first loaded and second idle; select first; execute actual shortcut twice. The loaded first tab floats when hidden as in the reference; reopening restores that selected tab and live session, with no session open/close calls. Also cover hidden initial panel, no existing tab, and other active surface without changing their established behavior. |
| G4-19 | `menu-keys.test.ts`, current Contract action fixture, final native drive | Hover opens through the real action and explicit source; sibling entry closes only child; pointer can enter child; disabled state cannot open it; keyboard/Escape behavior survives predecessor migration. Layout and screenshot show child beside parent and Zoom minus unobscured. A source-string assertion alone is insufficient. |
| G4-16 | `browser-viewport.test.ts`; `macos/tests/browser/navigation.swift` using its `/size` and `/first` fixture pages; `macos/tests/browser-automation` if parked sizing changes | Exact `innerWidth/innerHeight`, not plus/minus one, for responsive/custom and preset sizes at 100%, 125%, 150%, and a non-integral fit scale. Cover first load, resizing, zoom update, hide/remount and relevant parked view. Keep fill mode and device-pixel-ratio behavior intact. |
| G5-A3 | `browser-import-sources.test.ts`, `browser-import-safari.test.ts`, `browser-import.test.ts`, `browser-profiles.test.ts`; `browser-import-fixture.ts`; `macos/tests/browser-profiles` if native IO changes | Existing synthetic Safari discovery keeps counts undefined and the Configure count subtitle absent, as in the reference. Preserve existing non-Safari count behavior; no import or real Safari access is needed. |
| Existing Browser behavior | Existing browser capture/recording suites, chosen according to touched code | Session borrowing/parking, separate windows, recording lifecycle, agent download ownership, explicit Float and close remain intact. |

Run commands from the repository root with the pinned Bun selected. These are future
implementation commands, not checks claimed by this draft:

```sh
bun --version
bun test examples/t3-code/browser-navigation.test.ts examples/t3-code/browser-surface.test.ts examples/t3-code/browser-state.test.ts examples/t3-code/browser-viewport.test.ts examples/t3-code/browser-import-sources.test.ts examples/t3-code/browser-import-safari.test.ts examples/t3-code/browser-import.test.ts examples/t3-code/browser-profiles.test.ts examples/t3-code/menu-keys.test.ts
bun test examples/t3-code --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
bun scripts/exact.mjs contract build examples/t3-code/app.contract -o target/fix-wave/browser-panel-parity.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Use the existing `examples/t3-code/README.md` AppKit compilation recipe restricted to
`macos/tests/browser/` and affected `browser-profiles/` or `browser-automation/` owners.
Use its generated data keys and the existing module facade; do not add a new runner.
Run the root's required checks before landing through the integration owner. Stage only
owned files for caps, preserve the 1,500-line limit, and record every unrun requirement.

### One bounded native recipe after implementation

Use one controlled sequence for before/after, with a reference capture at the unchanged
pin. Record the actual source and binary identities. No matrix of repeated visual trials.

1. Prepare a fresh isolated lane using the existing Browser profiles fixture recipe from
   `tasks/closed/20261005-browser-surface-profiles.md:226-228`. Its published
   [browser-home maker](https://raw.githubusercontent.com/ccheever/exact2/0f4b4d0ca3b5cc40792b8663566aac4a74b4fe3a/browser-surface-profiles/make-browser-home.mjs.txt),
   [probe page](https://raw.githubusercontent.com/ccheever/exact2/a634c7f48ebe1a77f70bac9604144a433c0a8137/browser-surface-profiles/fixture.mjs.txt), and
   [first-layout probe](https://raw.githubusercontent.com/ccheever/exact2/95682cec2b9dfdd2452804fb97231652fb2f822c/browser-surface-profiles/fixture-first.mjs.txt)
   are existing evidence artifacts, not new apparatus. The maker removes its argument,
   so give it only a newly allocated lane-owned fixture directory. The generic probe
   reads its size on load, so reload it after changing size or use the existing `/size`
   fixture's resize-aware markup. Do not mistake stale page text for a new measurement.
2. Supply separate `T3_LOCAL_HOME`, `T3_BROWSER_IMPORT_HOME`, provider/XDG directories and
   a free non-3773 loopback port; set `T3CODE_TELEMETRY_ENABLED=false`. Use a development
   agent-mode bundle. `T3BrowserImportIO.resolve` must report `fixture: true` with the
   allocated home before Configure is opened. Never use a packaged bundle for this
   recipe: packaged policy resolves the account home even when a fixture env var exists.
   Do not launch Safari, grant Full Disk Access, inspect real stores, or send a provider
   message. Verify connected/backend-ready and completed onboarding before proceeding.
3. At 1280x840, open a local draft/thread and two Browser tabs. Load the fixture in the
   first and leave the second blank; explicitly select the first. Capture tab IDs,
   `shell.panel`, browser session IDs, mini-player state and page text. Send
   Shift+Command+J twice through the existing agent key path. Capture after each and
   assert hidden with the reference mini-player then shown, same loaded tab/session and no
   new session.
4. Open More and hover Appearance through the agent's existing pointer operation.
   Capture tree, popup/parent/Zoom-minus frames, and a screenshot. Move into the child,
   choose a scheme, then move to Zoom; child closes and Zoom minus works. Exercise Escape
   once. Repeat only the placement observation at 840x620 to expose edge collision, then
   return to the default size. Missing physical placement support is a blocker, not a
   reason to hard-code screen coordinates into the app.
5. Set a 390x844 preset at 125% and one responsive/custom size that produces a fractional
   fit scale. Read page `innerWidth/innerHeight` using the existing probe/automation
   measurement and capture the size fields alongside it. Hide/show the tab once and
   read again. Native tests carry the rest of the zoom cases; no extra UI sweep is needed.
6. G5-A3 is resolved: preserve omitted Safari counts. Existing synthetic discovery
   tests should keep `cookieCount` undefined. If a Configure regression drive is useful,
   use only the existing isolated metadata fixture, capture profile names and absent
   count subtitles, then cancel. No new count fixture or cookie-store access is needed.
7. Save before/after/reference observations, exact assertions and limitations under a
   lane-owned evidence directory. For the reference, G4-18's source/record conflict must
   be reported; G5-A3's missing counts cannot be labeled a failed clone parity check.
   Close the sessions and stop only process IDs recorded by this lane. Real global input
   checks require the desktop slot. The user reported the Mac unlocked on 2026-10-14;
   verify the current state and coordinate the slot before physical input.

### Controlled reference portion of that sequence

Use the pinned reference in a coordinator-provisioned isolated fixture lane. The existing
audit tools are `target/t3-audit/ref-app.sh`, `ref-cdp.mjs`, and `ref-stop.sh` in the original
integration checkout; their interface is documented in each file. `ref-cdp.mjs` accepts
`<cdpPort> <operation>...`, including `pages`, `aria`, `shot`, `press`, `clickrole`,
`hovertext`, and `eval`. These are local evidence tools, not portable repository commands.
The existing launcher has hard-coded source paths and overrides reserved home variables;
it is historical setup, not an instruction to invoke it unchanged. Have the coordinator
establish an approved isolated runtime, record its source/binary identity and CDP port,
and validate its fixture paths before this portion. No reference launch is authorized by
this preparation. If that safe runtime is unavailable, retain the reference blocker.

1. Use the same loopback probe URL as the clone and a fixture draft/thread. Start with no
   mini-player, open two Browser tabs, load the first and leave the second blank, then
   select the first through its tab. Record the selected tab, URL, all visible panels and
   the probe's page count before pressing the chord. For the app-owned fixture state,
   the exact right-panel storage key is `t3code:right-panel-state:v2`
   (`apps/web/src/rightPanelStore.ts:89`); record only its selected thread entry, surface
   IDs/resource IDs and `isOpen`, not an unrestricted storage dump.
2. Use the reference driver's `press Meta+Shift+J` operation twice, with an `aria` and
   `shot` observation after each press. Confirm whether a mini-player appeared and which
   loaded page returned. Do not automatically close a mini-player between observations:
   that would erase the disputed behavior. The source prediction is float-on-hide for
   an active preview with a resource ID, from
   [the pinned callback](https://github.com/pingdotgg/t3code/blob/1e2ecbd9758830669684b494d4398f626b0576e0/apps/web/src/components/ChatView.tsx#L5628).
   The controlled drive is already complete and the user resolved G4-18 on 2026-10-14;
   reuse that evidence unless a new finding requires another reference observation.
3. With the loaded tab active, use `clickrole button Preview menu`, then
   `hovertext Appearance`. Record the parent/child bounding rectangles and screenshot,
   move into a scheme item, then to Zoom and exercise `clickrole button Zoom out`.
   Those exact accessible button names are authored in `PreviewMoreMenu.tsx:102,164`.
   Capture default and minimum window observations in the same sequence; use the
   reference window's actual content dimensions rather than treating a CDP viewport
   override as proof that the native window has resized.
4. Select the same 390x844/125% and fractional-fit custom cases as the clone. Reload the
   on-load probe after each change or use the resize-aware page. Read the guest page's
   dimensions, not the outer Electron renderer's `window.innerWidth`. Preserve field
   text and the first-layout result alongside the page measurement.
5. G5-A3 must never use the account's Safari home. Only if the coordinator has verified
   that reference discovery resolves inside the synthetic fixture root may it open the
   Safari Configure page. Use the same named/default profiles, capture names and missing
   count subtitles, and cancel before import. If native reference isolation cannot be
   established, omit this unnecessary Safari UI drive and cite the resolved scope; the
   existing reference `Sources.test.ts:1097-1196` supplies temporary Safari metadata and
   asserts profiles without counts, but a source test is not a native UI receipt.
6. Save the result once, including any difference from the requested target. Do not patch
   the pinned reference to make the assertion pass. Stop only this lane's recorded PIDs.

## Progress

| Row | Implementation | Unit/AppKit tests | Native AFTER drive (2026-10-14) |
| --- | --- | --- | --- |
| G4-18 | Done: a selected Browser tab becomes its thread's active session | Added, fail on base | PASS |
| G4-19 | Done: hover opens beside the row; siblings and the Zoom row close it | Added, fail on base | PASS (one harness check misread; screenshots decide) |
| G4-16, viewport half | Done: whole-point page size from the requested CSS size | Added, fail on base | PASS |
| G5-A3 | No product change; omitted Safari counts asserted | Regression assertion (passes on base by design) | Not driven (optional step 6) |

Prepared by `repair_composer` on 2026-10-14 as a read-only handoff for the coordinator.
Local and remote feature heads both read `a218ee78fec42b1a4590b199297f30667876de67` during
inspection. GitHub readbacks establish #352/#354/#445/#549 merged; ancestry checks
establish their presence, first-wave checkpoint inclusion, main adoption and #527.
Search by exact browser and menu predecessor branch found no existing task PR to reuse.
This does not stand in for the required predecessor merge check at implementation time.

Current environment inspection: macOS 26.6.2, build 25G83, arm64; default Bun 1.3.14
conflicts with the repository's 1.4.2 pin. No installation or toolchain mutation was made.
Only this ignored draft was written. No source, test, branch, Git index, remote record,
Safari data, app process or global input was changed.

## Attempts and evidence

| Attempt | Revision | Method and result | Limits |
| --- | --- | --- | --- |
| Historical real-input report | `1f813789b`, reference `1e2ecbd975` | STATUS records float/wrong-tab, below-row submenu, one-pixel undersize and missing Safari counts | Archived observations; the two reference premises were corrected by the controlled reference evidence and user decision above |
| Prior Browser navigation/profile evidence | Merged #352/#354 | Existing native test and profile task explicitly accept/report 389x844 for 390x844 at 125%; fixture and permission policy already exist | Old passing status does not satisfy new exact-size acceptance |
| Prior recording/download review | Merged #445, `443380976` | Read task and PR receipt, including parallel recording requests and agent-versus-user panel revision repairs | Preserved regression constraints; recording was not rerun |
| This preparation | `a218ee78f`, framework `178ae7237`, reference `1e2ecbd975` | Read source and public API records; traced all four rows and existing tests; read remote PR/issue states; verified ancestry and fixture policy | No builds, tests, native launches, live reproduction or private-data reads; no implementation pass claimed |

### Implementation and verification, 2026-10-14

Environment: macOS 26.6.2 (25G83), arm64; Bun 1.4.2; rustc 1.97.0. Base `0560e9600`. The drive ran the
uncommitted diff that this commit holds (staged `examples/t3-code` diff without `.exact`, SHA-256 prefix
`c89bacb8aa5e1412`); bundle executable SHA-256 prefix `357a6ceb744a97ce`.

Changes (application only):

- G4-18: `browser-surface.ts` `selectBrowserTab` calls `store.setActiveTab` for a Browser surface's own
  thread. `r4-surfaces-panel.ts` calls it on `activate` (reference `activateRightPanelSurface`) and after a
  close selects the next surface (`syncActivePreviewSurface`). Root cause: the panel selection changed
  `PanelState.active` only, so ⇧⌘J reopened the newest session. Float-on-hide is unchanged.
- G4-19: `browser-surface.contract`: Appearance's hover runs `showPopover("browser-appearance-menu",
  source="browser-more-appearance")`; `BsMenuItem` hover and the Zoom row (`zoomRowHover`) run
  `hidePopover` on the submenu only. The submenu uses `position-area="right span-bottom"`,
  `position-try-fallbacks="flip-inline"` and `margin-top="-0.3125rem"` (MenuSubPopup's -5 px align offset);
  the negative left margin is gone. `adopt-main-fixes-r12.test.ts` accepts this one authored flip (an
  authored fallback row still selects the painted layer, LLP 1115.002 D7.3).
- G4-16: measured on this Mac, WebKit truncates a web view's frame to whole points, then floors
  points / page zoom (487.5 pt at 125% is 389 px; 488 is 390). `T3BrowserViewport.points` gives
  `ceil(css × zoom)` whole points. `browser-stage.contract` passes the requested size as `page-size`;
  `T3BrowserView.Host` sizes the bounds and the page frame from it at the page's zoom (the page frame is
  set directly, because AppKit reads bounds back as 487.99999). The initial `browserSync` size and the
  parked `hold` use the same rule. Fill is unchanged.
- G5-A3: an explicit assertion in `browser-import-sources.test.ts` that discovered Safari profiles carry
  no `cookieCount` and an empty Configure subtitle.

Tests and checks:

- New or changed tests: `browser-navigation.test.ts` (two-tab ⇧⌘J, close sync, authored route, other
  surface), `menu-keys.test.ts` (3 G4-19 tests, row inventory), `browser-viewport.test.ts` (stage
  `pageSize`), `macos/tests/browser/navigation.swift` (exact sizes at 100/110/125/150% and fit scales
  0.37, 0.413, 0.5 through first load, resize, zoom, park/hold and remount; 125% first layout now exactly
  390 × 844). Against base sources the G4-18, G4-19 and G4-16 tests fail (checked by swapping the base
  files in); the route and G5-A3 assertions pass on base by design.
- Focused Bun files: all pass. Full app Bun suite (335 tracked files): 5,175 pass, 1 skip, 0 fail.
- Strict tsc: clean. Contract build: OK. `cargo test -p t3-code-macos --lib`: 44 passed.
- AppKit `browser`: 30 passed; `browser-automation`: 36 passed.
- caps: within budget; `cargo fmt --check`: clean; `cargo check --workspace --all-targets
  --keep-going`: exit 0 (existing colour warnings only).

Native AFTER drive (agent mode, `scripts/agent.mjs` session API, 1280×840, isolated T3 home, provider and
XDG directories, browser-import fixture home, server port 16871, probe 16872; no provider message, no
Safari, no global input). Lane: `target/browser-panel/` (ignored); script `drive.mjs`, adapted from the
integration's `native-smoke.mjs`. Attempt 1 launched the unbundled binary (the harness did not set
`EXACT_MAC_BIN`) and stopped before step 3. Attempt 2 stopped at step 3 on a wrong tab target
(`panel-tab-…`, which has no press). Attempt 3 (`after-03`, the one retry) ran the whole recipe:

| Row | Result | Evidence (`target/browser-panel/after-03/evidence/`) |
| --- | --- | --- |
| G4-18 | PASS: first chord hides the panel and floats tab_1; second shows tab_1, same runtime id, tabs unchanged, no `t3.browser: open/close`, mini-player hidden | `10-…`, `11-after-first-chord`, `12-after-second-chord` |
| G4-19 placement | PASS: submenu x 845–1005, top 167.5 (row 172.5), flipped left of the menu (x 1000–1280); Zoom − at x 1135 uncovered. At 840×620 the same, left flip | `20-appearance-hover`, `27-min-window-appearance` |
| G4-19 behavior | PASS: pointer into Dark keeps it open; Dark applies and both menus stay; pointer on Zoom closes only the submenu; Zoom − 100% → 90%; one Escape closes the submenu, a second the menu. The script's node-presence check reported FAIL for the Zoom step: the tree keeps closed popover nodes. The layout frame (in-flow 1005, 195.5) and the screenshot show it closed | `21`–`26` |
| G4-16 | PASS: 390 × 844 at 125% gives page 390 × 844 (dpr 2.5); 1013 × 777 at 125% in a 519 × 398.09 box gives 1013 × 777; after ⇧⌘J twice and a reload the first layout is 1013 × 777 | `30-…`, `31-…`, `32-…`, `probe.log` |
| G5-A3 | Not driven: the profiles fixture's Safari home has no named profiles and stops at Full Disk Access | unit assertion only |

Cleanup: every recorded PID (`pids.txt` of each attempt) is gone; ports 16871 and 16872 are free.

Independent review (fresh agent): no blocking defects. Non-blocking notes, not changed: a press on
Appearance after hover toggles it closed (Base UI 1.5.0 only opens, `MenuSubmenuTrigger.js:135`;
pre-existing); no safe polygon, so diagonal travel through the Zoom row closes the submenu; a fixed
viewport at scale 1 draws 488 pt bounds into a 487.5 pt box (about 0.1% scale); below 100% zoom some
sizes cannot be exact in whole points (documented in `T3BrowserViewport.points`).

Host gaps: none needed a framework change. Unverified: physical pointer hover (agent pointer only),
real ⇧⌘J from the keyboard, G5-A3 in the native Configure step, zooms below 100%.

The source traces are reproducible through the named paths and symbols. The ignored
draft itself is local-only and uncommitted; the coordinator must publish an authorized
task record before relying on it for another machine. There have been zero browser-task
implementation or repair rounds in this preparation.

### Real-input session realinput-1015, 2026-10-15

Bundle of `db3c4540f` (code `aa4fd19c3`) in a lane copy (`com.exact.t3code.macos.ri1015-1`) launched through LaunchServices (a normal launch, not agent mode), port 16915, isolated homes from audit lane `ri1012-b10` plus the real-GitHub lane's sandbox (read only). Real keys and pointer through `orca computer`; the input source was `com.apple.keylayout.2SetHangul` (read only, not changed). While a painted popover or dialog is open the window's accessibility tree collapses and `orca` refuses input, so those keys went as System Events key codes to the lane (front process checked first). Evidence is local in the session worktree's `target/realinput-1015/` (`notes.txt`, `ev/`). 32 steps in all: 27 passed, 2 failed, 3 not verified.

| Row | Result | Evidence (`target/realinput-1015/ev/`) |
| --- | --- | --- |
| Physical ⇧⌘J (step 8) | PASS: the first hides the panel and floats the Browser as a mini player, the second shows the panel with the same tab (Files, Browser) | `s08-qr-pair.png` |
| Physical hover on Appearance (step 9) | UNVERIFIED: the More menu opens by a real click, but `orca` has no pointer move and refuses clicks over a painted popover | `s09-c-crop.png` |
| G5-A3 natively, zoom below 100% | UNVERIFIED: no Safari-profiles fixture; sizes need the probe | — |

## Next action

After realinput-1015 (2026-10-15): Keep `verification: unverified`. realinput-1015 passed the physical ⇧⌘J (STATUS row G4-18 removed). Physical hover on Appearance needs a pointer-move input the tool does not have; G5-A3 and sub-100% zoom stay open.

Merged as [#571](https://github.com/ccheever/exact2/pull/571), feature commit
`70eec59fd8b345df77c5932eeb2a474b40a16e54` (squash). The task stays unverified for physical
pointer hover, a physical ⇧⌘J, G5-A3 in the native app and zoom below 100%. Known
differences: a click on Appearance after the hover closes its submenu, and diagonal pointer
travel through the Zoom row closes it. Follow-up 6, `css-workaround-retirements`, starts from
this feature head.
