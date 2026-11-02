# The tool proxy refuses local imports that the design server accepts

**Status:** Closed
**Severity:** P2
**Systems:** `scripts/tamber-server.mjs`, Tamber tool proxy
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

Local token holders may import up to 16 MiB through the design server
(`scripts/tamber-server.mjs:544-552`), but `tamber tool --via` rejects every
body above its approximately 1 MiB limit before forwarding it
(`1439-1445`). A supported local DTCG or stylesheet import therefore fails
depending on whether the tool connects directly or through the usual dev-loop
proxy. Paired clients are still supposed to have the smaller limit.

Reproduced with the actual server and proxy on loopback, the existing
`target/debug/contract` binary, and a disposable app. Send a local-token
`import` request containing one DTCG color with a 2 MiB `$description`, first
directly to the design server, then through `forward` to that same server.
The same 2,097,311-byte body returns HTTP 200 with a held proposal directly,
and HTTP 413 with `a request is at most 1 MiB` through the proxy.
The probe is `/tmp/exact-review-via-import.mjs`; it uses no external service
and changes no project files. The normal tool CLI was not launched.

Preserve the design server's local-import allowance through the proxy while
keeping authentication, method-specific limits, and the smaller paired-client
limit enforced upstream. Add a proxy-path counterpart to the existing large
local import case in `scripts/tamber-pairing-server.test.mjs:65-85`.
The generic proxy limit test in `scripts/tamber-body-limit.test.mjs:102-139`
does not exercise a valid local import.

No duplicate found in this checkout or the open and closed issue corpora in
the local `exact2` and `exact` checkouts.

## Resolution

Allow token-bearing import requests through the proxy up to the local 16 MiB limit. Keep other methods at 1 MiB and leave token authentication and the paired-client limit to the upstream design server.

## Verification

`bun test --isolate scripts/tamber-pairing-server.test.mjs scripts/tamber-body-limit.test.mjs` passes an authenticated 2 MiB proxy import, paired-client/method refusals and generic proxy bounds. The suites use the prebuilt contract binary.
