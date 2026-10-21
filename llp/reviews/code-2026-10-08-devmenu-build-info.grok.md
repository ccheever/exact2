# Code review: build info in the development menu, round 1 (2685abe21), 2026-10-08 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, a detached worktree.
- **Method:** one brief (sha256 `faba537bad1c849d50579309c29d09f0655de540866eb7f47d2fee73db1b734a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** All taken, as Astra's: the bare macOS sidecar, the test, the cap, `production || distribution`, the iOS sample host, an `en_US_POSIX` Gregorian formatter in the device's zone with the full local string asserted.

---

LAND WITH CHANGES

1. **BLOCKING** — The bare macOS dev menu never shows the stamp. `DevMenu.info` reads `Bundle.main.infoDictionary` (`host/apple/Sources/ExactKit/Mac/DevMenuMac.swift:273`). A bare executable has no bundle id there; the keys are written to the sibling sidecar (`host/apple/build.mjs:1251-1260`), which `ExactEnv.appMetadata` already loads (`host/apple/Sources/ExactKit/Session.swift:72-79`). `bun host/apple/build.mjs --run` (no `--bundle`) shows `? — ?` and `version ? (?)`, and Copy copies that. The bundled `.app` path is intact: `Bundle.main` is the bundle. **Fix:** pass `ExactEnv.appMetadata` into `BuildInfo.lines`.

2. **BLOCKING** — `BuildInfoTests.testTheStampedBuildReadsAsLines` fails. The stamp is `2026-10-08T10:00:00.123Z` and `now` is `2026-10-08T12:00:00Z` (`host/apple/tests/ExactKitTests/BuildInfoTests.swift:7-17`). The fractional parser keeps the 123 ms, so the gap is 7199.877 s and `Int(s / 3600)` (`host/apple/Sources/ExactKit/BuildInfo.swift:31`) is `1 h ago`. The assertion requires `(2 h ago)`. Reproduced with the system Swift formatter. **Fix:** set `now` to `2026-10-08T12:00:00.123Z`, or drop the fraction.

3. **BLOCKING** — `host/apple/build.mjs` crosses the 1,500-line cap. `caps` counts `text.split('\n').length` (`scripts/caps.mjs:154`) and fails above 1500. This file is 1501; the parent was 1500. The new import (`host/apple/build.mjs:50`) is the extra line, and it is the only source file over the cap. **Fix:** delete one line so the split length is at most 1500.

4. **MATERIAL** — A macOS distribution build is stamped `debug`. Kind is `archive` only when `archive` is set, otherwise `release` only for production trust (`host/apple/buildinfo.mjs:29`). The mac call sites pass `{ root, production }` (`host/apple/build.mjs:1260`, `1289`). `exact release` builds with `--distribution` and development trust (`scripts/exact.mjs:115-118`, `244`), Swift is `-c release` (`host/apple/build.mjs:751-754`), and the menu says `debug`. An iOS `--archive` is `archive` via `archive: !!ipa` (`host/apple/build.mjs:1331`). **Fix:** pass `production: production || distribution` on every `buildInfo` call (`archive` still wins) and pin it in `buildinfo.test.mjs`.

5. **MINOR** — The iOS sample-host bundle is unstamped. Its `infoPlist` write does not receive `buildInfo` (`host/apple/build.mjs:1348`). The app writes do, by placing the dict in `icon` or `distribution` (`build.mjs:1260`, `1331`). `ExactHostIOS` does not install the dev menu, so this is the plist only. **Fix:** pass the same stamp into that call.

6. **MINOR** — Build time is not a fixed Gregorian clock. `local` sets `dateFormat` and leaves the locale alone (`BuildInfo.swift:36-39`). Locale `ar_SA` prints `١٤٤٨-٠٤-٢٧ ١٥:٠٠` for `2026-10-08T22:00:00Z`. The test only checks the prefix `built 2026-10-08 ` (`BuildInfoTests.swift:17`), so any hour on that calendar day passes. **Fix:** set `locale` to `en_US_POSIX` and `timeZone` to `.current`, and assert the full local string.

iOS simulator, device, and tvOS, including `--archive`, merge the stamp in the final `infoPlist` (`build.mjs:1331`). tvOS keeps the menu and omits Copy (`DevMenuIOS.swift:227-229`). iOS and mac Copy publish the whole `info()` string. An in-repo app omits `ExactAppCommit`; a separate git repo includes it, dirty for tracked changes only (`--untracked-files=no`); no repo and no `git` omit the commit keys (`buildinfo.mjs:8-31`). `buildinfo.test.mjs` covers those three. Plist booleans round-trip to Swift `Bool`. The stamp is written after the relink check (`build.mjs:1154` hashes the archive and entitlements), so compile reuse is unchanged. `codesign` already re-signs each build. `receipt.json` already carries a per-build `built` timestamp, so the archive already varied per build; `ExactBuildTime` also varies the signed plist. Custom `Exact*` keys are ordinary plist entries. Non-archive version and build stay the existing `0.1.0` / `1`; an ipa still gets `EXACT_VERSION` / `EXACT_BUILD_NUMBER` from `distributionKeys`.
