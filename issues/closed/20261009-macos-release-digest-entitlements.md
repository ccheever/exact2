# macOS release: entitlements for the app and its nested code, and a pre-seal step (rest of #119)

**Status:** Closed
**Resolution:** Built and qualified with Apple Development signing on 2026-10-16; the notarized helper proof, which needs a Developer ID and a notary profile, is a QUEUE item, and the docs claim no notarized support until it runs.
**Systems:** host/apple, delivery, app manifest
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/270

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): only the notarized proof remains (requires a Developer ID and a notary profile).

## Current scope

Fix signing-before-hashing first; coordinate #334. Amend LLP 1069.008 for scoped capability-derived helper entitlements and copied staging files. Arbitrary preseal callbacks are not approved; require a real signed/notarized helper proof.

## Implementation, 2026-10-12

The digest-order slice is implemented: `exact release` passes its identity into
the build, GPU modules receive hardened-runtime signatures before the bake hashes
them, and the release signing walk verifies and preserves baked GPU/Rust code.
Mach-O code in content-addressed assets is refused with a message naming
`host.macos.resources`. A directory merely named `*.bundle` is not treated as a
code bundle without its executable declaration. `scripts/exact.test.mjs` has
regressions for the asset refusal and preserved primary/named GPU and Rust module
digests, using real Mach-O files and `codesign --verify --deep --strict`.

The existing `host.macos.resources` copy already handles staging files before
sealing. Helper runtime exceptions followed on 2026-10-15 (below).

## Implementation, 2026-10-15: scoped runtime-exception grants

Built as LLP 1069.008 D9, accepted by Charlie on 2026-10-15 for helper
executables only; the app's own executable is deferred (`rules/DEFERRED.md`).
`runtime.<exception> <target>` is a grant line beside `device.*`, with three
rows: `allow-jit`, `allow-unsigned-executable-memory` and
`disable-library-validation`, each mapping to `com.apple.security.cs.<name>`.
The target is one helper executable's path under `Contents`, inside a
`host.macos.resources` destination; a line naming `app` is refused with the
other bad lines. The table is `runner/src/hardened.rs`, and
`io_grants` and the web host strip the family. The macOS bake derives
`reach.helperEntitlements` (path → keys), never `reach.entitlements`.
`exact release` signs through `signRelease`
(`host/apple/assets.mjs`): each named helper takes exactly its own keys, the app
only its device entitlements, and other nested code none. A target that is not a regular Mach-O executable the walk signs inside a
resources destination, reached through no symbolic link and with no other hard
link, is refused,
every one in one error, at `mac --bundle` and again at release. The lines are
grant-ceiling lines, so adding one classifies as a binary release with no new
classifier row. Pre-seal: a staging file goes in a `host.macos.resources` tree
under `Resources/`, copied before the seal; no hook was added. Data under
`Helpers/` makes `codesign` fail ("code object is not signed at all"), which
the docs now say.

**Verified, ad hoc (`-s -`), on macOS 27.0.1 with Xcode 27.0:** `bun test
./scripts/exact.test.mjs` (16 pass) signs a fixture through `signRelease`.
`codesign -d --entitlements - --xml` shows a helper with exactly
`allow-jit` and `allow-unsigned-executable-memory`, a nested `.app` with
`allow-jit` and `disable-library-validation`, the app with only its device
entitlement (`audio-input`), and an undeclared
sibling executable and a dylib with none. Run under the hardened runtime, the
granted helper maps `MAP_JIT` memory (exit 0) and its undeclared sibling and
the app are refused (exit 1). `codesign --verify --deep --strict` passes, then
fails once the staged stamp under `Resources/` is edited. With no runtime grant,
every path is signed with no entitlements. A nested bundle's grant follows the
executable `NSBundle` resolves for it: an `Info-macos.plist` or commented decoy
is refused, and an executable named with an XML entity keeps its grant. A
receipt naming `app`, a hard-linked target, or a path
outside the resources destinations, is refused before anything is signed. Bake
and runner unit tests cover the grammar, the derivation and the refusal of
`app`.

