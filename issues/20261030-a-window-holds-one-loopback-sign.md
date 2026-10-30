# A window holds one loopback sign-in, and a callback without a code ends it

**Status:** Open
**Systems:** runner auth (`runner/src/auth.rs`), the loopback receiver (`host/listen/src/loopback.rs`), LLP 1069.006 D3, LLP 1133.005 D9
**Author:** Claude (Opus 5.5), from the T3 Code clone
**Date:** 2026-10-30

**Problem.** T3 Code signs in two ChatGPT accounts at once, each on its own loopback port, and both complete
(`apps/desktop/src/app/CodexAuthCallback.test.ts`, "allows two accounts to complete independently"). In Exact the
second `openAuthSession` is refused 409 `already open`. T3 answers a callback with the right `state` but no `code`
or `error` 400 and keeps listening (`codexCallbackUrl`, `packages/shared/src/codexAuthHandoff.ts`); Exact's
receiver answers it 200 and stops, and the sign-in must start again.

**Cause.** `arm` refuses any session while another target's is live (`runner/src/auth.rs:673-680`, LLP 1069.006 D3,
made for a sheet). The receiver accepts on `state` alone (`host/listen/src/loopback.rs:215-246`) and stops (`:201-205`).

**Reproduction.** Two targets' loopback sign-ins under the agent: the second is 409.

**Fix.** A loopback sign-in shows no sheet: it neither refuses nor is refused by another target's session, and a
second one on a live one's host and port is 502 `port in use`, as its bind is. Its callback must carry exactly one
non-empty `code` and no `error`, or the reverse (RFC 6749 §4.1.2, §4.1.2.1); else the receiver answers 400 and keeps
listening, and the agent's `type @t` is refused before the hold is spent. The receiver drops a fragment before it
reads the query, as the runner does.

**Acceptance.** `cargo test -p contract --test it auth`, `-p exact-listen -- --include-ignored`; on macOS,
`smoke-auth.mjs` and an `EXACT_DEVICE=real` drive: two sign-ins at once, a stray callback 400.
