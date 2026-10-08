---
name: 20261005-usage-reset-and-feedback
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-usage-reset-and-feedback
pr_url: https://github.com/ccheever/exact2/pull/249
verified_commit: null
---

# Usage reset credits in the composer and Codex `/feedback`

## Outcome

**Part A (composer).** The `/usage-limits` banner shows, per account, the windows as bars (with the even-spending hairline, pace glyph and countdown), a "Manage usage" link when the provider reports external usage, and banked **reset credits** with "Use reset" behind a confirm. **Part B.** In a Codex thread, `/feedback [reason]` uploads the thread to OpenAI with a banner, a local exchange in the transcript and a "Copy ID" action.

## Scope and exclusions

Included (missing or partial; the clone has no `resetCredits`, `externalUsage` or `provider.uploadFeedback`, and the banner draws text lines, `composer-controls-usage.ts:60-66`):

1. **Data model.** Finish the port of `collectProviderUsageLimits` (`usageLimits.ts:625-735`: native versus hub credit routing, `resetCreditInput`, notices) with its helpers (`providersWithLimits`, `limitsNotice`, `remainingPercent`, `elapsedShare`, `paceOf`, `formatResetsIn`, `formatDuration`). The clone's `usageReport` (`composer-controls-usage.ts:52-69`) keeps only windows.
2. **Part A:** `LimitWindows` compact rows with `WindowBar`, `PaceIcon`, `barColor` (`UsageLimits.tsx:40-196`); `externalUsage` "Manage usage" button; `ResetCredits` row; `ResetCreditDialog` ("Use a reset credit?", "This redeems one credit on your account and clears the current rate-limit windows. It cannot be undone.", Cancel / Use credit); `provider.consumeResetCredit` with outcome text; account label through `RedactedText` when it contains "@" (`ComposerUsageLimits.tsx:31`).
3. **Part B:** port of `threadFeedback.ts`, the send hook, the banner, local rows and the "Sending feedback" send status.

Excluded: the Usage page's pooled limits view and the redeem row inside its popover (`20261005-usage-pooled-view`, which depends on this ticket and reuses the redeem machinery, `PaceIcon` and `barColor` created here); the ChatGPT plan row and dialogs (`20261005-managed-codex-chatgpt`); the Usage page cost, tokens and price tables (done).

## Context and guidance

