# Selected update files are not reverified at launch

**Status:** Closed
**Resolution:** Fixed by mandatory verified plan reads and cached lazy signed-card asset reads; fresh native launches reject modified plans, modified assets and missing assets before counting or blessing, while clean selections boot.
**Systems:** Update store, Delivery
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D2/D3; LLP 1026 D11

The store verifies card lengths/digests while downloading an entry, but its
normal launch validation does not verify the stored payloads again.
`Store::validate()` requires only that `app.plan` is a file, parses
`exact.json`, and compares the envelope digest, app id, and compatibility id
(`update/src/store.rs:655-676`). It neither rechecks the plan card nor any
asset card. `Client::selected_plan()` then reads the plan bytes directly
(`update/src/client.rs:131-139`), and both hosts expose the assets directory to
their resolver. `Store::activate()` rehashes the plan, but not assets, and that
does not protect the ordinary next-launch path.

A truncated/corrupted asset can therefore be displayed from an entry still
called whole; a modified but decodable plan can execute without matching the
signed card. This does not require a network race — one disk error or external
mutation between staging and launch is enough.

Done when selection proves the bytes it uses still match the signed entry,
falling back without counting/blessing a corrupt candidate. Keep the boot
budget explicit: the plan may be verified during its mandatory read and
assets may be verified lazily at first resolution or through an equally strong
immutable-store mechanism. Test plan and asset mutation after staging, a
missing asset, and a clean entry with no unnecessary extra reads.
