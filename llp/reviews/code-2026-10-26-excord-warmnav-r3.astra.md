# Code review: Excord warm address-following, round 3, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `ac29430998`.
- **Method:** one brief (sha256 `9f51142b3552b6a6a33c485cc741b5c00e1c07a237fb9854ae538181a4be2b66`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **MAJOR — Wide Back can retain a cancelled destination’s address.** [routing.contract:98](apps/excord/routing.contract:98): start with stack `[home, A, B]`, B open. Back requests A; another Back before A opens enters this branch and only supersedes. The stack remains `[home, A]`, but B stays displayed indefinitely. Unlike `askRoute("/")`, this branch never restores the open channel’s route.

- **MAJOR — `/nope` can strand A under B’s address.** [routing.contract:78](apps/excord/routing.contract:78): with A open, request `/c/B`, then `/nope` before B resolves. Superseding cancels B but retains its navigation entry. The host therefore retains/restores `/c/B` while displaying A’s transcript and draft; reloading opens B. The [new test](apps/excord/fixture/routing.e2e.test.ts:234) covers only an already settled channel.

- **MINOR — The beneath-visit header remains empty after crossing servers.** [app.contract:1218](apps/excord/app.contract:1218) resolves its title through `channels.concat(dms)`, but [channels contains only the selected server](apps/excord/app.contract:209). Open #general, follow a DM or another server’s channel, then edge-swipe: #general is absent from that lookup, so its header is blank. Same-server navigation is fixed.

- **MINOR — Automatic sign-out still retains the cancelled request’s server.** [toSignIn](apps/excord/app.contract:530) now calls `supersede`, fixing manual sign-out. However, after a gateway authentication failure, [signedOutHome](apps/excord/app.contract:339) runs with `signedOutNow=true`. That makes [wanting false](apps/excord/routing.contract:36), so [the rollback guard](apps/excord/routing.contract:57) skips restoring `wantServerWas`.

- **MINOR — An unconsumed guild hold leaks across explicit fixture resets.** [server.ts:761](apps/excord/fixture/server.ts:761) never clears `holdGuildsMs`; only [login consumes it](apps/excord/fixture/server.ts:470). If a test stops after injection but before IDENTIFY, resetting the fixture leaves the next test’s guilds delayed. An in-memory execution of the actual fixture reproduced this. Already scheduled deliveries correctly stop after reset via [the version/socket guard](apps/excord/fixture/server.ts:486).

The remaining claims are supported by source:

| Claim | Evidence |
|---|---|
| Same-channel links cancel, restore the address and rejoin | [follow](apps/excord/app.contract:980), [stayOpen](apps/excord/routing.contract:64). The router probe confirmed the pending entry is removed. |
| Wide `/` avoids reopening; narrow `/` leaves | [askRoute](apps/excord/routing.contract:80), with [unchanged-`now` assertion](apps/excord/fixture/routing.e2e.test.ts:257). |
| Channel requests defer leaving until replacement | [request branches](apps/excord/routing.contract:84), [replacement bookkeeping](apps/excord/app.contract:449). |
| Encoded open-channel addresses only rejoin | [takeDecoded](apps/excord/routing.contract:127). |
| No duplicate navigation writes or feedback loop found | [wrappers](apps/excord/app.contract:540) delegate once; [history echoes are consumed](host/web/navigation.js:344). |
| Launch test uses the root switcher before delayed guild delivery | [test](apps/excord/app.test.contract:430), [3-second injection](apps/excord/fixture/inject.ts:27). The Back variant remains [commented web-only](apps/excord/app.test.contract:461). |
| Both races now inspect 1-ms ticks | [tick helper](apps/excord/fixture/routing.e2e.test.ts:93), [Forward race](apps/excord/fixture/routing.e2e.test.ts:282), [Back race](apps/excord/fixture/routing.e2e.test.ts:297). |

Reviewed `ac29430998` against `86e0d85457`; no files changed. Full gate counts and the sequence-check mutation result were not independently reproduced: this worktree lacks build artifacts and dependencies.

**REQUEST CHANGES**
