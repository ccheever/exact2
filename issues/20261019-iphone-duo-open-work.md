# iPhone Duo and real touches on iOS: the open work, with the context to pick it up

**Status:** Open
**Systems:** Apple host (iOS), touch runner, iPhone Duo, smokes, docs
**Author:** Claude (Opus 5.5) for ide
**Date:** 2026-10-19
**Severity:** P2
**Related:** QUEUE.md (the entries named in each task below); LLP 1008 §9 (the Duo in the Apple host), LLP 1078 (foldables follow the web; `apps/duo-lab`; `smoke duo`), LLP 1080.000 (real touches on iOS: the touch runner, the dispatch log, D5's `viewGone`), LLP 1123 (content owners and local safe areas), LLP 1127 (a list and its detail as UIKit's split view, draft); `docs/recipes/foldables.md`, `docs/recipes/ios-driving.md`, `docs/testing.md`; PRs #782, #793, #795, #797, #800, #805, #808, #810

This file holds the open iPhone Duo and real-touch work as of 2026-10-19. Each task says what is wrong, what was measured, where the code is, how to reproduce it, what to try, and when it is done. QUEUE.md keeps each item's short state; this file adds the context a new agent needs. When a task lands, update or remove its QUEUE entry and its section here, and close this issue when no task is left (`bun scripts/issue.mjs close …`).

## What you need to know first

### The rules this work follows

- **UIKit is the source of truth.** Decide platform behavior from a measurement of a plain UIKit app beside Exact on the same simulator, and from Apple's Human Interface Guidelines. Exact matches UIKit where they differ, unless the HIG says otherwise. Do not decide by reading alone or by eye.
- **Use the Xcode 27.1 release**, `/Applications/Xcode-27.1.app` (build 27A9275), selected with `DEVELOPER_DIR=/Applications/Xcode-27.1.app/Contents/Developer`. Do not use or cite the 27.1 beta or RC. The Duo needs the iOS 27.1 runtime (24A94425), which supports only the iPhone Duo; other iPhones use iOS 27.0 (24A434). See `~/AGENTS.md`.
- **Name the Duo's postures in the HIG's terms**: fully open, partially folded, closed. Hinge angles (180°, 130°, 0°) appear only where a command takes one.
- **No reports to Apple.** UIKit behavior that Exact matches is recorded in QUEUE or the LLPs, with "look again on a later release" where useful (ide, 2026-10-19).
- **Tests follow `docs/testing.md`**: wait on events, not sleeps; combine setups; run the targeted tests while you iterate.
- **PRs get the full pass before they open**: self-audit and simulator verification first; then an adversarial review (Codex `gpt-6.1-sol` for code PRs; none for a QUEUE-only change); then a code simplification pass and a writing pass over all prose (comments, LLPs, QUEUE, guides, the commit message and PR text); then rebase and verify again on the rebased tree. A PR that opens before that is a draft. Commit bodies have Why, How and Test Plan sections (see `~/AGENTS.md`).

### The simulators

Create your own simulators, with a tag of yours in each name so that other sessions leave them alone:

```sh
export DEVELOPER_DIR=/Applications/Xcode-27.1.app/Contents/Developer
xcrun simctl create "Duo <tag>" com.apple.CoreSimulator.SimDeviceType.iPhone-Duo com.apple.CoreSimulator.SimRuntime.iOS-27-1
xcrun simctl create "iPhone 18 Pro <tag>" com.apple.CoreSimulator.SimDeviceType.iPhone-18-Pro com.apple.CoreSimulator.SimRuntime.iOS-27-0
xcrun simctl boot <udid>
```

Select one with `EXACT_SIM=<udid>` or `--sim <udid>`. The async lane (`bun scripts/async.mjs`) runs the touch smoke, and on 2026-10-19 it ran it on another session's booted iPhone simulator, so a booted simulator of yours can get a touch runner and Caltrain from the lane's tree; those are not your processes to stop. Use two Duo simulators when one runs the XCTest suite and the other runs smokes or probes.

