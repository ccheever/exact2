# The local-network grant admits the cloud metadata address

**Status:** Closed
**Systems:** Grants (`grants/src/local.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** `net.fetch local-network` and `net.websocket local-network` admit `169.254.169.254` and its IPv4-mapped form. That address is link-local, and on AWS and similar hosts it is the instance metadata service. A grant written for printers, a NAS and `.local` names also reaches instance credentials when the host serves IMDSv1 there.

**Cause.** `local_v4` is `is_private() || is_link_local()`, and with the development word, loopback too. `Ipv4Addr::is_link_local` is `169.254.0.0/16`. `local_v6` judges an IPv4-mapped address by that rule. The module documents link-local as in scope, and the tests treat `169.254.3.4` as local. CGNAT `100.64.0.0/10` is excluded. The metadata address is not.

**Reproduction.** `is_local_network("169.254.169.254", false)` is true. `is_local_network("[::ffff:169.254.169.254]", false)` is true. An app granted `net.fetch local-network` on a cloud VM can request `http://169.254.169.254/`.

**Fix.** Keep the rest of link-local, and refuse `169.254.169.254` and its IPv4-mapped form. Add a test that this address is refused while `169.254.3.4` stays admitted.

**Acceptance.** A local-network fetch of the metadata address is refused on a production bake. A fetch of another link-local address still proceeds. A development loopback grant is unchanged.

## Resolution

Local-network fetch and WebSocket grants exclude the metadata address and IPv4-mapped forms while retaining other link-local addresses. The permission-risk classifier still treats metadata as sensitive.

## Verification

The grants and runner suites passed. Regressions cover IPv4 and both mapped IPv6 spellings, ordinary link-local admission, development loopback, and an unknown-permission tool request to metadata.
