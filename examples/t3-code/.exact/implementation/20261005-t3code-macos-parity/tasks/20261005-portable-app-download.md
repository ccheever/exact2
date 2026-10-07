---
name: 20261005-portable-app-download
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# A downloadable `.app` archive that runs on another person's Apple Silicon Mac

## Outcome

One command builds a `T3-Code-<version>-arm64.zip` (plus a checksum file) from a clean export of the integration branch. Another person with an
Apple Silicon Mac on the stated minimum macOS (14, per the spec), and nothing else installed (no exact2 repo, no T3 Code checkout, no Bun, no Node, no T3), unzips it, passes
Gatekeeper with the documented steps, and uses the app: first launch unpacks the server runtime with visible progress, "This machine" connects, and the
app works with the recipient's own provider CLIs. No store, update feed or hosted web root is involved (deployment is out of scope).

## Scope and exclusions

Included:

1. **Package step** `package-app.mjs` (apparatus needing approval; in `20261005-embedded-server-runtime` this is the step that makes a build "packaged"): export the branch with `git archive` into a fresh directory with a different path from the dev tree, run the pinned stage step
   (`20261005-embedded-server-runtime`), build the bundle with `host/apple/build.mjs t3-code-macos --bundle` (the exact flags, build flavor and output path are read from the build at `prepare`; the library
   does not cover them), then write `Contents/Resources/distribution.json` (`{"flavor":"packaged"}`) into the finished bundle before signing (without it the build is a development build that refuses the real `~/.t3` and port 3773), then `ditto -c -k --keepParent` (keeps modes and links) and write `SHA256SUMS`. Intel Macs are not supported (the server runtime is arm64 only; there is no darwin-x64 CLI archive,
   `docs/operations/release.md:40`): `app.json` minimum OS stays at the stated minimum (see the clean-environment decision), and the README says "Apple Silicon only".
2. **Self-containment audit** `audit-bundle.mjs` (+ tests on synthetic bundles) over the built `.app` and over the tree a first launch unpacks in a scratch `HOME`; it fails on any of:
   - architecture other than arm64 (`lipo -archs`), `minos` above the stated minimum (`vtool -show-build`);
   - a dependent library outside `/usr/lib` and `/System` (`otool -L`), or an absolute `LC_RPATH`;
   - a build-machine path in any file: `/Users/`, `/private/var/folders`, `/opt/homebrew`, `/Volumes`, `.bun`, `DerivedData`, the export directory, the reference or mc-orch paths (`grep -a`; each allowed hit is listed in `bundle-allowlist.json` with its reason);
   - a leftover dev file: source maps (`*.map.json`), `app.contract.d.ts`, `UI-PARITY-TODO.tmp.md`, test fixtures, `.git`;
   - generated parts whose SHA-256 differs from `runtime-manifest.json`, or a manifest that differs from `runtime-pin.json`;
   - after the scratch first launch: `t3`, `node-pty`'s `spawn-helper` and every other entry the manifest marks executable are not 0755; `codesign --verify --deep --strict` fails on `t3` or any `.node`; a symlink points outside the runtime folder;
   - `codesign --verify --deep --strict` of the app itself fails, or `Info.plist` lacks `LSMinimumSystemVersion` equal to the stated minimum, `distribution.json` is missing or not `packaged`, or has a bundle id other than the example's (`com.exact.t3code.macos`, not the original's `com.t3tools.t3code`).
3. **First-launch experience** (new UI; the reference has nothing to unpack). Content and expected values (a design check, not a pair with the oracle):
   - A centered column, `max-width` 360, `gap` 12, `padding` 16 or more at 840×620: title "Setting up T3 Code…" (20 px, line height 20, weight 600, `light-dark(#27272a, #f5f5f5)`), a status line (14 px/20, `light-dark(#71717b, #9a9a9a)`):
     "Checking the server files…" then "Unpacking the server files… N%", and a progress bar 4 pt high, radius 2, track `light-dark(#27272a14, #f5f5f514)`, fill `light-dark(#18181b, #f5f5f5)`, full column width. These are the app's existing text and surface tokens.
   - No layout shift: the bar's top does not move (0 px) when the status text changes; a 120-character error wraps inside the column and nothing is clipped.
   - Failure (not enough disk space, damaged part, interrupted): the title becomes "T3 Code could not be set up", the error text uses `light-dark(#e7000b, #ff6467)`, buttons Retry (primary) and Quit (32 pt high, `autofocus` on Retry); a second launch while unpacking waits for the first; an interrupted unpack restarts cleanly.
   - Reduced motion: the bar fill changes in steps without animation. Size and time of the unpack are recorded.
   - Text contrast at least 4.5:1 against the window background in light and dark, measured from screenshot pixels.
