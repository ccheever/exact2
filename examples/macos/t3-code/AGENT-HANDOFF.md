# T3 Code agent handoff

Current as of the round-11 integration, 2026-10-05 (KST, about 10:50). Replaces the round-10
handoff; `UI-PARITY-TODO.tmp.md` is history only.

## Start here

Read `CLAUDE.md`, `rules/RULES.md`, `rules/DEFERRED.md`, then `README.md` in this
directory. The app is untracked in git: never delete, reset or recreate it wholesale.
No framework changes (kernel, runner, host, contract, scripts, js, plan); the app's own
`apple/` crate and `modules/apple/` are app code.

| Role | Location |
|---|---|
| Writable checkout | `/Users/daehyeonmun/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805` (the app is `examples/macos/t3-code`) |
| Root checkout | `/Users/daehyeonmun/Documents/work/0.projects/exact2`, read-only |
| Reference source | `/Users/daehyeonmun/Documents/work/3.open-source/t3code` at `f870c419fc`, read-only |
| Oracle runtime | `target/t3-ref/runtime-f870c41` (Nightly `0.0.46-nightly.20261005.1`). `lane-backend.sh`'s default still points at `runtime-f90b77d8`: export `T3_RUNTIME=$PWD/target/t3-ref/runtime-f870c41` for every lane backend |
| Pinned tools | Bun at `~/.bun-1.4.2/bin`; Hermes at the root checkout's `target/t3-tools/hermes-6badada76212` (`EXACT_HERMES_DIR=…/engine`, `EXACT_HERMESC=…/hermesc`) |

## Checks on the integrated tree (round 11)

Round 11 merged three lanes. r11-upstream ported the client half of `f90b77d809..f870c419fc`:
the row-action sweep (1826fb55cc; `r11-upstream-sweep.ts`, `sidebar.contract`), the draft
row's context menu and Discard behind the undo notice (95edeb753b; `r11-upstream-drafts.ts`),
Retry for a failed workspace preparation (737993303d; `r11-upstream-retry.ts`), usage shares
by metric (0c81120137), the version pill that no longer clips (9efb016900) and one way to open
the No project folder (845ddd9354; `r11-upstream-scratch.ts`). r11-device: the 3D phone's first
open no longer falls back to flat (`R11DeviceBacklog.swift`, the soft queue measured over a
1 s window), every right-panel tab kept per thread across launches (`r11-device-panels.ts`),
and the Diff of a project outside the server root asked again at the server's cwd
(`r11-device-diff.ts`). r11-misc: Load balancing / GitHub sharing counted as the reference
does (`r11-misc-connections.ts`), rendered HTML loading its siblings from the asset token's
directory (`R6MediaPreview.swift`), and Shiki's declaration rules (`r11-misc-ts-decl.ts`).

- `bun test examples/macos/t3-code` 991/0 (105 files; round 11 added r11-upstream.test.ts 14,
  r11-upstream-usage.test.ts 3, r11-device.test.ts 4, r11-misc-connections.test.ts 2,
  r11-misc-ts-decl.test.ts 13), strict `tsc` clean, `contract build` OK (1789 slots, 41
  resources, 41305 nodes), every source and `.contract` file ≤ 1,500 lines (`client.ts` 1455,
  `app.contract` 1327, `composer-controls.contract` 1010). `bun scripts/caps.mjs` needs
  `git add -A` and was not run (no git writes in the worktree).
