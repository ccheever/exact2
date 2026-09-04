# The update store allows cross-origin file cards

**Status:** Open
**Systems:** Update store, Delivery, Security
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D1/D2; update/src/envelope.rs

LLP 1023 D1 says every pointer is same-origin in v1. The production store's
resolver accepts any absolute `http` or `https` URL unchanged
(`update/src/envelope.rs:467-509`). Consequently a signed head fetched from
`https://updates.example` may direct plan or asset requests to another host,
another port, or from HTTPS down to HTTP. Relative cards are normalized, but
their resulting origin is never compared either.

The publisher emits relative URLs today, but client admission is the trust
boundary. A misconfigured publisher or compromised signing key can make every
installed client contact and download from an undeclared origin; signature
verification authenticates that instruction but does not amend the v1 network
contract. The Apple dev URL loader already enforces same-origin redirects, so
the two delivery paths disagree.

Done when card resolution compares normalized scheme, host, and effective
port with the head URL and refuses any cross-origin or HTTPS-to-HTTP result
before fetching. Cover relative paths, root-relative paths, absolute
same-origin URLs, cross-host and cross-port URLs, downgrade, credentials, and
redirect behavior in both host transports.
