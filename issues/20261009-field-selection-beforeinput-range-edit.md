# Text fields: selectionchange on caret moves, setRangeText and a cancelable beforeinput (rest of #125)

**Status:** Open
**Systems:** Contract, GUI hosts, text editing
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/275

## Current scope

Field selectionchange/beforeinput remain open. Native range-edit execution is implemented; its verification limits are recorded below. The original requested undo step is superseded; Charlie's ruling and the web slice are recorded below. Preserve selectionMode, document deviations, and test IME/noncancelable edits in the remaining work.

### Rangeedit first slice — 2026-10-12

Charlie subsequently ruled: “setRangeText: (b) fire one input
(insertReplacementText) as recorded deviation; selectMode preserve; no undo
step — for now”. This supersedes the original requested undo step and pins
the extra input as a deviation from HTML.

The first slice implements `setRangeText(id, text[, start, end[, selectionMode]])`
on the web's JS and wasm targets for plain text inputs and textareas. It uses
the DOM method, preserves selection by default, then emits one noncancelable
`insertReplacementText` input to synchronize the app's bound value. It adds
no undo step. LLP 1069.001 and the author guides record the deviation and limit.

### Native continuation — 2026-10-12

The same host command now executes on AppKit's NSTextField field editor and
NSTextView, UIKit's UITextField and UITextView, and Linux's field model.
It keeps focus, implements all four DOM selection modes with `preserve` as
the default, and emits one replacement input. `InputEvent.inputType` now
reports `insertReplacementText` on all implemented hosts, including both web
targets; other input types and `data` remain future work. Newline handling
counts the raw replacement for selection adjustment before sanitizing the
value, as Chrome does. A selection start exactly at the replaced range's end
moves to the replacement's start in preserve mode (also checked in Chrome).

No host adds an undo step. **Native limit:** a changed Apple buffer clears
that field's existing undo history, as an authoritative value write does;
Linux has no field undo history. Native strings replace split UTF-16
surrogates with U+FFFD. These limits are recorded beside the extra-input
deviation in LLP 1069.001.

AppKit's `FieldSelectionMacTests` passed all six tests after the new test
first failed on the unimplemented command. UIKit's `FieldSelectionIOSTests`
passed all five tests on an iPhone 17 / iOS 27 simulator booted by this lane.
Linux's command regression also first failed on the unimplemented command;
both `set_range_text` tests now pass. The runner replacement-input test and both named Contract integration tests pass.
The rebuilt web JS/wasm range-edit fixture matches on all 16 steps; the
Chrome method comparison passes 207 assertions.
The macOS app drive also passed: five successful commands produced five inputs,
UTF-16 selections matched, and a reversed range left state unchanged.
`contract-difftest quick` built but could not run because `lake`/Lean is absent.

### Review fixes — 2026-10-12

The replacement callback now records Apple's last-reported text, so deleting
an inserted value back to an unchanged empty prop emits input and a pending
change. Native URL replacements strip surrounding ASCII whitespace, retaining
non-ASCII whitespace. Native `select` is suppressed when the selection is
unchanged. Chrome assertions show the added input arrives synchronously and
`select` later, so the reviewed claim of select-before-input was rejected.

`insertReplacementText` is not exclusive to this command: the web hatch and
browser correction events also carry it. Apple hatch input now reports the
same type and retains its pending change. The web mapping is preserved;
script tagging is unnecessary because ABI kind 48 carries only the edit type,
not scripted-edit bookkeeping. Field-selection tests on both Apple platforms
and Linux, plus the existing Chrome range-edit test, cover these regressions.
This round's AppKit class passed ten tests; Linux passed 740 tests and the web suites
passed 391 assertions. The final UIKit bundle compiled, but three focused
runtime launch attempts never reached the test class (test-daemon startup
and simulator launchd/boot timeouts); no UIKit runtime pass is claimed for
these review fixes. The earlier five-test result above predates this round.

### Review round 3 — 2026-10-12

The synchronous native replacement `select` finding was real: the new AppKit
and Linux two-command tests first observed `input:A, select:A, input:AB,
select:AB`. Replacement selects now wait until the action's commands finish
and read the field at delivery, yielding `input:A, input:AB, select:AB,
select:AB`, as Chrome does. This corrects the previous round's incomplete
claim about native event timing.

The JS file-picker finding was real: its record had only five fields and
`e.inputType == ""` was false. The picker now supplies the sixth field as an
empty string. Auditing the other web producers found the shared JS
`inputRecord` already supplies all six; the wasm host sends ABI events and
the runner constructs its complete record. The browser regression exercises
the production picker callback and the empty-string branch.

