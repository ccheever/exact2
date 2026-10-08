---
name: 20261005-usage-pooled-view
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-usage-pooled-view
pr_url: https://github.com/ccheever/exact2/pull/263
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

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented in [PR #263](https://github.com/ccheever/exact2/pull/263) (draft), all five scope items.
- **Data model** (`usage-limits-pools.ts`): port of `collectLimitAccounts`, `collectLimitPools`,
  `displayLimitWindows`, `collectLimitNotices`, `collectExternalUsageLinks`, `CURSOR_USAGE_WINDOWS`,
  `cursorUsageWindowDetails`, the `LimitAccount` / `LimitPool` / `LimitPoolWindow` / `LimitPoolMember`
  types and UsageLimitsPooled's `accountInitials` / `accountHue`; each account carries
  `redeem {environmentId, input}`. `CHATGPT_USAGE_URL` is `chatgpt-plan.ts`'s (#256).
- **View** (`usage-pooled.contract`, `usage-pooled-view.ts`): PoolSection, PoolWindowCard (pooled % left,
  PaceIcon, "↻ +N%"), PoolBar with one equal-width segment per account (35% fill, the spent share
  hatched as 1px stripes 5px apart in one SVG path, since the kernel paints no repeating gradient;
  wide: name, % and a plate with the countdown and ticket count; narrow: the position and a legend),
  the segment popover (avatar or chip, title, the email blurred until pressed, Plan, Signed in / Via,
  Left, Resets, Restores, "N banked · expires in …" with Use reset), the status line under the bar,
  CursorEnableLimits, the external usage sections (ChatGPT copy), the LimitNotices warning alert and the
  empty text. The popover opens on hover (300 ms, the fade's delay), pins on a press (Return/Enter
  too), closes on Escape (the page's one Escape shortcut: a popover first, else back) or a second
  press; one shows at a time; its side follows the reference's collision flip on an unscrolled page
  (`popoverSides`).
- **Environments and the two refreshes** (`usage-environments.ts`, `usage-replies.ts`, `usage-refresh.ts`):
  the filter lists the focused environment and every EnvironmentFleet entry (one that is not connected
  and synchronized is disabled with its phase); the limits and the cost view read each selected
  environment (cost: `mergeUsage` over every environment that answered, partial totals, the skeleton,
  per-environment status, failed environments in the coverage note, the model dialog over all of them).
  Every per-environment request is detached: queued, sent by the next answer on that environment's own
  transport (`composer-replies.ts` for the focused one; `usage-replies.ts` with the transport's
  `deliver` key for a background one, settled in EnvironmentFleet's drain), so no answer waits on
  another environment. `refreshUsage` (rates, invalidate, refetch; a disconnect aborts the refetch; no
  session settles) and `refreshUsageLimits` (one check per environment, five-minute window, joins,
  `afterPending`) are ports with `now` as an argument (X19); the limits clock renews only on opening
  Limits and after a check. Fleet entries subscribe with `usageLimitSources: true`, and a background
  change redraws the open page (`usageFleetSynced`).
- **Redeem**: the confirm (ResetCreditDialog, now with a command `prefix`) sits over the window outside
  the popover; "Use credit" sends `provider.consumeResetCredit` with `redeem.input` on
  `redeem.environmentId`; the outcome reads "`<name>` `<text>`" under the bar; Cancel/Escape send nothing
  and give the focus back to the segment.
- **Cursor Keychain offer**: `needsCursorKeychainAccess` / `cursorKeychainAccessEnvironments`; the
  CursorEnableLimits card after Codex and Claude and the CursorEnableRow in the cost view's provider
  list; Enable writes `cursorKeychainUsageEnabled` on that environment, then rescans usage and runs a
  fresh limits check after the one in flight.
- `app.contract` -3 lines (the environment-off state, its action and argument removed; 1,459 -> 1,456 after
  #264 moved the root view into `app-window.contract`, where the toggle prop and argument went too).
- Two live AppKit agent sessions (one retry) found three defects, all fixed: Escape went back from the
  page while a popover was open (now the page's Escape closes it first); a mouse click on an unfocused
  segment never pressed it because its own `focus` handler restyled it (X56 draft; the host's focus
  ring stands in, as for every custom pressable since #189); the second card's popover opened above and
  was clipped (now flips below). The last two fixes were not re-driven (session budget).
- Independent review (2026-10-08): one blocking and three should-fix findings, all fixed in `9472711ee`
  with tests: (1, blocking) the redeem state and the confirm's target were keyed by the segment's position,
  so a reorder between "Use reset" and "Use credit" could redeem another account; both are now keyed by
  account and window, and the target is captured when asked; (2) a failed cost summary stuck across a
  reconnect; summaries re-read when the environment's connection generation changes, or on reopening
  after 60 s, and a non-ok transport reply (Disconnected, Closed, stale, superseded) counts as
  interrupted; (3) an EnvironmentFleet inbox reset left background waiters (and the busy segment)
  hanging; `usageFleetReset` settles them as lost, and liveness reads the connected phase; (4) a queued
  request could go to the newly focused environment after the focus moved; `flush` and the Cursor
  enable check the target first. Nit: the background Cursor branch checks the reply's generation.

Acceptance rows:
- **Pooling rules:** pass — `usage-limits-pools.test.ts`: "pools" (11 cases), "pooled account columns"
  (4), "Cursor limit presentation" (2), "collectLimitNotices", "external usage settings" (2), original names.
- **Two environments:** pass (fixture, live) — one account for the shared email signed in on "Build box,
  Studio", the hub account "Via Studio · Team hub", the notice "Studio · Old hub: token expired";
  1280×840 and 840×620, light and dark (PR images 01–04).
- **Segment popover:** pass (live) with one fix not re-driven — rows as listed (07, 08); Tab goes
  segment to segment (hidden popovers inert); one shows at a time; Escape closes it and the segment
  keeps the focus (session 2). A mouse press on an unfocused segment lost its press (X56); fixed by the
  host ring, not re-driven; the legend row's press (same press, no focus handler) worked live.
- **Redeem:** pass (fixture, live) — Use reset → confirm; Cancel/Escape sent nothing; Use credit sent
  `{"instanceId":"codex"}` once to Studio (the background environment whose snapshot showed the
  credits); "Using…", then "Codex Reset applied. Your windows have cleared." under the bar (09–11); the
  hub target `{sourceId, accountId, creditId}` on its environment and a typed failure in tests.
- **Environment filter:** pass — live: Studio off/on recomputes the pools and the cost view (13); none
  selected → "Select an environment to see limits." in tests.
- **Usage refresh:** pass (live) — one `server.refreshUsageRates` per selected connected environment,
  each followed by that environment's `server.getUsageSummary`, also after Build box's failed rates
  call; Studio stopped mid-refresh (recorded pid) made no refetch and Build box finished (14).
- **Limits refresh window:** pass (live) — one `server.refreshProviders` per environment on opening;
  none on returns inside five minutes; the button sends one; after the agent clock passed five minutes a
  return sent one again and the countdown renewed ("in 2h 12m" → "in 2h 6m").
- **External links and Cursor:** pass — ChatGPT section with its copy (live, 01); Cursor card and row
  (live, 06, 12); Cursor labels, descriptions and the hidden combined percentage in tests.
- **Hub subscription:** pass (live) — the background environment subscribed
  `{"usageLimitSources":true}` (the base sent `{}`), and its hub account and notice show (01).
- **Trace and pixels:** not run — user decision 2026-10-06 (no oracle / trace tools); RPC facts from the
  fixture's log (`rpc-log.txt`), UI checked against the reference source.
- **Real hover and redeem (attended):** real hover deferred to the real-input batch — screen locked
  (user away); a real redeem is blocked: it spends a banked credit on the user's account (user decision).
- **Ported tests:** pass — `usage-refresh.test.ts`: "manual usage refresh" (5, as 6 with `it.each`),
  "limits refresh cooldown" (2), "needsCursorKeychainAccess" (2); `usage-pooled.test.ts` "UsagePage
  refresh" (5 as logic tests with a `now` argument); `usage-limits-pools.test.ts` (above).
- **Keyboard focus, confirm Escape, reduced motion:** pass in part — segment Tab order and Escape in the
  confirm (focus back to the segment, nothing sent) live; under reduced motion the popover is opaque at
  once after the hover delay (15). Not verified: the Cursor Enable buttons with Return (needs one more
  agent session; no blocker other than the session budget).
- **Cursor Keychain enable prompt:** fixture only (the Keychain read is scripted away); the real macOS
  Keychain prompt is OS UI — deferred to the real-input batch (screen locked) and Cursor is on the user's
  free plan (403 plan_required).
- **Gates:** see Attempts.

2026-10-08 (real-input batch, records PR): hover delay, X56 first card, flip, keys and Cursor Enable pass; three clone bugs (hover popover closes on entry, wrong pinned popover, no focus return after the confirm). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `58eac6668` + `b5c7847be` | `bun test examples/t3-code` 2,910 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,392 slots; live session 1 (agent, two lane environments behind fixture proxies) | PR images 01, 06, 08–15; `drive-record.md`, `rpc-log.txt` | Escape went back from the page while a popover showed |
| 1a (failed approach, same day) | uncommitted | data-source bake refused `Date.now()` in `usage-replies.ts` (request keys) | build log | keys now come from the transport's `ids` |
| 2 (2026-10-08) | `ba541958d` (feature branch `b7761f556` merged) | live session 2 (the one retry): Escape fixed; found X56 (a click on an unfocused segment lost its press) and a clipped second-card popover | images 02–05, 07, 16; `drive-record.md` | both fixed in attempt 3, not re-driven |
| 3 (2026-10-08) | `aceedd9e2` | `bun test examples/t3-code` 2,970 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,472 slots; `cargo test -p t3-code-macos --lib` 11 pass; macOS bundle builds; caps within; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored (95 binaries), clippy ok, fmt ok, boot ok | PR checks table | real-input batch rows; user decisions (real redeem); Cursor free plan |
| 4 (2026-10-08) | `9472711ee` (independent-review fixes), merges `99466bb98` (feature branch `e784c8fb1`, #261) and `0697700e7` (`19714be51`, #264) | review fixes: `bun test examples/t3-code` 2,974 pass / 1 skip / 0 fail; after both merges: 3,036 pass / 1 skip / 0 fail; strict `tsc` clean; contract build 3,844 slots; `app.contract` 1,456 lines (base 1,459); `cargo test -p t3-code-macos --lib` 11 pass; macOS bundle builds; caps within; five checks: build ok, test 3,383 pass / 0 fail / 33 ignored (95 binaries), clippy ok, fmt ok, boot ok | PR "Independent review" and Checks sections; 4 new `usage-pooled.test.ts` cases | review fixes unit-tested, not re-driven (session budget); light dismiss needs a decision or an issue; rows above |

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| 3. Real hover on a segment | Delay PASS (nothing at 120 ms, open at 650 ms); FAIL (clone bug): moving the pointer into the popover closes it, so the blurred email cannot be clicked | [upv-hover](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/01-upv-hover.png), [upv-hover2](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/02-upv-hover2.png), [upv-hover3](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/03-upv-hover3.png) |
| 4. Real click on an unfocused segment (X56) | PASS on the first card (pin, unpin, Escape, Escape back); FAIL (clone bug): on the second card a click pins the previously clicked segment's popover | [upv-click](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/04-upv-click.png), [upv-click2](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/05-upv-click2.png), [upv-pin](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/06-upv-pin.png) |
| Popover flip | PASS: Weekly row popovers open below inside the window | [upv-flip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/07-upv-flip.png), [upv-f56](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/08-upv-f56.png) |
| 5. Keys: Tab through segments, Return, Use reset → confirm → Escape | PASS except: FAIL (clone bug) focus is not returned to the segment after the confirm's Escape; no redeem RPC sent | [upv-k2-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/09-upv-k2-small.png), [upv-k3-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/10-upv-k3-small.png), [upv-k4z](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/11-upv-k4z.png) |
| Cursor Enable with Return | PASS: one `server.updateSettings {"patch":{"cursorKeychainUsageEnabled":true}}` | [upv-k5-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/12-upv-k5-small.png), [upv-k6-after-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/13-upv-k6-after-crop.png) |
| Real Cursor Keychain prompt; real redeem | Blocked (user decisions; free Cursor plan) | — |

Full record: [usage-pooled-view.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/usage-pooled-view/usage-pooled-view.txt).

## Next action

Review and merge PR #263 into `feat(example)/t3-code`. The coordinator's real-input batch runs the steps
below (real hover, a real click on an unfocused segment for X56, the popover side, the Cursor Enable
buttons with Return). A real redeem waits for the user's decision on spending a banked credit; the real
Cursor Keychain prompt waits for a Cursor account on a paid plan. Light dismiss (an outside press closing a
pinned popover) is declared in `EXACT2-GAPS.md` against the local draft X53 only, with no GitHub issue:
it needs the user's decision or an issue. The review fixes (`9472711ee`) are unit-tested, not re-driven.

## Real-input batch steps

Deferred rows (screen locked, user away, 2026-10-08). Run them in one session with the real-input lock
(`target/t3-ui-parity/lanes/.realinput-lock`, owner note "usage-pooled-view: real input").

1. **Lanes.** In this worktree (or the merged feature branch's), `bun target/upv/lane/lane.mjs start`
   recreates two isolated servers (16410 "Studio", 16420 "Build box") behind the fixture proxies
   (16411, 16421). If this worktree is gone, copy the scripts from the evidence branch
   (`t3-code-evidence:usage-pooled-view/tools/`: `lane.mjs`, `fixture.mjs`, `scenario.json` into
   `<worktree>/target/upv/lane/`, `drive.mjs` and `send.sh` into `target/upv/`); set `epoch` in
   `scenario.json` to the session's minute. `bun lane.mjs pair a`,
   `pair b` write single-use links to `<a|b>/pairing-url` (0600; never print them).
2. **App copy.** `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`;
   copy the bundle to `target/upv/T3 Code (Lane Usage).app` with its own bundle id; launch it by path
   with `CFFIXED_USER_HOME=<lane dir>/home` (no `T3_LOCAL_HOME`/`T3_LOCAL_PORT`: no local server).
   Pair Build box in the wizard (paste with `set-value`, Korean input source), then Studio
   (Add a computer again), Continue, Agents Continue, "Do not import projects".
3. **Real hover.** Open Usage (sidebar gauge) → Limits at 1280×840. With `orca computer`, move the
   pointer onto the first Codex segment and hold: the popover appears after about 300 ms with the rows of
   image 07; move into it and onto the blurred email: it stays; click the email: it reveals; move off
   everything: it closes. Read back with a window screenshot (`screencapture -l <id>`), pixels ÷ 2.
4. **Real click on an unfocused segment (X56 fix).** Click the "Work" segment once: its popover opens and
   stays (pinned); click it again: it closes; click it, press Escape: it closes and the segment keeps the
   focus ring (host ring); press Escape again: the page goes back.
5. **Real keys.** Tab from the refresh button into the bars: each segment shows the host focus ring in
   order; Return opens; Tab reaches "Use reset"; Return opens the confirm; Escape closes it with the focus
   back on the segment. On the Cursor card, Tab to "Enable" and press Return: the fixture logs one
   `server.updateSettings {"patch":{"cursorKeychainUsageEnabled":true}}` on Build box (`b/rpc.log`).
6. **Cursor Keychain prompt (real).** Needs a Cursor account on a paid plan in the lane's
   CLAUDE/Cursor home and the real `cursorKeychainUsageEnabled` write against an unproxied lane server:
   the macOS Keychain prompt for the lane copy may be allowed for that copy only. Blocked today: the
   user's Cursor account is on the free plan (403 plan_required).
7. **Clean-up.** Quit the copy, delete its Keychain items (`com.exact.t3code.macos.access-token`,
   accounts `<origin>\n<environment id>` for 16411 and 16421) and preferences, `bun lane.mjs stop`,
   release the lock.
