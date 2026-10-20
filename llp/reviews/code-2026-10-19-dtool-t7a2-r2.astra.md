# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `55cefa3b2d`.
- **Method:** one brief (sha256 `cde5b4fcb74f0d69bdfbba58c1bb3095a5c6dcaf620a7a4b44abbdbd03d613e6`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commits 2a82f19e03 and 55cefa3b2d, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 BLOCKER, 4 SHOULD-FIX).
- **Disposition:** taken in the round's fix commit (named in round 3's records):
  - The page asks the runner what the app asked without applying a batch, so a question is never a commit that asks again (1).
  - Taking over the root size the app set commits at once, at the host's (the pinned) size. Tested (2).
  - A hold is reconciled against the last one: a fact held before and not now takes the app's latest choice, in the same commit. On the JS page, a released fact's choice applies. In a copy only the pinned axes are set, and an axis the last pin set and this one doesn't goes back. Tested on both pages and in the runner (3).
  - A refused hold commit rolls back what was held, the window, the root size and the commands (4).
  - A root size the app may not set is no choice, so the last valid one stands, in the runner and on the JS page (5).
  - Tests: the runner test moved to its own module (tamber_held.rs) and grew the takeover and the release by a smaller pin; dev-copies adds a pin that holds less (8, in part).
- **Not taken:**
  - A runner replaced in place (a resident reload) forgets what the app asked, as a JS reload does (6). The app's launch choices are asked again; a pressed choice isn't handed back after a reload. This is recorded as a limit, beside the JS one.
  - Tracking in production (7): one map entry per `setMode`, `setScheme` or `setRootFontSize` call, all rare and the app's own. `take_held` scans the commit's commands, a handful. That is no measurable cost, and recording from launch is what lets a copy know a choice made before its pin.

**Four blockers remain.** Reviewed `2a82f19e03` and `55cefa3b2d` statically. Changed nothing; ran no builds or tests.

1. **BLOCKER — Any pinned WASM copy enters a perpetual callback loop.**  
   [host/web/dev.js:466](/tmp/rv-dt-t7a5/host/web/dev.js:466) clears `due`, then calls `holdFacts("?")`. That uses `wasmCall`, which sends the answer through the batch applicator; [host/web/glue.js:900](/tmp/rv-dt-t7a5/host/web/glue.js:900) consequently calls `committed()` again.  
   **Scenario:** Apply a valid pin and leave the app idle. Each status read schedules another read indefinitely. Status deduplication prevents repeated reports, but not repeated WASM calls and batch application. Round 1’s retry problem remains, now even for valid pins. The query must not apply a batch or trigger its own observer.

2. **BLOCKER — Taking over an existing app font can leave rendered rows at the old size.**  
   [runner/src/runner/root_font.rs:228](/tmp/rv-dt-t7a5/runner/src/runner/root_font.rs:228) stages the host size in the kernel, while [held.rs:61](/tmp/rv-dt-t7a5/runner/src/runner/held.rs:61) returns no receipt. The subsequent host setter sees that pending size as already current and can skip committing.  
   **Scenario:** Browser size 16px; the app has applied `setRootFontSize(24)` or `"150%"`; the tool sends a root-only 16px pin. `drop_app_root()` stages 16, and `exact_set_root_font_size(16)` returns unchanged. With no mode change or subsequent app commit, existing `rem` rows retain their 24px resolution while status reports 16px. The takeover needs to publish the root-size change.

3. **BLOCKER — Replacing a pin loses app choices for facts removed from it.**  
   [runner/src/runner/held.rs:51](/tmp/rv-dt-t7a5/runner/src/runner/held.rs:51) replaces the held set without restoring released facts. The JS equivalent at [host/web-js/dev.mjs:714](/tmp/rv-dt-t7a5/host/web-js/dev.mjs:714) replays app choices only when the entire pin becomes null.  
   **Scenario:** Pin Light/Black/18px; the app requests Dark/Dim/24px; replace that pin with scheme-only Light; then hand everything back. Dim and 24px never return: the replacement drops their holds, and final hand-back considers only the remaining scheme hold.  
   Both pages also rebuild non-null mode pins from the initial snapshot, so updating a scheme-only pin can overwrite an app change on an axis that was never pinned. Reconcile the old and new held sets, preserving unrelated axes.

