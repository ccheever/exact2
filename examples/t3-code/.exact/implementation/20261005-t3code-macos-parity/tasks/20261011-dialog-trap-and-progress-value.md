---
name: 20261011-dialog-trap-and-progress-value
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-dialog-trap-and-progress-value
pr_url: https://github.com/ccheever/exact2/pull/431
verified_commit: null
---

# Tab stays inside every dialog; progress bars tell assistive tech their value

## Outcome

STATUS "Known differences" rows 7 and 8 (the user asked on 2026-10-11 to fix what can be fixed now).

| Id | Clone | Reference |
| --- | --- | --- |
| DT-1 | In the import wizard (Settings › Integrations › Browser profiles › Add profile › Brave, "Quit Brave to import"), Shift+Tab from Cancel leaves the dialog for the Settings page behind it ("Add host", then "Built-in default…") (`realinput-1010h`, [image](https://raw.githubusercontent.com/ccheever/exact2/7102c03c8286911606342d14bda00cec33d3883d/realinput-1010h/IW-c-1to4-brave-dialog-rings.png)). Tab past the last stop of the wizard and of the profile removal confirm leaves them too ([import-wizard-initial-focus](closed/20261010-import-wizard-initial-focus.md), Not done). Exact keeps no Tab inside `aria-modal` on macOS (X53, [#282](https://github.com/ccheever/exact2/issues/282)); most of the clone's dialogs carry their own `key` traps since [dialog-shortcut-focus](closed/20261008-dialog-shortcut-focus.md), these do not | Base UI's focus trap: Tab past the last stop goes to the first, Shift+Tab before the first goes to the last, in every Dialog and AlertDialog |
| PV-1 | Progress bars (the Antigravity runtime download, and any other `role="progressbar"` box) are drawn boxes with the percentage only in `aria-description`; on macOS a drawn box's `progressbar` role is not exposed either (X55, #278) and Contract carries no value (X49, main `issues/20261009-aria-range-accessibility-values.md`, open) | `<progress>` / Radix progress with its value: VoiceOver reads "Antigravity download, 30 percent" |

## Steps

1. DT-1: list every dialog, alert dialog and modal popup of the clone (grep `aria-modal`, `role="dialog"`,
   `role="alertdialog"`) and check Tab past the last stop and Shift+Tab before the first in each, in agent mode (`key Tab`,
   `key shift+Tab`, then `focused`). Give each one that leaves its dialog the shared trap the others use (the wizard's own
   `popupKeys` already handles the popup itself holding the focus, X79). Keep the reference's order of stops.
2. PV-1: with no framework change, expose the value as close to the reference as Contract allows on macOS, and check it
   with `tree --ax` (and an out-of-process AX read, as RG-4 did, if the tree is not enough): for example an indeterminate
   `progress` element (which macOS does expose) named with the percentage, or the percentage in the accessible name. Keep
   the drawn bar as it looks now. If nothing reaches assistive tech, record the measurement and leave the row open
   (X49 stays the framework gap).
3. Tests; one agent drive (the wizard with the lane's fixture browser home, the profile removal confirm, the download bar
   with the fixture install); before / after images (focused element read-outs for DT-1, the AX read for PV-1).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DT-1 | every dialog listed with its result before and after; tests; agent drive | focused-element read-outs |
| PV-1 | `tree --ax` / AX read before and after | text before/after |
| The traps and VoiceOver under real keys | real-input steps for the final session | — |

## What was built

**DT-1.** Every `role="dialog"`/`"alertdialog"` of the clone was classified against the reference (81 popups):

- **34 popups now carry Base UI's focus guards** (`dialog-trap.contract` `FocusGuard`): an invisible (`opacity=0`,
  1×1, `pointer-events="none"`), `aria-hidden`, `tabindex=0` box just inside each end of the popup, as Base UI's
  `FloatingFocusManager` renders its "inside" guards in every modal Dialog and AlertDialog. Tab past the last stop lands
  on the guard after it, whose `focus` sends the focus to the first stop; Shift+Tab before the first lands on the guard
  before it, which sends it to the last. Each dialog names its first and last Tab stops for the screen it shows (a
  disabled control is no stop; ids were added where a stop had none). The dialogs: the import wizard (its guards name
  `wizardTabStops`' first and last; `popupKeys` stays for X79), Remove “<profile>”?, Remove route, Remove environment,
  the legacy sidebar's Rename project and Project grouping, Custom model prices and the usage model dialog
  (`UsageDialogShell` takes `first`), the welcome wizard, the command palette and Link pull request (CommandDialog), Add
  provider, Add a CLIProxyAPI hub, Remove hub, the expanded video, image and diagram previews, the git commit, confirm
  and publish dialogs (`R4Dialog` takes `first`), Set up devices, Merge pull request?, Checkout pull request, Remove
  themes from a collection, Background Activity (`SaDialog` takes `first`), Add device host, Add a theme, Add/Edit
  Action and Delete action, Choose project icon, the favicon picker, Restore default settings?, New/Edit task, Set up
  snapshots, the SSH password prompt, Save plan to workspace, Edit from here?, and the clone's own project picker.
- **16 popups keep the `key` wraps they had** (AppConfirm, SettingsConfirm, Custom snooze, Add Environment, the
  network/Tailscale/pairing dialogs, the ChatGPT account and plan dialogs, the pull request action and stack dialogs,
  Local environment, Use a reset credit).
- **31 are no modal in the reference** (Popover, Menu, PreviewCard, a toast, the Settings page, the floating theme
  editor) and get no trap. `dialog-trap.test.ts` lists all three groups; a new dialog fails it until it is classified.

The reference's order of stops, where the clone's differed: the X is DialogPopup's last child, after the footer, in New
task and Set up snapshots (it came first in tree order; it is absolutely placed, so it looks the same); the four dialogs
that drew no X have the reference's (Add a CLIProxyAPI hub, Save plan to workspace, the legacy sidebar's two, all with
`showCloseButton` left on in the reference); a preview's backdrop button, the palette's and favicon picker's rows
(Base UI Autocomplete items) and the palette's accessory (`tabIndex={-1}` in the reference) are no Tab stops. Content
search's Match case, Match whole word and Use regular expression are (`SearchOptionButton` is a Base UI Toggle outside a
ToggleGroup, a native button): Tab from the field goes to Match case, and the palette's start guard sends Shift+Tab from
the field to Use regular expression while they show (review round 1).

