# LLP 1146: Work that outlives its answer, and OAuth sign-in on it

**Type:** RFC
**Status:** Draft, built (§9)
**Systems:** the TypeScript seam (`js/src/prelude.js` `__exact_adopt`, `js/src/binary.js` `__exact_module_work` and `timers`, `js/src/door.rs` op 22, ibex2's `bindings/abort.js` (local patch 22), `js/src/turns.rs`, `js/src/lib.rs`, `js/src/module_work.rs`); the bake (`js/bake/src/typescript.mjs`: the async helper's `route`); Runner (`runner/src/request.rs` `has_effects`, `runner/src/runner/module_work.rs`, `commit.rs`, `admission.rs`, `settlement.rs`, `source.rs`'s `DataSource`, `auth.rs`); the source composers that forward `background` (`data/src/placed.rs`, `data/src/mixed.rs`, `data/host/src/lib.rs`, `logic/src/lib.rs`, `logic/src/configured.rs`, `host/render/src/source.rs`); the Linux host (`host/linux/src/presenter.rs`, `host/linux/src/agent/stdio.rs`); the web JS target (`host/web-js/ts-data.js`, `rt.js`, `auth.js`, `src/main.rs`); the Contract test grammar and driver (`contract/syntax`, `contract/cli/src/tests_json.rs`, `scripts/agent-test.mjs`, `scripts/agent.mjs`); `scripts/exact.mjs` (`add`, `docs`); `packages/oauth` (`@exact/oauth`, its fixture server and cases); `apps/oauth-fixture`; `docs/recipes/oauth.md`, `docs/contract-for-agents.md`, `docs/contract-grammar.md`, `docs/start-here.md`, `docs/card.md`
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-26
**Implementer:** Claude (Opus 5.5), 2026-10-26
**Amends:** LLP 1016 D5 (a forgotten ticket's outcome is dropped on arrival, and it is not in flight for `clock settle`): a forgotten request with effects runs to its end, is in flight until it lands, and its outcome reaches the let-go answer's continuation (D2)
**Related:** LLP 1069.006 (`openAuthSession`, its D4 popup rule and D7 substitute); LLP 1018 (the store); LLP 1027.000 (no clock in data code); LLP 1133.002 D14 (an async function resumes under the answer that called it); LLP 1133.016 D9 (a runner ticket that belongs to no target). External: the Fetch Standard; RFC 9110 §9.2.1; RFC 6749, 6750, 7009, 7636, 8252, 8414, 9207, 9700; OpenID Connect Core 1.0 and Discovery 1.0

## Summary

An app that signs in with OAuth must sign a person out only when the
provider ends the session, and must keep every refresh token the provider
rotates. An app built on Exact broke both on iOS. The cause was in Exact's
native seam (reproduced here, §9):

1. **A native answer that is let go loses its request's reply.** When
   Contract asks a resource again, the host cancels the old answer's HTTP work
   (each native executor's `forget`, `host/apple/src/executor_core.rs`) and the
   prelude rejects its fetch as `Aborted`. A refresh that already reached the
   server spends the rotating token, and the replacement never arrives.
2. **A native store read with no store answered `null`.** The let-go chain runs
   with no store, where `store.get` answered `null` and `has` `false`, which
   reads as "signed out".

| | Decision |
|---|---|
| D2 | A request with effects (a POST) runs to its end when its answer is let go, as in a browser; its reply reaches the answer's continuation, and what that turn writes to the store stands |
| D3 | Where the executor offers no store, every store operation throws, reads included |
| D4 | A sign-in is held under the agent on every host; authored tests complete it with `authorize` against the app's own server |
| D5 | `@exact/oauth`: authorization code with PKCE, refresh, authorized requests, sign-out |
| D6 | A fake provider with nine provider profiles, and the cases |
| D7 | The bar: GPT-6 Luna at medium effort writes passing apps from the recipe |

D2 and D3 are not about OAuth. D2 adds no API: work several answers share is
ordinary JavaScript, a shared promise, as on the web, where it already worked.

## 2. D2 — A request with effects runs to its end

A browser lets a fetch finish when the code that started it no longer wants
its answer, and the continuation keeps what it wrote. Native hosts now do the
same for the requests whose cancellation would lose something:

```ts
let renewing: Promise<Tokens> | null = null;
const renew = (store: Store): Promise<Tokens> => (renewing ??= (async () => {
  const reply = await fetch(tokenEndpoint, { method: 'POST', body, headers });
  const next = await decode(reply);
  store.set(KEY, JSON.stringify(next));      // stands, whichever answer is let go
  return next;
})().finally(() => { renewing = null; }));
```

The answer that started the renewal may be let go while its POST is out; the
answers that await `renewing` meanwhile take the renewed tokens, and so does
the store.

### 2.1 What runs to its end

- **A request with effects** (`Request::has_effects`): an `http:` or `https:`
  request whose method is not safe (RFC 9110 §9.2.1: not GET, HEAD, OPTIONS or
  TRACE), and no stream, socket, long poll or held request. When the runner
  lets one go (newer arguments, a refresh, a newer send), the host goes on
  running it, and it counts as in flight until it lands.
- **A safe request** is still cancelled: cancelling loses nothing, and its
  continuation rejects as `Aborted`.
- **Exact's own requests** (`exact-auth:`, `exact-native:`, storage,
  continuations, surfaces) keep today's let-go: a superseded sign-in still
  dismisses its sheet.
- The answer is never given. Its continuation takes the reply in a module turn
  (§2.2); a fetch, yield or timer the continuation makes is refused, as a
  let-go answer's timer never runs (LLP 1027.000 D1). An `AbortSignal.timeout`
  the request carries, as it is or through `AbortSignal.any`, still aborts at
  the deadline the host holds the request to, and a later answer's request
  carrying the same signal has that deadline too.
- **The app's own abort** of a kept request (its `signal`) cancels it on the
  host (door 23) but does not reject it at once, as a browser's fetch would:
  the runner answers its ticket aborted, and the rejection reaches the let-go
  answer's continuation in a module turn (§2.2), so what that continuation
  writes stands even when the commit of the turn that aborted it is refused.
  An answer that aborts it and then awaits it resumes in a later turn. Only a
  kept request differs from the browser; any other aborted fetch rejects at
  once. The wasm target's module realms and a worker-placed source keep
  nothing, and on the JS target the fetch is the browser's: its rejection's
  continuation runs after the aborting turn's commit, outside it, where a
  store write stands whatever that commit does (`host/web-js/rt.js` `set`),
  so neither needs it.

### 2.2 Module turns

- **How a request is kept.** Where the runner lets go of a request with
  effects, it offers it, at the end of the commit and after
  `DataSource::forgotten`, to its source (`DataSource::keep_sent`), which
  answers the token it takes the reply under. The executor sets the let-go
  call aside until then (a re-ask replaces a parked call inside the new
  answer's turn), adopts its fetch when the runner keeps it (`__exact_adopt`),
  and forgets the call, rejecting its other fetches. A kept request is a
  runner ticket with no target, as a tool's is: `holds` covers it, so no host
  cancels it, and no answer claims it; until it lands it holds its place in a
  host's ordered completions. A request the host refused at admission never
  ran, and is not kept. A refused commit puts what it let go back in flight
  and keeps nothing; a call awaiting a request with effects that an equal one
  replaced in it is parked again (the first, if several were), and the
  replacement dropped; an open stream so replaced is not. A poisoned runner
  keeps nothing new, and what it kept before still lands.
- **The turn.** `fulfill` sends a kept request's outcome to
  `DataSource::module_landed(store, token, outcome)`, which delivers it with
  the store installed and no commit's checkpoint open; the commit after the
  turn reaches every host as any reply's, so no host changed. Every composer
  that forwards `background` forwards `keep_sent` and `module_landed`.
- **Turns follow frames.** The async helper asks the prelude before each step
  (`route`): in a module turn, a live answer's step waits for that answer's
  own turn, which `release` gives it, so its reads, writes and fetches are its
  own; the let-go answer's steps run in the module turn. A plain `.then`
  callback carries no answer and runs where it is; one a module turn runs
  cannot fetch, and the refusal says to await the result instead.
- **Liveness.** A kept request in flight and the steps it left for live
  answers count as outstanding, so an answer awaiting the kept work is not
  refused as pending on nothing; each module turn advances `progress`, so
  `release` asks waiting answers again.

### 2.3 What stands in the store

- A module turn's writes are in the store before the next checkpoint, so no
  refused commit undoes them. The commit after a turn that wrote asks again
  every resource that reads the store: one not in flight now; one in flight
  once its reply lands (`ask_again`), and until then settlement keeps its
  answer in flight until another write (`asks_again_after_reply`). A reader
  whose request may never land (a stream, a long poll, a held request) is
  asked again now. Only module work's commit waits for a reader's reply; any
  other store write asks a reader in flight again at once.
- In a live answer, what the turn that receives a reply to a request with
  effects writes stands even when the commit that lands the answer is
  refused: it keeps what the server did, as a browser's continuation keeps it
  outside every commit.
- A resource reads the store when any turn of its answer does, the first or
  one after an `await`. A step another live answer's turn settles runs in
  that turn (LLP 1133.002 D14), so a read it makes counts for that turn's
  resource.
- A store reader that failed is asked again when module work's turn or a
  reply's turn writes the store, kept by a refused commit or not, natively as
  on the web, where such a write comes outside every commit. A write an action
  makes in its commit leaves the failure standing until the arguments change
  or `refresh`, on every host.
- On the web JS target, a let-go answer's fetch already runs to its end, and
  its continuation writes the store outside every commit. Such a write now
  schedules a commit of its own, which persists the store and asks every
  store reader again, a failed one included: one in flight once its reply
  lands, marked before that commit so its refusal keeps the mark, and one
  whose request may never land now.
- An upload is the exception to each of these (LLP 1016.000 D10): it is asked
  once for its arguments, so neither the re-ask after a module turn's write
  nor that of a failed reader after a reply's turn sends it again
  (`module_work.rs` `module_commit` and `ask_failed_readers` pass over a
  resource whose arguments an upload was asked at), and on the web JS target
  the late write's commit answers it as its upload ended (`uploads.js`).

### 2.4 Scope and limits

- **Stale writes.** A POST used as a read (a search, a GraphQL query, a login
  sent as each letter is typed) that a newer one replaced runs to its end too,
  and what its continuation writes stands: the reply that lands last wins, as
  on the web. A source that must not keep a superseded reply checks, after its
  `await`, that what it read before still holds.
- **A safe request's continuation** differs by host: on the web it runs on and
  may write the store; natively it rejects as `Aborted`, with no store (D3).
- **A fetch a let-go answer's continuation starts** differs by host: natively
  it is refused as `Aborted`, as no answer waits for its reply; on the web it
  runs.
- **An `AbortSignal.timeout` whose answer was let go** differs by host:
  natively the signal keeps its deadline, and a request carrying it aborts
  at it; on the web the timer is the answer's, cleared when the answer is
  let go, so neither the request that runs on nor another answer's carrying
  the same signal has a deadline.
- **A live answer's write after an `await`** asks a store reader in flight
  again at once natively, where it runs in its reply's commit; on the web it
  is a write outside a commit, and the reader is asked again once its reply
  lands.
- **What keeps nothing**, its let-go requests cancelled as before: a Rust
  data source; a worker placement and the wasm target's module realm; and a
  JavaScript source in an ordered set with a worker Rust source sharing its
  `secret.keep` names (LLP 1027.002 D3), whose replies take their turns in the
  set. The store writes of a reply's turn with effects (§2.3) stand for every
  source the turn runs directly, not for a reply such a set delivers later.
- **Teardown, reload and update** end kept requests with the engine.
- **Several tabs, windows or extensions** share the store and coordinate no
  work.

## 3. D3 — No store, no store operation

In the let-go chain (a let-go answer's continuation after its fetch was
cancelled) and background storage deliveries (`HostState::between_answers`)
every store operation throws. `get`, `has`, `locked`, `unlock`, `protect`,
`unprotect`, `set` and `forget` say so, each naming itself: "no store here, in
background work or the continuation of an answer that was let go. Read the
store before the answer's first await. A write that must stand goes after a
request with effects (a POST), whose continuation keeps the store." `keepKey`
and `key`, an answer's, reject as called outside one. A let-go answer's continuation
in a module turn (D2) has the store. At bake and during module evaluation
`get` still answers `null`. The web target, whose store is always offered,
does not change.

## 4. D4 — Sign-in under the agent, and `authorize`

- Under the agent an `exact-auth:` request is held on every host, Linux
  included, one the boot makes too; outside it Linux answers 501.
- A build refuses an `auth.session` line that is not an origin, naming the
  line to write, a bare one too, whichever source declares it: LLP 1069.006
  D2 matches the sign-in URL's origin, so a line with a path (an issuer)
  admits no sign-in. A refusal's reason is in the journal on every host.
- The headless Linux agent applies a reply when it lands, between operations,
  as the web and Apple hosts do: host I/O moves on its own while the clock
  stands still (LLP 1012 §2). A test that holds a request at its server and
  waits there for the app's next request needs this.
- An authored test's app server is its **auth fixture** (`EXACT_AUTH_FIXTURE`
  natively, `?authFixture=` on the web in every browser the driver carries,
  kept across `reload`, read only under the agent). A held sign-in whose authorization URL has that origin shows the
  URL whole in `pending`; every other URL keeps LLP 1069.006 D7's redaction.
- `authorize` (`"authorize" [ "cancel" ] NL`) finds the one held sign-in,
  requests its URL without following redirects (bounded at 60 s, for a
  hanging server), and answers the hold with the 302 or 303's `Location`,
  which the host checks as any callback. `authorize cancel` answers 499.

## 5. D5 — `@exact/oauth`

A TypeScript package with no native component: `index.ts`, `index.d.ts` (the
API, staged for the type checker), `index.contract` (`OAuthSession`,
`OAuthResult`). `exact add oauth` installs it. The recipe is
`docs/recipes/oauth.md`; per-provider settings are `docs/contract-for-agents.md`
"Sign-in providers".

- **Time.** The app passes none. "Now" is the token reply's `Date`; without it
  (a browser hides it cross-origin unless exposed) the expiry is unknown and
  `exp` unchecked. An expired access token is learned from a 401.
- **One at a time.** A session's code exchange, refreshes and sign-out run
  one at a time, across every provider made for its key: each caller waits in
  its own frame for the work in flight, then runs its own there, so its
  requests are its own answer's. Each is a POST, which runs to its end when
  that answer is let go once sent (D2), within a 60 s deadline.
- **Sign-in.** `session()`, while signed out, prepares the next sign-in
  (discovery; `state`; a PKCE S256 verifier; `nonce` with `openid`), so
  `signIn()` calls `openAuthSession` before its first `await`. A preparation
  another answer started is that answer's discovery, cancelled if it is let
  go: the next caller then prepares again itself. Discovery
  reads OIDC Discovery's document then RFC 8414's, requires its `issuer` to
  equal the configured one (or fill `{tenantid}` with `issuerTemplate`), and
  absolute HTTPS endpoints except on loopback. The callback is checked in order: one
  equal `state`; `iss` (RFC 9207 §2.4); at most one `error` and one `code`,
  never both; `access_denied` and `cancelCodes`; one `code`. The session is
  kept with its issuer and endpoints.
- **Token replies.** JSON, or form-encoded by content type (a repeated
  parameter refused); `error` fails under any status; success needs 2xx, an
  access token and `token_type` `bearer` without case (`bearerByDefault` for
  providers that omit it); `expires_in` an integer or digit string; `scope` by
  spaces (`commaScopes`); no `refresh_token` keeps the old one, also on a
  sign-in again as the same account for the same client (the ID token's same
  `iss` and `sub`; OIDC Core §8, RFC 6749 §6), with the claims of the sign-in
  it came from, which its refreshed ID tokens are checked against.
- **ID tokens.** Required with `openid`; `iss` (or `issuerAliases`, or the
  filled template), `aud` (no audience but the client and `trustedAudiences`,
  §3.1.3.7 step 3), an `azp` present, `exp` against the reply's `Date` with
  five minutes' skew, `nonce`; a refreshed one keeps `iss`, `sub`, `aud`,
  `azp` (OpenID Connect Core §12.2). A refresh reply's ID token, an unchanged
  one too, is checked before anything in the reply is kept, and one present
  but empty or not a string is unreadable: one that fails ends the session,
  none of its reply's tokens used. `max_age` is refused, as its `auth_time` is
  not checked.
- **Refresh**: re-read; POST; re-read, dropping the reply if the session
  changed; keep. It awaits no discovery. Only
  `invalid_grant` (Entra's throttling AADSTS50196 excepted), the OpenID Connect
  interaction codes, and `endedErrors` (a code, or a code with its status) end
  a session. No reply keeps it, retryable; `retryLostRefresh` retries once for
  a provider set to accept the previous token for a while.
- **Requests.** `fetch` and `json` send the Bearer token only to `apis`
  origins (compared as the URL Standard serializes origins, RFC 6454 §4),
  over HTTPS except loopback, without following redirects; refresh
  once on 401; 403 never refreshes; `json` throws for a status that is not 2xx.
- **Sign-out** waits for a refresh in flight, forgets the session, then
  revokes its newest token (RFC 7009). When a revoke gets no reply (offline,
  or natively a superseded sign-out, whose request is refused) or a 429 or
  5xx (§2.2.1), the token waits for the next sign-out, which revokes every
  waiting token, newest first, and stops at the first such revoke.

## 6. D6 — The fixture and the cases

`packages/oauth/test/fixture.ts` serves profiles `spec`, `google`, `entra`,
`entra-v1`, `auth0`, `okta`, `onelogin`, `github` and `salesforce`, each
citing its PROVIDERS.md items, on a port and the next one (Google's token
endpoint, GOOG-1). `POST /control` expires access tokens, fails, cuts off
(before or after spending) or holds the next token request until
`release-token`, revokes, declines the next sign-in, sets claims in the next
ID token (`id-claims`), and asserts what a profile received (`expect`), in
all or since a `mark`, so each step of a case counts its own renewals and
reads. `await` long-polls until requests are held and counts are reached, so
no case waits out a duration, and the protocol's clock stands still from each
reset (a grace period, an expiry, an ID token's times and a token reply's
`Date` read one instant), so no case depends on how long a loaded machine
takes between requests. A profile's token endpoint answers a
browser or not (GitHub's does not, GH-8), no profile but `spec` lets a page
read `Date`, and revocation reads its token: a current one ends the grant, a
spent one ends nothing, and the counts say which a sign-out sent. A token or
revocation request is refused as RFC 6749 §5.2 and RFC 7009 §2.1 say, a
token is redeemed or revoked only for the client it was issued to, and the
servers keep a held request open as long as a case holds it.

- **Library cases** (`packages/oauth/test/oauth.test.ts`, Bun): among them
  two callers, and two providers for one key, renewing once while the refresh
  is held at the server, so the second waits for it; a sign-out revoking every
  session earlier ones forgot and a token a 503 left; a refreshed ID token for
  another account, issuer, audience or `azp`, an unchanged one past its
  expiry, one empty or not a string, and one in an unreadable reply, each
  ending the session with none of its reply's tokens kept; a wrong `azp`, an
  untrusted audience, and `exp` past the skew; a sign-in again keeping its
  refresh token and that token's sign-in claims, after a refresh too, but not
  another client's or another tenant's; discovery's RFC 8414 fallback and a
  relative endpoint; a form reply that repeats a parameter; no `Date`; HTTP
  Basic, with an empty secret too; the token request's 60 s deadline; a 403 and a redirect followed by
  neither refresh nor fetch, and a 401 that persists after one refresh; an API
  origin matched as the URL Standard serializes it (a default or padded port,
  an IDNA host), and no other port; an API read the grants refuse reported as
  setup; an error made in a realm whose `Error.prototype` is frozen, as a
  native host hardens it; and the declarations type-checked as the same types
  as the implementation's, member for member.
- **Seam cases**: `js/tests/it/outlive.rs` (the Apple host's bridge,
  headless, pumped only on the executor's wakes: a renewal one answer starts
  and others await, with no lock, is held at the server while three re-asks
  of its reader let their answers go, is kept once, lands once, and the live
  answer takes the kept token; a store read made after a reply makes a store
  reader; a refused commit keeps what a reply with effects wrote),
  `js/tests/it/forget.rs` (superseded POSTs run to their end and answer
  nothing; of the calls equal ones replaced in a refused commit, the executor
  puts back the first, which takes its reply), `js/tests/it/no_store.rs`
  (every store operation throws in a let-go continuation, each naming
  itself), `runner/src/request.rs` (which requests have effects),
  `runner/tests/it/keep_sent.rs` (what the source keeps runs to its end,
  what it does not is cancelled, one the host refused is not offered, a
  refused commit keeps nothing it let go, a refused commit after a module
  turn keeps the turn's writes, kept requests the host refuses settle once
  each, an ordered one behind ordered work, module work's wait lapses at the
  next write, a held reader is asked again at once after module work, and a
  plain request a later pass drops is forgotten, at boot too, where the
  source is then told what it may keep), `data/src/mixed_tests/keep_sent.rs`
  (module work lands in a JavaScript child under its own grants),
  `runner/tests/it/app_module.rs` (a write that is not module work's asks a
  store reader in flight again at once), `runner/tests/it/failed_readers.rs`
  (a failed store reader asked again by a reply's write, one whose
  reconciling ask failed at once included, also one a refused commit keeps,
  and not by an action's), `host/web-js/store-readers.test.mjs` (the web's
  store readers, read before, between or after `await`s, or before throwing
  at once; a failed one asked again by a write outside a commit; one in
  flight asked again after its reply, not replaced, also when the commit that
  marks it is refused; and a held one asked again at once),
  `host/web-js/rust-readers.test.mjs` (a web Rust source's store read reaches
  the runtime whether its call answers or fails, once, beside a TypeScript
  source's), `host/web-js/late-parse-reader.test.mjs` (a parse's store read
  stands when its answer is refused), `js/tests/it/timers.rs` (a kept POST's
  `AbortSignal.timeout`, as it is, composed or beside an `exactTimeout` as
  long, aborts at the request's deadline, which a later answer carrying the
  same signal has too),
  `js/tests/it/superseded.rs` and `host/web-js/answer-timers.test.mjs` (a
  let-go answer's continuation arms no timer) and
  `host/linux/src/app/boot_hold_tests.rs` (a sign-in the boot starts is
  held under the agent).
- **App cases** (`cases.template.contract`, written per profile by
  `cases.mjs`): sign-in and relaunch; cancelled and declined sign-ins; two
  renewals; relaunch with an expired token; a read asked again three times
  while its renewal is held, asserting no reused refresh token on the server;
  sign-out while a renewal is held, the renewal running to its end, revoking
  the current token and no spent or unknown one; 503 and lost replies, then a
  read that succeeds; the provider ending the session; and, for Okta, a lost
  reply after the token was spent. Each renewal and read a case names is
  counted at the server in its own step (a `mark`, then an `expect` since it):
  a successful refresh and a successful API read.

## 7. D7 — The bar

GPT-6 Luna at medium effort, in a fresh `exact new` app, with a fixed task per
profile (spec, Google, Entra, Auth0, Okta, OneLogin, Salesforce), the docs, and
the visible cases. Three apps per profile. An app passes when it passes every
visible and held-out case on the web and the Linux host. The held-out cases
were written by GPT-6.1 Sol from the task, the fixture and PROVIDERS.md without
the package or the recipe, validated against `apps/oauth-fixture`, hashed, and
kept outside every directory Luna reads. A round's inputs (the recipe, the
package, the fixture, the cases and the task) stay as they are for the
round. Luna runs in Codex's workspace sandbox, where a
browser cannot start: it runs the Linux cases itself, and the web cases run
after it finishes. GitHub is left out: its token endpoint does not answer a
browser.

## 8. Not in this RFC

- **A lock API.** Work several answers share is a shared promise, as on the
  web; what must finish once started is a request with effects (D2).
- **Draining kept requests across teardown, reload and update**, and through
  iOS suspension.
- **DPoP and PAR** in `@exact/oauth`; Sign in with Apple; a backend for
  providers whose token endpoint refuses browsers.
- **A clock in data code.** LLP 1027.000 stands.

## 9. Verified by running

**The cause.** With no let-go request kept (`keep_sent` keeping none, as
before D2), case 5 fails on the Linux host with a reused refresh token
(`{"reuse":1,"refresh":2}`), which is the reported failure; with D2 it passes.
The web never cancelled a let-go fetch.

**The tests.** `js/tests/it` passes in three runs in a row, and
`packages/oauth` in three runs. `apps/oauth-fixture` passes the visible and
held-out cases of every profile on the web, the Linux host, macOS and the iOS
27.1 simulator, except GitHub's on the web, where the app stays signed out and
says a server must exchange the code, as against GitHub (GH-8). No case waits
out a duration; the fixture's `await` and the driver's `request` bound only a
hanging test, at 50 and 60 s.

**The bar (D7).** GPT-6 Luna wrote 21 apps from this RFC's recipe, package,
fixture and cases. Graded on the web and the Linux host, every app passed
every visible and held-out case: the bar is met. The grading preceded the last
changes to the package (HTTP Basic with an empty secret) and the fixture (its
request checks and client binding, which a request the package makes meets);
the apps were not kept to grade again.
