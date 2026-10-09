# macOS release: entitlements for the app and its nested code, and a pre-seal step (rest of #119)

**Status:** Open
**Systems:** host/apple, delivery, app manifest
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/270

## Current scope

Fix signing-before-hashing first; coordinate #334. Amend LLP 1069.008 for scoped capability-derived helper entitlements and copied staging files. Arbitrary preseal callbacks are not approved; require a real signed/notarized helper proof.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #119. #199 made `exact release` sign every nested Mach-O file and code bundle, innermost first, with the Developer ID, the hardened runtime and a timestamp. Two asks of #119 remain, plus the question #199 left open:

1. **Entitlements for what the app ships.** The release signs the app bundle with only the entitlements its `device.*` grants derive (LLP 1069.008), and signs nested code with none. An app cannot give the hardened-runtime entitlements a bundled helper needs (for example `com.apple.security.cs.allow-jit`, `allow-unsigned-executable-memory` or `disable-library-validation` for a JavaScript runtime or a CLI it ships under `host.macos.resources`), so that helper fails under the hardened runtime of a notarized build.
2. **A pre-seal step.** No step runs between the release's assembly and the first `codesign`, so a file written at release time (a build stamp, a distribution channel) cannot go into the sealed bundle.
3. **Code under `Contents/Resources`.** The release now signs Mach-O files found there, after the bake recorded their digests (`QUEUE.md:38`); whether code may live there, and how it is signed before it is hashed, is undecided.

### Current and expected behavior

Current (main `0365ad1a4`; `scripts/exact.mjs`, `host/apple/build.mjs` and `scripts/app.schema.json` are unchanged on `e200397ec`):
- `app.json` with `"host": {"macos": {"entitlements": {"com.apple.security.cs.allow-jit": true}}}`: the build stops with `host.macos.entitlements: not a known key`. The schema's `host.macos` keys are `appTransportSecurity`, `designRequiresCompatibility`, `link`, `minimumOS`, `optimize`, `resources`, `team`, `urlSchemes`, `window`.
- `"host": {"macos": {"preseal": "…"}}`: `host.macos.preseal: not a known key`.
- `exact release` (`scripts/exact.mjs:254-266`): "The app itself carries the entitlements its `device.*` grants derive (LLP 1069.008 D4) …; the code nested inside carries none." `--entitlements` is passed only for the staged bundle; `macReleaseEntitlements` (`host/apple/build.mjs:370-378`) builds the plist from `compat.reach.entitlements` and associated domains only.
- Between `ditto` of the distribution bundle and the signing loop the release only strips symbols (`scripts/exact.mjs:249-252`); nothing an app supplies runs there.

Expected: an app can state the hardened-runtime entitlements of its main bundle and of each nested executable it ships, and add files to the bundle before it is sealed; both reach `codesign` in a release, and a build without them is unchanged.

Proposals (hypotheses; the choice is the decision below):
- A: `host.macos.entitlements` (an object of entitlement keys, merged with the derived ones) and a per-resource `entitlements` on `host.macos.resources` entries for nested code.
- B, keeping LLP 1069.008's single source: new grant rows (for example a `runtime.jit` family) from which the release derives the `com.apple.security.cs.*` entitlements, for the bundle and for the resources that declare them.
- Pre-seal: `host.macos.preseal`, a script `exact release` runs with the staged bundle's path before the first `codesign`; what it adds is sealed. Or a documented release staging directory copied in before signing.

### Reproduction and evidence

App: `bun scripts/exact.mjs new <dir>`, then edit `app.json`.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| App-declared entitlements | `host.macos.entitlements = {"com.apple.security.cs.allow-jit": true}`; `bun exact.mjs mac` | macOS 26.6.2, Apple Silicon | `0365ad1a4` | `app.json does not conform to scripts/app.schema.json: host.macos.entitlements: not a known key` | accepted; `codesign -d --entitlements -` on the released app shows the key | build output quoted |
| Pre-seal step | `host.macos.preseal = "scripts/preseal.sh"`; `bun exact.mjs mac` | macOS 26.6.2 | `0365ad1a4` | `host.macos.preseal: not a known key` | accepted; a file it writes is inside the seal (`codesign --verify --deep --strict` passes) | build output quoted |
| Nested code entitlements | source read: `scripts/exact.mjs:263-266` | — | `0365ad1a4` | nested paths are signed with `--options runtime` and no `--entitlements` | a nested executable carries the entitlements declared for it | source lines |

