# Review round 2, LLP 1121 stage 10 C1 (the canvas gateway), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — Round-1 URL rewriting remains incomplete.** [scripts/tamber-canvas.mjs:117](/tmp/rv-s10c-r2-astra/scripts/tamber-canvas.mjs:117) and [the matcher at :131](/tmp/rv-s10c-r2-astra/scripts/tamber-canvas.mjs:131). In-memory probes confirmed:
   - `http://localhost.:9000/` and `http://127。0。0。1:9000/` survive unchanged; the latter parses as `127.0.0.1`.
   - HTML `http://localhos&#116;:9000/` and JSON `http://localhos\u0074:9000/` survive and decode to loopback URLs.
   - `http://127.0.0.1%2F@evil.test/a`, whose actual host is `evil.test`, becomes `http://loopback.invalid/a`.

   **Fix:** handle escapes within each serialization, parse complete URL authorities without first decoding their delimiters, and rewrite only genuine loopback destinations. Cover these cases in headers, HTML, JSON and SSE. Astra R1 finding 4 and Grok R1 finding 2 remain open.

The other round-1 findings are addressed in the reviewed code: Astra **1, 2, 3, 5, 6**; Grok **1, 3, 4, 5, 6, 7**. Grok 4’s interface discrepancy is explicitly resolved by the approved plan additions.

Security pass:

| Area | Assessment |
|---|---|
| **1. Credentials** | Grants/sessions use 256-bit CSPRNG values and timing-safe hash comparisons. Grants are single-use, expire at 30 seconds, and travel in bodies. No gateway credential emission into URLs, logs or errors found; cookies never authenticate as paired tokens. A’s token implementation remains outside this review. |
| **2. Cookie** | All required attributes are present, without Domain. Validity comes from the server-side binding. Unknown/tossed values grant nothing; replacement/deletion loses access. Up to four duplicate values are checked. Actual two-port browser proof remains pending. |
| **3. CSRF** | Entry requires explicit tool/canvas Origin; proxied POSTs require canvas Origin. Strict cookies and CORS-header stripping hold. |
| **4. Forwarding mark** | Every client `forwarded`/`x-forwarded-*` header is stripped before the fixed marker is added. Both dev loops classify marked requests as non-local; JS→resident forwarding preserves it. Local token routes remain protected, and `/__tamber*` is blocked. |
| **5. Leaks** | Client Cookie/tamber credentials and upstream Set-Cookie are stripped. Loopback URL exclusion fails in finding 1. |
| **6. Routes/scopes** | Required write/read distinctions hold, including GET/HEAD open requiring write. Raw-target validation closes the reported normalization bypasses; unlisted reserved routes are refused. |
| **7. Revoke/expiry** | Cancellation now reaches uploads, backpressure and upstream fetches; responses remain tracked through close. Sessions/grants are deleted. Grant→session creation has no asynchronous gap after validation. Live expiry depends on A’s `onEnd`. |
| **8. Local behavior** | Local admission links remain available; non-local requests fail closed. The checkpoint-403 fallback reloads read/propose canvases. No local regression found by inspection. |
| **9. Limits** | Six grants/minute, burst six; eight grants, sixteen sessions and thirty-two live requests/client. Bodies: 1 MiB−64 bytes; checkpoint: 16 MiB after write authorization. |

Validation: two focused install-page gate tests passed. The canvas suite failed during fixture startup (`EADDRINUSE`, loopback port 0). Full install-page testing was skipped because it invokes Cargo. No Cargo, simulator runs or file edits.