4. **BLOCKER — A refused hand-back still changes facts and releases commands.**  
   [runner/src/runner/held.rs:63](/tmp/rv-dt-t7a5/runner/src/runner/held.rs:63) clears the holds, changes the mode/root size, and queues commands **before** calling `commit_again`. Its checkpoint is taken afterward at [admission.rs:39](/tmp/rv-dt-t7a5/runner/src/runner/admission.rs:39).  
   **Scenario:** While facts are held, the app requests Dim/Dark/24px. Hand-back then encounters a settlement refusal—for example, a time-dependent resource returns an invalid shape. Rollback preserves the already-changed facts and queued commands. The web host drains those commands even with an error and no receipt, so CSS can change while the tree remains from the refused commit. The holds are also gone, preventing a meaningful retry. Hand-back’s mutations must participate in the transaction.

5. **SHOULD-FIX — Invalid font requests overwrite the last valid app choice.**  
   [runner/src/runner/held.rs:138](/tmp/rv-dt-t7a5/runner/src/runner/held.rs:138) records and removes held font commands before the normal validation at [commit.rs:610](/tmp/rv-dt-t7a5/runner/src/runner/commit.rs:610). [host/web-js/commands.js:9](/tmp/rv-dt-t7a5/host/web-js/commands.js:9) likewise defers validation until replay.  
   **Scenario:** Under an 18px pin, the app requests 24px, then passes a computed `0` or invalid percentage to `setRootFontSize`. Hand-back retries only the invalid request and falls back to the old size, losing 24px. Invalid requests normally leave the previous valid size intact. Validate before replacing the choice retained for hand-back.

6. **SHOULD-FIX — WASM reloads now lose remembered app choices too.**  
   A replacement runner initializes `held` empty at [runner/src/runner.rs:881](/tmp/rv-dt-t7a5/runner/src/runner.rs:881); its carried modes contain the effective tuple, not the suppressed app choice. [host/web/dev.js:453](/tmp/rv-dt-t7a5/host/web/dev.js:453) then replaces the page’s remembered `asked` with the new runner’s empty answer.  
   **Scenario:** Pin Black, press Dim, make a structural edit, then hand back. The replacement runner carries Black; `app.mode` disappears; hand-back leaves Black. Round 1 explicitly deferred the analogous **JS** limitation, but this introduces it into the resident WASM reload path as well.

7. **SHOULD-FIX — Development tracking executes in production and native paths.**  
   [runner/src/runner/held.rs:116](/tmp/rv-dt-t7a5/runner/src/runner/held.rs:116) records every valid mode request, and `take_held` scans commands and clones their first argument even without any hold. [mode.rs:108](/tmp/rv-dt-t7a5/runner/src/runner/mode.rs:108) also adds a second mode-validation pass. JS creates retained replay closures in [modes.js:322](/tmp/rv-dt-t7a5/host/web-js/modes.js:322), `rt.js`, and `commands.js` outside the stripped development regions.  
   **Scenario:** A release app changes appearance normally: it pays the tracking/allocation cost despite having no design tool. The new WASM hold export is unconditional too. Tracking needs development-only admission before launch, so launch choices can be recorded without imposing this work on ordinary builds. No timing claim was measured here.

8. **SHOULD-FIX — The tests still do not establish several claimed guarantees.**  
   [scripts/dev-copies.test.mjs:178](/tmp/rv-dt-t7a5/scripts/dev-copies.test.mjs:178) exercises WASM mode only; its attribute assertion accepts a missing attribute as success. [contract/cli/tests/it/tamber.rs:817](/tmp/rv-dt-t7a5/contract/cli/tests/it/tamber.rs:817) sends three separate actions and inspects runner state, without covering the ABI/page path.  
   **Concrete escape:** Finding 2 can report the expected root size while its rendered rows remain wrong, and finding 1 can keep looping while every asserted status is correct. Coverage remains missing for idle quiescence, pre-pin font choices and percentages, pin replacement, refused hand-back, and held commands mixed with unrelated commands in one commit.

Round 1’s original cases for delayed mode correction, choosing the pinned value again, launch-time reporting, `"system"`/`"medium"` tracking, and full hand-back of a mode-free pin are addressed by the source hooks. Port 80 and UTF-8 byte counting are fixed. The root-font fix remains incomplete under finding 2; the “nothing retries” disposition is contradicted by finding 1. The recorded JS reload limitation remains unfixed.

I found no new hostname-gate bypass: [install-page.mjs:298](/tmp/rv-dt-t7a5/scripts/install-page.mjs:298) admits only the numbered pattern at the configured port and never makes it `local`. Separate storage is documented. Ordinary non-copy commands retain D3 precedence, although the narrowed hand-back restoration also applies to non-copy pages.

The asked-values JSON is correctly escaped and assembled. The server clears stale `app` when a later status omits it. Ordinary app-commit refusals occur before choice recording, and removing held commands preserves unrelated commands in the same suffix; the transactional defect above is specifically in hand-back.