Not run: `exact release` itself (no Developer ID identity or notary profile on this machine).

### Acceptance criteria

- With a Developer ID and a notary profile, `exact release` on an app that declares entitlements for itself and for one helper under `host.macos.resources`: `codesign -d --entitlements -` shows each declared key on the app and on the helper; the helper runs under the hardened runtime; notarization is Accepted; `spctl -a -t exec -vv` reports "Notarized Developer ID".
- A file a pre-seal step adds is inside the seal (`--strict` passes).
- An app that declares neither builds and releases as today.
- Code under `Contents/Resources` is either signed before the bake records its digest or refused with a message naming where code belongs.

### Constraints and related work

- Governing design: LLP 1069.008 derives entitlements from grants and deleted the manifest's `host.macos.permissions` (Q4, `llp/1069.008-permissions-from-grants.rfc.md:16`); app-declared entitlements widen what a release can grant.
- Workaround: the app re-signs the released bundle with its own script (nested code with its entitlements, then the bundle), and notarizes and staples again. It must follow the bundle layout as it changes.
- Related: #119 (closed by #199, nested signing order only), #103 and #215 (`host.macos.resources`), `QUEUE.md:38` (a Mach-O asset signed after the bake hashed it).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:18:36Z

## Decision needed

**Blocking rule.** LLP 1069.008 (built, Charlie's rulings): entitlements are derived from grants ("one line admits a device, and everything else is derived"), and Q4 deleted the manifest's hand-written `host.macos.permissions` (`llp/1069.008-permissions-from-grants.rfc.md:16`). The release applies that rule and gives nested code nothing (`scripts/exact.mjs:254-258`). An app-declared entitlement list reverses the derivation; a pre-seal step and code under `Contents/Resources` (`QUEUE.md:38`) have no ruling yet.

**Options.**
- **Entitlements A:** `host.macos.entitlements` for the bundle and an `entitlements` object per `host.macos.resources` entry, merged with the derived ones.
- **Entitlements B:** new grant rows for the hardened-runtime exceptions (JIT, unsigned executable memory, library validation off), from which the release derives the `com.apple.security.cs.*` keys, for the bundle or for the resources that name them. One source stays, and the deploy classifier already treats a grant-ceiling change as a binary release.
- **Pre-seal A:** `host.macos.preseal`, a script run with the staged bundle before the first `codesign`. **Pre-seal B:** a declared directory copied into the staged bundle before signing (no app code runs in the release).
- **Code under Resources:** sign nested Mach-O before the bake records its digest, or refuse Mach-O in content-addressed assets and require `host.macos.resources`.

**Recommendation.** Entitlements B, pre-seal B, and signing before the digest. B keeps LLP 1069.008's single source and its reach table honest; a copied directory covers the build-stamp case without running app code inside `exact release`; signing first fixes the update path `QUEUE.md:38` describes.

**Cost.** Entitlements B: grant table rows, `reach` derivation, per-path `--entitlements` in the signing loop, schema and docs, a test over `signingOrder`; one to two days. Pre-seal B: half a day. Digest order: about a day, with a delivery test.

### ccheever — 2026-10-08T08:07:42Z

**Decision: Fix signing-before-hashing; choose scoped derived helper entitlements and copied staging files.**

Keep open with the bounded scope below.

Code signed after a content digest is recorded can break delivered integrity. Keep entitlements derived from declared capability needs and scoped to the executable that needs them.

Do the digest-order correctness fix first. Amend LLP 1069.008 for any runtime exception grants; use the existing build/staging flow rather than an arbitrary preseal callback. Require a real signed/notarized helper proof before claiming release support.
