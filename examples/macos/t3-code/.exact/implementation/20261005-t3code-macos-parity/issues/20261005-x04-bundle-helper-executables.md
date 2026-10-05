---
name: 20261005-x04-bundle-helper-executables
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-embedded-server-runtime, 20261005-portable-app-download, 20261005-this-machine-network-access]
upstream_url: null
reproduced_on: null
---

# X4: Helper executables and large resource trees in the `.app`

## Summary

The T3 Code desktop app ships its own T3 server and runs it as a child process. The server is a program with native add-ons, not data.
exact2's macOS bundle carries only plain files under `assets/`, with restricted names and mode `0o644`, so it cannot carry a signed executable plus a Node-style `node_modules` tree.
The clone has no embedded server yet. The plan works around the limit by splitting the official archive into parts and unpacking them at first launch. The clone needs a supported way to ship executables and resource trees, with modes and names intact, inside the signed bundle.

## Why this issue arose

### The T3 Code behavior
- The desktop app starts a local server at launch and keeps it for the app's lifetime. That server is the "This machine" environment. The command is built in
  `apps/desktop/src/backend/DesktopBackendConfiguration.ts:538-595` and the server entry is found inside the packaged app
  (`apps/desktop/src/app/DesktopEnvironment.ts:174,218`, `backendEntryPath` = `<serverRoot>/apps/server/dist/bin.mjs`).
- Observed on this Mac on 2026-10-05, by `ls` and `du` of `/Applications/T3 Code (Nightly).app/Contents/Resources` (names and sizes only):
  `app.asar` 104 MB; `app.asar.unpacked` 9.7 MB (`@clerk`, `@crowecawcaw`, `@ff-labs`, `@napi-rs`, `@yuuang`, `node-pty` with `build/Release/pty.node` and `spawn-helper`);
  `node_modules` 9.9 MB (`@cursor`); `resource-monitor` 552 KB; `app-update.yml`. `Contents` also holds `embedded.provisionprofile`.
  The reference build keeps some helper assets outside `app.asar` on purpose: "so spawning helpers and loading native addons both use real paths"
  (`scripts/build-desktop-artifact.ts:945-946`, about the `@cursor/sdk-*` packages).
- The server is also published as a self-contained archive, `t3-<version>-<platform>-<arch>.tar.gz`. It holds a Node single executable `t3`, `client/`, `resource-monitor/` and `node_modules/`
  (`scripts/build-cli-archive.ts:1-16`; `docs/operations/release.md:40-41`). Release builds sign every native add-on in the macOS archive, "since the hardened runtime refuses unsigned libraries"
  (`release.md:43`; `scripts/build-cli-archive.ts:295-368`). The server executable carries its own entitlements (`apps/server/resources/cli-entitlements.plist`: JIT and executable memory).
  The archive size is not measured here (no network).
- Failure states in the reference: the server cannot start (`BackendProcessSpawnError`, `DesktopBackendManager.ts:158`), then a restart with a growing delay (`calculateRestartDelay`, `:347`).
  A server file without its executable bit, or an add-on with a broken signature, shows up as exactly this failure.

