# usage-reset-and-feedback: live drive record (2026-10-08 KST)

Host: this Mac (macOS, Xcode 27), screen locked during the session (the agent-mode app renders off the
lock screen; no real input was used). Each drive is one launch of the agent-mode app
(`bun scripts/agent.mjs macos --size 1280x840 --epoch <scenario time>`), paired through the welcome
wizard (or Settings > Connections) with a single-use pairing link from the lane server (never printed;
the link file is deleted after each drive).

| Item | Value |
| --- | --- |
| Lane T3 server | `t3` 0.0.46-nightly.20261005.2667 (staged release), pid 66868, 127.0.0.1:16410, isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME, telemetry off |
| Fixture proxy | Bun HTTP+WebSocket proxy, 127.0.0.1:16411 -> 16410 (uncommitted, `target/ufb/fixture.mjs`): overlays the Codex provider snapshot (ready, ChatGPT Pro, model GPT-5.5, `usageLimits` with a 5h and a weekly window, 2 banked credits, `externalUsage`), injects `usageLimitSources` (scenario `multi`: a hub account whose id holds an address, and a hub that failed with "token expired"), gives threads an `activeProviderThreadId`, and answers `provider.consumeResetCredit` / `provider.uploadFeedback` with scripted replies after a delay; logs every RPC (tag + payload, no tokens) |
| Project / thread | lane repo `demo` (git), Codex thread "Review the README" (its run fails: the lane's codex 0.151.0 is not signed in; the fixture only overlays the snapshot) |
| AFTER build | this branch (`d7fe611f9`), `host/apple/build.mjs t3-code-macos --bundle` |
| BEFORE build | `t3-code-evidence-base` at `da4f4512f` (accepted by the driver's freshness check; not rebuilt) |

## Drive A (AFTER, scenario `single`, redeem replies: warning, typed error, reset)

Ops: open the thread; type `/usage-limits` + Return; screenshot; tap `usage-reset-0`; screenshot
(confirm); Escape; three times: tap Use reset, tap Use credit, screenshot at +400 ms ("Using..."), wait
4 s, screenshot; keyboard pass; Manage usage; reduced motion; 840x620; dark; hover bar and pace glyph.

Read back (agent `state` / `tree`):
- After Use reset the focused node is `reset-credit-cancel` (inside the confirm); after Escape it is
  `usage-reset-0` again. Return on `usage-reset-0` opens the confirm with focus on Cancel.
- Shift+Tab from Use reset: `usage-manage-0` -> `usage-bar-0-secondary` (label "Weekly limit: 29% left,
  45% of the window left, resets in 3d 3h") -> `usage-bar-0-primary` ("5h limit: 62% left, 44% of the
  window left, resets in 2h 12m") -> "Dismiss usage limits".
- Status after each reply: "Redeemed, but the hub cooldown could not be cleared." / "This provider does
  not bank reset credits." / "Reset applied. Your windows have cleared."
- RPC log: exactly three `provider.consumeResetCredit {"instanceId":"codex"}`, one per Use credit; none for
  Cancel or Escape.
- Manage usage: the agent's open log records `https://chatgpt.com/#settings/Usage` (T3_REMOTE_OPEN_LOG).

## Drive BC (AFTER, scenario `multi`, upload replies: sent, failed)

- Before the reveal no tree string contains either fixture address; after tapping `usage-label-1-redacted`
  the hub label is revealed (that screenshot is not published).
- Send `/feedback broken diff` (send button): RPC `provider.uploadFeedback {"threadId":"cb6f49db-...","reason":"broken diff"}`.
  At +500 ms: composer empty, notices ["Sending feedback to OpenAI..."], sendStatus "Sending feedback",
  send button label "Sending feedback" and disabled, two local rows (`/feedback broken diff`, "Sending
  feedback to OpenAI..."). Typing into the composer during the upload landed at once (draft "typing while
  it uploads" read back 300 ms later).
- Reply after 2.5 s: banner "Feedback sent to OpenAI" / "Thread ID: 019a7c3e-lane-fixture-thread" with
  Copy ID; reply row "Feedback sent to OpenAI.\n\nThread ID: `019a7c3e-lane-fixture-thread`".
  Dismiss removed the banner and both rows.
- `/feedback second try` with a scripted ProviderUploadFeedbackError: banner "Could not send feedback to
  OpenAI" / "Failed to upload feedback for thread cb6f49db-6518-4bab-946d-67ba8533c336."
- New draft (no thread), `/feedback`: toast "Start a Codex thread first" / "Send a message before you
  submit feedback."; draft kept; no RPC.

## Timing probe (AFTER, 8 s scripted upload)

`clock +500 real` after Send returned in 514 ms with the upload in flight and "Sending feedback" shown;
the composer took typing 310 ms later; the reply arrived 9 s later. On the first build (the upload
awaited inside a queued mutation's answer) the same step took 8,412 ms and showed nothing until the
reply: the native data executor answers one request at a time. Hence the detached request.

## BEFORE drives (base)

- `/usage-limits`: text lines (`5h limit · 62% left · resets in 2h 12m`), no bars, credits or Manage
  usage; the failing hub reads "Could not read limits." (base read `error.message` of a string).
- `/feedback broken diff`: sent to the agent as an ordinary turn — RPC `orchestration.dispatchCommand`
  `message.dispatch` with text "/feedback broken diff"; no `provider.uploadFeedback`.