The proposed Apple direction change was not supported by the oracle.
Installed macOS Chrome 155.0.8059.39, in both headed and headless runs,
focused and unfocused, resets the forward 0–2 range to `none` and emits
`select`; it also resets the backward 1–4 range to `none` while moving its
end to 5. Both web targets match. Apple retains that measured behavior;
new native and browser assertions cover both directional cases. No claim
of preserving `forward` is made.

Round 3 verification: AppKit passed all 12 field-selection tests; the Linux
and web-JS Rust suites passed 960 tests (10 ignored); Chrome passed 262
assertions headed and the focused headless web suites passed 428 assertions.
The rebuilt JS/wasm conformance fixture matched all 16 steps with no failures.
UIKit compiled; after two interrupted/unavailable attempts, its final run
executed all 11 tests. Both new regressions passed, as did the existing
queued-select condition test. Ten tests passed; the existing
`testRangeReplacementDoesNotSwallowTheNextUserDeletion` failed two assertions
for `UITextField`: after deleting the inserted `"x"`, inputs contained only
`["x"]` rather than `["x", ""]`, and `edited` was nil rather than field 3.
No full UIKit pass is claimed; this remaining regression is not fixed in the
last review round.

A concurrent Apple test process initially held the app-build lock and its
temporary `build.mjs` edit exceeded the line cap. After that process exited
and restored its edit, caps passed. The rebuilt macOS app drive then passed:
one button issued both replacements and state reported `body="AB"` and
`input:A input:AB select:AB select:AB`. The simulator booted by this lane was
shut down after testing.

### UIKit deletion follow-up — 2026-10-12

The two failures above reproduce, but the AppKit reporting fix already
reaches UIKit: `FieldSelections.replacementInput` sets `NodeView.toldText`
on both platforms, and both UIKit delegates compare through `typedText`.
The first follow-up added UITextField's `.editingChanged` control event after
its programmatic `deleteBackward()`, checked the replacement's reported-text
baseline, and resigned first responder to check the pending `change`.
That simulation was still wrong: the orchestrator's subsequent run at
`168bc226a` executed all 11 tests and the deletion test still failed four
assertions. The callback correction and runtime proof are recorded below.

Verification commands from this follow-up (in this worktree):

```sh
cargo test -p exact-apple --lib --tests --no-fail-fast
cargo clippy -p exact-apple --all-targets -- -D warnings
cargo fmt --all -- --check
git add -A && bun scripts/caps.mjs
bun scripts/issue.mjs check
```

All passed: 406 Rust tests, 16 ignored. The original UIKit class reproduced
the reported two assertions before the test correction. A temporary mutation
removing `replacementInput`'s `toldText` assignment compiled, but its runtime
launch was stopped before tests began (first a simulator collision with
another lane, then an Xcode startup wait); no mutation result is claimed.
The assignment was restored before the final verification run.

```sh
/private/tmp/claude-501/-Users-ccheever-projects-exact2/5f1c298e-7e97-40bc-8a1a-486f1889060d/scratchpad/ios-verify.sh /Users/ccheever/projects/exact2-iss-rangeedit FieldSelectionIOSTests C25DD7E7-E600-415E-A7FA-A729AD1FC140
/private/tmp/claude-501/-Users-ccheever-projects-exact2/5f1c298e-7e97-40bc-8a1a-486f1889060d/scratchpad/ios-verify.sh /Users/ccheever/projects/exact2-iss-rangeedit FieldSelectionIOSTests AC228731-1097-4E4A-A81B-34A2BEEE90EC
```

The first command reproduced the original two assertions (11 tests, 10
passed), then built the corrected test for the mutation attempt without
reaching runtime. The second command was the final production-source run;
it stalled for several minutes in `simctl list devices available -j` before
Xcode and was stopped. **The corrected UIKit test compiles, but its final
runtime verification remains blocked; no UIKit pass is claimed.** The final
log is `/Users/ccheever/projects/exact2-iss-rangeedit.ios.log`; the original
failure log is `target/rangeedit-ios-before.log`. Temporary source and build
script edits were restored. Only this lane's stalled processes were stopped.
The iPhone Air booted for the retry could not be confirmed shut down:
`xcrun simctl shutdown AC228731-1097-4E4A-A81B-34A2BEEE90EC` timed out after
30 seconds. The other lane's iPhone 17 was left running.

### UIKit callback verification — 2026-10-12 (ios2)