4. **Distribution notes** (README "Distribution" section, not a new file): requirements; the Gatekeeper steps for the chosen signing; where data lives (`~/.t3`, the app's data folder, Keychain, `~/.t3/runtime/versions/<version>`); that provider CLIs are the user's own; that T3 Code (Nightly)
   and this app must not run at the same time on the same `~/.t3`; how to remove everything; license notices (T3 Code MIT, `LICENSE-T3`, runtime notices).
5. **Clean-account test** (record the environment chosen below) and a build-isolation check: the build runs under a `sandbox-exec` profile that denies reading the reference checkout, the mc-orch tree, the dev worktree and `~/.t3`.

Excluded: notarization or Developer ID unless the user picks it; a DMG; Sparkle or any update channel; Intel; Windows or Linux; testing with the recipient's real provider accounts (attended, optional).

## Context and guidance

Parent specification: [spec](../spec.md) (Delivery row). Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23); this ticket's own scripts (`package-app.mjs`, `audit-bundle.mjs`) live in the example directory.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`);
the clean-account rows run the packaged build, which defaults to the recipient's own `~/.t3` inside the clean environment, never this Mac's real home.
Provider CLIs are the recipient's own. A standalone install that needs no Node is the reference's own onboarding list (`apps/web/src/onboarding/providerReadiness.logic.ts:77-90`, "Neither needs Node or npm"): Claude `curl -fsSL https://claude.ai/install.sh | bash`,
Codex `curl -fsSL https://chatgpt.com/codex/install.sh | sh`; the README names them and the clean-account run uses them. Their current pages are checked at `prepare`.
Library revision: `20261005-platforms-v3`. Selected topics: platforms (macOS: build before driving; record OS, device, build identity), testing-and-debugging (static evidence is not runtime evidence; keep failed attempts), capabilities.
Bundling, signing, Gatekeeper and App Translocation are **unknown in the library**; the pinned main's build output and the clean-account run are the evidence.
Decision U11 (2026-10-05, the recommended options): **ad-hoc signature, a zip archive (made with
`ditto`), tested on a clean macOS 14 VM.** Consequences to document in the README and check:
the recipient approves the app once — on macOS 15 and later through System Settings > Privacy
& Security ("Open Anyway"), or `xattr -dr com.apple.quarantine <app>` (verify on the test
machine); each rebuild gets a new identity, so Keychain items written by an older build can
prompt and SnapShot's Screen Recording / Accessibility grants are asked again. A new user
account on this Mac is still useful as a second run (nothing in its home, the build tree
unreadable to it). Unpack location: `<T3 home>/runtime/versions` (U3). Read what the host build
signs by default (`codesign -dv`) at `prepare`.
Edge cases to cover: a quarantined app is run from a randomized read-only location (App Translocation), so nothing may write inside the bundle or assume its path; the extracted runtime files carry no quarantine attribute and must still run; `$HOME` differs from the build machine; a recipient with the original app installed (only one runs at a time).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-embedded-server-runtime](closed/20261005-embedded-server-runtime.md) | pending | Merged; spike go | pending |
| merged task PR | [20261005-terminal-surface](closed/20261005-terminal-surface.md) (vendored terminal assets enter the bundle) | pending | Merged | pending |
| merged task PR | [20261005-local-primary-environment](20261005-local-primary-environment.md) | pending | Merged: without it nothing in the UI connects to the unpacked server, so the clean-account run cannot show "This machine" | pending |
| recorded decision | Signing and clean environment (U11, decided: ad-hoc, zip, clean macOS 14 VM); apparatus approval (U2, open) | none | U2 answered at `prepare` | U11: user 2026-10-05 |

Re-run the audit and the clean-account test as the last step of the plan, because later tickets add bundled files (terminal fonts, sounds, icons).

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X4](../issues/20261005-x04-bundle-helper-executables.md) | Large trees, executables and file modes in the `.app` | `EXACT2-GAPS.md` X4; #103 closed by main #215: `host.macos.resources` copies a tree with modes, symlinks and any names into `Contents` and signs it (recorded in [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md)) | none expected (replaces the archive workaround; Developer ID signing of the tree unverified upstream) | The runtime spike decides; if it fails this ticket is blocked too |
| [X37](../issues/20261005-x37-distribution-signing.md) | Developer ID signing and notarization in the host build | #119 closed by main #199: `exact release` signs every nested Mach-O and bundle inside-out; no app-declared entitlements or pre-seal hook (recorded in [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md)) | depends on U11 (ad hoc needs nothing; Developer ID: `exact release`, plus an app-local step only for entitlements or notarization of what #199 leaves out) | Read the build output at `prepare`; ask the user per the signing decision |
| [X31](../issues/20261005-x31-deferred-window-readiness.md) | A view before any server exists | not in the library | nonblocking (the first-launch view is an ordinary first-window state) | none |

## Implementation notes

