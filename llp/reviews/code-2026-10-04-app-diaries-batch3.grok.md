# Code review: app diaries batch 3 (gaps/batch3), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7` via `grok -p`, reasoning effort xhigh, read-only by instruction.
- **Method:** three scoped runs over the integrated branch (A: integration and the driver; B: runtime and hosts; C: compiler, kernel and web build), then one delta run over the twelve fixes and the two origin/main merges. Codex and Astra budgets were exhausted, so there is no second family.
- **Transcription:** each run's findings and verdict, unedited; the progress narration before each run's first finding is dropped.
- **Verdicts:** A LAND WITH FIXES; B LAND WITH FIXES; C LAND WITH FIXES; delta LAND WITH FIXES (all twelve fixed; one should-fix and one nit new).
- **Disposition:** every finding fixed, each with a test, on gaps/batch3:
  - A1 `drag … mouse` throws before any pointer event: `9d75806c8`.
  - A2 a `pick` path with a space is split: `05c44f6e5`.
  - A3 an unmatched test glob skips the diary: `1bb56c4d2`.
  - B1 `display: none` written back as host intent: `c64d0f544`, `312e42609` (the placement test's marker); delta should-fix (the swipe projection; the setter's guard): `69b9a4c05`.
  - B2 iPad named one-character shortcuts dropped: `19b857397`.
  - B3 a JS-target bake that cannot answer yet never subscribes to `data.ready`: `0b90688df`.
  - B4 the JS target re-asks `else` rows: `79ec40a76` (settled rows rather than `void 0` arguments, which would also have asked them).
  - B5 an absolute list correction dropped on a same-commit port resize: `e01265b77`.
  - B6 `writeFile` on the chosen folder is not EISDIR: `c333ba629`.
  - C1 angle units on non-hue colour channels: `990296764`.
  - C2 `contract::check` swallows every runner error: `b444306b6`.
  - C3 key preventDefault/stopPropagation lost under a deferred commit: `dcccdcf57`.
  - Delta nit: `heldMouse` stays set when the down request throws: `85767e58e`.

## A: integration and the driver

Two driver defects are real. I reproduced both against `scripts/agent.mjs` on the web host. A third gap is in the generated command diary.

## 1. must-fix — `drag … mouse` throws before any pointer event

`scripts/agent-drag.mjs:50`, reached from `scripts/agent-test.mjs:112`, calls `s.tap(target, { down: true, mouse: true })`. `scripts/agent.mjs:1017-1019` rejects that pair on web, Linux, and Windows (`mouse cannot be combined with another input mode or an entity target`). On every other host the same flag throws `does not carry explicit mouse clicks`. iOS is refused earlier at `scripts/agent-drag.mjs:21`.

Failure: the authored step `tap "row" drag 0 -100 from 12 8 mouse` (the form `contract/cli/tests/it/tests_decl.rs` already parses and emits) throws before a pointer event. A probe drove a canvas `testId=world` with `s.tap('world', { drag: { dx: 10, dy: 0, from: [12, 8], mouse: true, over: 16 } })` and got `THREW mouse cannot be combined with another input mode or an entity target`. The web carrier at `scripts/agent.mjs:349-367` already presses the left button when a `down` carries `mouse`. The session check runs first. `scripts/windows.test.mjs` locks the public rejection of `tap({ mouse: true, down: true })`.

