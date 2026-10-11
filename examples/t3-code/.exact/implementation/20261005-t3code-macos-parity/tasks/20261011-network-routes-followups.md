---
name: 20261011-network-routes-followups
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-network-routes-followups
pr_url: null
verified_commit: null
---

# A learned LAN route connects with its borrowed credential; the network access dialog stays closed

## Outcome

The user decided on 2026-10-11 that these two leftovers are built now, matching the reference T3 Code.

| Id | Clone (as recorded) | Reference (check it first) |
| --- | --- | --- |
| NR-1 | A learned plain-HTTP LAN route is listed ("found automatically") but "this client never sends a credential over plain HTTP to a non-loopback host (`T3Endpoint`), so the walk counts it silent" ([environment-routes](closed/20261005-environment-routes.md), "Remaining differences and limits") | The desktop learns the server's reported LAN addresses (`allowInsecure`: the page is not https) and connects over one with the credential of the route it was learned over (`credentialConnectionId`) |
| NR-2 | After a failed Network access change, the "Enable network access?" dialog came back over the Appearance page later in the same session ([audit-wave-followups-2](closed/20261010-audit-wave-followups-2.md), "Found during this task") | The dialog closes after any outcome and never comes back by itself |

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
