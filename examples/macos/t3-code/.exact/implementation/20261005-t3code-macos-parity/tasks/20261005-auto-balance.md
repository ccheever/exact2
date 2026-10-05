---
name: 20261005-auto-balance
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

# A new thread can run on the machine with the most free capacity

## Outcome

In a new-thread draft on a project that spans several machines, with Settings › Load balancing on,
the Run-on menu offers "Auto balance". It picks the machine with the most free capacity from
`server.getHostResources`, using the user's per-machine weights. One banner covers the server updates
of all those machines.

## Scope and exclusions

1. **Auto balance (G7).** Applies when the draft spans several machines, the project is not Scratch,
   the setting Load balancing is on, selection is not manual, there are no attachments (unless a
   machine is already chosen), and there is no branch or worktree. The draft keeps
   `environmentSelection` (`auto` or `manual`) and `loadBalancedEnvironmentId`.
   - Candidates: connected, weight above 0 (default 50), with an enabled, installed, ready,
     authenticated, available provider of the selected driver (and instance).
   - For each candidate call `server.getHostResources` (5 s timeout; result fresh for 5 s; fetched only
     while an unresolved Auto draft is open; again when the user picks Auto). Skip a sample older than
     15 s by client receipt time, CPU at or above 0.95, memory free at or below 5 %, no CPU reading, no
     CPU count. Score = weight × CPU count × (1 − CPU) × free memory share; the highest wins.
   - Run-on menu item and label: "Auto balance", "Checking machines…" while pending, "Auto balance
     unavailable" when every candidate failed. A resolved choice sets `loadBalancedEnvironmentId`; a send
     in flight blocks the retarget. Picking Auto with attachments in the draft shows the warning toast
     "Keep attachments on this machine" ("Remove attachments before choosing automatic routing, then
     attach them on the selected machine."). Picking a machine sets manual and clears the choice.
2. **Multi-machine update banner.** Shown instead of the single-machine banner while the draft is on
   Auto. One item for all machines with a pending notice: "Update available for N machines" / "Updating N
   machines" / "Could not update N machines" (N = running, else failed, else all). Its title opens a popover
   with a row per machine: name; progress or failure row; "Manual update required" with the copy action
   ("Update the desktop app on that machine to update this server." for a desktop-managed server whose app
   cannot update); "Ready to update to X"; "Reconnect this machine to update". Text "N needs/need a manual
   update". Actions: **Update all** (all targets), **Update K machine(s)** (some targets), **Retry** (after a
   failure); none while one is running. **Update all** asks one confirm for every desktop app ("Update the
   T3 Code desktop apps on A, B? They will close and relaunch on those machines."); each machine updates
   independently and a failure toast names the machine ("<label> update failed"). Dismiss "Dismiss update
   notice" hides the current notices.

Excluded: the single-machine banner, its update flow and state, the offline banner and the version card
(`20261005-server-update-banner`); install-aware commands (`20261005-remote-scopes-and-update-commands`);
the Settings › Load balancing switch and weights (done: `connections.ts:98-105,437-443`); GitHub routing.

## Context and guidance

Parent specification: [spec](../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/components/ChatView.tsx:2947-2961,4242-4332,10690-10705,11358-11363`,
`.../hooks/useLoadBalancedEnvironment.ts`, `packages/client-runtime/src/load-balancing.ts`,
`packages/client-runtime/src/state/server.ts:1068-1076` (`hostResources`),
`.../components/chat/useAutoBalanceUpdateBanner.tsx`, `.../ServerUpdateAction.tsx` (`ServerUpdatesAction`),
`.../BranchToolbarEnvironmentSelector.tsx:51-155`, `packages/contracts/src/resourceTelemetry.ts:10-17`,
`apps/web/src/composerDraftStore.ts:323-324,1587-1630`.
Port with their names: `chooseLoadBalancedEnvironment`; the banner's pure derivation (machine list, counts,
title) as a function with `now`; `ServerUpdatesAction`'s batch rules (eligible targets, one confirm, per-machine
failure). Reuse from `20261005-server-update-banner`: the update store, `updateEnvironment`, and the banner item.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (late replies; a failed request keeps
its previous value; one mutation per path), components (a popover's state in a child that survives list
changes), layout-and-interaction, accessibility (menu radio roles, popover focus, Escape), motion (banner exit;
reduced motion), testing-and-debugging. **Unknown in the library:** data-module timers and per-environment RPC
for non-focused environments; the clone's `EnvironmentFleet.native(native, key)` (`connections.ts:248`,
`T3Fleet.swift:28-58`) is the basis.
Clone evidence: Load balancing settings only; no `server.getHostResources` call; Run-on switch `runOnEnvironment`
(`r4-git-env.ts:72`) and options `environmentOptions` (`r4-git-env.ts:37`); draft context in
`composer-controls-branch.ts` has `envMode`, `branch`, `worktreePath` only. Risk to test: `runOnEnvironment`
moves the focused environment, so an automatic choice that resolves while the user types must not steal focus or
text; the reference only retargets the draft's project reference.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Merged | pending |
| merged task PR | 20261005-server-update-banner | pending | Update store, single-flight update and banner item merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| scheduling preference | 20261005-main-fix-adoption | pending | Merged first (popover, menu) | pending |
| decision | U2: stub T3 RPC server (also answers `server.getHostResources`) | none | Deterministic scores need set CPU and memory values; real lane servers report the Mac's real load. | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X19](../issues/20261005-x19-data-source-timers.md) | 5 s freshness and 15 s sample age | `EXACT2-GAPS.md` X19 | nonblocking (workaround: pure functions take `now`; a root `every` task ticks only while an Auto draft is unresolved) | none |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Request/response per candidate over each environment's socket | X21 | nonblocking (workaround: Swift transports; fleet routing) | none |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap (`20261005-hot-file-split` makes room) | Keep the choice in the draft context, not new root state |
| [X17](../issues/20261005-x17-popover-position-try.md), [X11](../issues/20261005-x11-shadow-blur-parity.md) | Popover placement and shadow | X17, X11 | nonblocking (declared visible difference) | Declare |

## Implementation notes

- Extend the draft context with `environmentSelection` and `loadBalancedEnvironmentId`; keep them across project
  changes as `composerDraftStore.ts:1615-1630` does.
- Resolve the machine in a pure function over `{environmentId, resources, receivedAt, weight}` and `now`; the data
  source fetches resources through each environment's transport and records receipt time on the client.
- The multi-machine item joins the ordered notice list; it never shows with the single-machine banner.
- Never run an update against real servers; use the stub (decision U2).

## Acceptance and reproduction

Every row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`
(dev and lane builds refuse the real `~/.t3` and port 3773; see `20261005-embedded-server-runtime`).

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Choice | Three machines (stub `getHostResources`: idle, busy 0.96, stale 20 s) | Open a draft, pick Auto balance | Labels "Checking machines…" then "Auto balance"; the idle machine wins; one `server.getHostResources` per candidate | macOS 1280×840 and 840×620, light and dark | trace vs `target/t3-ui-parity/electron-oracle.mjs`; ports of `projectGrouping.test.ts` "load balancing shared project machines" (all cases, original names, incl. "uses client receipt time when host clocks differ") |
| Real servers | Two lane backends | Pick Auto | Valid request/response shapes; a machine is chosen | macOS | trace |
| Edges | Attachments in the draft; manual pick; all hosts failing; weight 0 on one machine | Pick Auto; pick a machine; fail all | Warning toast wording; manual clears the choice; "Auto balance unavailable"; weight 0 never chosen | macOS | screenshots |
| Focus safety | Type in the composer while the choice resolves | Pick Auto, keep typing | Text and focus unchanged; no focus switch to another environment | macOS | `--json` drive; `(attended session)` for real typing |
| Multi-machine banner | Three stub machines (one manual, one disconnected, one desktop-managed) | Open an Auto draft | Title, popover rows, description, Update all / Update K / Retry; one confirm for desktop apps | macOS both sizes | pixel pair vs oracle with the same stub; trace |
| Update all | Same, stub emits progress and one failure | Press Update all | Each machine updates alone; failure toast names the machine; Retry offered | macOS | trace; agent drive with `clock` |
| Menu and popover keyboard | Same | Tab to Run-on, arrows through items, Enter; Tab into the popover, Escape | Visible focus; `aria-label`s on icon buttons; Escape closes the popover and returns focus to its title; reduced motion removes movement | macOS | `tree --ax`; film in both modes; `(attended session)` for real keys |
| Logic ports | — | `bun test` ports: `projectGrouping.test.ts` load-balancing cases; `ServerUpdateAction.test.tsx` "ServerUpdatesAction" cases ("updates both supported machines with their own continuation preference and skips the manual machine", "names a failed machine while letting the other machine complete", "starts each machine once when double-clicked and disables the action until both finish", "asks once for desktop machines and cancels the entire batch") | Pass | host machine | log |
| Clone checks | `git add -A` | `bun test examples/macos/t3-code`, strict `tsc`, contract build, `cargo test -p macos-t3-code-apple --lib`, affected AppKit binaries, `bun scripts/caps.mjs`, the five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States covered: loading ("Checking machines…"), empty (no candidates: item hidden), error ("Auto balance
unavailable", failed update rows), disabled (Update while running), hover (tooltip on the label), keyboard
focus, Escape, motion: banner exit 220 ms; reduced motion: opacity only.
Task-owned source paths: new `load-balancing.ts`, `auto-balance-banner.ts` (+ tests), `r4-git-env.ts`,
`composer-controls-branch.ts`, `composer-controls-view.ts`, `connections.ts` hunks, `modules/apple/T3Fleet.swift` hunks.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build, the stub server if approved.

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

After `20261005-server-update-banner` and the base tickets merge: `prepare`, then `implement`.
