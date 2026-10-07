---
name: 20261005-usage-pooled-view
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Usage page: pooled subscription limits across environments

## Outcome

The Usage page's **Limits** view is the reference's pooled view. For each provider it shows every window as a card: the pooled "N% left", a pace glyph, the next refill ("↻ +N%"), and a bar with one segment per account. Wide, each segment carries its label; narrow, a legend lists the accounts. Hovering or focusing a segment opens that account's popover: Plan, Signed in / Via, Left, Resets, Restores, the email on request, and, when credits are banked, "N banked · expires in …" with **Use reset** (confirm, status text outside the popover). Accounts from every selected environment and from usage-limit hubs are pooled, one entry per distinct account. External-usage sections (including ChatGPT), source notices and the empty state complete the view. The environment filter lists every connected environment.

## Scope and exclusions

Included (partial today: one per-driver list from the focused environment, `pages-usage.ts:294-316, 380`, `pages-usage.contract:540-575`; the filter offers "All environments" and the focused one, `pages-usage.ts:362-363`):

1. **Data model.** Port `collectLimitAccounts`, `collectLimitPools`, `displayLimitWindows`, `collectLimitNotices`, `collectExternalUsageLinks`, `CURSOR_USAGE_WINDOWS`, `cursorUsageWindowDetails` and the types `LimitAccount`, `LimitPool`, `LimitPoolWindow`, `LimitPoolMember` (`usageLimits.ts:33-480`). Each account carries `redeem {environmentId, input}`: the environment whose snapshot supplied the credits on show, or the hub.
2. **View.** `PoolSection`, `PoolWindowCard`, `PoolBar`, `PoolSegment` with popover, `RedeemableSegmentPopup`, `AccountChip` / `AccountAvatar` / `AccountName`, `LimitNotices` alert, external-link sections, empty text "No provider on the selected environments reports subscription limits." (`UsageLimitsPooled.tsx:36-648`). The Cursor pool shows the labels and descriptions from `CURSOR_USAGE_WINDOWS` and hides the combined percentage when both allowances exist.
3. **Environment coverage and the usage summary (research gap CN4: the Usage page covers one environment).** This ticket is the single owner of the multi-environment usage summary and both refreshes in `pages-usage.ts`; `20261005-composer-fidelity` does not change them. The environment filter lists the focused environment and every connected `EnvironmentFleet` entry; the limits view and the cost view both honor it. Today `usageView` builds one environment (`pages-usage.ts:362-363`), fetches one summary, and refreshes the focused client only (`server.refreshProviders {}`, `:372-379`). The cost view already merges several summaries (`mergeUsage`, `pages-usage.ts:128`) but is fed one: feed it each selected environment's `server.getUsageSummary` through `EnvironmentFleet.native(native, key)`. Fleet entries need hub sources: `settings-b-fleet.ts:141` subscribes with `payload: {}`, the focused client sends `usageLimitSources: true` (`client.ts:360`).
   - **Usage refresh** (port of `refreshUsage`, `packages/client-runtime/src/state/usage.ts:85-125`): for each selected connected environment, in parallel, send `server.refreshUsageRates`, then invalidate and refetch that environment's `server.getUsageSummary`. The refetch is invalidated even when the rates call fails. An environment that disconnects during the call aborts its refetch; an environment with no usable RPC session settles without one; a slow environment never holds the others.
   - **Limits refresh** (port of `refreshUsageLimits`, `usage.ts:48-83`): per selected connected environment, `server.refreshProviders {}`, deduplicated for 5 minutes: an automatic refresh inside the window does nothing, a manual call joins the check in flight, and `afterPending` runs a fresh check once the current one ends (used after Cursor access is enabled). It runs once when Limits opens or the connected set changes, and on the refresh button; the limits clock `now` is renewed after it. The reference reads `Date.now()` for the window; the port takes `now` as an argument (issue X19) and records that change in its header. Today the refresh is keyed by a counter only (`limitRefreshes`, `pages-usage.ts:372-379`).
4. **Redeem** uses the machinery of `20261005-usage-reset-and-feedback` (`provider.consumeResetCredit`, the confirm dialog, the redeem state, `PaceIcon`, `barColor`) and sends the request on `redeem.environmentId` through that environment's transport.
5. **Cursor keychain enable prompt** (`CursorEnableLimits`, `UsagePage.tsx:149, 212, 483, 970`): the Usage page prompt that turns on Cursor usage limits; the settings switch already exists (`providers.ts:270-272`); the condition is `cursorKeychainAccessEnvironments` / `needsCursorKeychainAccess` (`usage.ts:30-58`). Keep it here: no other ticket covers it.