On macOS the same call throws even though a held contact is already an `NSEvent` left-mouse button (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:347`). On Linux a phase is always a finger: `host/linux/src/agent.rs:125` sets `agent_finger` unless the request says `mouse`, `host/linux/src/presenter/events.rs:232` rejects `mouse` combined with `phase`, and `host/linux/src/agent/contact.rs:37` rejects a `mouse` field on a phase. The shared native `input` (`scripts/agent.mjs:623`) also omits `mouse` on a phase, so Linux and Windows never hear it.

Fix: start the contact from `dragTap` through `carrier.input`, using the same box-offset to viewport translation `tap` uses, and record `s.contact` the way `tap` does after a down. On web, pass `mouse: true` on that `down`. On macOS, omit `mouse` and perform the ordinary contact. On Linux and Windows, add a phase that holds the mouse button (`agent_finger(false)` for the gesture) and forward `mouse` on the phase request. Keep `mouse_request` as the click path. Leave `s.tap({ mouse: true, down: true })` rejected so `windows.test.mjs` stays valid.

## 2. must-fix — a `pick` path with a space is split in half

`scripts/agent-test.mjs:126` resolves the test's paths, joins them with `\n`, and calls `s.type('@' + target, joined)`. `scripts/agent.mjs:1171` splits a `pick` answer on `/\s+/`. One path contains no newline, so a space becomes two paths. The document-picker branch at `scripts/agent.mjs:1161` already splits on `\n` only.

Failure: `pick "photo" "my photo.png"`. The grammar accepts the quoted string. A probe wrote `/tmp/my photo.png`, held a `pick` for `@photo`, and called `s.type('@photo', '/tmp/my photo.png')`. It threw `type @photo: no such file /tmp/my`.

Fix: when the answer contains a newline, split on `\n` only. Otherwise keep the whitespace split so the CLI form `type @id a.png b.png` still works. Have `agent-test.mjs` end the joined string with a newline so one path that contains a space takes the newline split.

## 3. should-fix — a test glob that matches nothing skips the diary

`game/new.mjs:400` (`process.exit(2)` inside the generated `exact.mjs`) returns before `log()` at line 389. The comment at line 384 says the log records which exact2, what ran, and how it ended.

Failure: `bun exact.mjs test ios none/*.test.contract` prints `no test file matches none/*.test.contract`, exits 2, and appends nothing to `.exact/commands.jsonl`. `game/new-app.test.mjs:61` depends on that hole: after the unlogged exit 2, the last two diary lines are the earlier `test ios` (exit 0) and the refused run (exit 7).

Fix: `process.exit(log(2))`, and expect that exit-2 line in `new-app.test.mjs`.

Event-record shapes in `contract/types/src/selection.rs` (`PointerEvent`, `KeyboardEvent`, `ScrollEvent`, `MouseEvent`, `ClipboardEvent`) match `Event::record` and the host payloads read for this pass: web-js records, the wasm press chord, and the scroll 6-tuples on web, Apple, and Linux. Schema prop ids 229–234 are unique (`target`, `enterKeyHint`, then the four accessibility props). The digest pin `0xc78c_1542_c75f_c056` at `kernel/src/wire/codec.rs:656` is the value the a11y merge wrote with the last edit to `schema.json`. The hash was not recomputed here. Launch lines, clipboard, modifiers, dblclick, into, append, reload, nested expect-state, `clock +N real`, and then-landing on the press and tap path are wired through. `Field` and `SecureField` live only on the text-area class. The split sources are under the 1,500-line cap (`host/web-js/rt.js` is 1,499). The tree has no conflict markers. The six integration diffs match those outcomes: the doc fences, the field-class move, the line-cap splits, the command-log assertion, `res(..., carried)` so a dev reload is not re-asked, and the glue fixtures named for the merged glue.

Verdict: LAND WITH FIXES

## B: runtime and hosts

Two host bugs lose user-visible behavior: a `display: none` node can stay hidden after it should show, and iPad shortcuts for Enter, Escape, Tab, Backspace, and Delete are registered and then dropped. The rest of this scope is consistent with the code that is there.

## Must-fix

**1. `display: none` is written back as host intent and sticks.** `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:66` and `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:49`.

The getter returns `super.isHidden`, which is true when either the host hid the view or `display` is `none`. The setter stores that value in `hostHidden` and then sets `super.isHidden = newValue || display == none`. Style updates reapply with `isHidden = hostHidden` (`NodeViewMac.swift:1010`, `NodeViewIOS.swift:1173`).

Any save/restore of `isHidden` therefore copies the CSS bit into host intent:

- A tablist projection saves `tab.isHidden` and later writes it back (`Mac/SegmentsMac.swift:75` and `:47`, `IOS/SegmentsIOS.swift:211` and `:179`).
- A canvas placement does the same. `placementHidden`'s `didSet` saves `isHidden` (`NodeViewMac.swift:138`, `NodeViewIOS.swift:224`). `readPlacements` sets `placementHidden` on every overlay child, including `display: none` (`Mac/GpuMac.swift:284`, `IOS/GpuIOS.swift:387`). `captureEach` skips those children before its own round-trip; `readPlacements` does not.

Failure: a tab, or a canvas child, is `display: none`, so the getter is true while `hostHidden` is still false. Projection or placement reads true and writes true. `hostHidden` stays true. The author then sets `display` to `block` or `flex`. Style apply does `isHidden = hostHidden`, and the node and its subtree stay invisible. On a canvas, the surface can also report the child as shown again and restore the saved `true`, with the same result.

Fix: keep the getter as the effective flag so AppKit and UIKit still skip painting. Save and restore `hostHidden`, not `isHidden`. Placement's `hiddenBeforePlacement` must be the host intent. The setter must not treat a write of "already hidden by CSS" as new host intent.

**2. iPad named one-character shortcuts are captured and dropped.** `host/apple/Sources/ExactKit/IOS/ShortcutsIOS.swift:93`.

`shortcutCommands` registers one `UIKeyCommand` per ARIA chord and sets `wantsPriorityOverSystemBehavior = true` (`ShortcutsIOS.swift:85`). `ExactViewIOS.keyCommands` delivers it to `performShortcut` (`IOS/ExactViewIOS.swift:137`).

`ChordIOS` stores named keys as `Enter`, `Tab`, `Escape`, `Backspace`, and `Delete`. UIKit delivers those as one-character inputs (`"\r"`, `"\t"`, `"\u{1b}"`, `"\u{8}"`, `inputDelete`). `performShortcut` calls `ChordIOS.name(of:)` only when `input.count != 1`, so the reconstructed key is the raw character. `matches` then compares `"\r"` with `"Enter"` and finds no button. The command has already taken priority over the system, so the key is gone.

Failure: a button with `aria-keyshortcuts="Escape"` or `"Meta+Enter"` (the gallery and onboarding cases) does not run on an iPad hardware keyboard. Escape does not dismiss, and Enter does not activate. The same drop hits Tab, Backspace, and Delete. Space, letters, arrows, and F-keys work. `NavigationRulesTests` sets `Meta+Enter` and never calls `performShortcut`.

Fix: resolve every input with `ChordIOS.name(of: input) ?? input` before `ChordIOS.init`, including count-1 inputs. `name(of:)` already maps those constants back to the ARIA names.

## Should-fix

**3. A JS-target bake whose source cannot answer yet never subscribes to `data.ready`.** `host/web-js/rt.js:437`.

When the first evaluation is still the bake and `ask` returns neither a value nor a request, `if (baked && eq(a, r.settled)) return r.value` returns before `data.ready` at line 440. The native runner marks that compiled row stale (`runner/src/runner/settlement.rs:518`) and `data_ready` asks it (`runner/src/runner/kept.rs:149`). `host/web-js/rust-data.js:8` says the runtime asks again at `ready`, and line 146 only drains callbacks that were queued.

Failure: a published JS target shows the baked answer, then the Rust module loads after first paint and replaces `data.answer`. This resource was never queued, so the bake stays on screen. Keep the `carried` dev-reload exemption (`res` already takes `carried`).

Fix: do not return there. Register `data.ready` and force, as the non-bake path below that line already does.

**4. The JS target re-asks `else` rows at first evaluation.** `host/web-js/src/emit.rs:412` and `host/web-js/rt.js:374`.

`initial_args` is emitted for every non-fact, non-reader row, including a row that is another resource's placeholder. `res` sets `baked` from that, and the first pass calls the source. The native runner never marks an `else` row stale (`settlement.rs:515`), because each one was a worker turn.

Failure: `else preview()` on the JS target runs `preview` again at launch. The web runner keeps the bake. A different preview answer replaces what the waiting resource is showing, and the worker turn runs again.

Fix: emit `void 0` for `initial_args` on placeholder rows so `baked` is false and the settled short-circuit keeps the bake.

**5. Web drops an absolute list correction when the port resizes in the same commit.** `host/web/collection-glue.js:529`.

The relative branch now applies a correction planned before a same-commit resize. The absolute branch still requires `s.dimensions === null || s.dimensions === dimensionsOf(g)` and skips the correction when the port changed. Apple accepts that correction by passing the post-resize sequence when the correction was planned at the pre-resize sequence (`host/apple/Sources/ExactKit/Collection.swift:395`). Linux accepts `resized_from` for the same offset (`host/linux/src/presenter/collection.rs:108`).

Failure: a list following its end, in the commit where the composer shrinks back after a send, stays above the new end on the web. Apple and Linux land on the end.

Fix: apply that absolute correction when the dimension change is the port resize in this commit, using the same sequence rule as Apple.

**6. `writeFile` on the chosen folder itself is not `EISDIR`.** `host/web/documents-glue.js:94`.

`readFile` checks `h.kind === 'directory'` and throws `EISDIR` (line 79). `writeFile` uses `found.entry` when the path is the chosen handle (`locate` returns that at line 39) and calls `createWritable`. A directory handle has no `createWritable`, so this throws `TypeError`. `failed` maps only `TypeMismatchError` to `EISDIR` (line 27), and this error's code is `failed`. A nested path whose last name is an existing directory still goes through `getFileHandle` and maps correctly. Native `run_document` writes through `perform`, which returns `EISDIR`.

Failure: `storage.fs.writeFile("doc:/1/Folder", bytes)` after the person picks a folder rejects with `failed` and "createWritable is not a function". Native code and the glue's own `MISMATCH` table say `EISDIR`.

Fix: if `found.entry?.kind === 'directory'`, throw `failure(..., 'EISDIR')` before `createWritable`, as `readFile` does.

## Checked, no defect found

Hermes provisioning holds the source-build lock, checks CMake before cloning, and refuses a cache at the wrong pin with a recovery path. iOS `NavigationHost.reset` from an empty route list runs inside `sync` while `syncing` is true, so `modalDidDismiss` cannot re-enter it. AppKit checkable roles, described-by, and field cells match the tree tests. The iOS radio trait is cleared before `setAccessibilityChecked`. Clipboard delivery and modal shortcut scope match their comments. Media retirement, audio lowering, list anchoring at offset 0, and Linux scroll extents are consistent with the writers that use them.

Verdict: LAND WITH FIXES

## C: compiler, kernel and web build

Three defects in the compiler, the web build check, and the JS key path. Button geometry, CSS `order`, `feOrder`, `currentcolor`, press-haptic, and `utcOffset` match the code and Chrome 154.

**Should-fix** — `motion/src/color/css.rs:171`

`component()` turns `deg`, `grad`, `rad`, and `turn` into numbers on every channel. Chrome 154 (headless, canvas `fillStyle` seeded to `#0000ff`) drops these and leaves `#0000ff`: `rgb(90deg 0 0)`, `rgb(90deg, 0, 0)`, `rgb(255 0 0 / 90deg)`, `hsl(120 90deg 50%)`, `lab(50 40deg 20)`. The same parser accepts them: `rgb(90deg 0 0)` becomes `rgb(90, 0, 0)`, an angle alpha clamps to `1`, and `hsl` saturation `90deg` becomes `90%`. Both hosts then emit that sRGB color. Hue angles are fine: `hsl(120deg, 100%, 50%)`, `hsl(0.5turn 100% 50%)`, and `lch(50 40 90deg)` match Chrome.

Fix: accept angle units only for hue (`hsl`/`hwb` channel 0, `lch`/`oklch` channel 2). Numbers, percentages, and `none` stay the only forms for `rgb`, `lab`, `oklab`, `color()`, and alpha.

**Should-fix** — `contract/cli/src/lib.rs:714`

`check()` turns every `BakeError::Runner` into success:

```714:716:contract/cli/src/lib.rs
    match first_frame(plan, Unanswered, false) {
        Err(BakeError::Runner(_)) | Ok(_) => Ok(()),
        Err(lint) => Err(lint),
    }
```

Native `bake` uses `first_frame(...)?` at line 646, so the same boot fails the build. The comment only excuses an unanswered `else source()` row, which is `RunnerError::Data { error: Unavailable }` from `runner/src/runner/settlement.rs:145`. Any other boot failure is discarded, and `lint()` never runs because it sits after `Runner::boot` (`runner/src/runner.rs:923`). A derive that traps at boot (list steps past `MAX_LIST_STEPS`, `1 << 16`, in `runner/src/vm.rs:43`, or any other `Trap` returned at `settlement.rs:334`) passes `exact-web-js` and fails a native bake. Shape and layout lints still run when boot succeeds.

Fix: swallow only `RunnerError::Data { error: DataError::Unavailable(_), .. }`. Propagate `Trap`, `Kernel`, `Shape`, `DeriveType`, `NotOneRoot`, and the rest.

**Should-fix** — `host/web-js/rt.js:183` and `host/web-js/shared.js:108`

`stopPropagation` and `preventDefault` run inside the commit tail (`rt.js:178`). On a plan that calls `pr()` (`exit-animation` or `layout-transition`), `Sh.commit` queues that tail in `document.startViewTransition` whenever a transition is already pending (`shared.js:90`) or the commit has a shared-element leaver (`shared.js:108`), then returns `true` before the callback (`shared.js:159`). The key handler clears `KeyEvent` in `finally` (`rt.js:786`) before the command runs, so `Hosts.stopPropagation` (`rt.js:214`) sees no event and never sets `$stopped`. Ancestor `key` handlers still run, and `preventDefault` is a no-op after dispatch. The wasm host applies the command inside `send()` before the keydown returns (`host/web/glue.js:581`, `glue.js:768`).

Fix: apply `preventDefault` and `stopPropagation` to the current key event before `commit` returns, including when the tree update is deferred.

Verdict: LAND WITH FIXES

## Delta: the twelve fixes and the origin/main merges

**A1. fixed.** `drag … mouse` starts at `scripts/agent-drag.mjs:53` through `carrier.input` at the viewport point, with `mouse` on that down for web, Linux, and Windows. Later phases keep the button (`scripts/agent.mjs:639` `heldMouse`, web `contact.mouse` at `scripts/agent.mjs:366`). macOS omits `mouse`. `s.tap({ mouse: true, down: true })` still throws at `scripts/agent.mjs:1037`. iOS still refuses at `scripts/agent-drag.mjs:21`.

**A2. fixed.** `pickedPaths` (`scripts/agent-keys.mjs:116`) splits on `\n` when the answer contains one, and on whitespace otherwise. The authored pick joins paths with a trailing newline (`scripts/agent-test.mjs:126`).

**A3. fixed.** A glob that matches nothing is `process.exit(log(2))` (`game/new.mjs:402`). `game/new-app.test.mjs:61` expects that exit-2 diary line, then the exit-7 run.

**B1. partly.** Tabs (`SegmentsMac.swift:75`, `SegmentsIOS.swift:211`) and canvas placement and capture (`NodeViewMac.swift:150`, `NodeViewIOS.swift:236`, `GpuMac.swift:251`, `GpuIOS.swift:360`) save `hiddenByHost`. The setter still stores every write (`NodeViewMac.swift:80`, `NodeViewIOS.swift:63`), and `SwipeActionsIOS.hide` still saves and restores `isHidden`.

**B2. fixed.** `performShortcut` resolves every input through `ChordIOS.names(of:)` (`ShortcutsIOS.swift:99`), including one-character Enter, Tab, Escape, Backspace, and Delete.

**B3. fixed.** A bake whose source is not ready stays shown and not pending, subscribes to `data.ready`, and is forced on ready (`host/web-js/rt.js:435`). The `carried` short-circuit is unchanged.

**B4. fixed.** An `else` row is emitted with `carried` (`host/web-js/src/emit.rs:381`), so `baked` is false and the settled match at `host/web-js/rt.js:404` keeps the build-time answer.

**B5. fixed.** An absolute correction whose sequence is still the current one lands when this commit resized the port (`host/web/collection-glue.js:534`). A sequence that has already moved does not.

**B6. fixed.** `writeFile`, `atomicWriteFile`, and `appendFile` throw `EISDIR` when the chosen handle is a directory (`host/web/documents-glue.js:97`). The merge at `90fcbb0d4` kept those three expectations in `host/web/request-refusal.test.mjs`.

**C1. fixed.** Angle units parse as `Arg::Angle` and are accepted only for hue (`hsl`/`hwb` channel 0, `lch`/`oklch` channel 2). `rgb`, `lab`, `oklab`, `color()`, and alpha reject them (`motion/src/color/css.rs:250`, `:269`, `:363`).

**C2. fixed.** `check` succeeds only for `Ok` or `RunnerError::Data { error: Unavailable(_) }` (`contract/cli/src/lib.rs:717`). Other boot failures propagate.

**C3. fixed.** `preventDefault` and `stopPropagation` run while `KeyEvent` is still the keydown, and are removed from the tail (`host/web-js/rt.js:168`), including when `shared.js` defers the tree update.

**New defects**

- **should-fix** — `host/apple/Sources/ExactKit/IOS/SwipeActionsIOS.swift:300` (restore at line 220; setter `NodeViewIOS.swift:63`). A swipe action or sibling with `display: none` is projected: `hide` reads `isHidden` (true because of CSS) and `restore` writes that true back, so `hostHidden` stays true. After the author sets `display` to `block` or `flex`, style apply does `isHidden = hostHidden` and the node stays invisible. Save and restore `hiddenByHost`. In the setter, a write of "already hidden" while `display` is `none` and `hostHidden` is false must not become host intent.
- **nit** — `scripts/agent.mjs:639`. `heldMouse` is set before `ask`. If that `ask` throws, the flag stays set, and a later finger `down`/`move`/`up` on Linux or Windows is sent with `mouse: true`. Clear `heldMouse` when the `down` request throws.

The merge resolutions in `db003d02c` and `90fcbb0d4` keep both sides' behavior (kernel paint isolation in the document walk, `doc:` refused before the Windows filesystem branch, an `app:/` destination required only for an `app:/` source, blur and focus still dispatched). They do not undo the twelve fixes.

Verdict: LAND WITH FIXES
