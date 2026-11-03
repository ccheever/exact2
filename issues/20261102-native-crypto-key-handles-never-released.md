# Native CryptoKey handles retain every discarded key

**Status:** Closed
**Severity:** P2
**Systems:** Hermes executor, WebCrypto
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

Every `crypto.subtle.importKey` appends an owned `EcKey` to the module's
native vector (`js/src/crypto.rs:80-82`, `97-102`). `generateKey` and
`store.key` append two (`84-92`, `129-131`). The JS wrapper stores the
integer handle in a WeakMap (`js/src/prelude.js:259`, `288-304`). Collecting
the CryptoKey releases that JS association, but no door operation removes
the native value, and module maintenance never clears or compacts the vector.

These flows are reachable through the standard prelude: `importKey` calls
bytes-door op 4 (`js/src/prelude.js:351-358`), `generateKey` calls op 2
(`315-325`), and `store.key` calls op 7 (`394-403`). Repeated temporary
imports or reading the same stored DPoP key therefore retain every key until
the entire module dies. An app need not keep any JS reference to them.

Reproduce through the actual native bytes door:

1. Generate an extractable P-256 key pair and export its public JWK.
2. Import that same JWK 1,000 times through bytes-door op 4, dropping every returned handle immediately.
3. Inspect the native key table. It retains 1,002 keys: the initial pair and every discarded import.

`/tmp/exact-review-crypto-handles.rs` includes the unchanged repository
`js/src/crypto.rs`, supplies its HostState/door plumbing, and links the
existing runner/data rlibs. It prints:

```text
1000 discarded import replies: 1002 native keys retained; 192 bytes/key, 196608 bytes vector capacity
```

This measures native ownership through the real door, without running
Hermes garbage collection or measuring whole-app RSS. The WeakMap and absent
release path explain why JS collection cannot reclaim those owned values.
Private keys can remain allocated through the same path after their JS
objects become unreachable.

Tie native handle lifetime to the JS CryptoKey lifetime, or otherwise
reclaim unused handles while preserving live keys. The ECDSA tests verify a
few keys and reload the module; none covers release. No matching issue was
found in either issue corpus.

## Resolution

The native key table uses non-reused handles. The shim records weak CryptoKey owners and removes unreachable keys on the executor thread after collection; unloading clears the table.

## Verification

`discarded_crypto_keys_are_reclaimed_after_collection` passed through Hermes. It imports and discards 1,280 keys, verifies signatures from a retained key after every batch, checks that fewer than 128 native keys remain, and verifies unload cleanup. The existing native JS integration suite passed 243 tests.