### What exact2 does today
Quoted from `examples/macos/t3-code/EXACT2-GAPS.md` section X4 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`; not re-measured for this plan):
- "`assets/` is the only way to add files to the `.app` (`host/apple/build.mjs` `copyAppleStaticTrees`)."
- "Path segments must match `[A-Za-z0-9._-]` (`update/src/envelope.rs:449-467`). 1,007 of 4,178 runtime files fail this rule (`@scope` folders and others)."
- "Files are written with mode `0o644` (`filesystem/src/directory.rs`). Executables (`node`, `spawn-helper`, `rg`) lose their exec bit."
- "The bake sends every asset as base64, with a 256 MiB buffer (`scripts/filesystem.mjs`). The runtime is 134 MB before Node."
- "`app.json` has no field for helper executables."
- From section X1 of the same file: "`host/apple/build.mjs:1021-1045`: module dependencies link only as static libraries or framework slices" and
  "`host/apple/build.mjs:1229-1244`: the macOS bundle copies only exact's binaries and `assets/`. No `Contents/Frameworks` for third-party frameworks, and no helper apps."
- The document does not state a per-asset size limit. Its plan uses parts of "≤ 60 MiB". The real limit is **to confirm at `issue-open`**.
- Bundled library (`20261005-platforms-v3`): bundle assets and helper executables are **not covered: unknown**.
- Observed in the clone on 2026-10-05: `examples/macos/t3-code/assets/` holds five files (four `.mp3` sounds and one `.png` provider icon); `app.json` has no host field for helpers or resources.

### Where the clone hits it
The clone's README says "The app does not bundle or modify T3 Code." (`README.md:7`). That changes: `20261005-embedded-server-runtime` must ship the official server.
The notification code already reads bundled files from `Resources/assets/` (`modules/apple/T3Notifications.swift:76-80`), which shows where `assets/` lands.
The workaround in the plan: split the official archive into parts below the per-asset limit, named `assets/t3-runtime.NNN`; verify and unpack them into `<T3 home>/runtime/versions/<version>` at first launch;
restore modes and links from a manifest; check signatures after unpacking. Differences a user sees: a first-launch "Setting up T3 Code…" view and a wait of unknown length; a second copy of the runtime on disk;
failure states the reference does not have (low disk, damaged part, interrupted unpack). The original app has no first-launch step.

## Why it must be resolved
The goal is a full clone, and the embedded server is the base of it: "This machine", the Local environment switch, Network access, authorized clients and `t3 app` all depend on it.
Without a supported way to ship it, `20261005-embedded-server-runtime` rests on three unmeasured points: the parts fit the bake buffer after base64, modes and links can be restored, and signatures survive.
`20261005-portable-app-download` must run on another person's Mac, so the unpacked files must keep valid signatures and must not add a Gatekeeper prompt.
The workaround adds code (manifest, installer, lock, staging, recovery) and risk that a bundle resource tree would not have. If the first measurement fails, both tickets stay blocked.

## Requested support
Web analogy: none. A web app cannot carry executables. The request is native-specific, on the macOS host first. iOS cannot spawn processes; the other hosts are not used by this example.
- **A (preferred).** An `app.json` field (for example `host.macos.resources`: a list of source directories copied into `Contents/Resources/<name>` or `Contents/Helpers/<name>`).
  It keeps file modes and symlinks, accepts any file name (including `@` and spaces) and large files, and brings the tree into the bundle's code signature
  (nested code signed, or its existing signature kept and verified).
- **B.** Keep `assets/`, but add an executable flag per asset, allow the full name character set, and lift the per-asset size limit. The app still archives the tree and unpacks it; only the mode and name problems go away.
- **C.** Document and support a first-launch download of a pinned archive (no bundle change). This needs the network on first launch and does not meet the "downloaded app runs offline" goal.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Make a minimal app (or use the clone) whose `assets/` has a shell script with mode 0755, a file under `@scope/pkg/index.js`, and a file larger than the per-asset limit.
2. Build the macOS bundle with `EXACT_APP_DIR=<app> bun host/apple/build.mjs <crate> --bundle`, then inspect `Contents/Resources/assets`.
3. Expected (what T3 Code's bundle does): the script keeps 0755, the `@scope` path exists, the large file is present, and `codesign --verify --deep --strict` passes.
   Actual (per `EXACT2-GAPS.md`): the name is refused, the mode is 0644, and the size is refused or exceeds the buffer.

## Acceptance for the fix
- A bundle test: a helper executable inside the built `.app` starts with `Process` (mode kept). A tree with `@scope` names and symlinks is byte-identical to its source (hash manifest).
- A file above the old per-asset limit, and a tree above 256 MiB after encoding, build and launch. If the new limit is finite, it is documented and tested at its edge.
- `codesign --verify --deep --strict` passes on the built bundle with the tree inside, for an ad-hoc signature and for a Developer ID identity (see X37).
- A case in the repository's checks (smoke or conformance) covers the three points above.

## App adoption after resolution
Remove the part-splitting stage and the first-launch unpack from `20261005-embedded-server-runtime`. Keep only the version-switch logic. Point the server path at the bundle.
In `20261005-portable-app-download`, drop the unpack stage from the first-launch view and re-run the audit rows for executable bits and signatures. Re-measure the bundle size.
`issue-close` checks that the stage script no longer splits parts and that a clean-account launch needs no unpack.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
