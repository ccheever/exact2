# Pinned original Electron terminal coverage

Original source pin **1e2ecbd975**, app0.0.45, Electron44.4.2. Original desktop and renderer were built and executed; the original server bundle was reused. `source-identity.json` compares eight relevant copied source/server files directly with the pinned reference; all match. Node26.7.0 differs from declared^24.13.1. `/bin/sh` was used. Isolated HOME/profile/backend/project; no real provider send, account login, installation, protocol registration, or user-project deletion performed. Fake ACP fixtures supplied harmless prompt and auth behavior.

This table is **bounded original coverage, not full39-row acceptance or a clone/reference pass**. A row marked Partial has an observed core path but unverified subcases. The original acceptance inventory defines the complete scope. Screenshots are renderer captures, not native OS menu screenshots. Native menu callback tests retained actual constructed Electron Menu objects while preserving popup behavior, then called the selected item's callback; they establish downstream behavior, not a genuine OS menu gesture.

Earlier baseline evidence means `target/terminal-verification/electron/REPORT.md` and its listed artifacts. That earlier run used the same pinned build and established drawer dimensions, full relaunch, three threads, flood, and small-window light/dark behavior. The extended run is detailed in `expanded-runtime-report.md`; only files in `PUBLISH-ALLOWLIST.txt` are approved evidence candidates. Raw action logs contain an unrelated clipboard observation and must remain private.

