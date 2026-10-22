# Code review: stage 10c R3, the relay service, round 3 re-check (403489af9a, 971e3485c5) (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `88cb7c90e2`.
- **Method:** one brief (sha256 `d59318570b8ea88d08e6573973dbfdc69977ed0711074488ea13bc0cd639e3b2`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

One blocking hole in the grace takeover. The account-boundary note holds: neither point is usable without that account's token. `bun test` on the two relay files passed 53, and the loopback gate met 7/7.

### Blocking

**A client queued on a lapsed session blocks the restart, and that refusal ends the run.** `scripts/tamber-relay-server.mjs:400`

Takeover requires `idle`, `carried`, and `waiting` all empty. During grace the names stay in `byName` (`:271`), so any TCP client that sends a ClientHello for that name is queued (`:298`) for `waitMs` (2s). With `waiting.size > 0`, `drop` is skipped and `:402` answers `REFUSED names in use by another session`. `connectRelay` treats every `REFUSED` as final (`scripts/tamber-relay.mjs:276`); `scripts/tamber-remote.mjs:405` does not dial again.

No token is required. At the defaults, one address (30 client connections a minute, each waiter held 2s) can keep a waiter up for the whole grace. A browser retrying the public name does it for one attempt. `drop` (`:433`) already destroys waiters, so a grace session with no idle connector and nothing carried can be handed over while clients are queued. The empty-grace case the new test covers does succeed.

### Should-fix

**The session cap is applied before the lapsed session is dropped.** `scripts/tamber-relay-server.mjs:387`

The grace session still counts toward `sessionsPerAccount`. At the default of 4, restarting one of four runs is refused with `more than 4 sessions on this account` before the drop at `:398`, and that refusal ends the run the same way. Drop the lapsed holder, then count.

### Nits

**The ancestor-parent check has no fixture.** `scripts/tamber-relay-server.test.mjs:123` loads `x.relay.test`, a name under the domain. The new branch at `scripts/tamber-relay-server.mjs:119` (`domain.endsWith('.' + parent)`) is right: `test` over `relay.test` is refused, and a sibling such as `notrelay.test` is not.

### Account boundary

Sound against an attacker who does not have the token.

- One token hash per label. `holds` (`scripts/tamber-relay-server.mjs:135`) runs before `connector` (`:328`). A run is that account only by presenting the token.
- BYO reclaim writes `byName` only on that path, and only drops a session with the same label (`:400`). Another account cannot use this account's parent: load rejects overlap (`:120`) and a parent equal to the relay domain, under it, or over it (`:119`). After grace the names are free, and registering them still takes the token.
- The 48-bit id is SHA-256 of `label` and `run`, cut to 12 hex digits (`:369`). The label and the id are public, so grinding a preimage needs no token. Registering that `run` still goes through `holds`. A hit is one of 2^80 preimages, a different session key, and it does not receive the victim's clients. Another account's generated parent is `<label>.<domain>`, which sits under the relay domain and cannot be a BYO parent.

The other fixes hold:

- The parser index-copies only the first record (`:52–56`); `route` passes at most the declared record (`:253–254`). The proxy test passed.
- Hello and greeting timers clear on `close` (`:249`, `:309`), including an early destroy.
- A label must end in a letter or digit, length 1–63 (`:33`); the connector uses the same pattern (`scripts/tamber-relay.mjs:49`); the generated canvas name must match `NAME` (`:113`).
- Per-address limits use `keyOf` (`:165`): the whole IPv4, `::ffff:a.b.c.d` normalized to that IPv4, and an IPv6 /64. Pending, client, and greeting counts all use it.
- `txtInFlight` defaults to 2, increments before the await, and decrements in `finally` (`:355–364`).
- After `holds`, refusals go through `answer` (`:330`) and do not touch the address silence counter. A bad token still does (`:311`, `:328`).
- A names line is absent or exactly two tokens (`:341`). Absent still mints the generated pair.
- A failed clear says `taken down` (`:359–363`).
