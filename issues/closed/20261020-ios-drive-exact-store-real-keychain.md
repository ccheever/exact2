# EXACT_STORE=real on an iOS drive does not keep secrets in the Keychain a normal launch reads

**Status:** Closed
**Resolution:** simctl and devicectl forwarded none of the drive's environment; an iOS drive now passes EXACT_STORE on (devices.mjs forwardedEnv), so EXACT_STORE=real reaches the Keychain a normal launch reads
**Systems:** agent (scripts/agent.mjs), Apple host (iOS), durable store (secret.keep), docs
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

`docs/contract-for-agents.md` says `EXACT_STORE=real` gives an Apple drive the Keychain. The lane
signed its app in during `EXACT_STORE=real bun exact.mjs agent ios …`; the drive reported that it
"keeps the store `default`", and the next `bun exact.mjs ios --run` opened on the sign-in screen, so
the secret had not reached the Keychain a normal launch reads.

Workaround: drives with `--touch platform` using the agent's scratch store. Cost: about 15 minutes.

## Reproduction

An app that keeps a secret (`secret.keep name`, `store.set`) at sign-in: sign in under
`EXACT_STORE=real bun exact.mjs agent ios "type …" "tap …"`, then `bun exact.mjs ios --run`.

## Expected

Either the secret is there on the next normal launch, as the docs say, or the docs describe what
`EXACT_STORE=real` does.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.
