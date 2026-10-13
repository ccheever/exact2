# `focusin`/`focusout` (or `:focus-within`): an ancestor hears the focus enter its subtree

**Status:** Open
**Resolution:** Partial: nested Apple stop flags, early web focus replay and UIKit read-only textarea responder entry are fixed; UIKit focus transitions now pass on iOS 27; native delivery to a wrapper destroyed by blur remains a declared deviation.
**Systems:** Contract, runner events, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/283

## 2026-10-11 events lane

Still open. The regression was reproduced: `focusin=` and `focusout=` both
produce `lower-unknown-attr` on a `column`. This lane completed capture-phase
keys first. Remaining: specify the Contract `FocusEvent` target and
relatedTarget representation, dispatch old-target `focusout` and new-target
`focusin` through each host's focus transition (including a removed focused
node), preserve the wrapper's existing Tab order, and drive programmatic,
click, Tab and removal cases against Chrome on web, macOS, iOS and Linux.

## Current scope

Implement approved bubbling focusin/focusout with target/relatedTarget information, without a new Tab stop. Correct the original acceptance: DOM focusout also fires between descendants. Cover programmatic/click/Tab focus and removal.

## Rebase verification (2026-10-12)

Rebased the five focus commits onto fetched `origin/main` `57ab536e3`.
`keycapture` keeps host ABI kind 44; `focusin` and `focusout` took 45 and 46
(since 46 and 47: the second rebase below) in the shared decoder, both bridges, DOM replay, headers and assertions.
Capture dispatch and focus replay keep their own stopped scopes. Both sides'
compiler vocabulary, font tests and agent pitfalls are retained. The obsolete
"approved next" focus paragraph in LLP 1005 is removed. Main's kernel schema
and pinned digest are unchanged by the lane. Linux's existing focus save/restore
helpers moved to its existing events module to keep the presenter under 1,500
lines. The declared destroyed-wrapper deviation above remains open.

Passed after rebase:

- `cargo build --all-targets --keep-going`.
- `cargo clippy --all-targets --keep-going -- -D warnings`.
- `cargo test -p exact-web-js --bins --no-fail-fast` — 42 passed.
- `cargo test -p exact-kernel -p contract -p contract-lower -p contract-analyze -p exact-runner -p exact-web -p exact-apple --lib --bins --tests --no-fail-fast` — 2,989 passed, 32 ignored, zero failed.
- `ONLY_CLASSES=ExactKitTests.ClickFocusMacTests,ExactKitTests.KeyUpMacTests bun host/apple/build.mjs --test` — 8 + 8 tests passed. The temporary `xctest.mjs` class filter was removed.
- `/private/tmp/claude-501/-Users-ccheever-projects-exact2/5f1c298e-7e97-40bc-8a1a-486f1889060d/scratchpad/ios-verify.sh /Users/ccheever/projects/exact2-iss-focus KeyboardFocusIOSTests AC228731-1097-4E4A-A81B-34A2BEEE90EC` — 11 passed on iOS 27. The simulator was Shutdown before selection and Shutdown after the run; temporary build-script filtering was removed.
- `CHROME=/Users/ccheever/Library/Caches/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell bun test host/web/tests/js-runtime.test.mjs host/web/tests/js-link-press.test.mjs host/web/tests/js-storage-scope.test.mjs host/web/tests/form-chrome.test.mjs host/web/tests/js-autofocus.test.mjs --timeout 60000` — 61 passed.
- With the same `CHROME`, `EXACT_GLUE_FAST=1 bun test host/web/tests/document.test.mjs --timeout 60000` — 21 passed, 8 skipped. Built-wasm-app cases lack a current artifact; the Weatherlight build is explicitly skipped. The focused `-t 'early focus replay|Chrome orders|capture'` run also passed (4 passed, 1 skipped).
- `cargo fmt --all -- --check`; `git add -A && bun scripts/caps.mjs`; `bun scripts/boot.mjs`; `bun scripts/issue.mjs check`; `git diff --check`.

The first broad web attempt used the default Chrome and failed to connect
(`Target.createTarget` timeout); it then began an unrelated Weatherlight cold
build. Only that run's recorded processes were stopped. The headless Chromium
runs above supersede that attempt. The first caps run found Linux's presenter
at 1,507 lines; the helper move above fixed it, and caps passed.

