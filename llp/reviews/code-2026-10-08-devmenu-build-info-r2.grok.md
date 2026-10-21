# Code review: build info in the development menu, round 2 (d5669cb9f), 2026-10-08 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, a detached worktree.
- **Method:** one brief (sha256 `e105755fef7655aa9f832a51c72425142eb7345505e897a82b9e7203cfd6e077`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** Nothing to take.

---

LAND

No findings.

Round 1 is fixed:

- `host/apple/build.mjs` is 1,500 lines by `caps.mjs` (`split('\n').length`, fail only above that). The stamp is `buildinfo.mjs`, re-exported from `assets.mjs`.
- Age: `09:59:59.500Z` to `12:00:00Z` is 7200.5s, so `Int` hours is 2 (`BuildInfoTests.swift:17-18`). `en_US_POSIX` plus `yyyy-MM-dd HH:mm` is Gregorian, 24-hour, device zone (`BuildInfo.swift:36-42`).
- The bare macOS sidecar `${executable}-Info.plist` is stamped (`build.mjs:1259`). The menu reads `ExactEnv.appMetadata`, which loads that sidecar when the process has no bundle id (`DevMenuMac.swift:273`, `Session.swift:73-79`).
- The iOS sample host passes `distribution: buildInfo(...)` (`build.mjs:1347`); `infoPlist` merges any truthy `distribution` (`build.mjs:443`).
- Kind is `archive` for `--archive`, `release` when `production || distribution` (Swift `-c release`, including `exact release`), otherwise `debug` (`buildinfo.mjs:30`; call sites `build.mjs:1259`, `1288`, `1330`).
- Dirty is `git status --porcelain` (`buildinfo.mjs:16`). `buildinfo.test.mjs` adds an untracked file and expects dirty. `bun test host/apple/buildinfo.test.mjs`: 2 pass. A non-repo omits both commit keys. An app inside this checkout omits `ExactAppCommit`.
- Copy writes the whole `info()` string (`DevMenuIOS.swift:223-228`, under `#if !os(tvOS)`; `DevMenuMac.swift:292-302`).

The plist is written before `codesign` (macOS bundle at `build.mjs:1298`, iOS and the sample host in the loop at `1375`). The icon-catalog cache key does not include the stamp (`assets.mjs:59-60`). `ExactBuildTime` changes every build, as the receipt's `built` field already does. `Exact*` keys sit beside `ExactLaunchMode` and `ExactWindow`, which these plists already carry.