### The Duo's geometry and how to move it

The facts below are measured under Xcode 27.1 with the iOS 27.1 runtime (LLP 1008 §9, LLP 1078; `scripts/smoke-duo.mjs` holds the full table, `TURNED`).

- **Inner display, upright:** 951×669 points; an 84-point safe area on the right (the status bar's column) and 34 at the bottom. A navigation bar puts its items in a vertical bar at one side (`traitCollection.verticalBarEdge`) and shows a 28-point title in a 58-point bar; a large title shows only with the inner display turned on its side (#797).
- **Partially folded:** the same window, with a 40-point division at x 455.5 (a book fold), so the segments are (0, 0, 455.5, 669) and (495.5, 0, 455.5, 669). The document reports `devicePosture` "folded" and two horizontal segments (`exactViewport()`, `env(viewport-segment-…)`). Folded the other way (a laptop), the segments are stacked.
- **Closed (outer display):** 466×678 points, the same side and bottom safe areas.
- **Folding:** `scripts/duo/hinge_helper.c` posts the HID event of Xcode's hidden hinge slider from inside the simulator. Compile it with the selected Xcode's simulator SDK and spawn it (`bun scripts/smoke.mjs duo` does this per run):

  ```sh
  xcrun -sdk iphonesimulator clang -arch arm64 -mios-simulator-version-min=17.0 -O2 \
    -framework IOKit -framework CoreFoundation -o /tmp/hinge_helper scripts/duo/hinge_helper.c
  codesign -f -s - /tmp/hinge_helper
  xcrun simctl spawn <udid> /tmp/hinge_helper set 0      # closed; 130 partially folded; 180 fully open
  xcrun simctl spawn <udid> /tmp/hinge_helper orient landscape-right   # portrait, pud, landscape-left
  ```

  `devicectl device orientation set` does not turn the Duo, and Xcode 27.1's `devicectl` has no `device motion`, so nothing reads the hinge back: judge a pose by what the app's session reports (`layout`'s `viewport`, `env['device-posture']`, the segments).

### Real touches on iOS (LLP 1080.000)

- `--touch platform` makes every agent `tap` and drag a real `UITouch`, played by a long-lived XCUITest runner (`host/apple/touches.swift`, driven by `host/apple/touches.mjs`) through XCTest's event synthesizer. Without the flag, the agent activates its target itself.
- The app's window (`ExactWindow` in `host/apple/Sources/ExactKit/IOS/TouchIOS.swift`) logs each touch it dispatches. The driver injects, then polls that log for one new touch that began and ended, and replies `delivery: platform` with where it landed. "no touch reached the app's window within 1 s" means the log saw nothing.
- XCTest presses its main display unless an event names another, and the Duo's main display is the outer one. The runner reads each display's size once, from a screenshot, when it starts (`Displays` in `touches.swift`), and names the display whose size matches the screen size the host reports with its aim (#782).
- A tap whose view left the window before the finger lifted replies `viewGone: true` (#808, LLP 1080.000 D5); the driver refuses nothing for it.
- The runner is detached and kept between drives; its state is in `$TMPDIR/exact-touches-<udid>.{runner.json,xctestrun,log}`.

### Commands you will use

```sh
export DEVELOPER_DIR=/Applications/Xcode-27.1.app/Contents/Developer
bun host/apple/build.mjs --ios caltrain-apple                 # Caltrain for the selected simulator
bun host/apple/build.mjs --ios duo-lab-apple                  # duo-lab, which smoke duo drives
bun host/apple/build.mjs --ios caltrain-apple --host          # the host app for smoke duo's host leg
bun scripts/smoke-touch.mjs --build --sim <udid>             # real taps, drags and the grouped list
EXACT_SIM=<duo udid> bun scripts/smoke.mjs duo [--only insets,keyboard-bar,lab,host,rotate]
EXACT_SIM=<udid> bun host/apple/build.mjs --test --ios       # the UIKit XCTests
bun scripts/agent.mjs ios --app caltrain --touch platform "clock settle" "tap change-station"
bun test scripts/agent-drag.test.mjs                         # the driver's real-touch logic
bun scripts/caps.mjs                                         # size budgets, a byte cap per recipe among them
```

A drive or smoke refuses a stale build ("ios build is stale … run bun host/apple/build.mjs …"): build first. The iOS test build regenerates Caltrain's `entry.rs`, after which a drive of the Caltrain built before refuses it as stale, so do not run a build in a worktree while a drive runs from it.

## The tasks

### 1. The first real tap after the Duo folds closed misses the outer display

QUEUE: "The first real tap after the iPhone Duo folds closed can miss the outer display".

**Measured (2026-10-19, Xcode 27.1).** On the closed Duo, Caltrain's `tap change-station` under `--touch platform` landed and opened the stations in 14 of 14 taps that were not the first after a fold. The first real tap after a fold from fully open to closed missed in 8 of 12 folds with "no touch reached the app's window within 1 s"; the driver then stopped the runner, and the next tap landed. A runner kept from the fully open pose landed as often as one started closed, so the runner's display table is not shown to be the cause. A loop that measures it (a reconstruction; the runs that gave 8 of 12 differed in detail):

```sh
H=/tmp/hinge_helper D=<duo udid>
for i in 1 2 3 4; do
  xcrun simctl spawn $D $H set 180 >/dev/null; sleep 3
  bun scripts/agent.mjs ios --app caltrain --touch platform "clock settle" "tap change-station" >/dev/null 2>&1
  xcrun simctl spawn $D $H set 0 >/dev/null; sleep 3
  for n in first second; do
    r=$(bun scripts/agent.mjs ios --app caltrain --touch platform "clock settle" "tap change-station" 2>&1 \
      | grep -oE "delivery platform|no touch reached[^;]*" | head -1)
    echo "$i closed $n: $r"
  done
done
```

**A hypothesis to test first (not verified).** The runner names the display by the screen size the host sends with its aim. If, right after the fold, the app still reports the inner display's size (951×669), the runner presses the inner display, which is off, and nothing reaches the window; the next aim reports 466×678 and lands. One related measurement supports looking there: in a resizable app session, the agent's `screen` trails a resize by one step, as `UIWindowScene.screen` does (QUEUE, "iPhone Duo: resizing is not in `smoke.mjs duo`"). Log, for the first tap after a fold, the aim's screen size, the display the runner chose (`Displays.showing`), the scene's `screen` and the window's `screen.bounds` at that moment, and when each changes after `hinge_helper set 0`. Other candidates: the first event after the display switch is dropped by the simulator; `XCUIScreen.screens` changes order or ids after the fold.

**Done when** the first tap after a fold lands as reliably as the others (measure at least 20 folds), with the cause named in the commit; then the smoke's closed legs can use real touches, and the QUEUE entry goes.

### 2. The first tap on a grouped row's switch after a launch can miss

QUEUE: "A real tap on a grouped row's detail button does nothing while UIKit rebuilds the button, and the first on its switch can miss too" (its last part; the detail button is answered by #808).

**Measured (2026-10-18/19, iPhone 18 Pro simulator, iOS 27.0).** The touch smoke's grouped fixture (`contract/corpus/grouped-touch.contract`: a row with a switch, a row with a detail button, a row that opens) was driven with eight real taps a launch (switch, switch, detail, row, detail, switch, row, detail) by a probe (appendix A). In the 160 launches whose logs timed the launch, 3 first taps were lost, every one on the switch and every one the first tap of its launch. Of first taps less than 0.40 s after the list's cells were first configured, 0 of 137 were lost; of those 0.40 s or later, 3 of 23. In a lost tap the switch stayed enabled and in the window, its cell unchanged, and its `UILongPressGestureRecognizer` ended in state 5 (failed), where every flip that worked ended in state 3 (ended). No accessory was replaced during it. A run that logged the switch's window moves, Exact's `setOn` and enable changes and every `_setNeedsUpdateAccessories` saw nothing between 0.2 and 1.0 s after launch (UIKit's accessory updates come at 0.0, 1.0–1.6 and 2.0–2.6 s) and lost no switch tap in 60 launches. Lost taps, detail and switch together, were more common under load: 1 in 320 taps with none, 12 in 1760 while another simulator ran `bun scripts/smoke.mjs duo`, so run the probe under that load.

**What to do.** First establish UIKit parity: a plain UIKit inset-grouped list with a `UISwitch` accessory, its first tap at 0.4–1.0 s after the cells are configured, through the same XCTest synthesizer (a down and a lift 50 ms later, no idle wait), over about 100 launches. The earlier probe app, `ListTapProbe`, was built for the detail button and has a switch row too (an XcodeGen `project.yml`, a `UICollectionView` inset-grouped list, a UI test that taps at set times); it is on the author's machine at `~/.config/tuft/sessions/1791430083.031489/notes/listtap/`, and is quick to rebuild from that description. If UIKit loses the first switch tap too, record it as UIKit's behavior in QUEUE (the smoke already settles first and could tap the switch again when nothing changed, as it does for the detail button). If only Exact loses it, find what Exact does in that window: log the touch's `gestureRecognizers` and their states at each phase, and the switch's `_UILiquidLensView`.

**Done when** the cause is known and either fixed or recorded as UIKit's, and the QUEUE entry says which.

### 3. The agent's wheel leaves a large title open on the Duo's inner display

QUEUE: "The agent's wheel leaves a large title open on the Duo".

**Measured (2026-10-16, an earlier iOS 27.1 runtime; not yet with 24A94425).** On the Duo's inner display (regular width), UIKit collapsed a large title only while the scroll view moved: an animated `setContentOffset` of 300 points moved the title away and the scroll view's top inset went from 82 to 24 points; an unanimated one to the same offset left the title open, in a plain UIKit app as in duo-lab's Reflow. The agent's `tap <scroller> wheel` sets the offset without motion (`Agent.scroll` in `host/apple/Sources/ExactKit/IOS/AgentIOS.swift`), so a drive sees the large title open where a finger would have collapsed it. On an iPhone the title collapses either way. With 24A94425 the upright inner display shows no large title (#797), so measure where one shows: the inner display turned on its side.

**The plan** (QUEUE's entry, from a design review by Codex Sol, its option B). The wheel stays instant on every host (LLP 1012). A drive that needs a person's scroll uses a real drag under `--touch platform`, and the guidance goes into `docs/recipes/ios-driving.md`, with a short link from the foldables recipe. The review rejected animating the wheel on one host, and calling `UIScrollViewDelegate` methods or private selectors to fake the end of a scroll. No wheel code changes.

**What to do.**
1. Measure under the release runtime, from identical starting geometry: a plain UIKit list and duo-lab's Reflow on the inner display, an iPhone 18 Pro as the control; for the instant wheel, the recognized drag (`AgentPanIOS.scrollDrag`, which also writes an unanimated offset), an animated offset, and a real drag under `--touch platform`. Record the offset, the adjusted top inset, the bar's and the title's frames and a content landmark through the motion and after it, in frozen and platform timing, and check that `clock settle` returns only after the native scroll and the bar have stopped (`nativeInFlight()` in `AgentIOS.swift` is not a general deceleration check). Do not reuse the 82→24 values; measure them again.
2. Qualify the real drag: it reaches the inner display, its travel and timing are right, the title collapses, a reverse drag restores it, and nothing moves after settle. A drag that ends past the screen falls back to the recognized drag (`scripts/agent-drag.mjs`, "past the screen a drag is the scroll view's own (recognized)"), so keep both ends on screen and check `delivery: platform`.
3. Write the guidance in `docs/recipes/ios-driving.md` (instant wheel versus real drag; narrow the claim that the recognized drag "scrolls as a finger would"; on-screen ends and the delivery check). The recipe is near its 3072-byte cap: put lookup detail in `docs/contract-for-agents.md`. Keep the wheel's exact-delta tests.

**Done when** the measurements are recorded, the real drag passes them, the recipe says so, and the QUEUE entry goes.

### 4. Two gaps in the foldables guidance

Found by a guidance evaluation on 2026-10-17 (round 2): a model wrote apps from the public docs alone, and each was driven on an iPhone and the Duo. The raw record is on the author's machine at `~/.config/tuft/sessions/1791430083.031489/notes/guardrail-eval-2/` (`REPORT.md`, the apps `groceries-b` and `calm`).

1. **A single pane on the trailing segment under a native large title.** Following `docs/recipes/foldables.md`, an app moved its one pane to segment 1 when partially folded; the native large title stayed on segment 0, over nothing of the pane's. The recipe says nothing about the bar. This was seen with an earlier iOS 27.1 runtime; with 24A94425 the upright inner display shows a 28-point title and a vertical bar instead (#797), so first check whether the gap is still there. duo-lab's two answers are `navigationBar="none"` with a header inside the pane, or staying full width under the native bar (Reflow). Measure what UIKit does with a plain app first (its bar and title partially folded), then say in the recipe which to choose and when.
2. **Art behind a native grouped list.** An app put a decorative, `aria-hidden` SVG behind a native grouped list; the list's opaque grouped background hid it except in the status-bar strip, and no guidance says how to do this. Find what UIKit allows (candidates: a clear list background over a background view, the cells' `backgroundConfiguration`) and what Exact exposes, then document it, or record the gap in QUEUE if Exact cannot express it yet.

Mind the recipe's byte cap (`bun scripts/caps.mjs`); lookup detail goes to `docs/contract-for-agents.md`.

**Done when** both are answered in the guidance, each checked by driving a small app on the Duo.

### 5. The closed Duo's rotate leg failed once under load

Not in QUEUE yet. In one of three `bun scripts/smoke.mjs duo` runs, run beside the grouped probe (task 2) on another simulator, the rotate leg's closed pose failed: after `hinge_helper orient landscape-right`, CoreDevice reported the device turned, but over the leg's wait (50 layout reads 100 ms apart) the session still reported `portrait 466×678` where `landscapeLeft 678×466` was expected. The next turn, `rotate-closed-pud` (upside down, which an iPhone app does not take, so it expects the last orientation to hold), failed with it. The screenshot kept at the failure shows the app upright on the outer display, 466×678. Earlier, 13 of 13 rotate-leg runs passed, eight of them during an XCTest run on another simulator, and a timing print in ten of them put each closed turn's scene at most 211 ms after CoreDevice's report. The record is on the author's machine at `~/.config/tuft/sessions/1791430083.031489/notes/rotate-fail-2026-10-19/` and `notes/g-load.log`.

**What to do.** Reproduce under the same load (`--only rotate` in a loop beside a probe or an XCTest run), timing each closed turn from CoreDevice's report to the scene's `interfaceOrientation` change, and read the layout again well after the leg gives up, to learn whether the turn comes late or never. If late, decide from the distribution whether the wait should be an event (the scene's orientation change) rather than a count of reads; if never, find what drops the turn, in a plain UIKit app as well. Record it in QUEUE either way.

**Done when** the cause is known and the leg is reliable under load, or the failure is recorded in QUEUE with its rate.

### 6. Tests that fail now and then in full parallel runs

Each passes when run alone. Look at them when one blocks a PR, and record new ones in QUEUE.

- `DueBeforeFrameIOSTests.testADriverOwnedClockAdvancesOnlyWhenTheDriverSaysSo` (iOS, once, on the Duo; QUEUE has it).
- `AccessibilityClientMacTests` (macOS; QUEUE: "Mac accessibility-client tests lose the first notification intermittently").
- `ContextMenuMacTests` and `KeptDocumentsMacTests` (macOS, `bun host/apple/build.mjs --test`): in three of four full runs on 2026-10-19, `ContextMenuMacTests` failed three times and `KeptDocumentsMacTests` twice; not in QUEUE.
- `TextRasterRaceIOSTests` (iOS, in parallel runs on 2026-10-19); not in QUEUE.

### 7. Duo items that wait on others or on a later release

These are known, and need no work from you unless they change:

- **A covered route column with a width of its own runs past the screen** (QUEUE): to be fixed by LLP 1123 stage 2's wrapper (D3), assigned to Krzysztof.
- **A grouped list in a pane that ends at the fold runs its cards to the fold** (QUEUE): matching UIKit's split view would mean telling UIKit the fold is a safe area of the list's view (LLP 1078, LLP 1123).
- **LLP 1127** (a list and its detail as UIKit's `UISplitViewController`): a draft, r18, waiting for Charlie's reading.
- **Resizing and Split View** (QUEUE: "iPhone Duo: resizing is not in `smoke.mjs duo`…"): resizing is measured but not in the smoke; Split View beside another app is unmeasured.
- **Look again on a later release:** navigation bar items under the status capsule; UIKit's alerts and sheets on a fold (QUEUE has both); the Duo keyboard's crash on a fold, to be recorded if it recurs.

## Appendix A: the grouped-tap probe

It repeats the touch smoke's grouped sequence and prints each tap. Run it from the repository root with `EXACT_SIM=<udid> bun <path>/grouped-probe.mjs <launches>`, after `bun scripts/smoke-touch.mjs --build --sim <udid>` (which builds Caltrain, whose bundle carries the fixture).

```js
import { spawnSync } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import process from 'node:process';

const ROOT = process.cwd();
const { open } = await import(resolve(ROOT, 'scripts/agent.mjs'));
const { HOST_DEV } = await import(resolve(ROOT, 'scripts/app.mjs'));
const rounds = Number(process.argv[2] ?? 10);
const plan = resolve(mkdtempSync(resolve(tmpdir(), 'grouped-probe-')), 'grouped-touch.plan');
const c = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/grouped-touch.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
if (c.status !== 0) { throw new Error(c.stderr); }
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
let lost = 0, taps = 0; const gone = { unchanged: 0, changed: 0 };
for (let round = 0; round < rounds; round++) {
  const s = await open({ host: 'ios', app: 'caltrain', plan, touch: 'platform' });
  try {
    await s.clock('settle');
    const facts = async () => byTestId(await s.tree(), 'facts')?.props.text;
    let [on, infos, opens] = [false, 0, 0];
    for (const target of ['toggle', 'toggle', 'info', 'open', 'info', 'toggle', 'open', 'info']) {
      if (target === 'toggle') { on = !on; } else if (target === 'info') { infos++; } else { opens++; }
      const was = await facts();
      let r = await s.tap(target);
      await s.clock('settle');
      let seen = await facts(); const want = `${on} ${infos} ${opens}`;
      if (r.viewGone) {
        console.log(`${round} ${target} viewGone on ${r.landed?.view}, facts ${seen === was ? 'unchanged' : seen === want ? 'changed' : seen}`);
        gone[seen === was ? 'unchanged' : 'changed']++;
        if (seen === was) { r = await s.tap(target); await s.clock('settle'); seen = await facts(); }
      }
      taps++;
      console.log(`${round} ${target} ${seen === want ? 'ok  ' : 'LOST'} view=${r.landed?.view} began=${r.touch?.began}`);
      if (seen !== want) {
        lost++;
        [on, infos, opens] = seen.split(' ').map((x, i) => i === 0 ? x === 'true' : Number(x));
      }
    }
  } catch (e) {
    console.log(`${round} error ${e.message}`);
  } finally { await s.close(); }
}
console.log(`lost ${lost} of ${taps}; viewGone ${gone.unchanged} with nothing changed, ${gone.changed} after the tap acted`);
```

For task 2, add timing: log each launch's time and each tap's `touch.began` against the first cell configuration (a temporary log line in the grouped list's cell configuration), and count first-tap losses by that delay.