**While a dialog is busy** (review round 1) a guard's target can be disabled or not drawn, and `focus(target)` then does
nothing. A guard that keeps the focus holds the key that would leave the dialog: Shift+Tab at a start guard, Tab at an end
guard, try the target again and otherwise leave the focus on the guard, inside the popup (Base UI keeps it on the popup
when it has no stop); the other way the key goes on into the dialog (`FocusGuard` `hold`, the guard's edge read from its
id, `-guard-start`). Four dialogs name a stop that stays enabled instead: Set up snapshots (Advanced after an error, else
none), the welcome wizard's Agents step (the Connect step, held only while importing), Add a CLIProxyAPI hub (its Label
field) and Add device host (Advanced options).

**Element ids** (review round 1): the shared controls a guard names take the id as their own prop, `elementId`, set only
where a guard names it and `""` elsewhere (MediaActions, UsagePrimaryButton, UsageGhostIcon, WelcomePrimary,
WelcomeGhost, WelcomeDisclosure, R4Button, R4Checkbox, PreviewStep, PaletteToggle), so a thread's markdown images and
videos carry no id (they all carried `markdown-image-actions` / `markdown-video-actions` with `id=testId`).

**PV-1.** `progress-value.contract` `ProgressValue`: an indeterminate `progress` (AppKit's busy indicator, which macOS
exposes, unlike a drawn box) laid over the bar's box, `opacity=0`, named with the label and the rounded percentage. The
drawn bars keep their look and are `aria-hidden` (no `role="progressbar"`, no `aria-description` left anywhere):
Antigravity's download (`providers-setup.contract`, `provider-install.ts` `progressLabel`: "Antigravity download, 27%")
and the context window meter (`composer-controls.contract`, `composer-controls-view.ts` `contextValueLabel`: "Context
window usage, 42%", its `aria-valuenow` rounded as the reference's).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| DT-1 | pass (agent drive, reference over CDP): 15 read-outs in 8 dialogs. Before: 11 left the dialog (the Settings page's "Add host" behind the wizard and the confirm, a toast's Settings button, the palette's backdrop) and 3 stayed on the field Shift+Tab started from; after: every read-out stays in its dialog and lands where the reference's does (the X before the first stop, the first stop after the last), but two the reference answers with what the clone has no stop for (Known differences below). The other 26 guarded popups are covered by `dialog-trap.test.ts` (their pair, first and last child, every target an element id; the plain ones' stops in tree order). Review round 1, a second drive (before, after, reference): content search's toggles are Tab stops again and the focus stays in it (3 read-outs); Custom model prices (2) and New task (3), which the first drive could not open, keep the focus in and land where the reference's does. New task's button sits under the lane's provider-update toast at 1280×840 (the first drive's tap landed on the toast; the reference's toast intercepts the click too), so both sides press it from the keyboard; Model prices opens on Cost but not on Limits, base and branch alike (an existing difference, QUEUE.md). The busy hold on a one-file probe: before, the second Shift+Tab or Tab from a guard whose target is disabled leaves the dialog; after, it stays on the guard | [dt1-quit.png](https://raw.githubusercontent.com/ccheever/exact2/f088b8c159948b3a004ba64e58039929a489193a/dialog-trap-and-progress-value/dt1-quit.png), [dt1-configure.png](https://raw.githubusercontent.com/ccheever/exact2/1603566d2de97a3a97fabdd32a0f1a00c4c2d5b1/dialog-trap-and-progress-value/dt1-configure.png), [dt1-remove.png](https://raw.githubusercontent.com/ccheever/exact2/8c21ea6995a2736eaf65cdabb479d1194cb5f29a/dialog-trap-and-progress-value/dt1-remove.png), [dt1-host.png](https://raw.githubusercontent.com/ccheever/exact2/c98f4966f1670f8d9520b58775cbb0aae9baa9fa/dialog-trap-and-progress-value/dt1-host.png), [dt1-provider.png](https://raw.githubusercontent.com/ccheever/exact2/800c832be717212fec3a0a6aff348854a63c7b60/dialog-trap-and-progress-value/dt1-provider.png), [dt1-hub.png](https://raw.githubusercontent.com/ccheever/exact2/986a3810d0f7049cbee681a8e72e4d6e2f345291/dialog-trap-and-progress-value/dt1-hub.png), [dt1-theme.png](https://raw.githubusercontent.com/ccheever/exact2/4020deef466a4c4cb8ada6fe496f816f4c094a6e/dialog-trap-and-progress-value/dt1-theme.png), [dt1-palette.png](https://raw.githubusercontent.com/ccheever/exact2/0499ea5bda6b2db87af058253c2df32dbd1e8a0c/dialog-trap-and-progress-value/dt1-palette.png), [focus-readouts.txt](https://raw.githubusercontent.com/ccheever/exact2/5ce88a8ebf560c57c7b9748da6c7cd4e3b829e2f/dialog-trap-and-progress-value/focus-readouts.txt); review round 1: [review1-dt1-content-search.png](https://raw.githubusercontent.com/ccheever/exact2/f1afac959a2ce2ad049efd7d9b6ab2db939f2201/dialog-trap-and-progress-value/review1-dt1-content-search.png), [review1-dt1-prices.png](https://raw.githubusercontent.com/ccheever/exact2/a7be52b03c1bba91e9edd4a9b09d0cc65740626e/dialog-trap-and-progress-value/review1-dt1-prices.png), [review1-dt1-newtask.png](https://raw.githubusercontent.com/ccheever/exact2/d986ee6a4263cfe2472beb480ddc96b4ec04d8c4/dialog-trap-and-progress-value/review1-dt1-newtask.png), [review1-focus-readouts.txt](https://raw.githubusercontent.com/ccheever/exact2/fb565f1c41c1172b5aa4e6dbe474e99667c6b052/dialog-trap-and-progress-value/review1-focus-readouts.txt), [review1-busy-guard.txt](https://raw.githubusercontent.com/ccheever/exact2/5f1b677b5a0603acd7cc3c0d720de6d71814baeb/dialog-trap-and-progress-value/review1-busy-guard.txt), [review1-element-ids.txt](https://raw.githubusercontent.com/ccheever/exact2/9d17077ee92c961a0cf9252752f092b06f3948a9/dialog-trap-and-progress-value/review1-element-ids.txt) |
| PV-1 | pass in-process (`tree --ax`, AppKit's accessibility attributes): the drawn box is no accessibility element at all (before); after, one element, AppKit's busy indicator (agent role `progressbar`), named "Antigravity download, 30%", the bar's look unchanged. Measured on a one-file probe that draws both markups (the component copied verbatim) on the clone's own host binary, base and branch: the lane can show neither bar (a real Antigravity download is a provider install, and the context meter needs a thread with token usage, i.e. a provider turn). The value is in the name, not AXValue, and VoiceOver hears a busy indicator, not a progress indicator (X49 stays the framework gap). The out-of-process read was tried: an agent-mode window is not among the app's AX children from another process, so the in-process tree is the measurement | [pv1-ax.txt](https://raw.githubusercontent.com/ccheever/exact2/a4d7c968bf39d5a2ef70ab1ebb5b8ae22996c543/dialog-trap-and-progress-value/pv1-ax.txt), [pv1-look.png](https://raw.githubusercontent.com/ccheever/exact2/69d9f9fb5bcf930eccc3bae143249a4a0d0ef9d8/dialog-trap-and-progress-value/pv1-look.png), [pv-probe.contract](https://raw.githubusercontent.com/ccheever/exact2/2c98ef25a90e077793d755721ad63af3782cf170/dialog-trap-and-progress-value/pv-probe.contract.txt) |
| The traps and VoiceOver under real keys | implemented, not verified (real input reserved for the coordinator's final session): Real-input batch steps below | — |

Drives: the evidence base `eb9752918` (`t3-code-evidence-base`, lane `dialog-trap-and-progress-value-before`) and this
branch's bundle (lane `dialog-trap-and-progress-value`), the same ops ([drive.sh](https://raw.githubusercontent.com/ccheever/exact2/b06180107dd142f0c461e9d814196f2dd0f1bca2/dialog-trap-and-progress-value/drive.sh.txt)),
`T3_BROWSER_IMPORT_HOME` = the import-wizard lane's fixture browser home. The clone's embedded server ran on 16142: the
clone accepts lane ports 16000–16999 only (`T3LocalBackend.swift` `laneRange`), so the given base 17140 served the
reference (17140/17141) and 16140 the clone. Reference: T3 Code 1e2ecbd975 over CDP with the same fixture browsers in its
lane home ([ref-trap.mjs](https://raw.githubusercontent.com/ccheever/exact2/4c53f50423c845e4742699200ca1e6168069983a/dialog-trap-and-progress-value/ref-trap.mjs.txt),
[ref-trap.json](https://raw.githubusercontent.com/ccheever/exact2/f7ecae1b90347b05d58a3587e919c339dc1a7a33/dialog-trap-and-progress-value/ref-trap.json));
images: [compose.py](https://raw.githubusercontent.com/ccheever/exact2/747c03cdf6c27bbb8f356a4cf76b215fd02356da/dialog-trap-and-progress-value/compose.py.txt).
The guard mechanism was first proven on a one-file probe on the base binary (Tab from a modal box's last button and
Shift+Tab from its first wrap through the guards) before the live drive, which then passed on its first run.

Review round 1 (an independent review of PR #431: four findings, all fixed): one more drive, the same ops on the evidence
base and on this branch's rebuilt bundle (lanes as above, clone server 16142; [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/6e29a49e43fe116274905d9bdd17eb587b90b706/dialog-trap-and-progress-value/review1-drive.sh.txt)),
the branch's on its first run; the reference over CDP on 17140/17141 ([ref-content.mjs](https://raw.githubusercontent.com/ccheever/exact2/1fef8a434576d2e334abcc8ee3ae347bcd276456/dialog-trap-and-progress-value/review1-ref-content.mjs.txt),
[ref-dialogs.mjs](https://raw.githubusercontent.com/ccheever/exact2/901a6bc3d480c66073a857f56fd26ce524038d46/dialog-trap-and-progress-value/review1-ref-dialogs.mjs.txt),
[ref-limits.mjs](https://raw.githubusercontent.com/ccheever/exact2/38066c71d6ac9194e0fa07dbf6214ebfa7ee652c/dialog-trap-and-progress-value/review1-ref-limits.mjs.txt));
images: [compose.py](https://raw.githubusercontent.com/ccheever/exact2/b71121be6d142642d15163d61a70d0d0ff9c4841/dialog-trap-and-progress-value/review1-compose.py.txt).
The busy hold was measured on a one-file probe (two modal boxes whose guard targets are disabled, the old and the new
FocusGuard) on the evidence base's host binary, which this branch does not change.

## Known differences (after this task)

- The command palette: Tab from the field goes to the reference's results list, which Chromium makes a Tab stop because
  it scrolls (keyboard-focusable scrollers); the clone's list is no Tab stop (an AppKit scroll view is no key view), so
  Tab stays on the field. Neither leaves the palette.
- Add a theme: the reference's Shift+Tab from the search field lands on its visually hidden file input (1×1); the
  clone's on the X, the last visible stop.
- PV-1: the value is in the accessible name of a busy indicator, not a progress indicator's value (X49, X55).
- On macOS a `select`, a time and a date input are no Tab stops (X52); the legacy Project grouping dialog's first stop
  is Cancel there (its Select comes first in the reference).
- While a dialog is busy, a guard whose target is disabled or not drawn keeps the focus itself, an invisible box inside
  the popup (no ring shows), where Base UI goes to the nearest enabled stop or the popup; the trap holds (the key that
  would leave the dialog stays on the guard). Set up snapshots, the welcome wizard's Agents step, Add a CLIProxyAPI hub
  and Add device host name an enabled stop instead. The Add provider wizard after an instance is created comes round to
  its X (its Sign in step's first stop is not named).

Found, not in this task: Model prices opens nothing on the Usage page's Limits view (`pages-usage.ts` presents the prices
only off Limits; the reference opens it on Limits too), and Custom model prices draws no X and no "Add model price" +
(both laid out and focusable), base and branch alike: both in QUEUE.md. The reference's ChatView "Switch to <branch>?" AlertDialog and PullRequestThreadLinks' thread
picker Dialog have no clone counterpart; the reference's ToggleGroup (Choose project icon's Icons / Emoji / Monogram) is
one Tab stop, the clone's three.

## Tests

- `dialog-trap.test.ts` (new, 56 after review round 1): the classification of every `role="dialog"`/`"alertdialog"` (guards, own `key`
  wraps, or no modal in the reference, with the reason); the FocusGuard component; for each guarded popup the start
  guard as its first child and the end guard as its last, and every name a target can take declared as an element id;
  the plain dialogs' Tab stops in tree order against their guards' targets; the X after the footer in New task and Set up
  snapshots, the four added X's, the shared frames' X after their content; the stops the reference never has. Removing
  one guard fails three of them (checked). Review round 1: the guard's hold (the edge from its id), content search's
  toggles as Tab stops with the start guard naming the last, the four busy fallbacks naming stops that take no `busy`,
  and the shared controls' `elementId` (set only where a guard names it, never `id=testId`, a thread's markdown media
  none). Each fails when its fix is reverted (checked: the toggles' `tabindex=-1`, the start guard without the toggles,
  the guard without `key=hold`, MediaActions back on `id=testId`, the host editor's start guard without its fallback).
- `progress-value.test.ts` (new, 5): `progressLabel` and `contextValueLabel` (rounded, empty without a size), the
  component, each drawn bar `aria-hidden` with its ProgressValue as a sibling in a positioned box, no drawn
  `role="progressbar"` left.
- Every existing test passes unchanged (`dialog-focus.test.ts`, `import-wizard-initial-focus.test.ts`,
  `provider-setup.test.ts`, `settings-escape.test.ts`, …).

## Checks

All on `2d013aeb0` (review round 1; `origin/feat(example)/t3-code` had nothing new to merge), each once, every exit 0:

| Check | Exit | Result |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4514 pass, 1 skip, 0 fail (307 files) |
| strict `tsc` on `app.ts` (README command) | 0 | clean |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6032 slots, 8227 actions, 109730 nodes; `app.contract` 1397 lines (unchanged) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3679 passed, 0 failed (95 binaries) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/caps.mjs` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

No Rust or Swift changed, so no `cargo test -p t3-code-macos` or AppKit binary ran. The bundle was rebuilt once for
review round 1 (`host/apple/build.mjs t3-code-macos --bundle`, exit 0) before its one live drive, which passed on its
first run. (The first round's checks, on `24de5e33a`, were the same: 4510 pass.)

## Real-input batch steps

Launch this branch's bundle normally as a lane copy (its own name and bundle id), with `T3_BROWSER_IMPORT_HOME` naming
`target/t3-audit/lanes/import-wizard-initial-focus/browser-home` (never the account's browsers). Keys, not the pointer,
after each dialog opens; the ring shows where the focus is.

1. Settings › Integrations › Browser profiles › Add profile › Brave ("Quit Brave to import"): Shift+Tab from Cancel → the
   ring is on the X; Tab → Cancel; Tab, Tab → "I’ve quit it", the X; Tab → Cancel again. Escape.
2. Add profile › Chrome: from "Personal", Shift+Tab → the X; Tab → "Personal". Escape.
3. A profile's "…" › Remove profile and data: Shift+Tab from Cancel → Remove profile; Tab → Cancel. Escape.
4. Device hosts › Add host: Shift+Tab from Name → the X; Tab → Name. Escape.
5. Settings › Providers › Add hub: an X shows at the top right; Shift+Tab from Hub URL → the X; Tab from Cancel → the X
   (Add hub is disabled while the fields are empty). Escape. Add provider: Tab from the X → the Provider step.
6. Settings › Scheduled Tasks › New task: Shift+Tab from Runs on → the X (top right); Tab from Create task (or Cancel
   while it is disabled) → the X; Tab → Runs on. Escape.
7. ⌘K: Tab and Shift+Tab keep the ring on the field. Escape. ⌘⇧F (Search project contents): Tab from the field → Match
   case; Tab, Tab → Use regular expression; Tab → the field; Shift+Tab → Use regular expression. Escape twice.
8. Usage on Cost › the environment menu › Model prices: Tab from the X (top right) → "Apply to"; Shift+Tab → the X.
   Escape.
9. VoiceOver (⌘F5), only if the session has a download or a thread with token usage: Settings › Providers › Antigravity
   while its runtime downloads, VO on the bar reads "Antigravity download, N%, busy indicator"; the composer's context
   window popover, "Context window usage, N%". Without either, leave the row open.

## Next action

Coordinator: review PR https://github.com/ccheever/exact2/pull/431 (review round 1 fixed, `2d013aeb0`) and run the real-input steps above in the final session. Model prices on the Limits view and the prices dialog's undrawn X and + are in QUEUE.md, not this task's.
