# Review round 1, LLP 1121 stage 10 A1 (the method→scope table), Astra (gpt-6-astra, max), security pass

Reviewed: 610842c715 (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

**Two BLOCKING findings for the intended scope boundary.** A1 still exposes only the existing local write token.

- **BLOCKING — Read-only inspect can overwrite outlines.** [tamber-server.mjs:644](/tmp/rv-rl-a1/scripts/tamber-server.mjs:644) permits refresh, but the resulting selection report unconditionally calls `channel.outline` at [line 246](/tmp/rv-rl-a1/scripts/tamber-server.mjs:246). Reproduced: the tool outlines site 1; a `read` client refreshes; the build’s selection reply replaces it with site 0, attributed to the tool. Refresh responses must avoid these write-only visual effects.

- **BLOCKING — Proposal overflow bypasses ownership protection.** [tamber-server.mjs:831](/tmp/rv-rl-a1/scripts/tamber-server.mjs:831) evicts the oldest proposal regardless of owner. Reproduced: one tool proposal followed by 64 `propose`-scope held proposals deletes the tool’s proposal with `reason: "too many held"`. No client spoofing is needed. Refuse overflow or restrict eviction to the caller’s proposals.

- **SHOULD-FIX — Enforcement is not wholly table-driven.** [tamber-server.mjs:484](/tmp/rv-rl-a1/scripts/tamber-server.mjs:484) hard-codes `read` for both streams; removing or strengthening their table entries changes nothing. Inspect’s conditional write requirement also remains in its handler at line 644. Route both stream policy and inspect variants through the central scope resolver.

- **NIT — Stream refusals have the wrong envelope.** [tamber-server.mjs:484](/tmp/rv-rl-a1/scripts/tamber-server.mjs:484) returns `{message, scope, needs}` directly, whereas normal requests return `{error: …}`. Confirmed for both streams. The [documented status table](/tmp/rv-rl-a1/docs/tamber-protocol.md:139) also omits 403.

What checks out:

- All 23 POST methods have entries. In-memory probes verified all 26 above-scope combinations return 403 before reaching the compiler stub. Commit, accept, scrub, hold, pin, preview and explicit outline are gated.
- Unknown, malformed, casing and prototype-name POST methods return 400. `needs("__proto__"/"constructor")` itself returns inherited values rather than `null` ([tamber-pairing.mjs:67](/tmp/rv-rl-a1/scripts/tamber-pairing.mjs:67)), but `METHODS` and `allows` prevent a bypass. An own-property lookup would satisfy the helper’s stated contract.
- Internal compare/import/revert planning calls use fixed method names behind their outer guards; import/revert hold proposals, and acceptance requires write. No source/git write bypass found.
- The protocol bump is consistent. Default local-write authorization remains unchanged; `hello` adds the advertised fields.

Two qualifications: ownership currently trusts the supplied `client`; spoofing it succeeds under the test scope hook. That is not a current privilege escalation with the sole write token, but A2 must bind identity on **methods and both streams**, and supply `local: false`. Also, `export` is only **design-read-only**: [tamber_serve.rs:775](/tmp/rv-rl-a1/contract/cli/src/tamber_serve.rs:775) writes/prunes the documented shared DTCG baseline cache.

The [tests](/tmp/rv-rl-a1/scripts/tamber-pairing.test.mjs:68) cover the current table’s denial matrix, but derive expected scopes from that same table. Missing: the reproduced side effects, successful refresh against a present build, stream-table enforcement, and a registered method missing its scope entry.

Review remained read-only; tree clean. Full suites were not run because they create files/start builds. Probes used the real JS handlers with in-memory compiler/build stand-ins.
