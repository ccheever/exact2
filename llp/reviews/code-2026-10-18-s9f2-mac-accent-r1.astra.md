# Code review: LLP 1121 stage 9 F2, the Mac's accent cells in a key window, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 3: high), sandbox read-only, a throwaway worktree.
- **Scope:** the F2 commit.
- **Method:** one brief (sha256 `f47c9700b76b6e82312d372593c9e0215adb866477c23594856a5511f6dfa46a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the agent's exit(0) skipped the restore; an interrupt skipped the script's finally; open -b; activation unverified; docs; a test).
- **Disposition:** fixed in rounds 1, 1b and 1c's commits.

---

**Two blocking restoration gaps.** The unflagged launch and `front()` behavior remain unchanged.

1. **Blocking — [main.swift:570](/tmp/rv-f2-1/host/apple/Sources/ExactMac/main.swift:570): normal agent shutdown bypasses restoration.** `agent.mjs` closes stdin; `Agent.exitAfterStorage()` calls `exit(0)` directly, so `applicationWillTerminate` never runs. A direct flagged drive therefore misses restoration on ordinary completion and protocol `quit`. The measurement wrapper compensates only for its own runs. **Fix:** invoke shared, opt-in restoration from both the agent exit path and AppKit termination; test EOF and protocol quit.

2. **Blocking — [measure-controls.mjs:151](/tmp/rv-f2-1/scripts/measure-controls.mjs:151): interruption skips `finally`.** Ctrl-C or SIGTERM delivered to the measurement process terminates Bun without unwinding this block. A foreground-group Ctrl-C also kills the Mac app without its termination callback, leaving neither restoration mechanism running. I reproduced the skipped `finally` with Bun 1.4.2. **Fix:** supervise the capture asynchronously, handle catchable signals, stop/reap the child, and restore before exiting. [Bun documents the required signal handling.](https://bun.sh/guides/process/os-signals)

3. **Should-fix — [measure-controls.mjs:154](/tmp/rv-f2-1/scripts/measure-controls.mjs:154): `open -b` can change application state.** If the saved app exits during capture, cleanup launches it again. A running app can receive a reopen event and create a window; multiple instances sharing a bundle ID also lose their identity. Activation-ineligible helpers are not filtered, and restoration errors are discarded. **Fix:** retain process identity and activate that surviving, eligible instance without launching or reopening it; report restoration failure.

4. **Should-fix — [measure-controls.mjs:152](/tmp/rv-f2-1/scripts/measure-controls.mjs:152): activation is requested but never verified.** If macOS denies activation, or focus changes before capture, inactive grey controls can still produce successful `none` measurements. `clock settle` does not establish application activation. **Fix:** require the app to be active and the captured window key at capture time; fail the measurement when that condition is absent. [Apple explicitly says activation requests may be denied.](https://developer.apple.com/documentation/appkit/passing-control-from-one-app-to-another-with-cooperative-activation)

5. **Nit — [reference.md:714](/tmp/rv-f2-1/docs/reference.md:714) and [control-roles-measured.md:41](/tmp/rv-f2-1/docs/control-roles-measured.md:41): “never key” is inaccurate.** Existing `AgentMac.type` paths call `makeKey()` without activating the app; launch also retains its existing `makeKeyAndOrderFront` call. **Fix:** distinguish application activation from window key status, and describe the flag as the activation opt-in.

6. **Nit — [control_roles.rs:63](/tmp/rv-f2-1/contract/cli/tests/it/control_roles.rs:63): the best-effort test now exercises `exact`.** Changing the enabled Mac checkbox cell means this test no longer proves its named best-effort behavior. **Fix:** use the disabled Mac prominent button for that assertion, and add explicit checks for the new disabled checkbox/radio warnings.

The table/doc consistency tests pass: **2 tests, 411 assertions**. The recomputed digest matches `0xe085d9a22f4a0882`. The new warnings correctly name disabled macOS checkboxes/radios and explain AppKit’s grey rendering. Environment forwarding, document/window routing, the agent relaunch refusal, and the app’s SIGTERM-to-AppKit path check out. No files were changed and no builds or native UI runs were performed.
