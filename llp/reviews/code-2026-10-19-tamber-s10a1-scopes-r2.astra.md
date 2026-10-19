# Review round 2, LLP 1121 stage 10 A1, Astra (gpt-6-astra, max), security pass

Reviewed: c3ba38c43c (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

Two blockers remain; round 1’s proposal-cap fix is incomplete.

- **BLOCKING — proposal eviction still races.** [tamber-server.mjs:597](/tmp/rv-rl-a1b/scripts/tamber-server.mjs:597) calls `room()` before asynchronous planning; [insertion at :851](/tmp/rv-rl-a1b/scripts/tamber-server.mjs:851) still evicts globally. Reproduced: 63 tool-owned proposals plus two concurrent `propose`-scope holds both return 200, evicting the tool’s oldest proposal. Carry scope through hold/import/revert and enforce capacity synchronously immediately before insertion. The [cap test](/tmp/rv-rl-a1b/scripts/tamber-pairing.test.mjs:120) is sequential and misses this.

- **BLOCKING — read-scoped export mutates shared import state.** [tamber-pairing.mjs:35](/tmp/rv-rl-a1b/scripts/tamber-pairing.mjs:35) permits `export` at `read`, but [the Rust handler](/tmp/rv-rl-a1b/contract/cli/src/tamber_serve.rs:775) persists its baseline. [keep()](/tmp/rv-rl-a1b/contract/cli/src/tamber_export.rs:149) writes files and deletes baselines beyond 16. Exporting a new current digest at capacity can therefore invalidate another client’s pending re-import. Make lower-scope exports non-mutating, or authorize baseline mutation separately. This finding is source-traced; filesystem writes were not executed.

- **SHOULD-FIX — selection cache is unbounded.** [tamber-server.mjs:240](/tmp/rv-rl-a1b/scripts/tamber-server.mjs:240) retains selections indefinitely; [refresh](/tmp/rv-rl-a1b/scripts/tamber-server.mjs:653) filters disconnected builds without deleting entries. Confirmed 300 retired selections remain recoverable despite the channel’s 256-build cap. Bound or prune the cache. The [new cache test](/tmp/rv-rl-a1b/scripts/tamber-pairing.test.mjs:93) exercises only an empty cache.

What checks out: cached refresh sends no build commands and returns the same selection payload already broadcast to read clients—no new cross-client confidentiality leak found. Inspect on/off, explicit drop ownership, unknown-method refusal, stream authorization/envelopes, documentation, `hello` fields, and `Object.hasOwn` are resolved. A2’s planned token-derived `client` and `local:false` need no additional A1 identity change.

Validation: eight focused tests passed, plus in-memory wire/security probes. Full integration tests were not run because they create fixtures and locks. Working tree remains clean.
