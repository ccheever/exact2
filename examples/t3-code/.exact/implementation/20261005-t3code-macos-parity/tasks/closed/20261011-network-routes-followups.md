---
name: 20261011-network-routes-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-network-routes-followups
pr_url: https://github.com/ccheever/exact2/pull/463
verified_commit: 1f813789b
---

# A learned LAN route connects with its borrowed credential; the network access dialog stays closed

## Outcome

The user decided on 2026-10-11 that these two leftovers are built now, matching the reference T3 Code.

| Id | Clone (as recorded) | Reference (check it first) |
| --- | --- | --- |
| NR-1 | A learned plain-HTTP LAN route is listed ("found automatically") but "this client never sends a credential over plain HTTP to a non-loopback host (`T3Endpoint`), so the walk counts it silent" ([environment-routes](20261005-environment-routes.md), "Remaining differences and limits") | The desktop learns the server's reported LAN addresses (`allowInsecure`: the page is not https) and connects over one with the credential of the route it was learned over (`credentialConnectionId`) |
| NR-2 | After a failed Network access change, the "Enable network access?" dialog came back over the Appearance page later in the same session ([audit-wave-followups-2](20261010-audit-wave-followups-2.md), "Found during this task") | The dialog closes after any outcome and never comes back by itself |

## Steps

1. Reference first: `routes.ts` `mergeLearnedRoutes` / `credentialConnectionId`, `registry.ts` `learnRoutes` (`allowInsecureRoutes`),
   `resolver.ts` (the credential a route reads), `driver.ts` `connectOverRoutes`, the server's `DirectEndpoints.ts` (what it
   reports); `ConnectionsSettings.tsx` `handleDesktopServerExposureChange` and the AlertDialog's `open` /
   `onOpenChangeComplete`.
2. NR-1: find what the clone does today with a learned `http://<private IPv4>` route (the transport's walk, the credential
   it reads, the socket, App Transport Security in the bundle), fix what differs, an AppKit test over a LAN address.
3. NR-2: find what reopens the dialog (its open rule is `connectionRemove == network:<on|off>:<serial>`), fix it so no
   dialog of the section can come back, a Bun test.
4. Tests; one agent drive of each build; before / after / reference evidence.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| NR-1 | reference comparison; AppKit test (`macos/tests/transport`) over this Mac's private IPv4; agent drive: a lane server bound to all interfaces, paired through a loopback proxy, the proxy stopped | before / after / reference images or text record |
| NR-2 | reference comparison; Bun test; agent drive: a failed Network access change (lane `desktop-settings.json` read-only), then another Settings page | before / after / reference images |

## Reference

- NR-1: `packages/client-runtime/src/connection/routes.ts` (`mergeLearnedRoutes`, `learnedConnectionId`,
  `credentialConnectionId`), `registry.ts` `learnRoutes` (`allowInsecureRoutes`: true off an https page),
  `resolver.ts:151` (a bearer route reads `credentials.get(credentialConnectionId(connectionId))`), `driver.ts`
  (`connectOverRoutes`, `checkRoute`), `apps/server/src/environment/DirectEndpoints.ts` (numeric private and tailnet IPv4,
  plain HTTP).
- NR-2: `apps/web/src/components/settings/ConnectionsSettings.tsx` `handleDesktopServerExposureChange` (closes the dialog
  on success and on failure), the network access `AlertDialog` (`open={isDesktopServerExposureDialogOpen}`,
  `onOpenChangeComplete` clears the pending mode).

## What the reference does (read and driven, lane `network-routes-followups`, base port 16500)

- NR-1: the server reports the numeric private and tailnet IPv4s it is bound to, plain HTTP (`DirectEndpoints.ts`); the
  desktop page is not https, so `allowInsecureRoutes` is true and `mergeLearnedRoutes` saves each new address as a learned
  route whose id names the credential it borrows (`learned:<env>:<origin>@<owner>`), placed by kind (LAN ahead of tailnet).
  `learnRoutesFrom` then asks `requestBetterRouteCheck`: a learned route that ranks above the one in use is preflighted at
  once (`checkRoute`, then `resolver.prepare`, which reads `credentials.get(credentialConnectionId(id))`) and the
  supervisor moves to it. Driven: Route Lab (a lane T3 server bound to every interface on 16503) added at its tailnet
  address `http://100.81.153.22:16503`; three seconds later the row reads "via LAN · Connected · 2 routes", LAN "In use" and
  "· found automatically", Tailscale second.
- NR-2: `handleDesktopServerExposureChange` closes the dialog on success and on failure
  (`setIsDesktopServerExposureDialogOpen(false)`), and the closed AlertDialog clears the pending mode. Driven with the lane's
  `desktop-settings.json` in a read-only directory: Restart and enable fails with the IPC error in the row and the toast,
  the dialog closes, and Appearance shows no dialog (no `alertdialog` in the page three seconds later).

## Cause and fix