`cargo test -p exact-linux --lib --tests --no-fail-fast` completed with four
failures outside the focus cases. All four focus integration tests passed.
The failures were an already-arrived reply in the initial "in flight" assertion,
405 ms for a 200 ms settle bound, a missing decoded bitmap after its wait,
and `(0, 0)` image dimensions after a three-second decode wait. All four passed
unchanged in isolated reruns of the binaries this command built:

- `target/debug/deps/exact_linux-ae4dd3cdc31ca326 agent::tests::clock_data_lands_the_replies_without_moving_the_clock --exact --nocapture`
- `target/debug/deps/exact_linux-ae4dd3cdc31ca326 agent::tests::settle_takes_one_bound_for_a_request_that_never_answers --exact --nocapture`
- `target/debug/deps/exact_linux-ae4dd3cdc31ca326 image::control_tests::sharing_a_large_live_raster_does_not_require_a_second_decode_budget --exact --nocapture`
- `target/debug/deps/it-2b4faf38f2a33d5b image::sampled_pixels_keep_natural_auto_axes_and_min_max_layout --exact --nocapture`

No unrelated test or timing bound was changed. This is a failed broad Linux
run with passing isolated reruns, not a full-suite green result.

`cargo run -p contract-difftest -- quick` built its binary but could not run
the semantics comparison: `lake build: No such file or directory (os error 2)`.
Lean/lake is not installed. No semantics pass is claimed.

`cargo clippy -p exact-linux -p exact-web-js --all-targets --keep-going -- -D warnings` also passed. Logs for this pass are `/tmp/focus-rebase-*.log`; the UIKit log is `/Users/ccheever/projects/exact2-iss-focus.ios.log`.

Main advanced to `a9901821c` during this verification; these results qualify
the `57ab536e3` base. A further rebase and verification follows.

## Third rebase (2026-10-13, onto `3ddbdb11a`)

