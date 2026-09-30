# An `--ipa` build with a launch background ships without its catalog app icon: the launch `actool` pass overwrites `Assets.car`

**Status:** Closed
**Resolution:** Fixed: iOS icons and launch colours compile in one actool pass, and distribution builds require AppIcon in Assets.car; a real Xcode build of the fixture catalog and assetutil inspection verify AppIcon plus both ExactLaunch appearances. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple build (`host/apple/build.mjs`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1030 (manifest `launch`), icon catalog `faa71219`

`host/apple/build.mjs:1094` compiles the icon catalog (`appIcon({catalog: ipa})`), then `launchScreen` compiles `Launch.xcassets` into the same bundle directory (`:591`, `:636`). Both passes write `Assets.car`.

**Reproduced with the real `actool`:** after the icon compile, `Assets.car` holds `AppIcon`. After the launch compile into the same directory it holds only `ExactLaunch`. The Info.plist still names an icon the catalog no longer has, and App Store Connect requires the catalog icon.

**Origin:** Charlie's icon catalog (`faa71219`) meets Seth's launch colour (`855a3662`). Neither side was wrong alone.

**Fix:** compile the icon and the launch colour sets in one `actool` invocation, and have the IPA path check that `Assets.car` contains `AppIcon`.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: iOS icons and launch colours compile in one actool pass, and distribution builds require AppIcon in Assets.car; a real Xcode build of the fixture catalog and assetutil inspection verify AppIcon plus both ExactLaunch appearances.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max; reproduced by the verifier with Xcode's actool (`DEVELOPER_DIR` set to Xcode.app). Verification: reproduced.