The diagnostic run names every assertion's control: only `line`
(`UITextField`, id 3) failed, with four assertions; `editor` (`UITextView`,
id 2) passed. The text became empty, but `sendActions(for: .editingChanged)`
did not invoke the registered callback in this XCTest host. This host limit
is also documented in `NavigationTabsIOSTests`. The corrected regression
asserts that `.editingChanged` is registered to `fieldChanged`, then calls
that callback after `deleteBackward()`, as `FieldCompositionIOSTests` and
`KeyboardFocusIOSTests` do. The text view still drives its real delegate
through `UITextInput` without a manual callback.

The regression requires `toldText == "x"` before the edit, then exactly one
empty-value input, a pending user change, and exactly one empty-value change
on blur. Temporarily removing only `replacementInput`'s `toldText` assignment
made both controls fail those assertions (10 assertion failures in this one
test; the other 10 tests passed). The target-registration assertion passed.
The mutation was restored; no production change is needed for this follow-up.
These are programmatic UIKit edits and callbacks, not physical-keyboard tests.

Final verification on iPhone Air / iOS 27 passed all 11 `FieldSelectionIOSTests`
with zero failures. The same command was run three times: diagnostic failure
(four assertions, `line` only), expected mutation failure (10 assertions,
both controls), then the restored production source passed:

```sh
/private/tmp/claude-501/-Users-ccheever-projects-exact2/5f1c298e-7e97-40bc-8a1a-486f1889060d/scratchpad/ios-verify.sh /Users/ccheever/projects/exact2-iss-rangeedit FieldSelectionIOSTests AC228731-1097-4E4A-A81B-34A2BEEE90EC
```

Logs: `target/rangeedit-ios2-before.log`, `target/rangeedit-ios2-mutation.log`,
`target/rangeedit-ios2-final.log`. The runner shut down the simulator; the
explicit `xcrun simctl shutdown AC228731-1097-4E4A-A81B-34A2BEEE90EC`
confirmed it was already in `Shutdown`. `cargo test -p exact-apple --lib --tests
--no-fail-fast` passed 406 tests (16 ignored); `cargo clippy -p exact-apple
--all-targets -- -D warnings` and `cargo fmt --all -- --check` passed.

**What remains (issue stays open):**

- Field `selectionchange` with `InputEvent` selection offsets on caret and
  pointer moves, independent of a text reader's `Selection` event.
- Cancelable `beforeinput`, other input types, and `InputEvent.data`.
- Paste cancellation, undo/redo reporting, and IME/noncancelable-event cases.
- Windows, Android, terminal and Markdown-editor range execution are unsupported.

Regression coverage for this slice: `set_range_text_is_a_typed_host_command_with_dom_overloads`
in the compiler/runner integration tests, `host/web/tests/field-range.test.mjs`
against Chrome's method, and `host/web-js/conformance/range-edit.*` for bound
state and selection on both web targets. Verification results belong to the
lane's commit report; this note does not claim the remaining acceptance passed.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #125, split out because #125 closed when its clipboard part landed (#209: a field's ⌘V, ⌘C and ⌘X fire `paste`, `copy` and `cut` first, cancelable). Three DOM editing primitives on `textarea` and `input` are still missing on every host:

- **`selectionchange` on a field.** A plain caret move (arrow keys, a click) is not reported. `select` fires only for a non-empty selection or `setSelectionRange`, and `input` only for an edit.
- **`setRangeText(id, text, start, end)`.** An app cannot replace a range of a field's value as one edit (one `input`, one undo step, the caret placed by the DOM's rules).
- **`beforeinput`.** An app cannot see an edit's `inputType` and `data` before it lands, or cancel it with `preventDefault()` (for example `insertFromPaste`, `historyUndo`).

An editor that turns a typed trigger (`@name`) into a reference, recalls history only when the caret is on the first line, or folds a large paste into an attachment needs all three. Without them such an editor is written natively per platform, and its logic and tests exist twice.

Atomic inline ranges, the other remainder of #125, are refused by `rules/DEFERRED.md` and are filed separately as a policy question.

### Current and expected behavior

Current, on main `0365ad1a4` (the relevant compiler and host files are unchanged on `e200397ec`):

- `selectionchange=` on a `textarea`: `lower-attr-tag` "`selectionchange` belongs to `text`: it reports the part of the reader's text selection inside one paragraph, not `textarea`".
- `beforeinput=` on a `textarea`: `lower-unknown-attr` "`textarea` has no attribute `beforeinput`".
- `setRangeText("editor", "[a.ts]", 4, 9)` in an action: `type-unknown-command` "`setRangeText` is not a host command".
- At run time, after `setSelectionRange("editor", 4, 9)`, two ArrowLeft presses report nothing (the app still holds `select 4-9`); typing `x` then reports `input 4-4`, so the caret had moved without the app knowing. Same on the web and macOS.

