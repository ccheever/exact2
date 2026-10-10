---
name: 20261005-x37-distribution-signing
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap (unconfirmed)
blocks: [20261005-portable-app-download]
upstream_url: https://github.com/ccheever/exact2/issues/119
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/270
---

# X37: Developer ID signing, notarization and a pre-seal hook in the host build

Moved to main `issues/20261009-macos-release-digest-entitlements.md` (2026-10-09); tracked there.

## Summary

T3 Code ships to users as a signed, notarized Apple Silicon app when the project's signing secrets are present. The
clone had to run on another person's Mac, so the Gatekeeper experience of its downloadable `.app` was part of the
goal. When the clone was planned, exact2's macOS host build had no documented way to sign with a Developer ID
identity, enable the hardened runtime, notarize, or add a file to the finished bundle before it is sealed.

## Why it arose

### The T3 Code behavior
- The release flow builds, signs and notarizes the macOS desktop app. "Omitting signing secrets only makes platform
  artifacts unsigned" (`docs/operations/release.md:436-437`). The secrets are a Developer ID certificate export, an
  App Store Connect API key, `APPLE_TEAM_ID` and a provisioning profile for Associated Domains (`release.md:440-478`),
  which serves desktop passkeys for T3 Connect sign-in (X38, out of scope).
- The desktop packager signs through `scripts/sign-macos.ts` only when signing is enabled
  (`scripts/build-desktop-artifact.ts:1578,2732`, `T3CODE_DESKTOP_SIGNED`).
- The server archive is signed too: every `.node`, `.dylib`, `spawn-helper`, `t3-resource-monitor` and the `t3`
  executable, with the hardened runtime and a timestamp under a real identity; the executable gets JIT entitlements
  (`apps/server/resources/cli-entitlements.plist`); without an identity it is signed ad hoc
  (`scripts/build-cli-archive.ts:295-368`; `release.md:43`).

### Where the clone hit it
`20261005-portable-app-download` built a zip that another person unpacks and opens; its package step wrote
`Contents/Resources/distribution.json` into the finished bundle before signing. Its decision U11 chose between an
ad-hoc signature with a documented one-time approval and an app-local Developer ID signing and notarization script.

## Clone workaround
Decision U11 (2026-10-05): the plan ships an ad-hoc signed zip, so the plan's download did not need this. Then
`portable-app-download` was dropped (2026-10-08), so no clone task consumes signing, entitlements or a pre-seal step.
[embedded-server-runtime](../../tasks/closed/20261005-embedded-server-runtime.md) keeps its first-launch unpack of
the server archive (main #215 re-signs nested Mach-O without its entitlements, so `t3` would lose its JIT
entitlements; see X4). The pre-seal hook was declined upstream (2026-10-08); scoped helper entitlements derived by
main would let the unpack go.

## Evidence and history
- Filed as [#119](https://github.com/ccheever/exact2/issues/119); closed by main #199 (`33aaa0b43`): `exact release`
  signs every nested Mach-O file and code bundle, innermost first, with the hardened runtime and a timestamp; #215
  reuses that order for local bundles. In the feature branch since main `463acda68`
  ([adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)); not adopted, no consumer.
- The rest filed as [#270](https://github.com/ccheever/exact2/issues/270) on 2026-10-08, reproduced on main
  `0365ad1a4`: `host.macos.entitlements` and `host.macos.preseal` are refused as unknown keys; `exact release` signs
  nested code with no entitlements (`scripts/exact.mjs:254-266`); `exact release` itself was not run (no Developer ID
  on the machine).