**NR-1.** The record's premise no longer held at the base: since 2026-10-08 (`fix-provider-auth-state`) `T3Endpoint`
accepts remote `http`, so the transport walks a learned plain-HTTP LAN route and reads the token it borrows (the new AppKit
test passes on the base modules). App Transport Security is not in the way either: a bundle with the clone's ATS keys
(`NSAllowsArbitraryLoadsInWebContent` only) loads `http://192.168.181.76` and `http://100.81.153.22` with `URLSession`
and refuses only a dotted host name (`localtest.me`, -1022); the server reports IP literals only. What differed is when
the credential reaches the learned route: the clone checked a better route only on its 60 s timer, on app activation or on
a network change, and a background environment (the fleet transport an added environment gets) that had connected over its
only route scheduled no timer at all, so it stayed "via Tailscale" with the LAN route listed above it. Before: the drive's
row read "via Tailscale · Connected", Tailscale "In use", 15 s after pairing. Fix: `setRoutes` on a connected transport
asks the better-route check at once (`T3Transport+Routes.swift`; the preflight reads the borrowed token only after the
descriptor matched, as before), and `learnRoutes` writes a background environment's routes through its own fleet transport
(`fleet: <key>`, `connection-routes-ops.ts`), so that transport reloads and checks them. A user's reorder takes the same
path, which also moves to a route dropped above the one in use at once (the reference reconnects over the new order). The
stale notes (T3Routes.swift, connection-routes.ts, T3Endpoint's "provisional: user decision pending") now say what holds.

**NR-2.** The section's dialogs open while the root's `connectionRemove` names them with their serial
(`network:on:<networkSerial>`, `pairing:create:<createSerial>`, …) and close when the serial moves; the root keeps the value.
The Connections resource answers for a closed page (any other Settings page) with `emptyNetworkView()`, whose serials are 0,
so `network:on:0` matched again and the dialog opened over Appearance. A successful Create pairing link came back the same
way. Fix: `closedNetworkView()` (`connections-network.ts`) carries the serials; the closed page and `networkProjection`'s
unmanaged view use it, so no closed dialog of the section matches again.

No new declared difference.

## Tests

- `network-routes-followups.test.ts` (new, 6): NetworkDialogs' own open rules, read from
  `connections-network-dialogs.contract` and run (each `when` of its view as JavaScript over `remove` and `net`), open each
  dialog on its name and serial and opened it again over the old closed page; after a failed Restart and enable the open
  Connections page opened the dialog before and not after, and the closed page and the bare projection keep the serial, so
  the rule no longer opens it; the same for Create pairing link; learnRoutes writes the focused environment's routes to the
  focused transport (LAN learned ahead of the tailnet route in use, borrowing its token) and a background environment's to
  its fleet transport; one background environment whose fleet transport stopped since the pass read it ("That environment is
  not connected.") does not keep the next from learning. Before (the closed page's `emptyNetworkView()` put back): the two
  NR-2 cases fail; with the contract's rule reworded to open on `network:on:0` too, the Appearance case fails; without the
  per-environment catch, the last case fails ("That environment is not connected.").
- `macos/tests/transport/routes.swift` (AppKit, +2): `testALearnedPlainHttpLanRouteConnectsWithTheTokenItBorrows` (the walk
  over this Mac's private IPv4 with the paired loopback route down: the descriptor first with no credential, then the borrowed
  token; passes on the base too) and `testALearnedRouteAboveTheOneInUseTakesOverAtOnce` (setRoutes with a learned LAN route
  ahead of the route in use: connected over it within 5 s, no timer). Before (the base `setRoutes`): the second fails, still
  on `localhost` with no request to the LAN address.

## Acceptance results

Drives (agent mode, 1280×840, one session each with clone-drive.sh's lane environment, script `nrdrive.mjs` not
committed): before on `t3-code-evidence-base` at `6db5cadea` (lane `network-routes-followups-before`), after on this
branch's bundle (lane `network-routes-followups`); the reference over CDP on lane `network-routes-followups` (backend
16500, CDP 16501). Route Lab: a lane T3 server from the pinned runtime, isolated home, bound to every interface on 16503
(it reports `http://192.168.181.76:16503/` LAN and `http://100.81.153.22:16503/` tailnet), paired at the tailnet address.
Each lane's `desktop-settings.json` sat in a read-only directory (NR-2's failure). Images are before | after | reference.

