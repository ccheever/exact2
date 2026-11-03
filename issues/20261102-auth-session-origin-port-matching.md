# Auth session origin matching rejects equivalent ports and ported wildcards

**Status:** Closed
**Severity:** P2
**Systems:** Auth sessions, Grant admission
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`
**Related:** Closed `20261006-auth-userinfo-disclosure.md`

`auth.session` is documented as using `net.fetch`'s origin grammar
(`runner/src/auth.rs:287-292`; `docs/recipes/oauth.md:11`). Network origins
compare by scheme, host and effective port (`grants/src/lib.rs:30-52`,
`613-616`). The auth matcher instead compares raw authority text and rejects
any colon in a wildcard suffix (`runner/src/auth.rs:320-338`).

An `auth.session https://login.example:443` grant refuses
`https://login.example/authorize`; the reverse spelling also fails. A
wildcard grant with a nondefault port, such as
`auth.session https://*.example.com:8443`, passes `session_grant_refusal`
but can never match a corresponding authorization URL because its suffix
contains `:`.

Reproduce with the public runner API and a session whose callback is
`test.app:/oauth` and whose state is `s1`. Add
`auth.callback test.app:/oauth` to each grant set:

| auth.session target | Authorization URL | Actual result |
| --- | --- | --- |
| `https://login.example:443` | `https://login.example/authorize` | Outside the app's grants |
| `https://login.example` | `https://login.example:443/authorize` | Outside the app's grants |
| `https://*.example.com:8443` | `https://login.example.com:8443/authorize` | Outside the app's grants |

`/tmp/exact-review-auth-origin.rs` calls `session_grant_refusal` and
`check_grants` from the existing runner rlib. The grant validator accepts
all three declarations; all three matching calls refuse. The probe opens
no browser or network connection and does not exercise host presentation.

Normalize authorization origins through the shared grant grammar and
compare effective ports, including wildcard ports. This finding concerns
`auth.session`; exact callback-URI matching is a separate policy and is not
part of the requested correction. No matching origin-port issue was found
in either issue corpus.

## Resolution

Authentication sessions use the shared origin matcher, including effective default ports and wildcard domains with explicit ports.

## Verification

`session_origins_use_effective_ports_and_wildcard_ports` passed. It checks both spellings of HTTPS port 443, wildcard port 8443, and wrong-port, bare-domain and lookalike-domain refusals.
