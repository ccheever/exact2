# The module-signature test fails when Homebrew's OpenSSL 3 is first on PATH

**Status:** Closed
**Resolution:** the test calls /usr/bin/openssl; 7 of 7 pass with Homebrew's OpenSSL 3 first on PATH
**Systems:** tooling (scripts/rust.test.mjs)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30

`scripts/rust.test.mjs` "a published module signature stands only for the same code and certificate" builds throwaway identities with whatever `openssl` is on `PATH`, then imports them with macOS `security`. With `/opt/homebrew/bin` first (OpenSSL 3.6.4), the import fails:

```
security import: security: SecKeychainItemImport: MAC verification failed during PKCS12 import (wrong password?)
```

With the system `/usr/bin/openssl` (LibreSSL 3.3.6) all 7 tests pass (checked 2026-09-30, macOS 27.0). OpenSSL 3 writes PKCS#12 with AES-256/PBKDF2 and a SHA-256 MAC by default; `security import` reads the legacy form.

Fix: call `/usr/bin/openssl` in the test (it is darwin-only already), or pass OpenSSL 3's `-legacy` to `pkcs12 -export` when the binary supports it.