- The first-launch view reads `localBackendStatus.install` from `20261005-embedded-server-runtime`; the hold seam `T3_LOCAL_UNPACK_DELAY_MS` exists only in the development flavor.
- The audit is the contract: add a rule whenever a clean-account run finds a leak the audit missed.
- Keep the package step free of network after the stage step; the stage cache lives in `target/`.
- Do not sign inside the archive step with a private key from the repository; the identity comes from the user's keychain at run time.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Isolated build | `git archive` export in a new directory; `sandbox-exec` profile denying the reference checkout, mc-orch, the dev worktree and `~/.t3` | `bun package-app.mjs` | Succeeds with only the staging network access; zip and `SHA256SUMS` written | macOS 26.6.2, Apple Silicon | build log, profile |
| Audit | The built `.app` and a scratch-`HOME` first launch | `bun audit-bundle.mjs` | Zero findings or only allowlisted hits with reasons; includes exec bits, signatures of `t3` and every `.node`, arm64, `minos` | macOS | audit report |
| Archive integrity | Unzip into another directory | `shasum -c`, `codesign --verify --deep --strict`, `diff` of the file list with the allowlist | Pass | macOS | log |
| Clean first launch (attended session) | The clean environment chosen in U11, running the stated minimum macOS (a VM or second Mac for 14); the zip downloaded through Safari from a local `python3 -m http.server` on a lane port (16xxx) so it is quarantined | Unzip in Finder, pass Gatekeeper with the documented steps, open | Progress view, then "This machine" connected; no repo, reference, Bun, Node or T3 path touched | macOS at the stated minimum (record version and device) | screen recording, `fs_usage -w -f pathname` (sudo) shows only allowed roots |
| Server identity equals the pin | The clean environment after first launch | Run the unpacked `t3 --version`; read `/.well-known/t3/environment`; `shasum -a 256` of `t3` and every manifest entry | The version string equals `runtime-pin.json` `version` both ways; every file hash equals `runtime-manifest.json`; the manifest digest chain ends at the pin's archive SHA-256; the pin's `sourceCommit` is the commit the release tag points to (checked at `prepare` from the release record) and has `1e2ecbd975` as ancestor or equal (read-only check in the reference repo). The descriptor and `apps/server/src` expose no commit (grep for `commitHash` found none), so the commit is a pin-time record | macOS | command log |
| Works for the recipient | Same | Add a folder as a project, open a thread, open Settings > Connections; then install a CLI with the standalone installer above (no Node on the machine; attended, network) and press refresh | Project listed; providers show the reference's not-installed states, then ready after the install; with the recipient's own account a message streams (optional) | macOS | screenshots, `command -v node` empty |
| Quit and relaunch | Same | Quit, check processes, relaunch | No `t3` left; second launch skips the unpack and connects within the normal start time | macOS | `pgrep -u`, timings |
| Failure paths | Same | Fill the disk quota (small disk image as `~/.t3`), kill during unpack, corrupt one part | Visible messages with Retry; no half runtime used; next launch recovers | macOS | recordings |
| Design check of the first-launch view | Lane build (`T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>`); the agent-only seam `T3_LOCAL_UNPACK_DELAY_MS` holds each stage | `layout` of the title, status, bar and buttons; screenshots at 1280×840 and 840×620, light and dark; a forced 120-character error | Every value listed in item 3: sizes, colors, 360 max width, 16 pt padding, bar top moves 0 px between stages, error wraps without clipping, contrast at least 4.5:1 from screenshot pixels, buttons 32 pt | macOS | `layout` JSON, png, pixel samples |
| Accessibility of the first-launch view | Same | `tree --ax` in each stage and in failure; `prefer prefers-reduced-motion reduce`; Tab | Bar has `aria-label` "Setting up T3 Code"; the status line is `aria-live="polite"` and changes only on a stage change and every 25% (no per-frame announcements); the error is `role="alert"`; Retry has initial focus (Tab order Retry, Quit; visible focus ring); the bar fill steps without animation under reduced motion; a VoiceOver spot check announces the stage and the failure (attended session) | macOS | `tree --ax`, recording |
| "This machine" pairs | Same fixture as the matrix | 1280×840 and 840×620, light and dark | The "This machine" section equals the pairs accepted in `20261005-local-primary-environment` | macOS | png |
| Removal | Same | Follow the README removal steps | App, data folder, Keychain item and `~/.t3/runtime/versions/<version>` removed; projects, threads and settings in the shared `~/.t3` left alone | macOS | listing |

Task-owned source paths: `package-app.mjs`, `audit-bundle.mjs` (+ tests), `bundle-allowlist.json`, `README.md` (Distribution), first-launch view (`.contract` + TS), `app.json`, `.gitignore` lines for `target/` outputs.
Required environment: Xcode 27.0, pinned Bun and Hermes tools on the build machine only; for signing option (b) the user's team and notary credentials (names only, no secrets); a clean account, VM or second Mac.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the dependencies merge and the two decisions are answered. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
