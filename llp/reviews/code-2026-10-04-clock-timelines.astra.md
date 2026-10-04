# Code review: clock timelines, LLP 1055.002 (20431ac5c), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `20431ac5c` (the branch rebased on `6274fbab3`).
- **Method:** one brief (sha256 `9344e5fd477541c4ed5409996184f189a14c0282b24aaaac5f9b08de96df2103`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** NOT READY.
- **Disposition (r1 fixes):**
  - 1 (a boundary read a hair early): taken, engine and web; `a_join_on_a_boundary_starts_on_it`.
  - 2 (pausing the sole web member resets the clock): taken; grok raised it too. `clocks.test.mjs` pauses a lone member.
  - 3, 4 (agent-clock pause/resume, and a timer crossed by one JS-agent seek): not fixed; LLP 1055.002 §5, known gaps. The adapter's author-pause tracking is main's for every animation.
  - 5 (a same-commit swap on native keeps the phase): not fixed; §5, known gaps.
  - 6 (members retained): taken; every sync lets go of retired members.
  - 7 (`symbols` and Lean skip the rewrite): taken; it runs in `sources::load` and Lean's single-file path. `every_entry_point_sees_the_clock`.
  - 8 (a local binding captured): taken as shadowing; grok raised it too. `a_local_binding_of_the_name_shadows_the_timeline`, `a_binder_of_the_name_shadows_the_timeline_in_its_body_only`.
  - 9 (hyphenated names): taken; the kernel takes `-` after the first character.

---

**NOT READY.** I found timing errors, inconsistent lifecycle handling between hosts, and compiler entry points missed by the integration.

1. **High — Floating-point rounding can discard an entire finite animation.** [motion/src/engine/clock.rs:91](motion/src/engine/clock.rs:91)  
   With origin `0`, duration `800ms`, alternate direction, and join time `4.8s`, the remainder is approximately `1.6`, so the engine chooses start `3.2s`. The web calculates in milliseconds and chooses `4800ms`. A one-iteration joiner therefore ends at `4.0s` natively—already finished—while the web plays it until `5600ms`. I reproduced this arithmetic without building.  
   **Fix:** use consistent time arithmetic across executors and handle rounding at cycle boundaries. Add finite-joiner cases at exact boundaries and immediately around them.

2. **Medium — Pausing the sole web member resets a clock that should remain busy.** [host/web/navigation.js:389](host/web/navigation.js:389)  
   `start()` removes `a` before checking membership, and `sync()` calls it on both pause and resume. A lone one-second animation starting at `0`, paused at `400ms`, changes its origin to `400ms`; resuming it alone resets the origin again. A joiner arriving while it is paused also inherits the wrong phase. This contradicts D3/D6 and differs from the engine. The browser test keeps another member running, masking this case.  
   **Fix:** retain a live member when evaluating pause/resume; reset the origin only when the timeline was actually idle.

3. **Medium — Both web agent adapters omit the required resume handling.** [host/web/navigation.js:345](host/web/navigation.js:345), [host/web-js/agent.js:153](host/web-js/agent.js:153)  
   `clocks.start()` is called only for previously unseen animation objects. `held` is likewise populated once and never cleared. An animation first registered as author-paused remains excluded from agent seeking after the author resumes it; its shared start is never recalculated. Existing animations also receive no clock-specific pause/resume reconciliation.  
   **Fix:** track authored play-state changes separately from the adapter’s own `pause()` calls, update holds, and calculate a fresh shared start on resume.

4. **Medium — A timer crossed by one JS-agent seek can reuse an expired origin.** [host/web-js/agent.js:180](host/web-js/agent.js:180), [host/web/navigation.js:383](host/web/navigation.js:383)  
   Start a one-second finite member at `0`; a timer mounts another one-second member at `1500ms`. Jump directly from `0` to `1500ms`. The timer’s post-commit hook registers the joiner **before** seeking existing animations. The old animation still reports the adapter’s `paused` state, so it incorrectly keeps origin `0`, giving the joiner start `1000ms` instead of `1500ms`. The wasm path seeks at its `at` marker before applying the timer’s changes.  
   **Fix:** determine liveness at the supplied agent time, or advance registered animations before assigning new starts.

5. **Medium — Native phase selection depends on commit processing order.** [kernel/src/motion.rs:103](kernel/src/motion.rs:103), [motion/src/engine/clock.rs:67](motion/src/engine/clock.rs:67)  
   Suppose A is the only member, with origin `0`. At `300ms`, one commit clears A’s animation and creates B on that clock. Created nodes are processed before touched nodes, so B sees A’s old animation as live and starts at `0`. Processing A first produces start `300ms`; the browser also sees A already cancelled when synchronizing B.  
   **Fix:** reconcile cancellations and surviving membership for the complete commit before calculating new starts. Cover animation removal and ancestor `display:none`, not just destroyed nodes.

6. **Medium — Removed animations retain detached DOM trees indefinitely.** [host/web/navigation.js:381](host/web/navigation.js:381)  
   `members` strongly retains `CSSAnimation` objects and their effect targets. Cleanup occurs only when another animation calls `start()` on that same clock. Removing a screen containing many clocked rows, then never using its clock again, retains those detached targets for the page’s lifetime. The early return when no clocked elements remain prevents cleanup entirely.  
   **Fix:** prune retired members during commit reconciliation, including commits that remove the last clocked element, and remove empty clock entries.

7. **Medium — `contract symbols` and the Lean backend bypass timeline resolution.** [contract/cli/src/symbols.rs:230](contract/cli/src/symbols.rs:230), [contract/cli/src/lean.rs:23](contract/cli/src/lean.rs:23)  
   A program declaring `timeline Pending` and using `animation-timeline=Pending` builds through the changed compile entry points, but these consumers parse/load another AST and type-check it without the rewrite. They encounter `Pending` as an unknown value. Timeline declarations/import references are also absent from the symbols graph. This is an integration gap with main’s compiler tooling.  
   **Fix:** share resolved-AST preparation across type-checking entry points and preserve authored timeline references for navigation.

8. **Medium — Timeline rewriting silently captures local bindings.** [contract/syntax/src/clock.rs:50](contract/syntax/src/clock.rs:50)  
   The walker uses only the merged global name set, ignoring component props, state, and `each`/`match` binders. An existing local `Pending = "--drag"` used by `animation-timeline=Pending` silently becomes `"clock(Pending)"` when an imported library introduces that timeline name. The existing drag binding changes meaning without a diagnostic.  
   **Fix:** respect lexical bindings or explicitly reject ambiguous names instead of silently replacing them.

9. **Low — Accepted declaration names can be unusable by the kernel.** [kernel/src/timeline.rs:90](kernel/src/timeline.rs:90)  
   Contract identifiers allow hyphens, so `timeline Pending-Work` parses and rewrites to `"clock(Pending-Work)"`; the kernel rejects that generated value because its grammar permits only alphanumerics and underscores.  
   **Fix:** align the identifier grammars, or diagnose the restriction at the declaration.

The conflict resolutions preserve main’s `followOnSeek`, `seek(to, sync)`, and `perf` integration; I found no obvious mechanical deletion there. The compiler-path omission above remains.

This was a read-only review. `git diff --check` passed and the checkout remains unchanged. No builds, tests, browser sessions, or native apps were run; runtime behavior, backgrounding, and performance remain unverified.