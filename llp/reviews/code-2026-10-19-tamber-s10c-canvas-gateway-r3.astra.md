# Review round 3, LLP 1121 stage 10 C1 (the canvas gateway), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — Non-loopback URLs are still altered.** [scripts/tamber-canvas.mjs:138](/tmp/rv-s10c-r3-astra/scripts/tamber-canvas.mjs:138). The matcher stops at legal userinfo punctuation before identifying the actual host. Reproduced: `http://127.0.0.1;@evil.test/a` becomes `http://loopback.invalid;@evil.test/a`, including in JSON and SSE. Conversely, `http://user;name@localhost:9000/a` survives unchanged. **Fix:** parse complete authorities before classification, preserve non-loopback URLs intact, and cover semicolons, commas and parentheses. These violate ruling (1)/(3), independently of the accepted exotic encodings.

2. **SHOULD-FIX — Text-body rewriting contradicts ruling (2).** [scripts/tamber-canvas.mjs:450](/tmp/rv-s10c-r3-astra/scripts/tamber-canvas.mjs:450), using [the shared rewriter](/tmp/rv-s10c-r3-astra/scripts/tamber-canvas.mjs:150). HTML, JS, CSS and plain text replace the loop’s own origin with the real canvas host. The ruling requires blanking those loopback URLs. **Fix:** separate header/JSON origin remapping from text-body blanking; align the tests and §10 draft.

Prior findings confirmed fixed: **R1 Astra 1–3, 5–6; R1 Grok 1, 3–7; R2 Grok 1–2.** The shared rewriter finding—**R1 Astra 4/Grok 2 and R2 Astra 1**—remains incomplete as described above. Accepted exotic encodings are excluded.

The security pass found no additional credential, authorization or revocation defect:

| Area | Assessment |
|---|---|
| **1. Tokens, grants, sessions** | All use 256-bit CSPRNG values. Grants/sessions and the local token use timing-safe comparisons; A’s paired tokens use SHA-256-indexed lookup. Grants are single-use and expire at 30 seconds. No generated credential leaks into URLs, queries, logs or errors found; cookies never authenticate as tokens. |
| **2. Cookie/tossing** | Required `__Host-`, Secure, HttpOnly, Strict, Path=/ attributes; no Domain. Only server bindings authorize. Unknown page-set values grant nothing; invalid replacement/deletion requires a fresh grant. Up to four duplicate values are examined. The tool ignores cookies for authentication. |
| **3. CSRF/origin** | Entry requires explicit tool/canvas Origin; proxied POSTs require canvas Origin. Strict cookies and downstream CORS-header removal hold. |
| **4. Forwarded mark** | Every client `forwarded`/`x-forwarded-*` header is removed before the fixed marker is added. Both dev loops, including JS→resident forwarding, classify it as non-local. Tamber/install tokens and native admission links remain withheld; `/__tamber*` is blocked. |
| **5. Header/URL leaks** | Client Cookie/tamber credentials and upstream Set-Cookie are stripped. Range headers are removed. Rewriter correctness remains affected by findings 1–2. |
| **6. Routes/scopes** | Required read/write distinctions hold, including GET/HEAD open requiring write. Unlisted reserved routes and ambiguous raw paths are refused. |
| **7. Revoke/expiry** | Fetches, streams, stalled uploads and backpressure waits abort; sessions/grants disappear and subsequent requests fail. Probes covered requests spanning revoke, newly minted sessions, expiry and other-client isolation. |
| **8. Dev-server changes** | Local admission links remain available; non-local requests fail closed. Checkpoint 403 triggers fresh reload; local successful checkpoint behavior remains unchanged. |
| **9. A integration** | Shared scope check precedes mint. Body client/token spoofing cannot change ownership. Local tokens are refused. The authenticated token stays in memory and never reaches the design child. Server changes are minimal. |
| **10. Limits** | Six grants/minute, burst six; eight grants, sixteen sessions and thirty-two live requests/client. Bodies: 1 MiB−64 bytes; write-authorized checkpoints: 16 MiB; entry: 4 KiB. |

Validation: two dev-gate tests and in-memory gateway/A probes passed. The canvas suite failed at fixture bind (`127.0.0.1:0`, `EADDRINUSE`). Full install/pairing suites invoke Cargo and were skipped. No files edited, Cargo or simulators run. Real dev-loop and B’s TLS/browser gates remain unverified.
