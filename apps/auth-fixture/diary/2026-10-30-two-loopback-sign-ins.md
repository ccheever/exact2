surfaces: macos
agent: Claude Code, Claude Opus 5.5
task: two loopback sign-ins at once, and a stray callback that does not end one

## Rough

- The second loopback sign-in was refused 409 `already open`, because the
  runner kept one auth session per window. A loopback sign-in shows no
  sheet. Fixed in exact2 (`runner/src/auth.rs` `arm`).
- A request to the receiver with the right `state` and no `code` ended the
  sign-in. Fixed in exact2: the receiver and the agent's `type @t` take
  only one `code` or one `error` (`host/listen/src/loopback.rs`,
  `one_code_or_error`).
- A second sign-in needs its own transaction and DPoP key, so each gets a
  second `secret.keep` name and grant (`fixture.pending-second`,
  `fixture.dpop-second`).

## Lean in

- Under the agent, both sign-ins wait as two holds, each showing its port,
  so a drive answers them in either order.

## Needed

- several loopback sign-ins at once: provided (this change)

## Checkpoints

- editing app.contract: smooth
- editing app.ts: smooth