- `cargo test -p macos-t3-code-apple --lib` 7/0. All 26 AppKit binaries green with the README
  recipe (`lanes/r11-integrate/tools/swift-all.sh`, `T3_DEVICE_GLB_DIR` from the f870c41
  runtime; mermaid against the lane's HEAD server; log `lanes/r11-integrate/swift-all.log`):
  attach 3, composer 45, composer-files 4, contextmenu 6, fleet 8, intent 4, menus 10,
  notifications 4, r10-connect 5, r10-device 4, r11-device 3, r11-upstream 3, r5-composer 3,
  r5-panels 5, r6-device 3, r6-media 5, r7-device 13, r8-keys 4, r8-pointer 3, r9-device 13,
  r9-input 10, sidebar 5, ssh 4 (1 skip), transport 31 (2 skips), snapshot 86 checks,
  mermaid 10 checks.
- No cross-lane breakage: nothing needed fixing, and no app source changed in this pass
  (only this file and `README.md`).
- `lane-build.mjs r11-integrate` built the bundle in 40 s; every native cell below ran on
  that build (sub-lanes hold copies of it).

## Matrix (HEAD oracle f870c41)

All references were re-shot against the f870c41 runtime. Rendering is deterministic under
the agent clock: most native shots are pixel-identical to round 10 (≤ 0.02), so where a score
moved, the reference drive moved. Several drives ran at once (load average 2–5).

| Set | Where | Pairs | Medians / range | Against round 10 |
|---|---|---|---|---|
| Round 3 (34 states) | `lanes/r11-integrate/matrix/pairs/` (`diff-main.json`; `tools/cmp10.py`) | 136 | outside the sidebar 1.58 (1280), 4.95 / 4.14 (840 light / dark) | per-cell median delta 0.00. palette-dark-1280 +1.21 and palette-light-1280 −1.17 (the two swap from round to round: the reference palette's backdrop). settings-connections-dark-1280 +0.45 was a transient "Some requests are slow" toast under load; the rerun (`matrix/rerun/`) scores 2.04, as round 10 |
| Round 4 (13 states) | `…/matrix/pairs4/` | 52 | 1.89 / 3.34–3.97 | equal (all deltas ≤ 0.3) |
| Round 5 (9 states) | `…/matrix/pairs5/` | 36 | 1.71 / 2.71–3.42 | equal |
| Pull request row | `lanes/r11-integrate-pr/evidence/pairs-r6pr.txt` (vs r6-pr's reference), `pairs-all.txt` (own) | 40 + 40 | 3.32–11.31 / 3.32–16.66 | equal against r6-pr's reference. Own-reference 840 cells moved −2.4 to +3.7: the native shots are identical to round 10 (≤ 0.02), the new oracle's reference shots differ by 0.8–10.7 |
| Media previews | `lanes/r11-integrate-mp/evidence/compose.txt` | 17 | 1.89–8.45 | equal |
| Round-7 polish | `lanes/r11-integrate-po/evidence/diff.json` | 16 | 2.39–7.84 | equal |
| Device (fixture hub, r7 cells) | `lanes/r11-integrate-dev/evidence/pairs/scores.txt` | 40 | 1280 crops 1.15–5.6, 840 up to 7.3 | equal, and ios-3d-light-1280 is fixed: 1.77 (round 10: 17.86, flat after a first-open fallback). ios-3d-light-840 +0.65 (transcript offset; native and reference both changed by the thread) |
| Round-8 states | `lanes/r11-integrate-r8/pairs/scores.txt` | 20 | 2.1–7.7 | equal |
| Round-9 input, Connections, PR checkout, Files | `lanes/r11-integrate/evidence/r9/`, `evidence/r9pairs.log` | 42 | 1.1–7.3 | equal; Connections 840 fell 1.2 / 1.6 (native unchanged, ≤ 0.09; the f870c41 reference moved) |
| Round-9 Duo and fold | `lanes/r11-integrate-duo/evidence/pairs-{ios,android}-*.png` (`cellpairs.txt`) | 40 cells | panel 1280 0.98–4.21, 840 1.46–6.0 | whole-window equal (≤ 0.02) |
| Round-10 Connections | `lanes/r11-integrate-c10/evidence/pairs/` | 5 | 2.6–5.7 | equal; "Backend added" now also shows Load balancing / GitHub sharing as the reference |
| Round-10 device | `lanes/r11-integrate-d10/evidence/pairs/scores.txt`; Closed scan `tools/r10-integrate-coverscan.py` | 20 + 20 launches | panel 0.66–6.5 | equal; the Duo cover painted in 20 of 20 Closed shots; physical hand-off reopened only `stream.avcc` |
| Round-11 upstream | `lanes/r11-integrate-up/evidence/pairs/scores.txt` | 17 | sidebar 3.2–12.5, panel 1.25–4.3 | new (below; the high sidebar numbers are reference-side leftovers) |
| Round-11 misc | `lanes/r11-integrate-ms/evidence/pairs/scores.txt` | 13 | 1280 2.6–5.0, 840 4.6–6.0; Files panel 1.29–6.5 | new |
| Round-11 device | `lanes/r11-integrate-dv/evidence/persist/pairs/`, `pairs3d/`, `stall-runs.txt` | 11 + 15 runs | panel 0.35–2.58 | new |

Round-11 state verdicts. Each pair was looked at. "Sidebar" is the 256 pt sidebar crop,
"panel" the right panel.

| State | Verdict | Specifics |
|---|---|---|
| Settle sweep mid-drag (1826fb55cc), 4 cells | match | Settle pressed on card 2 and dragged to card 4: cards 2–4 armed with the selected surface and Settle badges in both; button frame 177.875,193 63×20 in both. Sidebar 5.08 / 3.21 / 6.36 / 4.59 (light/dark 1280, 840): heavier native text and the reference's newest row still "Regenerating title" |
| Sweep release | match, real effect | "Settled 3 threads, ⌘Z to undo"; the three threads moved to Settled on the server (Settled (1) → (4)), three runs |
| Un-settle sweep | match, real effect | Settled shelf, Un-settle from the first swept row to the last: frame 215,651 26×24 in both; release un-settled all three. Sidebar 9.37: the reference's own leftover drafts from the draft cells push its list down, and word-boundary truncation ("fixture…" vs "fixture compl…", framework) |
| Draft discard and Undo (95edeb753b) | match | "Discarded 1 draft, ⌘Z to undo" in both; Undo restored the row and `t3-code.json` held `Second sketch` again. Sidebar light 5.27–6.02; dark 9.6–12.5 because the reference's dark run kept a second browser-local draft from the light run |
| Retry a failed workspace preparation (737993303d) | match, real effect | Disposable `repos/retry-demo`, New worktree from origin, origin renamed away: "Workspace preparation failed … fetchRemote …" with Retry at 306,283.75 61×24. Panel 1.25 light / 1.51 dark. Origin restored, Retry: the server created worktree `t3code-54ec8ae0`, the run completed, the failure row is gone; panel 4.3 against the reference after-state ("Worktree ready" card still shown natively in the live session, as the lane reported) |
| Version pill (9efb016900) | match | Pill 43.27×16 at brand end + 12 at the 256 pt sidebar, hidden at 208 (840). Paired against r11-upstream's f870c41 reference crops (the web reference has no traffic lights, so its 208 pt header still fits the pill: not comparable) |
| Load balancing / GitHub sharing with A off (r11-misc 1) | match except the known row | A switched off, B added: both sections, "This machine" and B's URL rows (Normal / Off), same text as the reference. Whole 2.57 / 2.78 / 4.79 / 4.99 at 1280, 4.63–6.03 at 840 (as the lane). Still differs: A's off row under Environments. At 840 the GitHub sharing toggle sits below the viewport, so the agent opened only Load balancing |
| Rendered HTML siblings (r11-misc 2) | match | `page.html` paints style.css (Georgia, cream, coloured heading), `img/logo.png` and "Script ran: js/app.js", as the reference iframe. Panel 1.29 light and dark; source 6.04 (wrap points) |
| Shiki declarations (r11-misc 3) | match | `decl.ts` source: identical colours line by line (`m`, `n`, `k`, `g`, `e`, `h` as constants, object keys, `case y:`), panel 3.36 (font rendering); `runaway.html` source 2.5 |
| 3D phone first open (r11-device 1) | match; round-10 bug fixed | 15 fresh launches, each with three SIGSTOP freezes of the app's own pid (10 × 1.0 s, 5 × 0.6 s; no CPU burners, other lanes shared the Mac): 15 of 15 kept the 3D phone in all 12 samples, never MJPEG (`stall-runs.txt`). Panel 1.75 against the f870c41 3D reference; the r7 matrix's ios-3d-light-1280 cell also painted 3D (1.77) |
| Right panel after a relaunch (r11-device 2) | match | Seed transfer (labelled synthetic): launch 2 reopened the thread with notes.md, #101, iPhone 18 Pro and Diff in order; light Diff active (lists fixture-result.md and README.md, the outside-root retry), dark device active and streaming 3D. Tab panels 0.35–2.58 against the reference after a reload. The reference also floats the device player on load (open item) |

## Spot checks of lane claims

Agent drives on isolated backends, AppKit binaries, and labelled synthetic seed transfers.

- r11-upstream: (1) the sweep: confirmed (three settle sweeps and two un-settle sweeps on
  the real backend; frames identical to the reference); (2) draft discard and Undo:
  confirmed, including the saved draft in `t3-code.json` after Undo; (3) Retry: confirmed,
  with a real worktree and a completed run after the retry. The pill: confirmed at 256
  (shown) and 208 (hidden).
- r11-device: (1) first open under stalls: confirmed (15 of 15, without CPU burners; the
  lane's 20-of-20 under 18 burners was not repeated because other lanes shared the Mac);
  (2) relaunch persistence: confirmed by seed transfer in both themes; (3) the Diff retry at
  the server's cwd: confirmed (pr-demo lies outside the server's root and its Diff lists the
  two files).
- r11-misc: (1) connections: confirmed at both sizes and themes; (2) sibling assets:
  confirmed in the native Files panel and by the AppKit test; (3) Shiki: confirmed on
  `decl.ts` and `runaway.html` against the f870c41 reference. Its corpus numbers (17448 →
  12368 characters) rest on the lane's own tools and were not re-measured.
- Test counts: 17 + 4 + 15 new tests as reported; the 991 total matches r11-device's report
  (r11-upstream reported 990 and r11-misc 988 before the other lanes landed).
- Refuted: none. Corrected: r11-upstream classes the missing keyboard context menu on rows
  as physical-input; it is an in-app gap (rows take no key handler), recorded so below.

## Deferred: needs real input

Not run in round 11: the screen stayed locked through this pass. No input was sent, no app
ran on the real data root, and neither `t3-code.json`, the preferences domain nor the
Keychain was touched. Last real-input results:

| Check | Last real-input result |
|---|---|
| R1 ⌘B / ⌘K under Korean 2-Set with the composer not focused (round 10) | never run; AppKit 5/5 |
| R2 Hover under a still pointer after a real ⌘Z (round 10 re-hover; round 9's fix) | round 8: fail; never re-run |
| R3 Checkout dialog: select-on-open and "Resolving" timing with real keys (round 10) | never run |
| R4 ⌘B after a click, ⌘B while composing (round 9) | round 8: fail; AppKit 10/10 |
| R5 First click after Hangul composing (round 9) | round 8: intermittent fail |
| R6 Real wheel, thread switch, back (round 9) | round 4: lands mid-transcript; agent: pass |
| R7 Connections with real pairings and a relaunch (rounds 9–11: saved rows, wrong code, "Backend added", pasted URL, Load balancing with A off) | never run with real input |
| R8 Duo pinch, trackpad orbit, a fast flick (round 9–10) | never run |
| R9 Row-action sweep with a real trackpad drag; Escape mid-sweep; hover card during a sweep (round 11) | never run; agent pass |
| R10 Right-click a draft row: Copy ▸ Path / Branch toasts, Project settings, Discard draft then ⌘Z (round 11) | never run; AppKit 3/3 |
| R11 3D phone first open on a cold, busy Mac (round 11) | never run; agent 15/15 with stalls |
| D2–D16, Return / Shift-Return, right-click, ⌘Q double press, ⌘K, pairing | round 8: pass (`lanes/r8-integrate/evidence/realinput/`) |
| Hold ⌘ for jump hints, ⌘Q hold, backend stop/restart, Files editor typed bytes, drags | round 4: pass; not re-run |
| IME marked text | never run |

Recipe (one session, one lane copy; the tools are in `lanes/r9-input/tools` and
`lanes/r8-integrate/tools`, retargeted copies in `lanes/r10-integrate-in/tools`):

1. Preconditions: the screen is unlocked; no other T3 Code app runs (`pgrep -fl "T3 Code"`);
   the desktop has been idle 60 s (`ioreg -c IOHIDSystem | grep HIDIdleTime`). Take the lock:
   `mkdir target/t3-ui-parity/lanes/.realinput-lock` (if it exists, another lane holds it:
   wait). Remove it as soon as the input is sent.
2. Back up `~/Library/Application Support/exact/com.exact.t3code.macos/data/t3-code.json`
   and `defaults export com.exact.t3code.macos <backup>.plist`; note the input source.
3. `bun target/t3-ui-parity/lane-build.mjs <lane>`; `T3_RUNTIME=$PWD/target/t3-ref/runtime-f870c41
   sh target/t3-ui-parity/lane-backend.sh <lane> <port>` (a fresh port); for R7 also a second
   backend with its own `home/userdata/environment-id`. Launch the lane copy by path
   (`open -n "<lane>/T3 Code.app"`, never by bundle id), record its pid, start
   `stray-watch.sh` (widened to every lane copy but yours).
4. Pair by typing the pairing URL into Add environment's Host with Orca `type-text
   --text-stdin` after an HID click (never printed). Gate every input with `gate.sh <pid>`
   (frontmost and unlocked); send keys and clicks with `k.sh` (`r8input`/`r9input`:
   `click x y [cmd|shift|right]`, `key <k> [cmd …]`, `sleep:ms`); read state with `st.sh`
   (secure values redacted) and `shot.sh`.
5. R1: select Korean 2-Set (`r9input` can switch the source), click the transcript (composer
   not focused), ⌘B (the sidebar toggles), ⌘K (the palette opens), Escape. Then in the
   composer type a Hangul syllable and press ⌘B while composing (the text stays and bolds).
6. R2: hover a thread row and keep the pointer still; Settle it with its row action, press
   ⌘Z within 5 s without moving: the row that comes back under the pointer is hovered (its
   actions show), the old one is not; move away: nothing stays painted.
7. R3: open a draft in a disposable repository with the fake gh (`lanes/r6-pr/fakegh`), branch
   picker, type `#101`, Checkout pull request: the field's text is selected (typing `102`
   replaces it); type `1`, `0`, `2` quickly: "Resolving pull request..." stays until about
   450 ms after the last key (film with `shot.sh` every 50 ms).
8. R9: make five fixture threads (`lanes/r11-integrate-up/steps/prep.py` does it through the
   app). Hover card 2, press its Settle button and drag down over cards 3 and 4 with the
   trackpad (a 6 pt move arms it): cards 2–4 show the selected surface and Settle badges;
   release: "Settled 3 threads, ⌘Z to undo"; ⌘Z brings them back. Repeat, press Escape before
   releasing (the reference cancels; this client applies: known in-app gap). Start a sweep
   while a row's hover card is open (the reference closes it; this client keeps it: known).
9. R10: type a draft in a new-thread draft, select another thread, right-click the draft row:
   Copy ▸ Path / Branch (toast "Path copied" / "Branch copied" with the text), Project
   settings opens the project's settings, Discard draft shows "Discarded 1 draft, ⌘Z to
   undo"; ⌘Z restores the draft text and images.
10. R11: with a device hub (lane `r11-integrate-dv` uses the fixture proxy), quit every lane
   app, launch the lane copy cold while the Mac is busy (a build running), open the Device
   surface and the iPhone: the phone stays 3D (no "3D requires the H.264 stream").
11. R4–R6 and R8 as items 16–18 of the checklist below. R7 as items 19 and 22.
12. Quit the lane app (⌘Q twice), stop the backends you started, restore `t3-code.json` and
   the preferences domain, delete only the Keychain item(s) for your fixture origins
   (`security delete-generic-password -s <service> -a <origin>` for the item you created),
   reselect the user's input source, remove the lock directory. Raycast owns ⌘1 on this Mac.

## Remaining gaps

| # | Gap | Kind |
|---|---|---|
| 1 | A switched-off loopback environment stays listed under Environments (the reference's primary has no row). During a row-action sweep the hover card or tooltip open at the press stays until release (hover is root state; the host sends no hover events during a pan), and Escape does not cancel a sweep (the section's local state gets no keys mid-pan). Rows (thread and draft) have no keyboard context menu (no key handler on rows). The sidebar's minimum width does not grow with font size (the clone has no interface font-size scaling). No project drafts cannot switch machine, so 845ddd9354's "Could not switch machine" toast has no path, and "not reached this device" rereads once instead of waiting up to 10 s. In a live session the "Worktree ready" card stays above a retried thread. The reference floats a thread's live device session on load; this client does not. Right-panel persistence was driven at 1280 only. Shiki: about 12k corpus characters still differ (type-alias right-hand sides, `|` in types, JSX in `.ts`, an initializer ending at a line-final operator). Rendered HTML loads only its token directory, not external hosts. Terminal drawer/surface not built; card script rows disabled; Toggle Developer Tools absent | in-app |
| 2 | Code-line wrap break positions; word-boundary truncation (sweep's settled rows "fixture…"); agent mode stores a 1970 onboarding time (virtual clock); dialog and popover shadows faint or missing; tooltip offsets; Send tooltip shifted; PR pending labels; flipped hover card overhang; open-state tint; AVKit chrome; `text-wrap: balance`; placeholder colour; opaque composer; textarea sizing; heavier text (pill crops 3.9 / 7.2); no cursor property; WebKit's default form controls in rendered HTML | framework |
| 3 | R1–R11 above: chords under Korean 2-Set outside the composer, the re-hover after a real ⌘Z, select-on-open and real-key debounce timing, ⌘B after a click and while composing, the first click after composing, a real wheel then a thread switch, Connections with real pairings and a relaunch, Duo pinch / orbit / flick, the sweep with a real drag, the draft row's right-click menu, the 3D phone's cold first open; ⌘1 (Raycast); IME marked text; ⌘Q hold; drags; physical keys into a device screen; Files editor caret after a press below the last line | physical-input |
| 4 | Open in editor (no `cursor` CLI); notifications; SnapShot capture; Save screenshot's NSSavePanel | os-grant |
| 5 | Real GitHub merge / ready / hand-off, Details links, linked PR snapshots, publish, PR creation, Pull Requests list data, Local PR checkout success (`gh pr checkout`) | live-credentials |
| 6 | A real device hub (serve-sim iPhone Duo with physical orientation, serve-emu foldable); a live setup-script run for the PR checkout's Worktree thread id; the reference's own Resolve/Fix press; subagents; terminal/element/review chips; provider update pill tooltips; Usage share by metric on real usage data (unit tests only) | fixture-cannot-produce |

## Lane tooling (`target/t3-ui-parity/`, ignored apparatus)

- `lane-build.mjs <lane>` builds and snapshots the bundle to `lanes/<lane>/T3 Code.app`.
  Rebuilding swaps that copy, so never rebuild a lane while one of its drives runs.
- `lane-backend.sh <lane> <port>` starts (or reuses) an isolated fixture server (HOME,
  CODEX_HOME, CLAUDE_CONFIG_DIR, XDG all lane-local) and writes single-use
  `pairing.json` / `browser-pairing.json`. A restarted server loses pending approvals
  and questions: create fresh `fixture approval|question` threads after a restart.
- `lanes/r4-integrate/tools/`: `session.py` + `drive.sh` (scripted native session:
  `shot`, `hover`, `ids`, `by_label`, `attach`), `steps/r4cell.py` + `rcell4.mjs`
  (round-4 cells), `ncell.mjs`/`idrive.mjs` + `rcell.mjs`/`ref.mjs` (round-3 cells),
  `full.sh`, `native-final.sh`, `compose.py`/`compose4.py`, `maindiff.py`, `pdrive.mjs`
  (seed-transfer relaunch), `swift-all.sh`, `closepanel.js` (closes the reference's
  remembered right panel). Thread ids: `*.thread`; the disposable repo:
  `repos/int-demo` (now on `spot-topic`, one dialog commit on `main`).
- Native hit-testing (round 5, r5-shell): `pointer-events="none"` is honoured only on SVG; on a box
  it still takes presses and the agent's hover. A window-level tooltip layer must also carry `inert=true`
  (the sidebar chrome tips covered the window's top 58 pt and a 150 pt strip of the chat before). Box
  `transform=`/`transform-origin=` are SVG presentation attributes: a box takes `translate`/`rotate`/`scale`
  about its centre. To prove a hit path, press by coordinates (`tap t3-code {down, at:[x,y]}` + `pointer up`),
  not by id. Lane tools: `lanes/r5-shell/tools` (session, ref.sh with a fresh-state `REF_STATE`), steps in
  `lanes/r5-shell/steps`.
- `lanes/r5-integrate/` (round 5): `steps/prep.py` builds the lane state through the app
  (adds `repos/int-demo`, the int thread with a table, README link, `notes.md` +
  `table.csv` and PRs #72/#73, fixture approval/question/Mermaid threads),
  `steps/r5cell.py` + `tools/rcell5.mjs` (round-5 cells; the reference dismisses the
  provider banner by script, since its × sits under the docked card), `tools/all5.sh`,
  `compose5.py`, `full.sh` (round 3 + 4, retargeted copies of r4-integrate's tools),
  `swift-all.sh` (15 binaries), `steps/spot-*.py`. `tools/drive.sh` exports
  `T3_SCRATCH_ROOT` so a `T3_DRIVE_SEED_PREFS` seed lands in the data root the app
  reads. `repos/many-refs` has 251 refs for paging.
- Native hit-testing also ignores overflow: a child drawn outside its parent's frame
  (negative margin) takes no press; widen the parent instead (fix 2 above).
- `lanes/r6-pr/` (round 6, pull request row): `fakegh/` is a local GitHub CLI stand-in (`gh.mjs`
  over `state.json`; `state.seed.json` restores it) that serves the HEAD server's pull request reads
  (auth, `api user`, the core detail and viewer-permission GraphQL, `pr view/list`, `repo view`,
  stacks) and applies `pr merge` / `pr ready` to its own state; every other write is refused and no
  call leaves the machine (`calls.ndjson` logs them). The lane backend finds it first because the
  isolated home's `.zprofile` prepends `fakegh/bin` (the server rebuilds PATH from a login shell).
  `repos/pr-demo` has `origin` = `https://github.com/t3-fixture/pr-demo.git` (the identity) behind a
  repo-local `http.proxy=127.0.0.1:9` guard and a `/dev/null` push URL; the PR heads live in the bare
  `repos/pr-demo-origin.git`, which `gh repo view` names as the clone URL. PRs 101 conflicting
  (Resolve), 102 failing (Fix), 103 draft (Ready), 104 and 107 merged by the drives, 105 running,
  106 unlinked (tab icon from detail). Removing a hand-off worktree under a running server leaves its
  status stream dying and the reference's detail reads interrupted: restart the backend after resetting
  the repo. Steps: `steps/overlays.py <theme> <size>` (`OVERLAY_NS`), `steps/effects.py
  resolve|fix|ready|merge`, `steps/tab.py`; `tools/refcells.py`, `tools/pairs-all.py`.
- Driver pitfalls: zsh does not split `$var` (use `sh` scripts); agent key names are
  `Enter`/`Escape` ("Return" is typed as text); at 1280 the provider banner × sits under
  the docked card (a tap there opens the card's menu); a partly visible sidebar row
  takes a tap without selecting (scroll the list first); the git options chevron needs
  `{down:true, at:[w-16, h/2]}` then `pointer up`.
- Round 6 integration (`lanes/r6-integrate*`): `r6-integrate` is r5-integrate's tooling
  retargeted (port 14890; `tools/full.sh`, `all5.sh`, `natives-final.sh`, `swift-all.sh`,
  compose/maindiff; repos copied from r5-integrate). `r6-integrate-pr` (port 14891) is r6-pr's
  fake gh, repos and steps retargeted (`tools/matrix.sh`, `refs2.sh` with `REF_NS`/`REF_WAIT`,
  `pairs-r6pr.py`; `steps/effects.py resolve ready merge` mutates the fixture: restore
  `fakegh/state.json` from `state.seed.json` and reset the repo before reuse, then restart the
  backend). `r6-integrate-mp` (port 14892) holds the media cells (`tools/matrix.sh`,
  `compose.py`), the polish spot (`steps/r6i-a.py`) and the relaunch pair
  (`steps/r6i-persist1.py`, `r6i-persist2.py`). Lane apps there are copies of the
  r6-integrate build. In zsh, `for c in "a b"; set -- $c` does not split: loop in `sh`.
- Round 7 integration (`lanes/r7-integrate*`, all copies retargeted with `sed`; each sub-lane
  holds a copy of the r7-integrate app): `r7-integrate` (port 14910) is r6-integrate's tooling
  (`steps/prep.py` builds the state through the app; `tools/full.sh` then `all5.sh`, then
  `compose.py`/`compose4.py`/`compose5.py` and `maindiff.py pairs|pairs4|pairs5`;
  `swift-all.sh` sets `T3_DEVICE_GLB_DIR`). `r7-integrate-pr` (14911) is r6-integrate-pr with
  pristine `repos/` from r6-pr and r7-handoff's steps (`setup-script.py`, `handoff.py`,
  `send.py` with `evidence/seed-after-handoff.json`, `badge-open.py` with
  `evidence/seed-persist-strip.json`); `tools/pairs-r6pr.py` pairs with r6-pr's reference shots,
  `pairs-all.py` with this lane's own. `r7-integrate-mp` (14912) is the media matrix.
  `r7-integrate-po` (14915) is r7-polish's cells (`tools/matrix.sh`; its `wt` copy had its
  stale worktree registration removed and `origin` repointed). `r7-integrate-dev` (backend
  14913, fixture proxy 14914 started with `bun tools/devproxy.mjs 14914 14913`; `tools/repair.sh`
  re-pairs through the proxy; `tools/matrix.sh`, `steps/effects.py <ios|android>`,
  `steps/rotate.py`, `steps/probe3d.py`). The proxy keeps device orientation across sessions:
  restart it before a rotation pair. Ports in use by earlier lanes: r7-device 14902/14903 and
  r7-polish 14903 collided; pick fresh ports.
- Round 8 integration (`lanes/r8-integrate*`; each sub-lane holds a copy of the r8-integrate
  app): `r8-integrate` (port 14930) is r7-integrate's tooling retargeted with `sed`
  (`steps/prep.py`, `tools/full.sh`, `all5.sh`, `compose*.py`, `maindiff.py`, `swift-all.sh`;
  `tools/mksub.sh <pr|mp|po|dev>` makes a sub-lane from its round-7 counterpart). Sub-lanes:
  `r8-integrate-pr` (14931; pristine `repos/` from r6-pr, fake gh from `state.seed.json`),
  `-mp` (14932), `-po` (14935; `wt` origin repointed), `-dev` (backend 14933, fixture proxy
  14934). `r8-integrate-r8` (14936) holds the round-8 states: `steps/prep.py` (D15, then a
  Parity fixture "fixture complete table" thread), `steps/r8cell.py <theme> <size>` (D9, D10,
  D6, D5 in that order: the table menu first leaves the timestamp hover flaky),
  `tools/ref-r8.mjs <theme> <size>` (re-run `lane-backend.sh` first), `tools/pair.py`,
  `steps/persist1.py` / `persist2.py` (seed transfer). Real input: `lanes/r8-integrate/tools/`
  `gate.sh` (our pid frontmost, screen unlocked), `k.sh` (gated `r8input` steps: `click x y
  [cmd|shift|right]`, `key <k> [cmd …]`, `sleep:ms`), `st.sh` (Orca AX snapshot, secure
  values and tokens redacted), `shot.sh`, `stray-watch.sh` (stops LaunchServices launches of
  stale lane copies while `ri/real-input.on` exists). `r8input type` corrupts digits after
  capitals and types Hangul under Korean 2-Set; use Orca `type-text` for text (it doubles text
  in the composer). Pair a Mode B app by typing the full pairing URL into Add environment's
  Host field with the code empty (`--text-stdin`, never printed).

- Round 9 integration (`lanes/r9-integrate*`; each sub-lane holds a copy of the r9-integrate
  app; a missing `scratchpad/lane-<lane>` directory makes `drive.mjs` refuse, so create it
  first): `r9-integrate` (port 14950) is r8-integrate's tooling retargeted with `sed`
  (`steps/prep.py`, `tools/full.sh`, `all5.sh`, `compose*.py`, `maindiff.py`,
  `swift-all.sh`, `r9pair.py` for side-by-side pairs, `subchain.sh` and `r9chain.sh` that run
  every sub-lane in order; `tools/mksub.sh <s>` makes `r9-integrate-<s>` from
  `r8-integrate-<s>`, and repos/fake gh must then be copied by hand). Sub-lanes: `-pr`
  (14961; r6-pr's pristine repos, fake gh from `state.seed.json`, isolated `.zprofile`), `-mp`
  (14962), `-dev` (backend 14963, proxy 14964), `-po` (14965; `wt` origin repointed), `-r8`
  (14966; also `steps/persist1.py`/`persist2.py`). Round-9 states: `-in` (14969; r9-input's
  tools, `steps/scroll.py` makes the long thread, `steps/r9cell.py <theme> <size>` drives
  items 6, 4, 5 and 2 with names matching `tools/ref-r9.mjs`, `steps/spot6.py`), `-cn`
  (14970, with backend B `-cnb` on 14971 whose `home/userdata/environment-id` was given a
  fresh id before its first start; r9-connect's tools: `conn.py`, `conn2.py`, `conn3.py` (B's
  environment id prefix is hard-coded: `08e1e362`), `conn4.py`, `pr1.py`, `pr2.py`,
  `onb1.py`/`onb2.py`; references `refconn.mjs pairshots|wrong` (it now clicks toast close
  buttons first, since a toast covered Add environment) and `refpr.mjs shots|local`). Re-run
  `lane-backend.sh r9-integrate-cnb 14971` before every use of B's single-use pairing. `-duo`
  (backend 14967, proxy 14968; r9-device's tools, `wt` from r9-device, `steps/prep.py` from
  `-po`; `tools/matrix.sh <theme:size:platform>…`, `tools/cellpairs.py`, `steps/files.py`).
- Round 10 integration (`lanes/r10-integrate*`; each sub-lane holds a copy of the r10-integrate
  app): `r10-integrate` (port 14970) is r9-integrate's tooling retargeted with `sed`;
  `tools/mksub.sh <s> [source lane]` makes `r10-integrate-<s>` (default source
  `r9-integrate-<s>`), copies repos (pr/cn/c10 take r6-pr's pristine `pr-demo`; origins
  repointed), resets the fake gh from `state.seed.json` and writes the isolated `.zprofile`.
  Chains: `tools/main-chain.sh` (round 3 + 4 via `full.sh`, then `all5.sh`; then
  `compose*.py` and `maindiff.py pairs|pairs4|pairs5`), `subchain.sh` (pr 14972, mp 14973,
  po 14976, r8 14977), `devchain.sh` (dev backend 14974 + proxy 14975, then duo 14991 + proxy
  14992), `r9chain.sh` with `R9="in cn"` (in 14978, cn 14979, cnb 14990: its home was made from
  `backend/home` with round 9's B environment id `08e1e362…` copied in before the first start),
  `c10chain.sh` (round-10 Connections: `r10-integrate-c10` 14993, B `r10-integrate-c10b` 14994
  with r10-connect-b's environment id), `d10chain.sh` (round-10 device: backend 14995, proxy
  14996; duo10 twice, tips, `DUO_PHYSICAL=1` physical, Files, crumbs relaunch). Pairs:
  `tools/r10-integrate-r9pairs.sh`, `r10-integrate-d10pairs.sh`, `r10-integrate-pair.py` (whole
  and right-panel means), `r10-integrate-coverscan.py` (flags a black Duo cover). Spot steps:
  `r10-integrate-c10/steps/r10-integrate-spotstate.py`,
  `r10-integrate-d10/steps/r10-integrate-persist1.py` / `persist2.py`. Port collisions found
  when copying lane tools: r10-device used 14961 and r10-connect 14962, the round-9 sub-lane
  ports; grep a copied lane for its ports as well as its lane name.
- Round 11 integration (`lanes/r11-integrate*`; each sub-lane holds a copy of the r11-integrate
  app): `tools/mkr11.sh main|<s>` makes `r11-integrate[-<s>]` from `r10-integrate[-<s>]` with `sed`
  (lane names, ports 14970 → 14990 and the sub-lanes to 151xx, `runtime-f90b77d8` →
  `runtime-f870c41`), copies repos (pr/cn/c10 take r6-pr's pristine `pr-demo`), resets the fake gh
  and writes the isolated `.zprofile`; file names keep their `r10-integrate-` prefix
  (`r10-integrate-pair.py`, `-r9pairs.sh`, `-d10pairs.sh`, `-coverscan.py`; `r11-integrate-pair.py`
  is a copy the d10 script calls). The fake gh's `state.seed.json` named round 9's bare repo as
  the clone URL; the r11 copies point at their own `repos/pr-demo-origin.git`. B homes
  (`-cnb` 15110, `-c10b` 15114, `-msb` 15124) are `backend/home` with round 10's / r11-misc-b's
  `userdata/environment-id` copied in before the first start. Chains (export `T3_RUNTIME`
  first): `main-chain.sh` (after `steps/prep.py`), `subchain.sh` (pr 15102, mp 15103, po 15106,
  r8 15107), `R9="in cn" r9chain.sh` (in 15108, cn 15109 + cnb 15110), `c10chain.sh` (15113 /
  15114), `devall.sh` (`devchain.sh`: dev 15104 + proxy 15105, duo 15111 + proxy 15112; then
  `ONEPASS=1 d10chain.sh`: 15115 + proxy 15116); `cmp10.py pairs|pairs4|pairs5` and `cmpsets.py`
  compare with round 10. Round-11 cells: `r11-integrate-up` (15121; r11-upstream's steps with the
  Parity fixture project id, `steps/retry1.py` adds a fresh disposable `repos/retry-demo` through
  the app, `unsweep.py` reads the rows the sweep settled from `swept.json`; `tools/upchain.sh`.
  `refsweep.mjs` takes row indexes: after a native sweep the settled rows follow the active ones,
  so pick `from`/`to` from its printed `keys`), `r11-integrate-ms` (A 15123, B `-msb` 15124;
  `tools/mschain.sh`: Files `page.html,decl.ts,runaway.html`, `r11conn.py` at both sizes,
  `refconn.mjs`), `r11-integrate-dv` (15125 + proxy 15126; `tools/dvchain.sh`: persistence by
  seed transfer, `r11-refpersist.py`, `STALL=<s> r11-runs.sh` first opens). The reference's
  `refdraft.mjs` keeps browser-local drafts across runs: later reference shots of the sidebar
  show them.
- Copied tools may still point at their source lane through a path built from parts
  (r9-connect's `refconn.mjs` read `T+'/r9-connect'`): grep a copied lane for the old lane name
  before running it.

## Safety notes

- Round 11 integration: no real input (the screen was locked), so the real data root,
  preferences and Keychain were never touched. Agent-mode drives only, each on its own lane
  copy and isolated f870c41 backend. Written: disposable `lanes/r11-integrate*/repos/*` (a
  `retry-demo` repository created for the Retry cells, its origin renamed away and back, and a
  server worktree under `-up`'s isolated home; a PR checkout worktree under `-cn`'s), the fake
  gh state under `-pr`/`-cn`/`-c10`/`-dv`, and the fixture hubs' input logs. The first-open
  stall runs sent SIGSTOP / SIGCONT only to their own lane app's pid. No pairing URL or token
  was printed. Nothing was pushed or sent to GitHub. Every backend, proxy and app this pass
  started is stopped (the two `realinput2*` backends on 15001 / 15002 belong to another lane
  and were left alone).

- Round 10 integration: no real input (the screen was locked), so the real data root,
  preferences and Keychain were never touched. Agent-mode drives only, each on its own lane
  copy and isolated backend. Written: disposable `lanes/r10-integrate*/repos/*` (a PR checkout
  worktree under `-cn`'s isolated home), the fake gh state under `-pr`/`-cn`/`-c10`, and the
  fixture hubs' input logs. The wrong-code shots (`lanes/r10-integrate-cn/evidence/native-add-
  wrong-*.png`, `ref-wrongcode-dialog-light-1280.png`) show a wrong code derived from backend
  B's single-use code, as in rounds 9 and 10's connect lane; B is stopped. No pairing URL or
  token was printed. Nothing was pushed or sent to GitHub. Every backend, proxy and app this
  pass started is stopped.

- Round 9 integration: no real input (the screen was locked throughout), so the real data
  root, preferences and Keychain were never touched. Agent-mode drives only, each on its own
  lane copy and isolated backend. Written: disposable `lanes/r9-integrate*/repos/*` (a PR
  checkout worktree under `-cn`'s isolated home; `wt` remotes repointed), the fake gh state
  under `-pr`/`-cn`, and the fixture hubs' input logs. The headless reference's wrong-code
  screenshot (`lanes/r9-integrate-cn/evidence/ref-wrongcode-dialog-light-1280.png`, as in
  r9-connect's lane) shows a wrong code derived from backend B's single-use code; B is stopped.
  Nothing was pushed or sent to GitHub. Every backend, proxy and app this pass started is
  stopped.

- Round 8 integration: one real-input session on its own lane copy against its own fixture
  backend, gated on that pid being frontmost; started only after 60 s of desktop idle. It
  backed up and restored `t3-code.json` and the preferences domain, deleted only the Keychain
  item it created (14930), left the 14773 item alone, and re-selected the user's Korean 2-Set
  input source. A corrupted, never-valid pairing token was printed once in the transcript;
  its backend is stopped. Orca's own screenshots of the pairing dialog were deleted. Nothing
  was pushed or sent to GitHub.
- r4-timeline's live "Copy as Markdown" check wrote to the real NSPasteboard; later
  drives (rounds 5 and 6 included) never press Copy.
- Round 7 integration printed no pairing URL or token, used no real input (the desktop may
  have a real-input agent running), wrote only disposable `lanes/r7-integrate*/repos/*`, a
  hand-off worktree under `r7-integrate-pr`'s isolated home and that lane's fake gh state, and
  sent device input only to the fixture proxy (`lanes/r7-integrate-dev/hub-input.ndjson`).
  Nothing was pushed or sent to GitHub.
- Round 6 printed no pairing URL or token. Repositories written: only disposable
  `lanes/*/repos/*` (r6-integrate-pr: a hand-off worktree under its isolated home; the
  fake gh applied `pr ready 103` and `pr merge 107` to its own state file). Nothing was
  pushed or sent to GitHub; pr-demo keeps its `http.proxy=127.0.0.1:9` guard and
  `/dev/null` push URL, and acme/shared points at `git.example.invalid`.


## Manual verification checklist (human, physical input / OS grants)


1. Build with `bun host/apple/build.mjs macos-t3-code-apple --bundle --run`, pair a
   disposable server, dismiss the Nightly notice and a provider-update toast, pick
   Tokens / 7 days on Usage, type drafts on a thread and a new-thread draft, drag the
   sidebar, hide the Files explorer and switch a Markdown file to rendered; quit and
   relaunch: all should persist (real `~/Library` data root).
2. Keyboard: Return / Shift-Return / ⌘↩ in a new draft (starts in background) and on a
   thread; ⌥⌘↩ (sends, opens a fresh draft); hold ⌘ over Send; ⌘K, ⇧⌘M, ⌘B/⌘I, ⇧⌘V, ⌘Z
   after a sidebar action, ⌘Q hold/double-press; Escape in the device wizard (only the
   wizard closes) and the table Copy menu; P in the surface chooser.
3. Typing: in the composer and in the Files editor type `"`, `'`, `--` and `-->`; the
   sent prompt and the bytes on disk must be exactly what was typed.
4. Pointer: hover the sidebar's right edge (2 pt line, then drag 16 pt wide strip; no
   resize cursor is expected), hover the settled row near the list bottom (card opens
   above), click the model picker at its very left edge; right-click a thread row, a
   message and the header title; ⌘/⇧-click rows; drag a thread onto the composer and
   between shelves; drag a queued message; hover a timestamp, a message row, the model
   picker, composer chips, a details-card PR row (tooltip); scroll the transcript toward
   its end with a 5-line draft and scroll a long thread so the composer rests (xs labels).
5. Attachment preview: Save file shows NSSavePanel and writes the file where chosen;
   Copy contents shows "Copied" for 2 s.
6. Menus: App menu and Help › Check for Updates… ("not available" alert).
7. Grants: notifications (banner, sound, click opens the thread); SnapShots capture;
   Open in Cursor from the card and the Files subheader.
8. Network: Wi-Fi off/on with a thread open; background and foreground the app.
9. Credentials (optional, disposable accounts only): PR row detail (state, checks,
   Ready), linked PR checks/review, PR checkout from the branch picker, Publish
   repository, Commit, push & PR, Pull Requests list.
10. Pull request row (disposable repository with a linked PR, or the lane's fake gh): hover the
    row with a real pointer (card opens above, flips near the window edge), open the checks
    popover and "Show all", press a check's Details (opens the browser), Resolve/Fix (worktree,
    draft prompt, "Checkout ready" toast), Ready and Merge (confirm sheet, toast).
11. Attachment previews: click a link inside a rendered HTML attachment (opens the default
    browser, the preview does not navigate); an HTML `alert`/`confirm` shows as a sheet; scrub
    and play audio/video with the pointer (AVKit controls reveal on hover); Try again on a
    broken file.
12. Devices (a server with the device hub enabled, a simulator and an emulator): open each,
    stream in 3D and flat, Home/Rotate (Android: Back, Recents, Portrait/Landscape), type on the
    focused screen with the physical keyboard (letters, modifiers, Escape as Back on Android;
    ⌘R stays with the app), drag on the screen (touch) and outside it (orbit), ⌥-drag, two-finger
    trackpad orbit and snap, Restore 3D view, the iPad Magic Keyboard toggle, the Tools drawer
    (Open URL, Launch, Terminate, appearance, location, permissions, the accessibility frames),
    hover the rail and drawer tooltips, Save screenshot (NSSavePanel), Float over chat, drag the
    floating player, Close and Power off; check the device itself did what the drawer said.
13. Sidebar Working counter and worktree setup counters advance in real time during a New
    worktree send with a Setup script; the scroll-to-end pill beside the 840 attachment sheet
    after scrolling the transcript up with a real wheel.
14. Pull request hand-off with a worktree setup script (disposable repository, fake gh or a
    disposable account): Resolve/Fix; the worktree's setup script runs once for the draft's
    thread; quit and relaunch, send the draft: the thread that starts is the one the setup
    terminal belongs to. Hover the composer's pull request chip with a real pointer (fill,
    tooltip list) and press it (opens the pull request surface).
15. Settings › Connections (round 9): with a real pairing, the paired server is a row under
    Environments (switch, ⋯ Icon / Remove from this device…); pair a second server: Load
    balancing and GitHub sharing appear; pair with a wrong code, with and without a
    connection: no row, no Keychain item, `t3.server.origin` unchanged; quit and relaunch: only
    the saved rows return.
16. Round-9 keyboard, with your usual input source and with ABC: click into an existing draft
    (do not type), ⌘A, ⌘B (bold, the sidebar stays); type a Hangul syllable and press ⌘B while
    it is still composing (the text stays and is bolded); with the composer not focused, ⌘B
    under Korean 2-Set (framework: does not toggle); ⌘1 with no global hotkey on ⌘1; ⌘N with
    the sidebar closed; ⌘D, open a file tab, ⌘W (the file tab closes), ⌘W (the panel closes),
    ⌘W (the window closes).
17. Round-9 pointer: Settle a row, ⌘Z within 5 s, then move the pointer away (nothing stays
    painted); type Hangul in the composer, then click a header button or a sidebar row once
    (it acts on the first click; compare with `R9_INPUT_HOLD_COMPOSITION=1`); scroll a long
    thread up with a real wheel, open another thread, come back (same position, Scroll to
    end pill); press below the last line of a Files editor and type; quit and relaunch twice
    (same frame, reconnects by itself, onboarding time is a real 2026 date).
18. Devices with a real hub: open an iPhone Duo, press Closed / Book / Open / Laptop / Tent
    (the cover paints after Closed; check it under load too), pinch the trackpad over it (the
    hinge follows), orbit to the other display; open a foldable emulator, Fold / Unfold.
19. Round-10 Connections, with real pairings: switch off the only environment, Add environment
    with a valid pairing URL: Settings › Connections stays, the "Backend added" toast shows;
    a wrong code with nothing connected saves nothing and `t3.server.origin` is unchanged;
    paste a full pairing URL into Host: Host and Pairing code fill; quit and relaunch: only the
    saved rows return, the switched-on one reconnects.
20. Round-10 keyboard and pointer: with Korean 2-Set and the composer not focused, ⌘B toggles
    the sidebar and ⌘K opens the palette; open Checkout pull request from the branch picker
    with `#101`: the field's text is selected, typing replaces it, and "Resolving pull
    request..." stays until about 450 ms after the last key; Settle a row and press ⌘Z with
    the pointer resting on the list: the row under the pointer is hovered.
21. Round-10 Files and devices: open an `.html` file (rendered; the code / eye toggle switches
    to source and back), quit and relaunch (the thread's Files panel, the file and the mode
    return; breadcrumbs end at the file); with a real device hub, press Closed on an iPhone
    Duo several times, also while the Mac is busy (the cover paints every time), hover the
    stands and Fold / Unfold (tooltips on the left), orbit to the other display and flick fast.
22. Round-11 sidebar and panels: on a thread row press Settle and drag across two more rows
    with the trackpad (they arm; release settles all three, "Settled 3 threads, ⌘Z to undo";
    ⌘Z restores), then the same with Un-settle in the Settled shelf; right-click a draft row
    (Copy ▸ Path / Branch, Project settings, Discard draft, then ⌘Z); with two environments,
    one switched off, Settings › Connections shows Load balancing and GitHub sharing with "This
    machine"; open an `.html` file whose page uses a sibling stylesheet, image and script (all
    load); open a Diff, a device, a pull request and an attachment in one thread's right panel,
    quit and relaunch (all four tabs and the active one return); with a device hub, open the
    iPhone right after a cold launch while the Mac is busy (stays 3D).
