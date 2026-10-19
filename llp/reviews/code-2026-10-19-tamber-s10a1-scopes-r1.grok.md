# Review round 1, LLP 1121 stage 10 A1, Grok (xhigh), security pass

Reviewed: 610842c715 (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

No BLOCKING findings. The table is on the POST path and fails closed for names outside it. Two scope rules are weaker than the ruling they implement, and they become reachable when a non-write token exists.

## SHOULD-FIX

**1. `drop`, and stream presence, trust the body's `client` — `scripts/tamber-pairing.mjs:71`**

`needs` treats a drop as `propose` when `owner(proposal) === request.client`. That field is the caller's. A `propose` client who sets `client` to the owner's name (the id and the owner are on the read stream) gets `propose` and `forget` runs. The same field is the presence key: `subscribe` (`scripts/tamber-server.mjs:496`) and `events` (`:513`) call `attend` for any string `client`. A `read` client can keep another client's pins, inspect mode, outlines, holds, and scrub preview up, and `channel.clear` / `scrubs.absent` run when that subscription is the last presence (`:504`).

Today every token is `write`, so this adds no power. It is the check A2 will inherit. It matches the ruling only if A2 overwrites `request.client` before `needs` and before `attend`. `dispatch` does not do that.

**2. A `propose` client can drop everyone else's held proposals — `scripts/tamber-server.mjs:831`**

`hold` (used by `propose` with `hold`, `import`, and `revert`, all `propose`) keeps 64 proposals and then `forget`s the oldest, any owner, reason `too many held`. That is a drop of another client's proposal without `write`. Server-setting `client` does not close it. Refusing a new hold at the cap, or evicting only the caller's, matches the drop rule.

## NIT

**3. `needs` uses a plain-object lookup — `scripts/tamber-pairing.mjs:67`**

`METHOD_SCOPE[request.method]` sees inherited names. `__proto__`, `constructor`, and `toString` are not `undefined`, so `needs` does not return `null`. `allows` then gets a non-scope value and returns false (403, no handler). The HTTP gate is `METHODS.has` (`scripts/tamber-server.mjs:545`), which rejects those names with 400 first. `Object.hasOwn` would make the helper match its "null means unknown" contract. No crash, no open.

**4. Stream 403 is a different JSON shape — `scripts/tamber-server.mjs:484`**

POST refusals are `{error: {message, scope, needs}}` (`:552`). The stream sends `refusal(...).error` as the body. Status is still 403. The status table (`docs/tamber-protocol.md:137`) does not list 403; the Scopes section (`:170`) writes the fields as a top-level object. `hello`'s field list (`:205`) does not mention `scope` or `local` (the 1.13 bullet at `:88` does).

**5. The stream does not read the table — `scripts/tamber-server.mjs:484`**

`subscribe` and `events` are `read` in `METHOD_SCOPE`, and the gate is a hardcoded `allows(scope, 'read')`. The values match. Changing the table does not change the stream.

## What checks out

- **One gate for methods.** `POST /__tamber` rejects a method outside `METHODS` with 400, then `dispatch` calls `needs` / `allows` before `absent()` and every handler (`:564–569`). `commit`, `accept`, and `drop` are inside that function. Unknown scope, unknown `needs`, and a missing table entry deny. Bracket lookup does not lower a real method: the only downgrade is `drop`, and it only moves between `propose` and `write`.
- **Classification.** `transaction` / `undo` / `commit` / `accept` / pin / preview / outline / hold / scrub* are `write`. Non-held `propose` is `transaction(..., false)`, which returns before `commit` (`contract/cli/src/tamber_serve.rs:1222`). Held `propose`, `import`, and `revert` only `hold`. `history` and `designCommit` are git reads (`log` / `show` / `diff` / `cat-file` / `rev-parse`). `inspect` stays `read` in the table; `on` / off is refused below `write` in the handler (`scripts/tamber-server.mjs:644`) before `channel.inspect`. With `refresh: true`, `channel.inspect` sends only `{kind:'inspect', refresh:true}` and does not touch the inspector map (`scripts/tamber-builds.mjs:519`); the hosts return on `refresh` before `setInspect`.
- **`export`.** Classified `read`, and it does write: `keep()` creates `.exact/tamber-dtcg/`, writes the current design's DTCG baseline and a `*` gitignore, and deletes baselines past 16 (`contract/cli/src/tamber_export.rs:128`). The bytes are the server's serialization of the current design. The protocol already calls that read-only for the design. Caller-chosen paths and design-file edits are not in that path.
- **Refusals.** Unknown method is 400. A known method above scope is 403 with `scope` and `needs`. The body echoes the caller's method and their own scope. Token failure stays 404 and happens before the scope check.
- **Local behavior and 1.13.** `scopeOf` defaults to `() => 'write'`. No production caller passes another. For `write`, the removed handler checks and the table allow the same methods, including `inspect` `on`. `hello` adds `scope` and `local`; minors are specified as ignorable. `PROTOCOL` and the hello assertions are `tamber/1.13`. Remaining `tamber/1.12` strings are history. `dispatch(request, scope)` (`:550`) leaves `local` at its default `true` (`:564`). No method's scope is `local` yet, so that default grants nothing. A2 has to pass `local: false` for a paired token or `hello` keeps reporting `local: true` and a future `local` method is allowed.
- **Tests.** `scripts/tamber-pairing.test.mjs` shows the table matches `METHODS`, scope order, `shutdown` → `null`, honest `drop` (own `propose`, other `write`), and a live server 403 for every `METHODS` entry above `read` and above `propose`, with both fields in the message. `hello` reports scope. A `read` client gets 200 on `/__tamber/events`. `inspect` `on` is 403 `write`; `refresh` gets past the scope check to 409.
- **Tests missing.** No case sends `drop` with `client` set to the other owner. No case fills the hold cap. `subscribe` / `events` are skipped (`!METHODS.has`) and nothing asserts a stream 403. No `__proto__` / `constructor` / wrong-case method through the server. The 403 loop does not show the handler was skipped, only the status. `preview` is covered only as a table entry in that loop.