Rebased onto batch 10 (`intersect=` with host kind 45 and prop 260,
`setRangeText`'s replacement input as kind 48, menu-bar props, `::placeholder`).
Both sides kept: `HANDLERS` is 63; the `FocusEvent` and
`IntersectionObserverEntry` shapes, event records and named-type lists; the
grammar's event names; `format.json`'s events (`intersect`, then `focusin`,
`focusout`). Dispatch now covers exactly what each host sends: the web
`28..=48`, Apple `28..=38 | 40..=44 | 46..=48` (Apple sends no 45, so the
bridge test refuses it as unknown). The commit that renumbered the kinds now
reaches the code through the earlier commits' resolutions. The plan format
digest moved, so `NativeFieldsIOSTests`' fieldless plan is compiled again from
its commented fixture (only bytes 8 to 15 differ). Caps: the lane's lines put
`PresenterIOS.swift` and `glue.js` over 1,500 after main's growth, so
`onFocusEvent`, `focus` and `blur` are now a `Presenter` extension in
`FocusEvents.swift` (the closure kept on `FocusEvents`), `onFocus, onBlur`
share a declaration, and the wasm glue's `focus` and `blur` share a branch.

Passed after this rebase: `cargo fmt --all -- --check`; `cargo build
--all-targets --keep-going`; `cargo clippy --all-targets -- -D warnings`;
caps; `cargo test -p contract --bins docs_card_md_is_what_vocab_prints` (1,
card unchanged); the core seven crates (3,086 passed, 32 ignored, 0 failed);
`exact-linux` (974 passed, 10 ignored, 0 failed); `exact-web-js` (45 passed);
the five JS-target web files (68 passed); `document.test.mjs -t focus` (2
passed, 7 skipped for a stale wasm dist); AppKit `ClickFocusMacTests`,
`KeyUpMacTests`, `FieldSelectionMacTests` (8 + 8 + 12 passed); UIKit
`KeyboardFocusIOSTests`, `FieldSelectionIOSTests`, `NativeFieldsIOSTests`
(11 + 11 + 13 passed, `ios-verify.sh` on Shutdown iOS 27 iPhone 17 `C25DD7E7`, Shutdown
after); `bun host/apple/build.mjs --tvos --no-install` (built and signed); `bun scripts/boot.mjs`; `bun scripts/issue.mjs check`.

## Second rebase (2026-10-12, onto `2c56cea56`)

Rebased the seven focus commits onto `origin/main` `2c56cea56`. `intersect=`
lands in the same batch with host kind 45, so `focusin` and `focusout` take 46
and 47 (`28..=44 | 46 | 47` in both bridges' dispatch, leaving 45 to that
lane's merge). Main's `42e570e28` (the focus follows the route on the painted
hosts) moved Linux's commit-end focus into `presenter/arrive.rs`
(`release_focus`, `autofocus`, `arrive`); the conflicts in `presenter.rs` were
resolved onto that shape: `clear_removed_focus` sends the removed or inert
node's `focusout` path and then clears through `release_focus`, so `arrive`
still finds the inert node; `autofocus` focuses through `set_focus` (its
`focus` and `focusin`, and a surface's `autofocus_from` hand-off) and returns
its error; `restore_focus` (now in `events.rs`) also resets the route state.

Passed after this rebase:

- `cargo fmt --all -- --check`; `cargo build --all-targets --keep-going`;
  `cargo clippy --all-targets -- -D warnings`; `git add -A && bun scripts/caps.mjs`;
  `bun scripts/boot.mjs`; `bun scripts/issue.mjs check`; `git diff --check`.
- `cargo test -p exact-kernel -p contract -p contract-lower -p contract-analyze -p exact-runner -p exact-web -p exact-apple --lib --bins --tests --no-fail-fast` — 3,055 passed, 32 ignored, 0 failed.
- `cargo test -p exact-linux --lib --tests --no-fail-fast` — 959 passed, 10 ignored, 0 failed (main's `arrive_tests` and the focus tests included).
- `cargo test -p exact-web-js --bins --no-fail-fast` — 43 passed.
- `bun test` of `js-runtime`, `js-link-press`, `js-storage-scope`, `form-chrome`, `js-autofocus` (headless Chromium `CHROME`, `--timeout 60000`) — 68 passed.
- `bun test host/web/tests/document.test.mjs -t focus` — the two focus replay cases passed (kinds 46/47 asserted); 7 built-wasm-app cases skipped for a stale dist.
- AppKit `ClickFocusMacTests` and `KeyUpMacTests` (temporary `ONLY_CLASSES` filter in `xctest.mjs`, removed) — 8 + 8 passed.
- UIKit `KeyboardFocusIOSTests` (`ios-verify.sh` on Shutdown iOS 27 iPhone 17 `C25DD7E7`; `AC228731` was Booted by another session) — 11 passed. Two earlier attempts stopped before any test under machine load (`simctl boot`: launchd_sim did not respond; then xcodebuild found no simulator destination); the simulator is Shutdown after each.

## UIKit verification follow-up (2026-10-12)

The orchestrator's renamed-ID failure reproduced: 11 tests ran, with two
assertion failures in `testFocusTransitionUsesTheRenamedId`. The first
assertion showed that the node still had `id="a"` **before** Tab moved focus.
The test's `sendActions(.editingChanged)` did not deliver the editing callback
in this unhosted UIKit test, so `onInput` never applied the rename. This was
a regression-test setup defect, not another stale `FocusSnapshot`: the shared
`FocusEvents.named` already reads the current ID on UIKit as on AppKit.

The test now calls the production `fieldChanged()` callback, as the existing
UIKit composition tests do, after inserting text. It retains the assertion
that the node really becomes `renamed`, then drives the hardware Tab command
and checks both outgoing `target` and incoming `relatedTarget`. No production
focus change or new test was needed.

Command (run before and after the test correction):

```sh
/private/tmp/claude-501/-Users-ccheever-projects-exact2/5f1c298e-7e97-40bc-8a1a-486f1889060d/scratchpad/ios-verify.sh /Users/ccheever/projects/exact2-iss-focus KeyboardFocusIOSTests AC228731-1097-4E4A-A81B-34A2BEEE90EC
```

**Passed: all 11 `KeyboardFocusIOSTests`, zero failures**, on iPhone Air,
iOS 27. This includes `testFocusTransitionUsesTheRenamedId`,
`testReadOnlySelectionSendsFocusAndBlur`, passive ancestor transitions/removal,
and hardware-keyboard navigation. The runner booted and shut down only the
specified simulator. The final log is
`/Users/ccheever/projects/exact2-iss-focus.ios.log`; the reproduced failure is
preserved in `/tmp/focus-ios-followup-red.log`. These results supersede the
unqualified UIKit entries in the earlier rounds below.

Additional verification:

- `ONLY_CLASSES=ExactKitTests.ClickFocusMacTests bun host/apple/build.mjs --test`
  — **passed, 8 tests, zero failures**. A temporary `ONLY_CLASSES` filter in
  `xctest.mjs` was removed after the run. Log: `/tmp/focus-ios-followup-mac.log`.
  An initial concurrent attempt was refused by the active iOS build's per-app
  lock; the successful run was sequential.
- `cargo fmt --all -- --check`, `bun scripts/boot.mjs`,
  `bun scripts/issue.mjs check`, `git diff --check`, and staged
  `bun scripts/caps.mjs` — passed. No Rust or production Swift changed in this
  follow-up; no crate test/clippy rerun or standalone app drive is claimed.

The issue stays open only for the declared destroyed-wrapper native deviation.

## Review fix round 3 (2026-10-12)

- **A1 — real, fixed:** nested Apple focus delivery reset the enclosing key event's stop flag even without focus listeners. `FocusEvents.emit` now saves and restores that flag. `testFocusRemovalPreservesTheEnclosingKeysStopFlag` covers removal from a stopped child key handler, with and without focus listeners. Both variants of the AppKit regression failed before the fix (`[6, 1]` instead of `[6]`). A standalone test compiling the production `FocusEvents.swift` also reproduced the failure before the fix and passed afterwards.
- **A2 — real, fixed:** early document focus replay queued ancestors independently. Each captured bubbling focus event now carries one stopped scope through replay and the wasm command dispatcher. The Chrome test `early focus replay stops at the inner handler for each transition` reproduced three unwanted outer deliveries before the fix and passes afterwards.
- **A3 — real code gap, code fixed; UIKit run unqualified:** UIKit's read-only selection sessions omit the editing-began delegate. `TextArea.becomeFirstResponder` now announces successful responder entry; the existing explicit resign callback can therefore deliver blur and the bubbling exit. `testReadOnlySelectionSendsFocusAndBlur` selects read-only text then focuses another field and asserts the direct and bubbling events. The code fix is included as explicitly requested even without a pre-fix UIKit test result.
- **B1 — not real for the reported scenario:** `blur()` is queued by the press, then `run_commands` executes it after `dispatch_press` and its fallback have returned. `press_blur_leaves_no_focus_for_pointer_and_keyboard` passes on unchanged Linux production code for both tap and Enter, with `in:b;out:b;` and no focused node. The first draft omitted the command pump and saw only `in:b;`; that was a test setup error, not a reproduction. No Linux production change was made.
- **B2 event order — not real:** Chrome's actual edited-field transition is `change`, `blur`, `focusout`, `focus`, `focusin`. The new browser assertion passes with that order. Native order is unchanged.
- **B2 destroyed-wrapper subcase — real, deferred:** the same Chrome test confirms that removing the wrapper in `blur` still lets its detached listener hear `focusout`. Native dispatch drops destroyed handler bindings and skips that wrapper, retaining surviving ancestors from the old path. Chrome uses the detached path. Matching it requires retaining bindings beyond destruction; this final round records the deviation in LLP 1005 and the grammar instead of extending that lifetime. The issue remains open.

### Round 3 verification

Passed:
- `cargo test -p exact-linux --test focus press_blur -- --nocapture` — the reported pointer/Enter blur case passes on unchanged production code.
- `bun test host/web/tests/document.test.mjs -t 'early focus replay|Chrome orders' --timeout 180000` — 2 Chrome tests, including both the normal and detached event paths. The replay assertion failed before the fix (extra outer handlers). Initial CDP launches timed out; these probes use the already installed Playwright Chrome launcher. A draft body-listener assertion used unsupported `onfocusout`; it was corrected to `addEventListener` and the final run passed.
- `bun test host/web/tests/js-runtime.test.mjs -t 'focus handlers stop|a key handler stops' --timeout 60000` — 2 tests. The focus test now imports a fresh runtime so an earlier deliberate poison test cannot disable its handlers.
- `bun /tmp/focus-r2-browser.mjs` — production JS runtime driven in Chrome, stopped focus transitions and payloads passed.
- AppKit: `bun host/apple/build.mjs --test` with a temporary `ClickFocusMacTests` class filter reproduced both new failures before the fix. The fixed rerun waited behind the Cargo build lock, so it was stopped; `EXACT_TESTS=1 EXACT_LIB_DIR=/Users/ccheever/projects/exact2-iss-focus/target/host-dev EXACT_LIB=caltrain_apple EXACT_APP_COMPOSITION=embedded MACOSX_DEPLOYMENT_TARGET=14.0 swift build --build-tests --scratch-path <existing-test-scratch>` (from `host/apple`, with the absolute library path), then `EXACT_CONTRACT=$PWD/target/debug/contract xcrun xctest -XCTest ExactKitTests.ClickFocusMacTests <scratch>/out/Products/Debug/ExactKitTests.xctest` passed all 8 tests. The temporary class filter was removed. Logs: `/tmp/focus-r3-swift-green.log`, `/tmp/focus-r3-mac-tests-green.log`.
- `swiftc host/apple/Sources/ExactKit/FocusEvents.swift /tmp/focus-r3-nested.swift -o /tmp/focus-r3-nested`, then `/tmp/focus-r3-nested` — production state machine failed before the change and passed after it.
- `cargo fmt --all -- --check`, `bun scripts/boot.mjs`, `bun scripts/issue.mjs check`, `git diff --check`, staged `bun scripts/caps.mjs`.

Not qualified:
- UIKit: booted dedicated simulator `2F9DEA8D-D9AE-48D2-9C39-E60DDE113D8B`, ran `xcodebuild build-for-testing -scheme Exact -destination 'platform=iOS Simulator,id=2F9DEA8D-D9AE-48D2-9C39-E60DDE113D8B' -derivedDataPath <worktree>/target/focus-r3-ios-tests` with the existing simulator archive. At roughly 15 minutes it was still compiling test sources and had produced no `.xctestrun`; `test-without-building` could not start. Stopped only the owned build processes; shut down and deleted only that simulator. No UIKit pass or pre-fix UIKit failure is claimed. Log: `/tmp/focus-r3-ios-red.log` (the build read the responder fix while running, so its name is not a red-test claim).
- `bun test host/web/tests/js-runtime.test.mjs host/web/tests/js-autofocus.test.mjs --timeout 60000` — initial run: 50 pass, the focus test failed because a prior test deliberately poisoned the shared runtime. After test isolation: 50 pass, 1 unrelated timeout in `decimal, currency and percent numbers are Intl's`; the focused key/focus rerun above passed.
- `EXACT_WEB_DIST=/tmp/focus-r3-webdist bun host/web/build.mjs caltrain-web --wasm` — cold build stopped to reduce shared-machine contention; no new complete wasm-app drive is claimed.

- `cargo test -p exact-linux -p exact-apple -p exact-web --lib --tests --no-fail-fast` — stopped after about 16 minutes still compiling test targets; no full-suite pass claimed. Ran the already built current focus integration binary directly instead: `target/debug/deps/focus-7b504caffe36036b --nocapture` — all 4 tests passed (the same test source built by the targeted Cargo run, with only formatting afterwards).
- `bun host/apple/build.mjs` — stopped during the cold standalone app build to end this bounded round; no new standalone Mac agent drive or tvOS build is claimed.

`cargo clippy -p exact-linux -p exact-apple -p exact-web --all-targets -- -D warnings` passed after the Cargo build lock cleared.

## Review fix round 2 (2026-10-12)

- JS `stopPropagation()` was accepted by Contract but had no current event in the JS focus binder. The binder now uses the runtime's event scope and stopped flag. The new `js-runtime.test.mjs` regression failed before the fix and passed after it; a Chrome drive of the production runtime checked both bubbling directions and payloads. `focus-bubble.contract` and its conformance steps now exercise nested stopped handlers.
- Linux autofocus skipped event dispatch. `tests/focus.rs` reproduces initial autofocus, a press mounting an input (with the button removed or retained), and a focused input replaced by another autofocused input. Autofocus now calls the focus transition; a pointer press permits autofocus without temporarily erasing the old focus, so its outgoing event precedes the incoming event. The three integration tests also check stopped bubbling and replacement autofocus with nested listeners: the outgoing path finishes before the replacement enters, and each focus event consumes only its own stop flag.
- Apple kept the ID from focus entry. `FocusEvents` now refreshes the name while retaining the removal path. A small test of the production state machine failed with the old ID and passed after the fix. `ClickFocusMacTests.testFocusTransitionUsesTheRenamedId` passed. The corresponding UIKit test is added but was not run in this round.
- The reported Mac assertion failure did not reproduce: the original `testPassiveAncestorHearsFocusMoveBetweenChildren` assertion and removal check passed unchanged. All seven `ClickFocusMacTests` passed with the temporary class filter; that filter was removed.

At the end of round 2 (superseded by round 3 above): verify the read-only `UITextView` selection-session report on UIKit, then fix it if reproduced. No read-only responder code was changed in this round. The first dedicated simulator attempt was blocked by the cold AppKit build. A second dedicated simulator attempt, after that build completed, reached the Rust build and fixture preparation but no UIKit XCTest within roughly 20 minutes under severe machine load. Both owned simulators were shut down and deleted; other simulators were left alone. The earlier closure is reopened until this remaining finding is settled. The UIKit renamed-ID test also needs its platform run.

### Round 2 verification

Passed:
- `cargo test -p exact-linux --test focus -- --nocapture` — 3 tests, after reproducing the autofocus and nested stop-flag failures.
- `bun test host/web/tests/js-runtime.test.mjs -t 'focus handlers stop|a key handler stops' --timeout 60000` — 2 tests; the new focus test failed before the fix.
- `bun test host/web/tests/js-autofocus.test.mjs --timeout 60000` — 11 tests.
- `ONLY_CLASSES=ExactKitTests.ClickFocusMacTests bun host/apple/build.mjs --test` — 7 tests, using a temporary class filter, since removed.
- `cargo clippy -p exact-linux -p exact-apple -p exact-web-js --all-targets -- -D warnings`, then `cargo clippy -p exact-linux --all-targets -- -D warnings` again after the final Linux change.
- `cargo fmt --all -- --check`, `bun scripts/boot.mjs`, `bun scripts/issue.mjs check`, `git diff --check`, staged `bun scripts/caps.mjs`.
- A temporary Chrome drive importing the production JS runtime checked native DOM focusin/focusout, stopPropagation and target IDs (`/tmp/focus-r2-browser.log`).

Not qualified:
- `ONLY_CLASSES=KeyboardFocusIOSTests EXACT_SIM=AC94A046-579D-4F36-9D99-E6C3205A3648 bun host/apple/build.mjs --test --ios` — stopped at the roughly 20-minute limit before XCTest; no UIKit pass claimed. The unrun read-only test draft remains outside the repository (`/tmp/focus-r2-readonly-test.swift`).
- `bun host/web-js/conform.mjs --synthetic --only focus-bubble --strict --steps 20 --out /tmp/focus-r2-conform` — JS build passed, but comparison refused the stale wasm build before comparing any steps.
- `cargo run -p contract-difftest -- quick` — unavailable because `lake` is not installed.
- A broad JS test run hit unrelated default 5-second timeouts under machine load; the targeted tests above were rerun with 60-second timeouts. Full crate suites and a new standalone macOS app drive were not completed in this round.

## Implementation slice (2026-10-12, issues/20261011-focus)

Contract now admits both handlers and an optional `FocusEvent { target: string, relatedTarget: option<string> }`, and the web JS target uses the browser's bubbling focus events without adding a Tab stop. The synthetic fixture lists movement into, within and out of a wrapper, `focus()`, click, Tab and focused-child removal. The compiler and JS emission regressions pass, and a built JS app driven in Chrome confirms the event order and both IDs for Tab, click, programmatic focus and removal.

Native hosts and the wasm web host (2026-10-12, same branch): host ABI kinds 46 and 47 carry the event to the shared runner (`Event::of_host_kind`); the wasm glue binds DOM `focusin`/`focusout`; AppKit and UIKit share `FocusEvents.swift`, which snapshots the bubbling path at each focus and sends it on a move, a blur and the focused node's (or an ancestor's) release; Linux dispatches the runner's `focus_bubble` path in `set_focus` and, for a removed focused node (a pressed button its own handler removes included), the path snapshotted before the removing commit. Driven with `focus-bubble.contract`'s steps: wasm web, JS web and Linux produce the same log (conformance 11/11 steps, Linux 11/11), and macOS the same except `in:b:after` for `in:b:` after the programmatic step, because an AppKit button does not take the focus on a click, so the field still had it (the platform's focus model, as WebKit's).

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A container sometimes needs to know that the keyboard focus entered or left its subtree. Two examples:

- A collapsed, clipped block expands when Tab reaches a link hidden below its fold, so the focus never lands on something out of sight.
- A row reveals its hover-only actions while any control inside it has the focus.

The web gives this with `focusin` and `focusout`, which bubble, and with `:focus-within`. A listener on a `<div>` hears them without the `<div>` joining the Tab order.

Contract carries `focus` and `blur` only, on the element that takes the focus. An element with a `focus` handler becomes a Tab stop of its own (`docs/contract-grammar.md` §Focus order), so a wrapper cannot listen without adding a stop.

### Current and expected behavior

- **Current:**
  - `contract vocab focusin` and `focusout` say "not a tag or an attribute".
  - `focus-within` is not an attribute either.
  - A `column focus=open` around a `link` is itself a Tab stop, and Tab onto the link does not run `open`.
- **Expected:** `focusin` and `focusout` as DOM's (they bubble, payload as DOM's or none), heard by an ancestor without making it focusable, on every host. Or a `:focus-within` fact a binding can read. Either covers both examples above.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

```text
component X54Focusin
  state opened = 0
  action open
    opened = opened + 1
  view
    main testId="root" padding=24
      column gap=12
        button testId="before"
          text "Before"
        column focus=open testId="wrapper" padding=8
          text `opened ${opened}` testId="count"
          link href="https://example.com" testId="inner"
            text "A link inside"
        button testId="after"
          text "After"
```

View ids: before 3, wrapper 5, inner (the link) 7, after 9.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Vocabulary | `bun exact.mjs contract vocab focusin`, `vocab focusout`, `vocab focus-within` | any | main `0365ad1a4` | "not a tag or an attribute" (all three) | an event an ancestor can hear | CLI output |
| Tab walk, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --json "type before key Space" state` then `"type root key Tab" state` three times; then `"type before key Space" "type root key Tab" "tree count" "type root key Tab" "tree count"` | macOS 26.6.2 | same | focus 3 → 5 → 7 → 9 (the wrapper is a stop); `opened 1` when the wrapper takes the focus, still `opened 1` when the link does | 3 → 7 → 9, with the ancestor's listener run when the link takes the focus | `state`, `tree` |
| Tab walk, web | the same ops with `agent web` | Chrome 154 (the agent's) | same | 3 → 5 → 7 → 9; `opened 1`, then `opened 1` | same as above | `state`, `tree` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- `column focusin=open` (or the chosen form) around the link: Tab from `before` goes to the link, `opened` counts 1, and the column is not a stop.
- `focusout` runs when the focus leaves the subtree, and not when it moves between two of its descendants (or the payload says so, as DOM's `relatedTarget` does).
- The same on macOS, iOS with a hardware keyboard, the web and Linux.

### Constraints and related work

- Workaround: make the hidden content's controls reveal themselves with their own `focus` handlers, or put a separate "show more" stop before the content. Neither handles focus from a click or from `focus(id)`.
- Not tested: iOS and Linux.
- Related: #140 (keyboard capture phase, another event-delivery gap).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:27Z

**Decision: Add bubbling focusin/focusout with enough target information.**

Keep open with the bounded scope below.

An ancestor listener must not become a new Tab stop. Correct the acceptance: DOM focusout also fires when focus moves between descendants; a subtree-exit test uses relatedTarget, or a separate focus-within semantic.

Test movement into, within and out of a subtree, focus(), clicks, Tab and removal. Do not silently give a DOM event focus-within-only behavior.
