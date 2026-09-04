# Production updater with no keys accepts unsigned heads

**Status:** Closed
**Systems:** Update client, App manifest, Delivery
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; LLP 1030 D1; LLP 1030.000 D4

The updater has no explicit development/release trust mode. It infers one
solely from whether the baked verification-key vector is empty:
`Envelope::verify` immediately succeeds when `keys.is_empty()`
(`update/src/envelope.rs:380-390`). `Baked::from_compat` maps a missing,
`null`, or empty `inputs.keys` object to that same vector while independently
accepting an update origin and a linked Level A store
(`update/src/binary.rs:54-115`).

That combination is valid under `scripts/app.schema.json`: `deploy.signing`
and both of its members are optional, with no constraint tying a non-empty
key set to a production origin or an update-capable native artifact. The
Apple and Linux build scripts produce release-profile binaries and embed the
resulting `compat.json`; there is no later artifact marker that can turn the
empty set into a refusal policy. `exact deploy --yes` requiring a private
signing key does not protect the client: LLP 1026 deliberately treats the
CDN/origin as untrusted.

The behavior is already executable in the test suite. The signed publisher
fixture passes `head.verify(&[])` (`update/tests/publisher.rs:116-120`), and
`an_unsigned_head_is_refused_with_keys_and_admitted_without_them`
(`update/tests/store.rs:466-490`) stages the unsigned head for an empty-key
client. Give that same baked client an origin — through its manifest or
`EXACT_UPDATE_ORIGIN` — and an attacker controlling the response needs no
private key to replace its plan and assets.

This contradicts LLP 1026 D11: dev bundles may be unsigned, but a release
binary refuses them. Today “dev binary” is neither an authenticated property
of the artifact nor a policy named by the build; it is just an absent trust
root. A damaged or incompletely configured release therefore fails open.

Done when unsigned admission is an explicit development-only property that a
production artifact cannot acquire by omission. Production builds with a
linked store or update origin must fail their bake when no trust root is
present, and runtime parsing must fail closed for missing/malformed trust
policy. Tests must cover a release artifact with missing, `null`, and empty
keys; each refuses an unsigned and an unverifiable signed head, while an
explicit dev artifact can still use the unsigned local-network loop.

Resolved 2026-09-04. The bake writes an explicit `inputs.trust` value into
`compat.json` and the compatibility digest. Direct Cargo/Contract bakes default
to production; `EXACT_UPDATE_TRUST=development` is the deliberate development
choice, also set by the shared developer build scripts. Deploy always bakes
production. Production with a linked store or origin requires at least one
well-formed public key; omitted or malformed runtime trust policy refuses to
open the updater. The verifier never accepts an empty trust root. Only an
explicit development artifact admits unsigned heads, and even that artifact
verifies any supplied signature.

Validation: Contract and update regression suites cover missing/null/empty
keys, malformed trust, unsigned/unverifiable signed heads, and distinct trust
cohorts. Actual Contract CLI probes confirmed default production refusals and
explicit development bakes. A native Caltrain agent staged an unsigned bundle
from a loopback HTTP origin under development trust; the default production
rebuild refused unsigned and unverifiable signed heads before any payload
request, even with a development environment variable at runtime. Linux smoke
passed, as did focused Clippy with warnings denied.