| Row | Status | Observed original behavior / evidence | Still unverified |
|---|---|---|---|
| A01 | Partial | Correct project cwd/open/attach and toolbar/shortcut toggle; baseline and curated RPC | No-project action state and full env-error matrix |
| A02 | Partial | Draft terminal before send; first worktree send retained draft identity,8 sessions/groups and old-cwd sessions;26 | All draft cancellation/replacement cases |
| A03 | Partial | Baseline280/min180/max630,400 drag/relaunch,840×620 reclamp;400ms/reduced/reversal JSON | Full visual motion matrix on native OS reduced-motion setting |
| A04 | Partial | Real raw keys/paste, UTF-8 CJK+emoji, ANSI,1049 alternate-screen/cursor restore;24/25 | Mouse-reporting program and selection-scroll edge cases |
| A05 | Partial | User-operated original Korean 2-Set `한글`+Space produced exact PTY bytes once; MANUAL-IME.md | Controlled Backspace/cancellation and Korean-layout shortcuts; phase2 mixed input is inconclusive |
| A06 | Partial | Exact close text, Cancel/Escape/Confirm; baseline shell exit closes | Close failure fallback and loading/exit races |
| A07 | Partial | Hide focuses composer; baseline delayed open did not steal composer; Add-to-chat focuses composer | Every loading/race permutation |
| A08 | Partial | Baseline3-thread and full normal relaunch without re-pair; draft/group state preserved26 | >11 eviction, disconnect/reconnect and failure recovery |
| A09 | Partial | Baseline real seq200000 ends with200000 and responsive app | Bounded buffer/Ack instrumentation,44 streams |
| A10 | Unknown | No actual restricted-scope original UI fixture | Permission denial/retry matrix |
| A11 | Partial | Disposable2-pane thread deletion: whole-thread close(deleteHistory:true) before thread.delete, then pane unmount closes | Worktree deletion variant |
| A12 | Partial | Light/dark settings1280×840/840×620; advanced terminal18px grid;14–17/23, baseline small terminal | Full custom palette/cursor/selection and hydration matrix |
| B01 | Partial | New group, sidebar labels, active group, lowest available ID reuse;03/04 | All server/client/panel collision arrangements |
| B02 | Partial | Horizontal/vertical/4 panes; fifth click no fifth; max control visually muted, not HTML-disabled;02 | Full resizing/focus matrix at both dimensions |
| B03 | Partial | Last-group close/Cancel/Confirm and freed ID reuse;03 | Dynamic subprocess labels and every fallback position |
| B04 | Partial | Panel empty surface→Terminal; new panel and split; independent drawer retained;05 | Complete active-owner matrix |
| B05 | Partial | Surface2-pane confirmation/cancel/confirm06; Close others/all bypass dialog and close owned IDs, retain drawer | Genuine OS menu selection, middle-click, mixed Browser close guard |
| B06 | Partial | Cmd+J/D/Shift+D/N/W through renderer;04 | Custom user binding and all composer-focus alternate meanings |
| B07 | Partial | Repeated Meta+W keydowns yield one dialog; Escape retains session | OS menu interception and Korean layout variant |
| B08 | Covered core | Real raw PTY bytes for option navigation,command navigation/delete/clear,arrows,Tab,Ctrl+C/D | Kitty key-release protocol variant |
| B09 | Partial | Focused-terminal Cmd+B changes sidebar geometry; Cmd+J hide focuses Message | Remaining unconditional app commands |
| B10 | Unknown | Hidden-shell script runs did not yield reliable busy metadata | Visible busy icon/count/pulse and reduced-motion |
| C01 | Partial | Actual multline drag highlights selected output;09 | Word/line multiclick,scrolling,viewport clamp |
| C02 | Partial | Actual constructed menu Add-to-chat/Copy; callback→chip; bulk menu records | Genuine OS popup navigation,paste race,late error supersession |
| C03 | Partial | Copy fixture matches clipboard; callback inserts correct line-range chip; typing lands after chip; preview exact | Arbitrary middle-caret position and normalization edge cases |
| C04 | Partial | Real UI Submit→actual fake ACP session/prompt contains referenced record once;sent-context.txt | Draft relaunch/removal,expired/empty warning,64k rejection |
| C05 | Covered core | Real chip preview exact captured3 lines and sent label;20 | Full typography/tooltip dimension matrix |
| C06 | Unknown | No file/editor fixture activation performed | File/line/column/editor/error cases |
| C07 | Partial | Real Integrations preference→T3 Code; plain URL click creates Browser webview at correct localhost URL;21 | External app outcome,modifier/fallback/error/profile matrix; clone dependencyX1 remains |
| C08 | Partial | Auth terminal has no app Add-to-chat menu; default Electron editing menu remains | URL failure/no-local-api messages |
| D01 | Partial | UI-created script idle term1 reuse and ordered open/write;12/13 | Reliable busy/default-shell new terminal and preferNew; hidden /bin/sh case reusedterm1 |
| D02 | Unknown | No failed-open/write injection | Error and preview-script matrix |
| D03 | Partial | Fake ACP completed single-line sh fence; real Run in terminal→open/attach/one write/resize;22 | Invalid language/control/newline/streaming matrix |
| D04 | Covered core | Real worktree setup stage Open terminal attaches server-chosen setup ID/worktree/env;27 | Later Start agent was fixture-confounded after provider replacement; not a product failure |
| E01 | Partial | Actual safe ACP Sign in UI creates256px auth terminal,13px font independent of advanced18;28 | Truncation/reset/new-interaction sequence |
| E02 | Partial | Real10000Q paste emits4096/4096/1808 provider.auth.respond; all bytes arrive; resize empty-data recorded | Original read-only and failed-slice runtime matrix |
| E03 | Partial | Tab moves focus with no byte;Escape1b;Cancel removes UI and PTY process exits;28 | Link external outcome and stale response rejection |
| E04 | Partial | Wizard Use existing CLI→missing Codex→Install opens256px terminal and pretypes without Enter;29 | Other drivers/platform quoting and Sign-in wizard branch |
| E05 | Partial | Onboarding namespace/UUID/cwd/providerInstanceId;one write has no CR/LF;Close deleteHistory:true;29 | Retry,write failure,rapid replacement/Done/Skip/exit matrix |

Motion details: with real Appearance setting400ms, the clipping parent grows0→280 while the inner terminal aside remains280. Media emulation `prefers-reduced-motion: reduce` jumps to280; it does not prove native OS setting propagation. Reversal after110ms peaked131.375px then returned0. The fixture logs preserve sample timestamps.

The backend remains running for continued verification. The original window is hidden and no global input is pending. Source files in the implementation repository were not edited by this oracle task.