Expected (Chrome, by the DOM's names):

- `selectionchange` on a field fires on every caret or selection change, keyboard or pointer, with the field's `selectionStart`, `selectionEnd` and `selectionDirection` (the existing `InputEvent` fields).
- `setRangeText(id, text, start, end)` replaces the range, fires `input` once, places the caret after the text (or by the DOM's `selectMode`), and is one undo step.
- `beforeinput` hands `inputType` and `data`; `preventDefault()` cancels the edit; `historyUndo` and `historyRedo` arrive as input types.

Proposal (a suggestion, not a required design): the payloads reuse `InputEvent` with `inputType` and `data` added; `setRangeText` joins `setSelectionRange` as a host command by the field's `id`.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` answers no sources:

```contract
component TextareaEditing
  state body = "see @a.ts now"
  state caret = "none"
  state selects = 0
  action edited(v: string, e: InputEvent)
    body = v
    caret = `input ${e.selectionStart}-${e.selectionEnd}`
  action selected(e: InputEvent)
    selects = selects + 1
    caret = `select ${e.selectionStart}-${e.selectionEnd}`
  action pick
    setSelectionRange("editor", 4, 9)
  view
    main testId="root" padding=24
      textarea id="editor" testId="editor" value=body input=edited select=selected
      button press=pick testId="pick"
        text "Select @a.ts"
      text caret testId="caret"
      text `selects ${selects}` testId="counts"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Caret move | `bun exact.mjs agent <host> --json "tap pick" state "type editor key ArrowLeft" "type editor key ArrowLeft" state "type editor key x" state` | web (Chrome 154) and macOS 26.6.2 (`bun exact.mjs mac`) | `0365ad1a4` | `caret` stays `select 4-9` after both ArrowLeft presses; `x` gives `seex @a.ts now`, `input 4-4` | a `selectionchange` reporting 3-3 after the second press | agent `state` slots, both hosts identical |
| `selectionchange` on the field | add `action moved(s: Selection)` and `selectionchange=moved` on the `textarea`; `bun exact.mjs contract build app.contract --json` | compiler | `0365ad1a4` | `lower-attr-tag` (quoted above) | compiles; fires on a field | compiler output |
| `beforeinput` | add `beforeinput=selected` on the `textarea`; `contract build --json` | compiler | `0365ad1a4` | `lower-unknown-attr` | compiles; cancelable | compiler output |
| `setRangeText` | replace `pick`'s body with `setRangeText("editor", "[a.ts]", 4, 9)`; `contract build --json` | compiler | `0365ad1a4` | `type-unknown-command` | compiles; one `input`, one undo step | compiler output |

### Acceptance criteria

- In the app above, on the web and macOS: ArrowLeft, ArrowRight and a pointer click in the field each report the new offsets through `selectionchange`.
- `setRangeText("editor", "[a.ts]", 4, 9)` gives `see [a.ts] now` with one `input`; one ⌘Z (Ctrl+Z) restores the old value.
- `beforeinput` with `preventDefault()` on `insertFromPaste` leaves the value unchanged, with a real ⌘V on macOS and in Chrome; `historyUndo` is reported on ⌘Z.
- A conformance case against Chrome over the same steps for the three primitives.
- iOS and Linux: the same, or a stated limit (UIKit's undo calls no delegate, so `historyUndo` may not be observable there).

### Constraints and related work

- Related: #125 (closed by #209, which covers only the clipboard events), #132 (selection on `text`).
- Not tested: iOS, Linux, IME composition, a real keyboard on macOS (the agent delivers keys as platform events).
- A way to end the current undo group has no DOM equivalent; it is left out of this issue's acceptance and needs its own proposal if wanted.
- Workaround today: a native text view per platform with its own delegate, outside Contract and the agent's reach.

## Discussion at transfer

### ccheever — 2026-10-08T08:07:29Z

**Decision: Add field selectionchange and beforeinput; pin range-edit semantics first.**

Keep open with the bounded scope below.

The primitives are useful. Chrome 155 showed setRangeText emits select/selectionchange, no input, and preserves the existing selected range by default. It did not create the requested undo step in the focused-field probe.

Correct the acceptance before implementation. Preserve DOM selectionMode and observable events; if Exact deliberately adds input synchronization or an undoable edit, record that deviation explicitly. Test IME and noncancelable beforeinput cases.
