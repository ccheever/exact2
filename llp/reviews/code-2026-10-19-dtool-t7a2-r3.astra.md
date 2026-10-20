# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `9f5ec71884`.
- **Method:** one brief (sha256 `c32903843a2684cd430dd4ebc1c680371a65ef148219b0805f16f102c5e73c15`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commits 2a82f19e03, 55cefa3b2d and 9f5ec71884, with the records of rounds 1 and 2. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 BLOCKER, 4 SHOULD-FIX).
- **Disposition:** post-round, in the commit named in the sign-off request:
  - The wasm host delivers a hold's commit (a takeover, a release) to the page. The test checks what paints after a mode is released on the wasm copy, not only its status (1, 6).
  - A hand-back the runner refuses is said (`unsupported` `pin.hold`) and asked again after each commit until it is taken (2).
  - The JS page's size check is `appRootFontSize`'s own grammar (exponents too) and its positive f32 range (3).
  - A copy's pin refused for its modes keeps the axes held before and sets no mode; the snapshot is not restored in a copy (4).
  - `app.mode` is cut to the status's bounds (5).
  - The JS copy's root size is checked as it paints, under the pin and handed back (6).
- **Not taken:** held commands mixed with others in one commit, and a refused hand-back, as page tests (6). The runner test covers what takes held commands out of a commit, and the rollback is the runner's.

**Two blockers remain.** Reviewed all three commits through `9f5ec71884` read-only. No files changed; no builds or test suites run.

1. **BLOCKER — The WASM host discards successful hold-change receipts.**  
   [host/web/src/host/mode.rs:71](/tmp/rv-dt-t7a6/host/web/src/host/mode.rs:71) maps `Ok(Some(receipt))` to an empty receipt list for every non-null hold. The runner now commits correctly, but its changed rows never reach the page.

   **Scenario:** The host size is 16px, the app has set 24px, and the tool sends a font-only 16px pin. The takeover commits 16px, its receipt is discarded, and the subsequent setter sees 16px already current. Existing `1rem` rows remain painted at 24px while status reports 16px. Likewise, replacing Black with a scheme-only pin releases the app’s Dim in the runner but can leave Black painted. Round 2’s takeover and partial-release fixes remain incomplete at this boundary.

2. **BLOCKER — The page declares hand-back complete even when the runner refuses it.**  
   [host/web/dev.js:569](/tmp/rv-dt-t7a6/host/web/dev.js:569) ignores the result of `holdFacts("-")`, after already removing the pin’s CSS and restoring host facts. It then clears `channel.held` and acknowledges the new revision.

   **Scenario:** Pin Black/Light/18px; the app requests Dim/Dark/24px; hand-back encounters a settlement or `TaskKey` refusal. The new runner rollback correctly retains the hold, but the page reports it released and stops reporting `app`. Subsequent app commands remain suppressed by an invisible hold. Preserve the page’s pin state and report the refusal until hand-back succeeds.

3. **SHOULD-FIX — The JS font hook still loses valid latest choices and retains invalid ones.**  
   [host/web-js/commands.js:49](/tmp/rv-dt-t7a6/host/web-js/commands.js:49) duplicates validation with a different grammar and numeric range from [appRootFontSize](/tmp/rv-dt-t7a6/host/web/navigation.js:981).

   **Scenario:** Under an 18px pin, request 24px, then `"125e0%"`. The existing command accepts that percentage, but the new hook bypasses recording it. Hand-back replays 24px instead of the latest percentage. Conversely, a computed `1e100` passes the hook’s finite-number check, replaces the remembered 24px, and is refused only during replay. Use the existing command’s percentage grammar and positive-f32 validation. Round 2’s invalid-font fix is only partial on JS.

4. **SHOULD-FIX — A refused replacement mode pin overwrites app choices.**  
   The WASM fallback at [host/web/dev.js:539](/tmp/rv-dt-t7a6/host/web/dev.js:539) releases the previous mode hold, but [line 564](/tmp/rv-dt-t7a6/host/web/dev.js:564) subsequently restores the entire initial snapshot.

   **Scenario:** Pin Black, let the app request Dim, then request invalid Grey. The fallback releases Dim, then overwrites it with the original Black. Final hand-back restores nothing because the runner no longer holds that axis. On [the JS page](/tmp/rv-dt-t7a6/host/web-js/dev.mjs:714), replay repairs previously held axes, but the same whole-snapshot fallback still overwrites an app change on an unrelated, unheld axis.

5. **SHOULD-FIX — A valid long app mode value freezes status reporting.**  
   [host/web/dev.js:449](/tmp/rv-dt-t7a6/host/web/dev.js:449) and [host/web-js/dev.mjs:708](/tmp/rv-dt-t7a6/host/web-js/dev.mjs:708) copy mode values into `app` without bounding them, while [scripts/tamber-builds.mjs:134](/tmp/rv-dt-t7a6/scripts/tamber-builds.mjs:134) rejects values longer than 64 characters.

   **Scenario:** Declare an axis with a short default and a valid 65-character alternative; pin the default, then have the app request the alternative. The effective mode remains reportable, but `app.mode` makes the entire status return 400. The pages ignore that response, so the tile never reports the disagreement and later location/status changes remain stale.

6. **SHOULD-FIX — The tests still pass while the rendered copy is wrong.**  
   [scripts/dev-copies.test.mjs:188](/tmp/rv-dt-t7a6/scripts/dev-copies.test.mjs:188) checks reported Dim after partial release, which passes despite finding 1. The earlier attribute assertion accepts a missing attribute as success; the WASM fixture exercises no font command. [contract/cli/tests/it/tamber_held.rs:73](/tmp/rv-dt-t7a6/contract/cli/tests/it/tamber_held.rs:73) proves the runner returns a receipt, but never exercises its host delivery.

   Add assertions for rendered mode/font values through the ABI/page path, refused hand-back, exponent percentages, and held commands mixed with unrelated commands. The `/hosts` phase establishes six registrations and statuses; it does not establish six simultaneous scrub updates.

The gate looks sound: automatic copy admission is restricted to names 1–99 at the configured port, port 80 handles the omitted suffix, unrelated rebinding names fail, and copies never become `local`. Separate storage is documented.

Round 1’s source interception fixes address launch-time choices, repeated commands, choosing the pinned value again, and mode-free pins. Round 2 fixes the callback loop and the runner’s own reconciliation/rollback; findings 1–4 identify remaining integration gaps. Asked-values JSON is correctly escaped, command removal preserves unrelated commands in the same commit, and an accepted status without `app` clears the stored value.

Non-copy commands still apply immediately and retain D3. Their hand-back behavior is not literally unchanged: both pages now restore only the last pin’s axes, rather than the entire initial tuple.

The recorded reload limitations remain. Production tracking also remains as explicitly accepted in round 2: extra mode validation, retained choices/replay closures, and command scanning occur with nothing held. This review establishes no timing claim for that cost.
