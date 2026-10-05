---
name: 20261005-usage-reset-and-feedback
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
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

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/macos/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `packages/shared/src/usageLimits.ts` (`:478-545, 625-735`), `W/usage/UsageLimits.tsx:40-337`, `W/chat/ComposerUsageLimits.tsx:1-135`, `packages/contracts/src/providerUsageLimits.ts:35-154`, `packages/client-runtime/src/state/threadFeedback.ts:1-130`, `W/ChatView.tsx:8654-8706, 11163-11178`, `W/chat/ComposerFeedback.tsx:12-49`, `packages/contracts/src/provider.ts:122-142`, `packages/contracts/src/rpc.ts:367`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (a command with a settled reply), design (all states), accessibility (dialog, bar names with figures, status text), motion (none besides the dialog; reduced motion), layout-and-interaction (confirm dialog), testing-and-debugging, platforms. Unknown in the library: app-local Swift modules. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed: the hub path is preferred for redemption even when the native snapshot is fresher (`usageLimits.ts:662-671` comment). The reference sends `/feedback` only for the Codex driver with no attachments, contexts, annotations or comments, not for multi-model drafts, and needs a started server thread (`ChatView.tsx:8661-8680`). A redeem and a feedback upload are real, irreversible requests. The banner is built from the thread's own environment; no cross-environment pooling is needed here. The clone's local-command precedent: `/usage-limits` is answered in `client.ts:866-867` and shown as a composer notice (`composer-controls-view.ts:180`); the send status chain is at `composer-controls-view.ts:114` (reference order: Preparing machine, Rewinding conversation, Sending feedback, Messages loading, Preparing worktree, project clone reason).
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (provider-setup fixture and `RedactedText`) | pending |
| recorded decision | Plan decision U2 / U23 (apparatus): the fixture `target/t3-ui-parity/provider-setup-fixture.mjs` from `20261005-provider-sign-in-and-install` extended with a config overlay (`usageLimits`, `resetCredits`, hub sources) and scripted `provider.consumeResetCredit` / `provider.uploadFeedback` replies | none | User approves at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X21](../issues/20261005-x21-two-way-websocket.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X19](../issues/20261005-x19-data-source-timers.md), [X11](../issues/20261005-x11-shadow-blur-parity.md), [X17](../issues/20261005-x17-popover-position-try.md), [X13](../issues/20261005-x13-hover-keys-during-pan.md).

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
| Gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p macos-t3-code-apple --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/usage-limits.ts`, `C/reset-credits.ts`, `C/thread-feedback.ts`, `C/usage-bars.contract`, `C/composer-controls-usage.ts`, `C/composer-controls-view.ts`, `C/composer-controls.contract`, `C/presentation.ts`, the area file for the send hook, their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, isolated lane backend; attended row: a disposable Codex account (name only). Never port 3773, `~/.t3` or the `t3code` scheme. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the prerequisites merge; `20261005-usage-pooled-view` follows this ticket.
