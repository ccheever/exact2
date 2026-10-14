---
name: 20261014-native-helpers-and-text-menu
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-native-helpers-and-text-menu
pr_url: https://github.com/ccheever/exact2/pull/544
verified_commit:
---

# Native permission helpers and selected-text Services

## Outcome

Address the independently reproducible native helper and text-menu findings from `realinput-1011c`, authorized with
the first repair wave on 2026-10-14. Parent: [macOS parity specification](../spec.md). This handoff fixes Services and
tracking-mode grant polling; the original real-grant and Finder observations remain open.

| Row | Recorded difference | Required outcome |
| --- | --- | --- |
| G5-B1 | Clone helper remained open for 3.5 s after the Screen Recording drop, then macOS quit it. Reference helper disappeared about 10 s after the permission sheet, while its app remained running. | Detect an observable grant and close the helper; establish the real-grant behavior without confusing another close path with grant detection. |
| G5-B3 | After the app row revealed its bundle in Finder, the clone hid the helper; the reference kept it visible. | Explain and reproduce the native activation difference before changing visibility behavior. |
| R11-SEL | A secondary click on selected page text showed Cut, Copy, Paste and Select All; the reference also showed Services. | Allow AppKit to supply Services for the selected page text, keeping the shell's editing actions. |

## Scope and exclusions

App-owned Swift under `modules/apple/`, focused checks in the existing `macos/tests/snapshot` and
`macos/tests/contextmenu` binaries, and this task's records. No framework changes, permission changes, real-input
automation, provider messages, or new verification scripts. Text inputs and embedded browser menus retain their
existing suppression of AutoFill/Services/Writing Tools.

## Context and guidance

- Exact knowledge library `20261005-platforms-v3`: foundations, macOS platform matrix, accessibility, testing and
  debugging. The current checkout and `docs/testing.md` govern the native APIs and event-driven test waits.
- Reference checkout: T3 Code `1e2ecbd9758830669684b494d4398f626b0576e0`, read-only.
  `apps/desktop/src/permissions/MacPermissionHelper.ts`, `MacSettingsWindow.ts`,
  `snapShot/DesktopSnapShot.ts`, and `window/DesktopWindow.ts`.
