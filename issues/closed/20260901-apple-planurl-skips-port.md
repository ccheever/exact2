# Native URL loaders do not check the origin they claim

**Status:** Closed
**Resolution:** Apple URL authority checks now compare scheme, lowercase host, and effective port.
**Systems:** Apple host, Linux host
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001 §2 step 3 (same host:port as the page; “v1's same-origin rule”)

Apple `PlanURL.takeEnvelope` (`host/apple/swift/PlanURL.swift`) compares `planURL.host == page.host`. Foundation's `URL.host` is the hostname without port. `http://127.0.0.1:8771/` will accept a plan from `http://127.0.0.1:9999/evil.plan`, and `http://` will accept `https://` on the same host. The events URL uses the same host-only check.

Linux `same_host` (`host/linux/src/fetch.rs`) includes host:port in the string after `://`, so different ports refuse. It still ignores scheme: `http://host:8771` and `https://host:8771` compare equal.

The spec's words are host:port; the parenthetical is same-origin. Apple misses port. Both miss scheme.

Fix: compare scheme, host, and port (use `port_or_known_default` so `:80` and implicit 80 match). Same helper on Apple and Linux. Test: a plan URL on another port of the page's host is refused; a scheme change is refused.
