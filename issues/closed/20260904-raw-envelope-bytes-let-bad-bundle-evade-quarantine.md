# Raw envelope bytes let a bad signed bundle evade quarantine

**Status:** Closed
**Resolution:** Bundle entries and crash quarantine now use the SHA-256 of the authenticated canonical envelope body, so transport reserialization retains one identity.
**Systems:** Update store, Delivery, Security
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; update/src/envelope.rs; update/src/store.rs

The signature authenticates `canonical_bytes(envelope)`: whitespace and object
key order are deliberately normalized (`update/src/envelope.rs:48-86`). The
store's identity is instead `sha256(raw)`, the exact serialized JSON bytes
(`update/src/envelope.rs:183-191,358-361`). Entry directories, selection,
`current`, and the eight-element crash quarantine all use that raw digest.

The mismatch was reproduced on the committed publisher fixture. Pretty-printing
its `exact.json` changed the raw digest from
`113c0fe07fb73104f3494df87a27d6846cf7b0c3fcc513914fbc2a8046942ec1` to
`0a282172f14c914668281e8e5a3f1ffd1592b45a8fc90359319e056987514e75`, while
the canonical bytes stayed equal and the original Ed25519 signature still
verified.

An untrusted CDN can therefore reserialize a valid signed head after the bundle
fails to reach first pixel twice. The same semantic bundle gets a fresh entry
identity, is absent from `record.bad`, and is staged and crashed again. It can
repeat with arbitrarily many whitespace/key-order encodings. Even without a
crash, a harmless reserialization makes the selected bundle look new and
duplicates it on disk. This defeats D11's guarantee that a demoted bundle is not
restaged.

Done when entry/current/quarantine identity is derived from the authenticated
semantic bytes (or an explicit signed bundle id), not transport serialization.
Pair it with the anti-rollback rule that a used seq cannot equivocate to another
bundle. Test compact versus pretty/key-reordered forms with the same signature,
two-crash demotion followed by each form, and genuinely different signed
content at the same and higher seq.
