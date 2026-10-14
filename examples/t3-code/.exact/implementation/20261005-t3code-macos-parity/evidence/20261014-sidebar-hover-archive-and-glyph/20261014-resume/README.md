# Sidebar resume, 2026-10-14

This attempt finishes the implementation at `dd2de793c` and preserves its earlier failures. The task remains
unverified until independent review and the coordinator's final real-input session.

## Source and target

- Integration base `609f392d5`, task preparation `a2def63d0`, prior implementation `dd2de793c`.
- Reference T3 Code `1e2ecbd9758830669684b494d4398f626b0576e0`, read-only. Source inspected in `Sidebar.tsx`,
  `ThreadStatusIndicators.tsx`, `useThreadActions.ts`, `SettingsPanels.tsx`, and client-runtime `pullRequests.ts`.
- macOS AppKit desktop, macOS 26.6.2 build 25G83, Bun 1.4.2. This attempt uses agent-mode input only.
- Runtime and development commands run from the task's own checkout. `EXACT_APP_DIR=$PWD/examples/t3-code`.
- App-source hashes are in `source-fingerprint.json`. Build output and the full raw drives are local ignored files.

## Preserved before and after

`prior-drive-summary.json` extracts the relevant fields from the original local JSONL drives. Operation indexes
refer to the zero-based lines in `target/shag/drive-before/drive.jsonl` and `drive-after/drive.jsonl`.
The original before drive used the feature base `609f392d5`; after used implementation `dd2de793c` before its
unfinished async repair. These are historical evidence, not proof of every later repair.

- SW-2: `drive-before-sw2-3-back-in-row.png` has no row hover or card after leaving Settle.
  `drive-after-sw2-3-back-in-row.png` shows both again. The images were inspected during this resume.
- SW-3a: the before held-press capture still shows `Settle thread`; the after capture hides the tooltip while
  keeping the row hovered. No repeated pixel matrix was performed.
- R11-X63: after Archive, the before drive has `TextArea` focus, and Cmd-Z clears `undo check` without restoring
  the row. The after drive has no editor focus; Cmd-Z restores `Undo me` and keeps `undo check` unchanged.
- G4-14b: the old tree declared `Unarchive Archive fixture B`; the new button has no explicit label and its text
  is `Unarchive`, with a hidden decorative icon. This was a logical tree read, not a VoiceOver or AX-name test.
  The delete fallback `this thread` is checked by the Bun test and reference source; the native dialog is still
  in the final input recipe.
- B12: the earlier after drive drew PR #113 but also reported `Cannot set property 'reading' of undefined`.
  This attempt finishes the existing repair by giving each asynchronous summary read its own call arguments.

## Checks and repaired regression

See `checks.txt` and `outside-press-before-failure.txt`. The full app suite exposed a separate regression from the new pointerdown bindings.
The controls had taken the window's outside press, leaving a pinned Usage popover open. They now forward
`outsidePressDown` through the new `sidebarPressDown` action. A behavior test checks that the primary press
counts when released and a secondary press does not. This also preserves the existing skill-chip and
thread-details outside-press behavior.

The async regression baseline is the native error in `prior-drive-summary.json`. Bun does not reproduce that
runtime capture fault; its added tests cover separate concurrent answers, deduplication and aborted-read retry.

The full suite passes with 4,876 tests and one existing skip. Focused tests cover concurrent PR summaries,
duplicate in-flight reads, an aborted refresh retaining its prior summary, hover restoration, held-press tooltip
closure, archive focus, archived delete copy, and both sidebar PR presentations. Strict TypeScript, Contract,
caps and boot checks pass. The first native build passed; the plan-only rebake failed at the existing markdown
source with `Unknown T3 source: renderMarkdown`. The full native fallback then built successfully.

## Final native drive

`native-build.json` identifies the final binary and receipt, with no changed source inputs. The first launch's
saved scratch store had no connected environment, so Studio branch was absent. Its setup failure is preserved
in `fixture-startup-failure.txt`. The successful drive paired that store to the already running isolated lane
on port 16523, using `tools/github-lane/lane.mjs pair primary`. The pairing URL was read directly from the
lane's local file and is absent from these artifacts. No GitHub write or provider message was sent.

The connected drive passes in `native-observations.json`:

- Both sidebar snapshots identify Studio branch with PR #113, open state, and the pull-request glyph.
  The legacy snapshot gives `PR #113 - Open: Document the release steps`. `error` stays empty.
- Leaving Settle for the row restores the exact row hover key. `native-row-hover-restored.png` shows
  the row's hover card and the green 113 badge.
- While Settle is held, `sidebarHoverY#1=-10000` and `outsideDown#1=true`.
  `native-control-held.png` shows no tooltip and retains row hover. The press was cancelled without activating Settle.
- `native-legacy-pr.png` shows the legacy glyph. The final screenshots were inspected.

Reproduction uses the same driver operations as the task acceptance recipe. With a named scratch store,
open Connections, pair the isolated lane, wait for Studio branch's PR summary, read state, and switch General >
Legacy features > Legacy sidebar. In the default sidebar, hover `row-<thread-id>`, then `settle-<thread-id>`,
then `thread-<thread-id>`; advance the agent clock 900, 800 and 900 ms respectively. Read state. Hover Settle,
advance 800 ms, issue `tap settle-<thread-id> down`, read state/screenshot, and issue `tap cancel`.
The raw local drive is `target/shag/drive-resume-connected`.

## Final input recipe

The coordinator runs these after integration, using the existing isolated fixtures and no provider messages.

1. Hover a row, move to Settle or Snooze, then back to the title. Its hover surface and detail card return.
2. Hover Settle, press and hold without movement. Its tooltip disappears while the row remains hovered.
3. Type `undo check`, use a row's Archive menu item, then Cmd-Z. The thread returns and the draft stays unchanged.
4. In Settings > Archive, inspect the Unarchive accessibility name and open Delete from the row's menu.
   Expect `Unarchive` and `Delete thread "this thread"?`; cancel the destructive confirmation.
5. In the real GitHub lane, inspect Studio branch in each sidebar. Expect PR #113 and the legacy tooltip
   `PR #113 - Open: Document the release steps`. No GitHub mutation is needed.

Global input, VoiceOver speech, dark/narrow pixel matrices and final integrated Cargo checks were not run by
this worker. No PR was published and the reference checkout was not changed.
