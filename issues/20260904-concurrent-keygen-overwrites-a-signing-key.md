# Concurrent keygen overwrites a signing key

**Status:** Open
**Systems:** exact deploy, Security
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; LLP 1030 D8; scripts/deploy.mjs

`keygen` promises that a signing key is never overwritten, but implements
that promise as `existsSync(path)` followed by key generation and an ordinary
`writeFileSync(path, ..., {mode: 0o600})` (`scripts/deploy.mjs:146-155`). The
write does not use the exclusive `wx` flag. Two processes can both observe a
missing path, generate different keys, report success, and have the later
write replace the earlier private key.

Reproduced in a fresh temporary key directory by starting two
`node scripts/deploy.mjs keygen race --keys <dir> --json` processes together.
Both exited 0 and printed different public keys; the final `race.pem` matched
only the second result. The temporary directory was removed after the check.
An operator who records the first public half gets a manifest that no longer
matches the only private key on disk. If the first key has already shipped in
a binary, the overwrite destroys the publisher's ability to sign for that
trust epoch unless it has an independent backup.

Done when creation is one exclusive filesystem operation after the parent
directory exists (for example, `writeFileSync` with `flag: "wx"`), and an
`EEXIST` race produces the same clear refusal as a pre-existing file. Add a
concurrent-process test proving exactly one invocation succeeds, the winner's
reported public key matches the PEM, and the loser never changes its bytes.