Parent specification: [spec](../../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `packages/shared/src/usageLimits.ts` (`:478-545, 625-735`), `W/usage/UsageLimits.tsx:40-337`, `W/chat/ComposerUsageLimits.tsx:1-135`, `packages/contracts/src/providerUsageLimits.ts:35-154`, `packages/client-runtime/src/state/threadFeedback.ts:1-130`, `W/ChatView.tsx:8654-8706, 11163-11178`, `W/chat/ComposerFeedback.tsx:12-49`, `packages/contracts/src/provider.ts:122-142`, `packages/contracts/src/rpc.ts:367`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (a command with a settled reply), design (all states), accessibility (dialog, bar names with figures, status text), motion (none besides the dialog; reduced motion), layout-and-interaction (confirm dialog), testing-and-debugging, platforms. Unknown in the library: app-local Swift modules. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed: the hub path is preferred for redemption even when the native snapshot is fresher (`usageLimits.ts:662-671` comment). The reference sends `/feedback` only for the Codex driver with no attachments, contexts, annotations or comments, not for multi-model drafts, and needs a started server thread (`ChatView.tsx:8661-8680`). A redeem and a feedback upload are real, irreversible requests. The banner is built from the thread's own environment; no cross-environment pooling is needed here. The clone's local-command precedent: `/usage-limits` is answered in `client.ts:866-867` and shown as a composer notice (`composer-controls-view.ts:180`); the send status chain is at `composer-controls-view.ts:114` (reference order: Preparing machine, Rewinding conversation, Sending feedback, Messages loading, Preparing worktree, project clone reason).
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (provider-setup fixture and `RedactedText`) | pending |
| recorded decision | Plan decision U2 / U23 (apparatus): the fixture `target/t3-ui-parity/provider-setup-fixture.mjs` from `20261005-provider-sign-in-and-install` extended with a config overlay (`usageLimits`, `resetCredits`, hub sources) and scripted `provider.consumeResetCredit` / `provider.uploadFeedback` replies | none | User approves at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X21](../../issues/20261005-x21-two-way-websocket.md), [X9](../../issues/20261005-x09-root-component-across-files.md), [X19](../../issues/20261005-x19-data-source-timers.md), [X11](../../issues/20261005-x11-shadow-blur-parity.md), [X17](../../issues/20261005-x17-popover-position-try.md), [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X21 | RPC from a data module | Existing Swift transport | nonblocking | none |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | New modules; send hook in an area file |
| X19 | Timers in data sources | `now` is an argument; redeem has no timer | nonblocking | Pass `now` |
| X17, X13 | Tooltip on a bar, hover | Reference bar tooltip ("The line is where even spending would be.") | nonblocking (workaround: `title`-style tooltip main maps natively; declare the delay difference) | Measure at `prepare` |
| X11 | Dialog shadow | Confirm dialog | nonblocking (visible difference declared) | Cite in matrix |

## Implementation notes

- Ports with provenance headers and listed changes: `usage-limits.ts` (functions above; the clone's partial copies in `composer-controls-usage.ts` import from it), `reset-credits.ts` (`resetCreditsSummary`, `OUTCOME_TEXT`, the redeem state of `useResetCredit` as a pure reducer: confirming, busy, status), `thread-feedback.ts` (`parseCodexFeedbackCommand`, `codexFeedbackNotice`, `beginCodexFeedbackSubmission`, `codexFeedbackMessage`, `submitCodexFeedback` unchanged). Contract: `usage-bars.contract` (bars, pace glyph) and a redeem row component, both reused by `20261005-usage-pooled-view`.
- Redeem reply text: `warning` if present, else `OUTCOME_TEXT[outcome]` (reset, nothingToReset, noCredit, alreadyRedeemed); an error shows its message, else "Could not use the reset credit." Hide the row when `availableCount` is 0 and no status is showing. The confirm is a sibling of any popover.
- Feedback: one upload per thread key; clear the draft before the upload ends; local user row = the typed command, assistant row = "Sending feedback to OpenAI..." then "Feedback sent to OpenAI.\n\nThread ID: `<id>`" or "Could not send feedback to OpenAI.\n\n<error>"; interrupted shows no notice. Without a started Codex thread: warning toast "Start a Codex thread first" / "Send a message before you submit feedback." and nothing is cleared.
- Banner rows go through `composerNotices` (`composer-controls-view.ts:180`); it returns early without a thread (`:181`). Local transcript rows are not persisted and do not survive a relaunch; no local-only row source exists in the clone's transcript today (verify at `prepare`).
- States: loading (none; snapshots are live), empty ("No limits reported."), error (provider notice text; source errors as lines), disabled ("Using…", "Sending feedback"), hover and keyboard focus on bars (the reference bar is focusable, `tabIndex=0`), permission (the server answers a failure line). `aria-label`: bar summary "`<label>`: N% left, N% of the window left, resets in …"; "Dismiss usage limits"; "Dismiss feedback notice"; pace glyph labels ("Ahead of pace…", "On pace…", "Under pace…").

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Credit routing | Fixture config overlay: native Codex with credits, a hub account for the same email with fresher or staler credits, a hub-only account | bun tests | `resetCreditInput` equals the reference rule in every ported case | macOS host machine | test log |
| Banner | Lane backend + overlay; thread with a Codex provider | `bun scripts/agent.mjs macos --json tree "type composer-input /usage-limits" "key return" tree state "screenshot out.png"` | Per account: summary (when more than one), window rows (bar, hairline, pace glyph, countdown) or notice, "Manage usage", "N reset credits banked · next expires in …", "Use reset" | macOS, 1280×840 and 840×620 | transcript, screenshots |
| Redeem outcomes | Scripted replies: reset, nothingToReset, noCredit, alreadyRedeemed, warning, error | Use reset → Cancel; Use reset → Use credit | Cancel sends nothing; confirm sends the right input once; "Using…" while busy; status text per outcome; row hides at 0 credits with no status | macOS | trace, transcript |
| Feedback | Real fixture server with a Codex thread; scripted upload outcomes; a draft without a thread | Type `/feedback broken diff`, send; repeat during upload; fail; interrupt | Draft cleared at once; banner Sending → Sent with Copy ID (`copyText` of the id) / Failed; two local rows; second send blocked; send status "Sending feedback"; no thread → warning toast | macOS | transcript, trace |
| Gating | Thread with an attachment; non-Codex provider; multi-model draft | Type `/feedback` | Sent as an ordinary message (no upload), as the reference | macOS | trace |
| Redaction | Account label with an email | Open the banner | Label shows the placeholder; tap reveals | macOS | transcript |
| Trace and pixels | Oracle and clone on one lane backend | `target/t3-ui-parity/trace-diff.mjs`; pairs at both sizes, light and dark, for states the oracle reaches | Same calls (`provider.uploadFeedback` payload `{threadId, reason?}`); every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Real redeem and upload `(attended session)` | Disposable Codex account with a banked credit; a real Codex thread; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Redeem once; send `/feedback` once | Credit spent once; feedback id shown; the user confirms each irreversible action first | macOS, real input (hover) | session notes |
| Ported tests | — | `bun test` | `threadFeedback.test.ts` (4 describes, original names); `usageLimits.test.ts`: "pace" (`:58`), "limitsNotice" (`:83`), "providersWithLimits" (`:101`), "/usage-limits" (`:863`–`:1072`, 6 cases), "remainingPercent" (`:1112`), "isUsageLimitsCommand" (`:1121`); new tests for `resetCreditsSummary` and the redeem reducer (no reference test exists) | macOS host machine | test log |
| Keyboard focus, Escape, reduced motion | Banner with credits for two accounts | Tab to each window bar, "Manage usage", "Use reset"; Return; Escape in the confirm; Tab to "Dismiss usage limits"; `prefer prefers-reduced-motion reduce` | Each bar takes focus and carries its summary label (the reference bar has `tabIndex=0`); Return on "Use reset" opens the confirm with focus inside; Escape closes it, sends nothing and returns focus to "Use reset"; the dismiss button is reachable; reduced motion changes only the dialog transition | macOS | transcript |
| Gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/usage-limits.ts`, `C/reset-credits.ts`, `C/thread-feedback.ts`, `C/usage-bars.contract`, `C/composer-controls-usage.ts`, `C/composer-controls-view.ts`, `C/composer-controls.contract`, `C/presentation.ts`, the area file for the send hook, their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, isolated lane backend; attended row: a disposable Codex account (name only). Never port 3773, `~/.t3` or the `t3code` scheme. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented in [PR #249](https://github.com/ccheever/exact2/pull/249) (draft), all three scope items.
- **Data model** (`usage-limits.ts`): port of `providersWithLimits`, `limitsNotice`, `remainingPercent`,
  `elapsedShare`, `paceOf`, `formatDuration`, `formatResetsIn`, `hasProviderUsageLimits`,
  `withUsageLimitsCommands` and `collectProviderUsageLimits` (hub path preferred for redemption, fresher
  balance shown, native duplicates folded, source errors as notices; a source's `error` is the contract's
  string, which the old copy read as an object: the base showed "Could not read limits." for "token
  expired"). `reset-credits.ts`: `OUTCOME_TEXT`, `resetCreditsSummary` and `redeemStep`, a pure reducer
  for `useResetCredit` (confirming, busy, status).
- **Part A** (`composer-controls-usage.ts`, `usage-bars.contract`): per account the summary (when
  several), `LimitWindows` compact rows (label and % left, `WindowBar` with the even-spending hairline
  and its three-line tooltip, `PaceIcon`, countdown), the notice line, "Manage usage" (the clone's
  `openExternal`), `ResetCredits` ("N reset credits banked · next expires in …", Use reset / Using…,
  the outcome) and `ResetCreditDialog` over the window (`app-overlays.contract`). Labels with "@" are a
  blurred placeholder until clicked (`UsageRedacted`, RedactedSensitiveText's composer variant: sans
  12 px). Bars are focusable (ring, tooltip on focus), named by their figures; the confirm takes the
  focus and gives it back (`focus:` command messages, `app.contract` commandCompleted, two lines).
- **Part B** (`thread-feedback.ts`, `composer-feedback.ts`): threadFeedback.ts ported line for line;
  the send hook (Codex driver, no image, chip or context, not a several-model draft, a started thread;
  one upload per thread key; `/usage-limits` first), the banner (Sending / sent with Copy ID / failed;
  none when interrupted), two local transcript rows inserted by time (never forked), "Sending feedback"
  in the send-status chain, `canSend` and compaction blocked while it runs; without a started thread the
  warning toast and the draft kept.
- **Detached requests** (`composer-replies.ts`, `T3Transport.swift` `deliver`): the native data
  executor answers one request at a time, so an upload awaited inside an answer held every other answer
  (the timing probe: 8.4 s with nothing drawn). The two long writes now start and return; the transport
  files the reply in the inbox and the drain settles it (a connection change or an inbox overflow settles
  the waiter as interrupted / lost). A delivered request may wait 300 s (the reference has no deadline).
- Independent review (2026-10-08) found one blocking defect (a staged file or folded paste did not stop
  the upload) and several should-fixes (lost replies, draw-time settling, bar focus ring, Disconnected
  as failure, `/usage-limits` before the guard); all fixed in `e96af64ec` with tests.

Acceptance rows:
- **Credit routing:** pass — `usage-limits.test.ts` "/usage-limits" (6 cases, original names).
- **Banner:** pass (fixture) — 1280×840 and 840×620, light and dark, one and two accounts (PR #249 images 01–06).
- **Redeem outcomes:** pass — live: Cancel and Escape send nothing, Use credit sends `{instanceId:"codex"}`
  once each, "Using…" while busy, warning / typed error / reset words; tests: nothingToReset, noCredit,
  alreadyRedeemed, transport failure, hub credit input, row hidden at 0 credits.
- **Feedback:** pass (fixture) — draft cleared at once, Sending → sent with Copy ID / failed, two local rows,
  second send blocked ("Sending feedback", disabled), typing stays live, no thread → warning toast;
  interrupted and lost replies in tests.
- **Gating:** pass (tests + base drive) — non-Codex provider, image, staged file / folded paste, thread and
  terminal chips, several-model draft send a turn; the base drive shows the old behaviour (a turn).
- **Redaction:** pass — no tree string holds the address before the click; one tap reveals.
- **Trace and pixels:** not run — user decision 2026-10-06 (no oracle / trace tools). The RPC facts
  (`provider.uploadFeedback {threadId, reason?}`, `provider.consumeResetCredit` input) come from the fixture
  proxy's log; the UI is compared with the reference source.
- **Real redeem and upload:** blocked — T3 marks the lane's Codex 0.151.0 unsupported (the managed runtime
  is `20261005-managed-codex-chatgpt`); a real redeem needs a disposable account with a banked credit (it
  spends the user's credit: user decision); during this session the Mac's screen was locked, so no Chrome
  login was possible (`codex login` was started in the lane and stopped).
- **Ported tests:** pass — `thread-feedback.test.ts` (4 describes, original names); `usage-limits.test.ts`
  pace, limitsNotice, providersWithLimits, /usage-limits (6), remainingPercent, isUsageLimitsCommand;
  new `reset-credits.test.ts`, `composer-usage-limits.test.ts`, `composer-feedback.test.ts`.
- **Keyboard, Escape, reduced motion:** pass — Return on Use reset opens the confirm with focus on Cancel;
  Escape closes it and refocuses Use reset; Shift+Tab: Use reset → Manage usage → each bar (labelled) →
  "Dismiss usage limits". The clone's confirm dialogs have no open transition (as AppConfirm), so reduced
  motion changes nothing here.
- **Gates:** pass — see Attempts.

2026-10-08 (real-input batch, records PR): the approved real redeem was not possible: no banked credit on the account. Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `d7fe611f9` | `bun test examples/t3-code` 2559 pass / 1 skip / 0 fail (base 2512); strict `tsc` clean; contract build 2677 slots; `cargo test -p t3-code-macos --lib` 11 pass; AppKit transport 57 tests, 0 failures; caps within; five checks: build ok, test 3383 pass / 0 fail / 33 ignored (94 binaries), clippy ok, fmt ok, boot ok | drives A, BC, timing probe; PR #249 images 01–13, `drive-record.md` | review: staged chips not gated (blocking) |
| 1a (failed approach, same day) | uncommitted | the upload ran inside a queued `composerJob` mutation: the drive's `clock +500 real` took 8,412 ms and drew nothing until the reply | timing probe in `drive-record.md` | replaced by detached requests |
| 2 (2026-10-08) | `e96af64ec` | `bun test examples/t3-code` 2562 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 2685 slots; `cargo test -p t3-code-macos --lib` 11 pass; AppKit transport 57 tests, 0 failures; caps within; five checks: build ok, test 3383 pass / 0 fail / 33 ignored (94 binaries), clippy ok, fmt ok, boot ok | drive R (bar focus ring and tooltip, focus after Use credit); image 14 | real-account rows (above) |

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| Real reset-credit redeem (user approved one) | Not done: no banked credit on the account. Signed in to managed Codex with the user's ChatGPT account (#256 batch); the composer `/usage-limits` shows only the ChatGPT-tracked notice, Usage › Limits shows no limit window or credit row after a refresh | [mcc-21-usage-limits](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-reset-and-feedback/01-mcc-21-usage-limits.png), [mcc-17-usage-refresh](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-reset-and-feedback/02-mcc-17-usage-refresh.png) |



## Next action

Review and merge PR #249 into `feat(example)/t3-code`. Real redeem and real `/feedback` wait for the
managed Codex runtime (`20261005-managed-codex-chatgpt`), a signed-in Codex lane login (provider-lane
`READY-codex`) and, for the redeem, the user's decision on spending a real banked credit.
`20261005-usage-pooled-view` can now reuse `PaceIcon`, `barColor`, `reset-credits.ts` and the detached
request.