Excluded: the composer banner, `collectProviderUsageLimits`, `LimitWindows` and `/feedback` (`20261005-usage-reset-and-feedback`); the cost, token and price views themselves (done).

## Context and guidance

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `packages/shared/src/usageLimits.ts:33-480`, `W/usage/UsageLimitsPooled.tsx:1-648`, `W/usage/UsageLimits.tsx:322-337` (`UsageLimitsSection`), `W/usage/UsagePage.tsx:149-165, 225-320, 470-500`, `packages/client-runtime/src/state/usage.ts:30-125`, `packages/contracts/src/providerUsageLimits.ts:35-175`.
Edge to `20261005-usage-reset-and-feedback`: the pooled view imports `ResetCreditDialog`, `resetCreditsSummary`, `useResetCredit`, `PaceIcon` and `barColor` from `UsageLimits.tsx` (`UsageLimitsPooled.tsx:30-36`), the same file the composer banner uses. That ticket creates those and the redeem command; nothing in the banner needs the pooled code. So this ticket depends on `20261005-usage-reset-and-feedback`, which depends on `20261005-provider-sign-in-and-install` (it owns `RedactedText`, used at `UsageLimitsPooled.tsx:173`).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (late replies after a changed filter; mutation then refresh), layout-and-interaction (grid of segments; a confirm outside a popover), design (all states), accessibility (segment names with figures), motion (popover only; reduced motion), testing-and-debugging, platforms. Unknown in the library: hover popovers from native pointers, app-local Swift. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed: `EnvironmentFleet` already keeps live `config` (with `providers`), `shell` and `scopes` per switched-on environment (`settings-b-fleet.ts:12-18, 138-146`); `EnvironmentFleet.native(native, key)` addresses its transport (`connections.ts:248`, `T3Fleet.swift:28-58`). The page advances `now` only on an explicit refresh (reference comment in `UsageLimits.tsx`); the clone passes `now` as an argument (`pages-usage.ts:296`). The pooled bar divides equal widths per account on purpose.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-usage-reset-and-feedback](20261005-usage-reset-and-feedback.md) | pending | Merged (redeem machinery, bar pieces, `usage-limits.ts`, config overlay in the fixture) | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X17](../issues/20261005-x17-popover-position-try.md), [X13](../issues/20261005-x13-hover-keys-during-pan.md), [X11](../issues/20261005-x11-shadow-blur-parity.md), [X10](../issues/20261005-x10-text-rendering-parity.md), [X19](../issues/20261005-x19-data-source-timers.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X21](../issues/20261005-x21-two-way-websocket.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X17, X13 | Popover side and hover-open | Reference uses `side="top"` and open-on-hover on a focusable segment | nonblocking (workaround: anchor on the side main allows; open on press and focus; declare the hover difference) | Measure at `prepare` |
| X11, X10 | Popover shadow; text truncation | Segment labels, popover rows | nonblocking | Cite in the matrix |
| X19 | Timers in data sources | `now` is an argument; no timers | nonblocking | none |
| X9 | Root cap | `app.contract` 1,327 of 1,500 lines | nonblocking until the cap | New Contract file for the view; extend `usagePage`; no new resource |
| X21 | RPC from a data module | Existing transports (focused and fleet) | nonblocking | none |

No new exact2 gap found. App-side work: hub sources and usage summaries for fleet environments (scope item 3).

## Implementation notes

- Ports with provenance headers and listed changes: `usage-limits-pools.ts` (functions in scope item 1; `Equal`-style comparisons use the clone's `canonical` helper, `settings-core.ts:183`), plus a Contract file `usage-pooled.contract` replacing `LimitSection`. Reuse `AccountChip` hue/initials logic as pure functions, `DriverMark`/`ProviderMark`, and the redeem reducer from `20261005-usage-reset-and-feedback`.
- A popover per segment with a stable `id`; the confirm dialog is a sibling of the popover and the status line sits in the card grid (`UsageLimitsPooled.tsx:419-433`).
- Environment switch: the per-environment data (`config`, `usage summary`) comes from the focused client or `EnvironmentFleet.entries.get(key)`; an environment that is not `connected` is listed disabled with its status and contributes nothing.
- States: loading (skeleton), empty (text above), error (per-source notices in a warning alert; a failed environment named), disabled ("Using…"), hover/focus on segments, permission (a failed redeem shows the server text). `aria-label` of a segment: "`<name>`: N% left, `<resets in>`, K reset credits banked"; "Account `<initials>`" on the chip. Motion: popover open/close; none under reduced motion.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Pooling rules | bun; fixtures from the reference tests | Run the ported tests | Same accounts, columns, credits and redeem targets as the reference in every case | macOS host machine | test log |
| Two environments | Two lane backends (ports 16000–16999) with overlays: same email natively on both, a hub-only account, a failing source | `bun scripts/agent.mjs macos --json tree state "tap usage-metric-limits" "clock settle" tree "screenshot out.png"` | One account for the shared email, environments named "Signed in", hub account "Via"; notices alert names the failed source | macOS, 1280×840 and 840×620 | transcript, screenshots |
| Segment popover | Same | Focus then press a segment; Tab through segments | Popover rows as listed; one open at a time; Escape closes and returns focus | macOS | transcript |
| Redeem | Same plus scripted `provider.consumeResetCredit` replies | Use reset → Cancel; Use reset → Use credit | Request goes to the right environment or hub (`redeem` input); confirm outside the popover; status "`<name>` `<text>`"; credits row appears only with credits | macOS | trace, transcript |
| Environment filter | Same | Toggle the second environment off and on | Pools and the cost view recompute from each selected environment's summary; "Select an environment to see limits." when none | macOS | transcript |
| Usage refresh | Two lane backends, both connected; scripted `server.refreshUsageRates` replies (ok, failure) | `bun scripts/agent.mjs macos --json tree "tap usage-refresh" "clock settle" tree logs`, then stop one backend (recorded PID only) mid-refresh | Trace shows exactly one `server.refreshUsageRates` per selected connected environment, each followed by that environment's `server.getUsageSummary`; after a failed rates call the summary is still refetched; the stopped environment makes no refetch and the others finish | macOS | trace, transcript |
| Limits refresh window | Same, Limits open, `now` advanced by the agent clock | Open Limits; return to it and remount inside 5 minutes; press the refresh button; advance past 5 minutes and return | One `server.refreshProviders` per connected selected environment on the first open; none on the rapid returns; the button joins a check in flight and otherwise sends a new one; a return after 5 minutes sends one again; the countdown text renews after each | macOS | trace, transcript |
| External links and Cursor | Overlay with `externalUsage`, ChatGPT sharing, Cursor pools | Open the view | One link section per destination; ChatGPT copy "View usage in ChatGPT with your connected account."; Cursor labels and descriptions | macOS | screenshots |
| Hub subscription | Fleet entry for a backend with hub sources | Inspect trace | Fleet `subscribeServerConfig` carries `usageLimitSources: true`; `usageLimitSourcesUpdated` updates the view | macOS | trace |
| Trace and pixels | Oracle and clone on the same lane backends | `target/t3-ui-parity/trace-diff.mjs`; pairs at both sizes, light and dark, for states the oracle reaches | Same reads; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Real hover and redeem `(attended session)` | Real pointer; a disposable account with a banked credit; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Hover each segment; redeem once | Popover timing as the reference; credit spent once | macOS, real input | session notes |
| Ported tests | — | `bun test` | `usage.test.ts`: "manual usage refresh" (5: settles on disconnect, waits for healthy environments without a recovering one, settles without a usable RPC session, replaces a scan that started before pricing was refreshed), "limits refresh cooldown" (2), "needsCursorKeychainAccess" (2, for scope item 5); `UsagePage.refresh.test.tsx` (5, as logic tests of the page view model with a `now` argument: the refresh-button countdown for both buttons, "uses the current time when returning to limits from tokens", "refreshes once on opening Limits and suppresses rapid returns and remounts", "waits for connection and refreshes new environments during a slow refresh", "keeps manual refresh busy until the already-running automatic check settles"); `usageLimits.test.ts`: "pools" (`:150`–`:539`, 11 cases, original names), "pooled account columns" (`:670`), "Cursor limit presentation" (`:761`), "collectLimitNotices" (`:808`), "external usage settings" (`:1130`) | macOS host machine | test log |
| Keyboard focus, confirm Escape, reduced motion | Same, one account with a banked credit | Tab to a segment; Return; Tab to "Use reset"; Return; Escape; then the Cursor prompt buttons; `prefer prefers-reduced-motion reduce` and reopen a popover | Focus order follows the segment order; Escape in the confirm sends nothing and returns focus to the segment; the Cursor enable buttons take focus and activate with Return; popover open and close are instant under reduced motion | macOS | transcript |
| Gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/usage-limits-pools.ts`, `C/usage-pooled.contract`, `C/pages-usage.ts`, `C/pages-usage.contract`, `C/settings-b-fleet.ts` (config subscription payload), `C/usage-limits.ts` (read only), their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, two isolated lane backends; attended row: a disposable account (name only). Never port 3773, `~/.t3` or the `t3code` scheme. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision). Tasks that need a sign-in (GitHub, provider accounts, T3 Connect) do not start until the user lifts the hold.

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

Starts when the user lifts the sign-in hold: `prepare` from `feat(example)/t3-code`, covering sign-in rows with lane fixtures (fake provider, seeded data).