**Not verified:** only the notarized proof, which the ruling requires before
release support is claimed. It needs a Developer ID and a notary profile.
The real-bundle qualification follows.

## Qualification, 2026-10-16: real bundle and Apple Development signing

Tested from `issues/20261016-relbundle` at `cc3079cfe`, on Apple Silicon,
macOS 27.0.1 (26A434), Xcode 27.0 (27A266a), Bun 1.4.2, with
`CARGO_BUILD_JOBS=4`. `security find-identity -v -p codesigning` found one
identity: `Apple Development: Charles Cheever (HYDYFSV326)`, SHA-1
`3DEE86312211B1E06395C5EEFA527A59B9BBF505`; no Developer ID Application.

**Hermes.** The pinned resolver uses `~/.cargo/hermes-lean-sys`, not the older
`~/.cache/exact/hermes` cache. `hermesBundle()` reported the installed host
bundle `hermes-vanilla-d412d3bd8512-v5`, digest
`bc71c7b398f1d41a5390761b75ce72e7be234aa48cb1ddec947224188ed6a4f2`.
The installer's actual offline verification passed from this worktree:

```sh
CARGO_BUILD_JOBS=4 cargo run --locked --offline \
  --manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml -- --check
```

This is the command `checkHermesBundles()` ran. It printed
`Verified aarch64-apple-darwin` and the digest-addressed directory above.
The reported worktree-nesting failure did not reproduce; no installer change
or download was needed for macOS.

**Fixture.** Created with
`bun scripts/exact.mjs new target/relbundle-proof/relbundle`. Kept the generated
TypeScript greeting app, set its sole grant to
`runtime.allow-jit Helpers/helper/jit`, and set `host.macos.resources` to
`[{"from":"helper","to":"Helpers/helper"}]`. Compiled the C source below
from `.exact/jit.c` with `clang` to `helper/jit`, then copied it to
`helper/jit2` (separate files, identical bytes before signing). The C source
stays outside `helper/`, which contains only the two executables.

**Real build: passed.** From the worktree root:

```sh
CARGO_BUILD_JOBS=4 \
EXACT_RUST_SIGN_IDENTITY=3DEE86312211B1E06395C5EEFA527A59B9BBF505 \
bun target/relbundle-proof/relbundle/exact.mjs mac --bundle
```

It exited 0 and produced
`target/relbundle-proof/relbundle/target/clients/a3a5589ca2889c409dbe26f9/com.example.relbundle-deb3b4/macos/Relbundle.app`.
The real bake receipt contained `reach.entitlements: []` and exactly
`reach.helperEntitlements: {"Helpers/helper/jit":["com.apple.security.cs.allow-jit"]}`.
The app and both helpers were signed by the Apple Development identity
(`TeamIdentifier=UD2ZVW5WU5`). `codesign -d --entitlements - --xml` emitted no
entitlements for any of the three, and `codesign -dv` showed
`flags=0x0(none)` for each. This is the local build's intended behavior
(LLP 1069.008 D9): it validates helper targets but does not enable the hardened
runtime. `codesign --verify --deep --strict` on that app exited 0.

**Hardened-runtime proof: passed.** Copied that complete app to
`target/relbundle-proof/hardened/Relbundle.app` and called the existing
`signRelease` from `host/apple/assets.mjs` with its own baked
`receipt.build.compat`, the fixture manifest and app ID, the Apple Development
SHA-1 above, `scratch: "target/relbundle-proof/hardened"` (resolved to an absolute
path), and `timestamp: ["--timestamp=none"]`. This exercises the release's
signing walk on the real assembled app without bypassing `exact release`'s
Developer ID check. No secure timestamp or notarization is claimed.

With `APP` naming this hardened copy, ran:

```sh
codesign -d --entitlements - --xml "$APP"
codesign -d --entitlements - --xml "$APP/Contents/Helpers/helper/jit"
codesign -d --entitlements - --xml "$APP/Contents/Helpers/helper/jit2"
codesign -dv "$APP"
codesign -dv "$APP/Contents/Helpers/helper/jit"
codesign -dv "$APP/Contents/Helpers/helper/jit2"
codesign --verify --deep --strict "$APP"
"$APP/Contents/Helpers/helper/jit"
"$APP/Contents/Helpers/helper/jit2"
```

