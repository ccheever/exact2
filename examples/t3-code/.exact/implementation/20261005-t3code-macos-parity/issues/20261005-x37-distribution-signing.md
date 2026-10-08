---
name: 20261005-x37-distribution-signing
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap (unconfirmed)
blocks: [20261005-portable-app-download]
upstream_url: https://github.com/ccheever/exact2/issues/119
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/270
---

# X37: Developer ID signing, notarization and a pre-seal hook in the host build

## Summary

T3 Code ships to users as a signed, notarized Apple Silicon app when the project's signing secrets are present. The clone must run on another person's Mac, so the Gatekeeper experience of its downloadable `.app` is part of the goal.
exact2's macOS host build has no documented way to sign with a Developer ID identity, enable the hardened runtime, notarize, or add a file to the finished bundle before it is sealed (not in `EXACT2-GAPS.md`; to confirm at `issue-open`).
The plan's workaround is an ad-hoc signature with a documented one-time approval, or an app-local script that signs and notarizes the finished bundle. The clone needs the host build to support the signed path, or the user must decide that ad-hoc is enough (decision U11).

## Why this issue arose

### The T3 Code behavior
- The release flow builds, signs and notarizes the macOS desktop app and publishes it. "Omitting signing secrets only makes platform artifacts unsigned" (`docs/operations/release.md:436-437`).
  The required secrets are a Developer ID certificate export (`CSC_LINK`, `CSC_KEY_PASSWORD`), an App Store Connect API key (`APPLE_API_KEY`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`), `APPLE_TEAM_ID` and a provisioning profile for Associated Domains (`release.md:440-478`).
  The profile serves desktop passkeys for T3 Connect sign-in (issue X38), so a clone without T3 Connect does not need it.
- The desktop packager signs through a custom hook, `scripts/sign-macos.ts` (a wrapper over `@electron/osx-sign` with batched `codesign` calls), only when signing is enabled
  (`scripts/build-desktop-artifact.ts:1578,2732`, `T3CODE_DESKTOP_SIGNED`; `:3795-3834` clears the signing variables for unsigned builds).
  The build writes an `entitlements.mac.plist` only for the passkey case (`:3651-3669,3713-3715`). The macOS build also declares the schemes `t3code` and `t3code-dev` and a screen-capture usage text (`:2720-2731`).
- The server archive that the desktop and the CLI use is signed too. Every `.node` file, every `.dylib`, `spawn-helper`, `t3-resource-monitor` and the `t3` executable are signed one by one. A real identity adds the hardened runtime and a timestamp.
  The executable gets entitlements (`apps/server/resources/cli-entitlements.plist`: JIT, unsigned executable memory, executable page protection off, dyld environment variables, library validation off).
  Without an identity the archive is signed ad hoc, "which still runs from `curl`/`tar` installs" (`scripts/build-cli-archive.ts:295-368`; `release.md:43`).
  Notarization runs on a zip of the executable, because a bare executable cannot be stapled (code comment in `build-cli-archive.ts`, same range).
- Observed on this Mac on 2026-10-05 (`ls` only): the installed T3 Code (Nightly) bundle contains `Contents/embedded.provisionprofile`.
- Not traced for this issue: what a user sees when they open a downloaded T3 Code for the first time (Gatekeeper prompt text).

### What exact2 does today
- Not in `EXACT2-GAPS.md`. Bundled library (`20261005-platforms-v3`): bundling, signing and Gatekeeper are **not covered: unknown**.
- The repository's `CLAUDE.md` (project instructions, read at the start of the session) says of delivery: "on a Mac it signs production macOS Rust modules, so set `EXACT_RUST_SIGN_IDENTITY` to an identity listed by
  `security find-identity -v -p codesigning` (an Apple Development one does)", and that `--device --run` signs an iPhone build "with a team profile on this Mac". This covers Rust modules for delivery and device runs.
  It does not say what the macOS `.app` from `host/apple/build.mjs` is signed with, whether the hardened runtime is on, or whether any notarization step exists. **To confirm at `issue-open`** (the portable ticket reads `codesign -dv` on the built bundle at `prepare`).
- Observed in the clone on 2026-10-05: no signing script, no entitlements file, no `host.macos` signing field in `app.json` (bundle id `com.exact.t3code.macos`, `minimumOS` 14.0).

### Where the clone hits it
`20261005-portable-app-download` builds a zip that another person unpacks and opens. Its package step writes `Contents/Resources/distribution.json` into the finished bundle before signing, then zips with `ditto -c -k --keepParent`.
The ticket lists two choices (decision U11):
(a) Ad-hoc signature. The recipient approves the app once. On macOS 15 and later the right-click "Open" shortcut no longer bypasses Gatekeeper, so the steps are System Settings > Privacy & Security > "Open Anyway", or `xattr -dr com.apple.quarantine <app>` (to verify on the test machine).
The ticket also records that each rebuild gets a new identity, so Keychain items written by an older build can prompt and the Screen Recording and Accessibility grants for SnapShot are asked again.
(b) Developer ID signature and notarization with an app-local script: it needs the user's Apple Developer team, certificate and notary credentials (names only in the plan, never secrets).
Differences from the original: its release is signed and notarized when secrets exist; the clone under (a) asks every recipient for a manual approval, and under (b) depends on a script that must know the bundle layout the host build produces.

## Why it must be resolved
The goal is a downloadable clone that works on another person's Apple Silicon Mac with nothing else installed. The first thing that recipient meets is Gatekeeper.
Under (a) every recipient must follow documented steps, and every rebuild resets Keychain and permission identity. Under (b) the script must sign nested code inside-out, keep the hardened runtime and timestamps, add `distribution.json` before the seal, and survive changes to the host's bundle layout,
all without framework support. When issue X4 is resolved and the server tree moves into the bundle, the nested code (the `t3` executable, every `.node`, `spawn-helper`) must be signed again or verified, so the script grows.
`20261005-portable-app-download` carries the choice; its Gatekeeper rows, README steps and clean-account run depend on it. If the user picks (a) only, this issue closes by that decision.

## Requested support
Web analogy: none. A web app has no code signature; the closest idea is HTTPS plus Subresource Integrity, which does not apply. The request is native-specific, on the macOS host build first.
- **A (preferred).** In `host/apple/build.mjs`: a signing identity option (flag or environment variable) that signs the `.app` and all nested code inside-out with the hardened runtime and a secure timestamp;
  entitlements declared in `app.json` (for example `host.macos.entitlements`); a notarization option that submits with `xcrun notarytool`, waits, and staples; and a documented pre-seal hook (a directory of extra files, or a script path) so an app can add `distribution.json` before signing.
  Without the option the build stays as it is today.
- **B.** Document the output layout and a supported recipe (order of signing, required flags, entitlements the host needs under the hardened runtime, the place for extra files). The app owns the script. This costs the framework less but leaves the layout as a promise the framework must keep.
- The entitlements the exact2 host itself needs under the hardened runtime are part of the answer. They are not established here.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Build the clone or a minimal app bundle: `EXACT_APP_DIR=<app> bun host/apple/build.mjs <crate> --bundle`. Run `codesign -dv --verbose=4 <app>` and record `Signature`, `flags` (is `runtime` present?), `TeamIdentifier`, `Authority`.
2. Run `codesign --verify --deep --strict <app>` and `spctl -a -t exec -vv <app>`. Add a file to `Contents/Resources` and run the verify again.
3. Search the build script's options for a signing identity, entitlements and notarization. Record that none exists, or what does.
4. Expected (reference behavior, signed release): Developer ID authority, `flags` with `runtime`, `spctl` says "accepted" with "source=Notarized Developer ID", `stapler validate` passes.
   Actual (to confirm): the result of steps 1 to 3 on the host build.
Do not publish or upload anything during the check. Use a local `python3 -m http.server` on a lane port for the quarantine test.

## Acceptance for the fix
- With an identity set: the built `.app` passes `codesign --verify --deep --strict`; `flags` include `runtime`; every nested Mach-O file has a timestamp and the same Team ID as the app (or keeps its own verified signature, as in the X4 case).
- With a notary profile set: `spctl -a -t exec -vv` reports "Notarized Developer ID"; `xcrun stapler validate` passes; the stapled zip opens on a clean macOS 14 account without "Open Anyway" after download through Safari (the clean-account row of the portable ticket).
- A file added through the pre-seal hook is inside the seal (`--strict` passes) and an app without the hook is unchanged.
- Without an identity the build stays ad hoc and every existing check passes.
- The app launches under the hardened runtime and its child server starts (the child is signed by its own team; confirm that it runs).

## App adoption after resolution
`20261005-portable-app-download`: replace the app-local signing script with the supported option; the README Gatekeeper section loses the manual steps; decision U11 is recorded as Developer ID; add the `spctl` and `stapler` rows to the audit and the clean-account run.
Keep ad-hoc as the fallback when no identity is present. `issue-close` checks the four acceptance rows on the pinned `main`. If the user chooses ad-hoc only, close this issue by decision and keep the documented approval steps.

**Decision U11 (2026-10-05):** the plan ships an ad-hoc signed zip, so this issue is not needed
for the plan's download. It stays open only for a later Developer ID build; at `issue-open`, ask
the user whether to keep it or close it by decision.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval), after the signing decision (U11) is answered at `prepare` of `20261005-portable-app-download`.

## Partly fixed upstream; available, not adopted here (2026-10-07, adopt-main-fixes-r4)

[#119](https://github.com/ccheever/exact2/issues/119) was closed by main #199 (`33aaa0b43`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)):
`exact release` signs every nested Mach-O file and code bundle, innermost first, found by its magic bytes,
with the hardened runtime and a timestamp; #215 reuses that order for local bundles with
`host.macos.resources`. Not on main (#199's open points): app-declared entitlements
(`host.macos.entitlements`; nested code is signed with none, so a helper that needs JIT under the hardened
runtime cannot get it), a pre-seal hook, and a decision on code under `Contents/Resources`; main's QUEUE also
notes that a Mach-O asset is signed after the bake hashed it. Developer ID signing, notarization and
stapling were not run upstream (no identity). Not adopted here: `20261005-portable-app-download` uses it.

## Rest filed upstream (2026-10-08)

Upstream (the rest): https://github.com/ccheever/exact2/issues/270 (#270, [Design] macOS release: entitlements for the app and its nested code, and a pre-seal step (rest of #119)). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) before filing: `host.macos.entitlements` and `host.macos.preseal` are refused as unknown keys; `exact release` signs nested code with no entitlements (`scripts/exact.mjs:254-266`); `exact release` itself was not run (no Developer ID on the machine). LLP 1069.008 derives entitlements from grants; one "Decision needed" comment. Searched open and closed issues and PRs: no duplicate.