| Row | Result | Proof |
| --- | --- | --- |
| NR-1 | pass (agent mode + AppKit + Bun): before, 3, 6, 10 and 15 s after pairing the row reads "via Tailscale · Connected", Tailscale "In use", the learned LAN route listed above it unused; after, from 3 s "via LAN · Connected", LAN "In use" and "· found automatically", as the reference (3 s). AppKit: the walk over a learned plain-HTTP LAN route sends the descriptor request with no credential, then the borrowed token (both builds); a learned route above the one in use takes over within 5 s (after; before, still on the old route with no request to the LAN address) | [image](https://raw.githubusercontent.com/ccheever/exact2/915699e6a20caa3dbf84b8bdd2ebff4256e16f61/network-routes-followups/nr1-learned-lan-route.png), [record](https://raw.githubusercontent.com/ccheever/exact2/5fd0822dbbc03154f5b6ffae6fc2598525e690bf/network-routes-followups/nr-record.txt) |
| NR-2 | pass (agent mode + Bun): Restart and enable fails with the IPC error in the row and the toast and the dialog closes (all three); then Appearance: before, "Enable network access?" opens again; after, no dialog three seconds later, as the reference. Bun: the closed page keeps the serials (before: 0) | [failed](https://raw.githubusercontent.com/ccheever/exact2/572688dab5430153fd87ecf20f1fdd0358c179bc/network-routes-followups/nr2-failed.png), [Appearance](https://raw.githubusercontent.com/ccheever/exact2/6b3070c3ac8eea0b5ab81eabbee37a686f8b24e3/network-routes-followups/nr2-appearance.png), [record](https://raw.githubusercontent.com/ccheever/exact2/5fd0822dbbc03154f5b6ffae6fc2598525e690bf/network-routes-followups/nr-record.txt) |

## Review round 1 (2026-10-12)

An independent review found three should-fix problems; all three are fixed
([text before/after](https://raw.githubusercontent.com/ccheever/exact2/a25e8ced7abb00d56ded958e70d6af03f89880ce/network-routes-followups/review-r1.txt)).

| Finding | Fix |
| --- | --- |
| A failed fleet write dropped learning for the rest of the sync pass: a background environment's `setRoutes` (`fleet: <key>`) throws "That environment is not connected." when its transport stopped since the pass read it, and the only catch was around the whole `learnRoutes` | `learnRoutes` catches each environment's write on its own (a let-go answer still propagates); the next pass learns that environment again. Bun case added; it fails without the catch |
| The private-IPv4 selection was copied word for word in both new AppKit tests | One private helper, `privateIPv4()`, beside `status` / `until` (`macos/tests/transport/routes.swift`) |
| The NR-2 Bun test checked a hand-written predicate tied to the contract by a string `toContain` | The test reads NetworkDialogs' `when` rules from the contract and runs them over the views the pages answer (`openDialogs`); the string check is gone. A reworded rule (one that opens on `network:on:0`) fails the Appearance case |

No UI change: no new drive; the images above stand.

## Checks

Review round 1, on the final head (`origin/feat(example)/t3-code` merged at `e91bdc8af`):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4674 pass, 1 skip, 0 fail (314 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 28,865,666 bytes; `app.contract` 1,398 lines (unchanged) |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed |
| AppKit `macos/tests/transport` (README recipe) | 0 | 64 tests, 0 failures (both NR-1 tests pass with the shared helper) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

The bundle was not rebuilt and the app not driven again: this round changes no UI (an error path, tests).

Before review round 1 (`origin/feat(example)/t3-code` merged at `6db5cadea`, nothing new on it):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4658 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 28,820,298 bytes; `app.contract` 1,398 lines (unchanged) |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed |
| AppKit `macos/tests/transport` (README recipe) | 0 | 64 tests, 0 failures (RouteTests 9, two new) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,679 passed, 0 failed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once, before the live drive (the app code of `393326063`, unchanged since) |

## Not done / not verified

None.

## Seen while building, not a finding row

- With these changes, reordering routes so that another route sits above the one in use moves the session to that route at once,
  if its preflight passes. The reference reconnects over the new order every time (`replaceRoutesLocked` → `installEntryLocked`).
  Reordering that keeps the route in use first keeps the live session here.

## Real-input batch steps

None: both rows are agent mode (no native menu, drag or OS dialog).

## Progress

2026-10-12: record created; the reference read; NR-1 traced (the base already walks and authenticates a learned LAN route;
the takeover waited for the 60 s timer, and a background environment never checked) and NR-2 traced (the closed page's
serials); both fixed with Bun and AppKit tests; ATS probed with a bundle; the before build driven once, the reference driven,
the branch bundle built and driven once; images composed and uploaded; the final checks; draft PR #463.
2026-10-12: review round 1: a failed fleet write stays with its environment, NR-2's test runs the contract's own open rules,
one private-IPv4 helper in the AppKit tests; `origin/feat(example)/t3-code` merged at `e91bdc8af`; the final checks again.

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

This task has no real-input steps (both rows are agent mode). The g2 rows over its surfaces ran on the bundle: G2-19 passes (a route order saved on the drop and kept across a relaunch, [G2-19-dragged-pair.png](https://raw.githubusercontent.com/ccheever/exact2/1d184b4144eade685d467e2f3949b7942c6e6be1/realinput-1011c/b4-servers-routes/pairs/G2-19-dragged-pair.png)); G2-14 passes steps 1, 2, 4 and 5 (step 3, Reconnect, is reached in neither app while the stub answers 503); G2-18 fails on keys (the route grips do not move by keys, and the remove confirm's Escape gives the focus to the window, X53's class), not this task's rows. Both are in `STATUS.md` "Known differences".

## Delivery

Merged on 2026-10-12 as `e801aab21` (#463, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; what stays open is listed in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