- Native test commands: app README's module AppKit/XCTest recipe, pinned Bun 1.4.2.
- Original observations: [B8 grants notes](https://raw.githubusercontent.com/ccheever/exact2/377b8fd9f7d730b9920476b34bfec9df5c19e9c0/realinput-1011c/b8-grants/00-b8-notes.txt)
  and [selected-text menu items](https://raw.githubusercontent.com/ccheever/exact2/dae1a9619a430b2ecdea34ac21af9c34cdd37a1d/realinput-1011c/b14-r11/R11-SEL-items.txt).

## Dependencies

Prepared from `origin/feat(example)/t3-code` at `609f392d52a5c3f65cda2f50e5e5824a01a4184e`.
No dependent task PR is required. The coordinator owns upstream adoption and the final real-input batch.

## Implementation notes

The initial permission diagnosis is not established. Electron 44.4.2's
[`GetMediaAccessStatus`](https://github.com/electron/electron/blob/v44.4.2/shell/browser/api/electron_api_system_preferences_mac.mm)
calls Chromium's screen-permission check, which ends in
[`CGPreflightScreenCaptureAccess`](https://github.com/chromium/chromium/blob/152.0.7977.130/ui/base/cocoa/permissions_utils.mm),
the same function the clone uses. The B8 reference's setup row still read **Allow** after its helper disappeared.
The reference helper also closes when Settings disappears. Its closure alone does not establish grant detection;
the clone's 3.5 s observation and reference's approximately 10 s observation are not equivalent.

For G5-B3, the clone's `sync()` explicitly orders the panel out when Settings is covered and the panel is not key.
`hidesOnDeactivate = false` cannot override that explicit call. The reference has the same condition using
`window.isFocused()`, which Electron implements as
[`isKeyWindow`](https://github.com/electron/electron/blob/v44.4.2/shell/browser/native_window_mac.mm).
The source explains why the clone hides, but not why the reference stayed visible in the recorded run. A different
focus state or watcher observation remains an inference, not a verified cause.

ExactKit's selected paragraph already implements `NSServicesMenuRequestor` and supplies its actual selection.
The app's `T3TextContextMenu.present` disables context-menu plug-ins for every view, preventing AppKit from asking that
requestor. It now enables plug-ins for a non-text-view responder accepting a string with no return type. AppKit adds
Services to the shell's editing template; the existing text-input and browser suppression stays in place.

The helper grant poll used a default-mode scheduled timer, unlike the Settings watcher's common-mode timer. A native
test reproduced a stalled grant transition while the event-tracking run loop ran. The grant timer now uses the same
common modes, so a menu or drag cannot suspend that check. This is a separately demonstrated defect; it does not prove
why the real G5-B1 grant was not observed before macOS quit the clone.

No visual metrics, motion, focus policy, permission API, or Finder activation policy changed.

## Acceptance and reproduction

1. Native menu test: present the shell's menu over a read-only view with a selected-string services requestor.
   Read the drawn menu during tracking, verify Services is present, and verify the requestor writes only its selection
   to a private pasteboard. With no selection, no Services item; text-field/browser regressions remain green.
2. Native helper test: with fake grants and a fake Settings feed, grant while AppKit's event-tracking run loop is active.
   The real helper's polling timer must close it and release tracking without returning to the default run-loop mode.
3. Preserve helper activation-transition checks. Do not declare G5-B3 fixed without reproducing the different native
   states against the reference.
4. Run one practical local proof of the changed native behavior; agent mode suppresses actual popup tracking, so its
   logs alone cannot prove AppKit's Services insertion.
5. Final real-input batch: B8 isolated grant lane, equal observation intervals on clone/reference with **Later** chosen;
   capture helper close reason and setup status. Repeat Finder reveal with helper key state, Settings reading and
   visibility recorded. R11-SEL: select page text and secondary-click it, then inspect/use Services.

## Progress

Implementation is ready for focused review and the coordinator's integrated app check. Both native suites pass.
G5-B1's real grant and G5-B3's reference activation difference remain unverified; no speculative permission probe or
visibility exception has been added. The task stays active and unverified for those acceptance cases.

Changed task-owned code:

- `modules/apple/T3TextContextMenu.swift`: selected page text may supply Services.
- `modules/apple/T3PermissionHelper.swift`: grant polling runs in common run-loop modes.
- `macos/tests/contextmenu/text-menu.swift`: read the actual drawn menu with/without selection, and exercise a private
  Services pasteboard through the requestor fixture. The fixture represents the existing ExactKit boundary; it is not
  an end-to-end test of a framework text selection or an installed third-party Service.
- `macos/tests/snapshot/permission-helper.swift`: grant during event tracking, then verify helper closure, owner return,
  completion callback, and watcher cleanup.

## Attempts and evidence

One repair round. Host: macOS 26.6.2 (25G83), arm64; Apple Swift 6.4, compiled in Swift 5 mode; Bun 1.4.2.
The original native observation artifacts are linked above. Raw native-suite output is published on the existing `t3-code-evidence` branch, pinned below; the plan keeps verification logs out of the application branch.

| Check | Before the runtime fix | After the runtime fix |
| --- | --- | --- |
| Context-menu XCTest suite | [70 tests, 2 failed assertions](https://raw.githubusercontent.com/ccheever/exact2/3a82181872531a270472140bd7cdaa765cdbfdb7/native-helpers-and-text-menu/20261014-review/contextmenu-red.log), both in the new selected-text test: actual menu had four roles and never consulted the Services requestor. Exit 1. | [70 tests, 0 failures](https://raw.githubusercontent.com/ccheever/exact2/3a82181872531a270472140bd7cdaa765cdbfdb7/native-helpers-and-text-menu/20261014-review/contextmenu-green.log). Selected text drew Cut, Copy, Paste, Select All, Services; no selection drew the four roles. Composer, text-field, and browser regressions also passed. Exit 0. |
| Snapshot/native-helper suite | [Failed the tracking-mode grant assertion](https://raw.githubusercontent.com/ccheever/exact2/3a82181872531a270472140bd7cdaa765cdbfdb7/native-helpers-and-text-menu/20261014-review/snapshot-red.log). Exit 133. | [165 checks passed](https://raw.githubusercontent.com/ccheever/exact2/3a82181872531a270472140bd7cdaa765cdbfdb7/native-helpers-and-text-menu/20261014-review/snapshot-green.log), including 58 helper checks. The grant transition closed the helper and released its watcher while the event-tracking run loop ran. Exit 0. |

These are actual AppKit menu/panel executions with synthetic events and injected grant/Settings state. They neither
request OS grants nor send global input. Actual drawn-menu inspection is the practical local proof: agent mode only
logs the authored items and suppresses popup tracking, so it cannot verify Services insertion.

Reproduce from the repository root with Bun 1.4.2 on `PATH` (the app README's existing native-suite recipe):

```sh
T3_NATIVE_PROOF="$PWD/target/native-helper-review"
T3_TEST_XCODE="$(xcode-select -p)"
T3_TEST_FRAMEWORKS="$T3_TEST_XCODE/Platforms/MacOSX.platform/Developer/Library/Frameworks"
T3_TEST_LIBS="$T3_TEST_XCODE/Platforms/MacOSX.platform/Developer/usr/lib"
mkdir -p "$T3_NATIVE_PROOF"
T3_DK="$T3_NATIVE_PROOF" bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");'
for T3_SUITE in contextmenu snapshot; do
  T3_OUTPUT="$T3_NATIVE_PROOF/green/$T3_SUITE"
  mkdir -p "$T3_OUTPUT/fixture"
  xcrun swiftc -swift-version 5 -module-name "T3${T3_SUITE}Tests" \
    -F "$T3_TEST_FRAMEWORKS" -I "$T3_TEST_LIBS" -L "$T3_TEST_LIBS" \
    -Xlinker -rpath -Xlinker "$T3_TEST_FRAMEWORKS" -Xlinker -rpath -Xlinker "$T3_TEST_LIBS" \
    host/apple/modules/ExactNativeModule.swift "$T3_NATIVE_PROOF/ExactDataKeys.swift" \
    examples/t3-code/modules/apple/*.swift "examples/t3-code/macos/tests/$T3_SUITE/"*.swift \
    -o "$T3_OUTPUT/$T3_SUITE-tests" || break
  T3_APP_DIR="$PWD/examples/t3-code" T3_BROWSER_TEST_DIR="$T3_OUTPUT" \
    T3_SNAPSHOT_TEST_ROOT="$T3_OUTPUT/fixture" "$T3_OUTPUT/$T3_SUITE-tests" || break
done
```

The red checks used the new regressions against unchanged runtime source at the recorded base. The green checks used
the runtime fixes above. No framework or shared plan-status files changed. The coordinator owns the five integrated
repository checks and the combined macOS app build/launch; those are not claimed by this task's module-suite results.
After staging the task-owned changes, `bun scripts/caps.mjs` (Bun 1.4.2) and `git diff --cached --check` both passed.

## Next action

Review this diff, build and launch the combined macOS app, then execute acceptance item 5 with real input. For G5-B1,
record the helper's close reason rather than treating disappearance as proof of a grant. For G5-B3, capture the actual
key-window and Settings-frontmost states before choosing any visibility change. Final acceptance, verified commit,
PR delivery, and task closure belong to the coordinator's verification pass.

## Publication and review

Draft PR [#544](https://github.com/ccheever/exact2/pull/544) publishes implementation `d488d1057c12039d98359846a4efb0c8bac5545f`. Independent source review found no blocking issue in the two native changes and their red/green checks. This bookkeeping update moves raw logs to the established evidence branch; it changes no runtime or test inputs. Integrated checks and the real-input cases above remain open.

## Coordinator publication, 2026-10-14

Draft PR [#544](https://github.com/ccheever/exact2/pull/544) was read back at `4f81dc73ffb3984524ada9f8542ab3a67254e6ea`, based on `feat(example)/t3-code`. Native Services and tracking timer proof passed; original real-grant/Finder observations remain unresolved. The combined repository gates and app drive are pending. This local integration bookkeeping does not close the task or replace the earlier verification revision.