| Object | Entitlements | `codesign -dv` flags | Helper exit |
|---|---|---|---|
| App | none | `0x10000(runtime)` | not run |
| `Helpers/helper/jit` | exactly `com.apple.security.cs.allow-jit = true` | `0x10000(runtime)` | 0: `MAP_JIT` allocation succeeded |
| `Helpers/helper/jit2` | none | `0x10000(runtime)` | 1: `MAP_JIT` allocation refused |

All `codesign` commands exited 0, including deep strict verification. Each
signature reported `TeamIdentifier=UD2ZVW5WU5` and runtime version 27.0.0.
The entitlement dictionaries, flags and helper exit codes were also asserted
by the one-off verification command. The fixture and raw build, signing and
verification logs remain under the ignored `target/relbundle-proof/`.
No product code or regression test was changed: no defect reproduced.
`CARGO_BUILD_JOBS=4 bun test ./scripts/exact.test.mjs` passed all 16 existing
tests (0 failures).

**Release boundary.** With `EXACT_APP_DIR` naming that fixture and
`EXACT_RUST_SIGN_IDENTITY` set to the Apple Development SHA-1,
`bun scripts/exact.mjs release relbundle` exited 1 before the distribution
build or notarization, with this complete diagnostic:

```text
exact: no "Developer ID Application" certificate is on this Mac, and notarisation needs one.
  An "Apple Development" certificate is not it — Apple will not notarise a build signed with one.
  Get it from https://developer.apple.com/account/resources/certificates (a paid Apple Developer
  account), download it, and open it once so it lands in the login keychain. Then run this again.
  EXACT_DEVELOPER_ID=<sha1> names one explicitly.
```

No Developer ID override, notary submission, Gatekeeper notarization assessment
or stapler validation was attempted.

**The notarized proof, for a Mac with a Developer ID and a notary profile.**
Use an app with a helper under `host.macos.resources`
(`{"from": "helper", "to": "Helpers/helper"}`), built from
`int main(void){return mmap(0,16384,PROT_READ|PROT_WRITE|PROT_EXEC,MAP_PRIVATE|MAP_ANON|MAP_JIT,-1,0)==MAP_FAILED;}`
(with `#include <sys/mman.h>`) as `helper/jit`, and the same binary again as
`helper/jit2`. Its source grants hold `runtime.allow-jit Helpers/helper/jit`
and nothing for `jit2`.

```sh
xcrun notarytool store-credentials exact-notary --apple-id <id> --team-id <TEAM> --password <app-specific>
bun exact.mjs release                       # or: bun scripts/exact.mjs release <app>
APP=<the .app path exact release prints>
codesign -d --entitlements - --xml "$APP"; echo
codesign -d --entitlements - --xml "$APP/Contents/Helpers/helper/jit"; echo   # exactly allow-jit
codesign -dv "$APP/Contents/Helpers/helper/jit" 2>&1 | grep flags          # runtime
"$APP/Contents/Helpers/helper/jit"; echo "jit exit $?"                      # 0
"$APP/Contents/Helpers/helper/jit2"; echo "jit2 exit $?"                    # 1: no grant, no JIT
spctl -a -t exec -vv "$APP"                                                 # source=Notarized Developer ID
xcrun stapler validate "$APP"
```

When every line answers as marked, the issue can close.

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

## Closed, 2026-10-18

Everything the ruling asked for is built: code is signed before the bake hashes it, Mach-O in content-addressed assets is refused, helper exceptions are `runtime.<exception> <path>` grants (`runner/src/hardened.rs`, `host/apple/assets.mjs` `signRelease`), and pre-seal files go through `host.macos.resources`. The 2026-10-16 qualification above passed with the hardened runtime. The notarized proof (the commands under that qualification) moved to QUEUE.md's "Notarize the granted helper".
