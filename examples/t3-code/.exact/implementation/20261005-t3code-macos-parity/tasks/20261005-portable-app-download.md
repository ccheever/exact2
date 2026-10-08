---
name: 20261005-portable-app-download
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-portable-app-download
pr_url: https://github.com/ccheever/exact2/pull/260
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
| merged task PR | [20261005-local-primary-environment](closed/20261005-local-primary-environment.md) | pending | Merged: without it nothing in the UI connects to the unpacked server, so the clean-account run cannot show "This machine" | pending |
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

Implemented on `feat(example)/t3-code-portable-app-download` (2026-10-08) from `d82fb6a47`. Decisions:
U11 (decided: ad hoc signature, zip, clean macOS 14 VM). U2 (apparatus: `package-app.mjs`,
`audit-bundle.mjs`, `bundle-allowlist.json`, the `sandbox-exec` profiles) is open: built as the ticket
specifies, **provisional, user decision pending**.

| Scope item | Built | Where |
| --- | --- | --- |
| 1. Package step | `git archive` of a commit into the fixed `/tmp/t3-code-package/exact2` (outside every checkout; it names no user, see X54); two `sandbox-exec` phases that deny reading and writing this checkout, `~/.t3` and every `--deny` (the reference checkout, the mc-orch tree): the stage step (online, cache kept beside the export), then `bun install --offline`, the terminal page and `host/apple/build.mjs t3-code-macos --bundle --distribution` with outbound IP denied; `EXACT_IDENTITY=-` (ad hoc), Rust paths remapped (`CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS`), SwiftPM's nested sandbox off through a `swift` wrapper (`--disable-sandbox`; `sandbox_apply` cannot nest); then `strip -x` of every Mach-O, toolchain rpaths deleted, dylib ids `@rpath/<name>`, appIcon's empty `.icon-*` folder removed, `distribution.json` `{"flavor":"packaged"}` and `LICENSE-T3` written, ad hoc signature inside out (`signingOrder`), `codesign --verify --deep --strict`, `ditto -c -k --norsrc --noextattr --keepParent`, `SHA256SUMS`, then the audit | `package-app.mjs` (+ `package-app.test.ts`), `stage-runtime.mjs` (`T3_RUNTIME_CACHE`) |
| 2. Audit | Rules arch, minos, dylib, rpath, machine-path (home, checkout, `~/.bun`, `~/.t3`, the temporary folder, every `--forbid`; never allowed), build-path (generic shapes and `--build-root`; allowed only by an entry with its reason), dev-file (source maps, `app.contract.d.ts`, the TODO file, tests, fixtures, any hidden name, empty folders), unexpected (the allowlist's file list), runtime-part, runtime-tree (manifest equality, executables 0755, `codesign --verify --deep --strict` of every Mach-O, links in the folder, `.install-complete`), signature, info-plist, distribution | `audit-bundle.mjs` (+ `audit-bundle.test.ts`, 13 tests on clang-built synthetic bundles), `bundle-allowlist.json` |
| 3. First-launch view | "Setting up T3 Code…", "Checking the server files…" / "Unpacking the server files… N%" (N in 25 % steps: the live region), a 4-high bar that follows every report (translate, 160 ms; none under reduced motion), failure "T3 Code could not be set up" with the reason, Retry (autofocus) and Quit; covers the window and its toasts (z-index 200) and makes the base inert; disk space checked before tar (bytes + 4 KiB per entry) and after a tar failure; tar's last error line; `T3_LOCAL_UNPACK_DELAY_MS` holds each stage in a development build only; ops `localBackendRetry`, `localBackendQuit` | `first-launch.contract`, `first-launch.ts` (+ test), `pages-welcome.ts`/`.contract`, `app-overlays.contract`, `app.contract` (the `modal` line), `T3LocalRuntime.swift`, `T3LocalBackend.swift`, `T3Module+Local.swift`, `macos/tests/local-backend/install.swift` (4 new XCTests) |
| 4. Distribution notes | README "Distribution": requirements (Apple Silicon, macOS 14+), making the zip, opening it the first time (the `xattr` route first, then Open Anyway), first launch, standalone provider installers, where data lives, one app at a time on `~/.t3`, removal, licenses; "Source and checks" describes the audit | `README.md` |
| 5. Clean-account test and build isolation | The VM is blocked (none on this Mac); the substitute is below | `t3-code-evidence/portable-app-download/clean-environment-run.txt` |

### Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Isolated build | **Pass.** `9e9675e36` exported to `/tmp/t3-code-package/exact2`, built in 293 s under `sandbox-stage.sb` (stage, cache hit; the first sandboxed run downloaded it under the same profile) and `sandbox.sb` (offline: `(deny network-outbound (remote ip "*:*"))`), both denying this checkout, `~/.t3`, the reference checkout and the mc-orch tree. Zip `T3-Code-0.0.46-nightly.20261004.1-arm64.zip` 93,146,865 bytes, sha256 `5b3619a5…c75` | [clean-environment-run.txt](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/clean-environment-run.txt) §1 |
| Audit | **Pass, 0 findings** on the packaged app (27 files, 7 Mach-O; 30 allowed hits: 28 hermesc file names under the fixed export folder, X54; 2 `/Users/` literals of `terminal-links.ts`) and, after a first launch in a clean HOME, on app + runtime tree (4,318 files, 18 Mach-O, 4,290 files equal to the manifest, every Mach-O passes `codesign --verify --deep --strict`, executables 0755; 4,277 allowed hits, all the release's upstream bytes or literals, each with its reason). No machine path anywhere | §1, §5 |
| Archive integrity | **Pass.** `shasum -a 256 -c SHA256SUMS` OK; Archive Utility's unzip lists the same 27 files as the zip (no `._*`); `codesign --verify --deep --strict` valid; Signature=adhoc, no team | §2 |
| Clean first launch | **Blocked** for the VM: no VM tool or macOS image on this Mac (U11 needs a clean macOS 14 VM). **Substitute, pass headless:** download (local http server, quarantine written as Safari does), Archive Utility unzip (quarantine inherited), `spctl --assess`: rejected; the quarantined copy's `open` started no process (Gatekeeper; its alert is not on screen, the screen is locked); a second copy with the quarantine removed first starts in a clean HOME with no Bun/Node/codex/claude on the server's PATH: unpack 14.3 s (machine under load), ready with the bearer, `/.well-known/t3/environment` on 127.0.0.1:16356. **The alert, Open Anyway and Finder unzip: deferred to the real-input batch — screen locked (user away).** Found: a copy whose Gatekeeper refusal is unanswered stays at `_dyld_start` even after `xattr -dr` (README says to answer the alert first) | §2–§4, [05](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/05-packaged-first-launch.png) |
| Server identity equals the pin | **Pass.** `t3 --version` and the descriptor both `0.0.46-nightly.20261005.2667`; `t3` sha256 = manifest; manifest archive sha256 = pin = bundled archive; tag `v0.0.46-nightly.20261005.2667` → `37de6cbde65c` = the pin's commit; GitHub compare `1e2ecbd975...37de6cbde65c`: ahead 4, behind 0 | §5 |
| Works for the recipient | **Pass headless (agent mode on the packaged copy, clean HOME):** wizard lists This machine Connected; Agents: Claude "Not found · … not found on PATH", Codex "Continue with ChatGPT"; folder `~/work` added as a project, its thread draft open; Settings › Connections shows This machine; Providers: both not found. Standalone `claude.ai/install.sh` into the clean HOME (225 MB, scratch, deleted after), then a relaunch: Claude v2.1.293 found. It reported Authenticated because the clean HOME shares this Mac's login Keychain (the user's own Claude Code item): no message sent; a separate account or VM is what isolates it. Sending with the recipient's account (optional) not run | §6, [06](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/06-packaged-recipient.png) |
| Quit and relaunch | **Pass.** A quit Apple event to the pid (as ⌘Q): server gone 2.76 s, app 2.81 s, nothing on the port; relaunch: no `installing`, ready with the bearer in 1.38 s, one server | §5 |
| Failure paths | **Pass headless.** 48 MB disk image as `~/.t3`: "There is not enough disk space to set up T3 Code: it needs 278.2 MB in …/runtime/versions, and 49.6 MB is free.", Retry fails again, Quit quits, nothing half-installed. One byte flipped in the bundled archive: "…does not match its pinned SHA-256.", Retry fails again, Quit quits. `kill -9` during the unpack: the next launch removes `.staging-*` and installs | §7, [08](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/08-packaged-failures.png) |
| Design check of the first-launch view | **Pass.** Column 360 wide, centered (x 460 at 1280, 240 at 840), gap 12, title 20/20/600, status 14/20, bar 4 high at y 450 in all three stages (0 px shift), buttons 32 high, a 130-character reason wraps to 4 lines inside the column (342 px of 360); contrast from pixels 14.52, 4.71, 4.65 (light) and 18.16, 7.04, 6.85 (dark) | [design check](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/first-launch-design-check.txt), [01](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/01-first-launch-1280-light.png)–[04](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/04-first-launch-failure.png) |
| Accessibility of the first-launch view | **Partly.** Retry has the initial focus; Tab: Retry → Quit; heading and stage text in the AX tree; the stage text changes only per stage and per 25 % (unit test). The macOS host does not expose `progressbar`, `status`/`aria-live`, `alert` or the modal dialog (new gap **X55**; the value: X49): the Contract tree carries them. VoiceOver spot check: deferred to the real-input batch — screen locked (user away) | design check §Accessibility |
| "This machine" pairs | **After only (pass):** the packaged clean copy's Settings › Connections at all four cells; the section shows the same rows as the pairs accepted in local-primary-environment (this task does not change it) | [07](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/07-packaged-this-machine-cells.png), [06](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/06-packaged-recipient.png) |
| Removal | **Pass in the clean HOME:** the README's commands removed the app, `runtime/versions/<version>`, both Application Support folders and the caches; `statev2.sqlite` (projects, threads) and the settings stayed. The Keychain step was not run (the login Keychain is shared with other lanes; these runs saved no item) | §7 removal |

Real data: `~/.t3` and `~/.t3/userdata` mtimes, the real preferences plist and port 3773 were unchanged across the session; T3 Code (Nightly) was not running.

## Real-input batch steps

Screen locked (user away) for these; run them in one session with the real-input lock held:

1. **Gatekeeper alert and Open Anyway** (macOS 26 here; macOS 14 needs the VM). Build: `bun examples/t3-code/package-app.mjs --out <dir> --deny ~/Documents/work/3.open-source/t3code --deny ~/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805` (or reuse the zip, sha256 `5b3619a5525b65535e715e3233f8d3d1f31b8f83f2e85a2f513f267cd7173c75`). Serve it: `cd <dir> && python3 -m http.server 16353 --bind 127.0.0.1`; download `http://127.0.0.1:16353/T3-Code-…-arm64.zip` in Safari; double-click it in Finder. Open the app (a distinct copy, e.g. renamed "T3 Code (Lane PAD).app", launched with `open -n --env HOME=<clean> --env CFFIXED_USER_HOME=<clean> --env T3CODE_PORT=16352 --env SHELL=<clean login shell>`). Read back: the "Apple could not verify" alert (screenshot), choose **Done**; System Settings › Privacy & Security › **Open Anyway** (needs the user's password: stop there if it is asked and record it); then the app starts translocated or not (`ps` path under `/private/var/folders/…/AppTranslocation/` or not), "Setting up T3 Code…" visible in the window (`screencapture -l`), then This machine Connected.
2. **The pending alert from this session**: `/private/tmp/t3-clean/home/Applications/T3 Code (Exact).app` was opened once while quarantined (2026-10-07 18:50 UTC); its alert may be waiting behind the lock. Answer **Done** (or **Move to Trash**) for that app only.
3. **VoiceOver spot check**: lane dev build with `T3_LOCAL_HOME=<empty>`, `T3_LOCAL_PORT=16351`, `T3_LOCAL_UNPACK_DELAY_MS=6500`; turn VoiceOver on (⌘F5), listen for the stage line and, with `T3_LOCAL_HOME=/Volumes/T3LANESMALL` (48 MB image), the failure; Tab Retry → Quit with the focus ring visible. Expected today: the heading and text are read; the live region and the alert are not announced (X55).
4. **Clean macOS 14 VM** (U11): needs a VM tool and a macOS 14 image, neither on this Mac. Ask the user before installing one (tart/UTM and a ~15 GB image). Then repeat the clean-environment run (`clean-environment-run.txt` §2–§7) inside it.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `a9f600c04` | Package `--no-sandbox`: 42 audit findings (hermesc names, rlib paths in two dylibs, toolchain rpaths missed because otool reads `T3 Code (Exact)` as an archive member) | lane logs | fixed in attempt 2 |
| 2 | `9b7905e2b` | Sandboxed package: SwiftPM `sandbox_apply: Operation not permitted` (nested sandbox) | lane logs | fixed with the `swift` wrapper |
| 3 | `85a67be2b` | Sandboxed package: 0 findings, 30 allowed | lane logs | — |
| 4 | `49d1e21ac` | Design drives d1–d4, f1–f2, m1–m2: pass; the Nightly toast covered the first-launch view (z-index 60 < the toast region's 100): fixed (200) | [01]–[04] | — |
| 5 | `49d1e21ac` | Clean prep: the zip carried `._*` AppleDouble entries (macOS 26 provenance xattrs) and appIcon's empty `.icon-*` folder, which the audit did not catch | lane logs | fixed in attempt 6 (`--norsrc --noextattr`, the folder removed, the audit flags hidden names and empty folders) |
| 6 | `9e9675e36` | Final package and clean-environment substitute run; screen locked: GUI rows deferred | [clean-environment-run.txt](https://raw.githubusercontent.com/ccheever/exact2/df11fb0cc934f4a7c91233950759cbaf8b4b3cfa/portable-app-download/clean-environment-run.txt), [05]–[08] | VM (none on this Mac), real-input batch |

## Next action

Review the draft PR. The user answers U2 for the apparatus, and whether to install a VM tool and a
macOS 14 image for U11's clean VM row. The coordinator runs the real-input batch above. Re-run the
package, the audit and the clean-environment run as the plan's last step (later tickets add bundled files).
