---
name: 20261011-progress-native-bars
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-progress-native-bars
pr_url: https://github.com/ccheever/exact2/pull/462
verified_commit: 1f813789b
---

# Progress bars say their value: main's determinate `progress` and ARIA range values

## Outcome

The user decided on 2026-10-11 that the progress follow-up of round 9 is built now, matching the reference T3 Code.
Stacked on the main adoption round 9 (`adopt-main-fixes-r9`, [#458](https://github.com/ccheever/exact2/pull/458)),
which brought main's LLP 1116 D8: `43f6d298c` (`aria-valuenow`/`-valuemin`/`-valuemax`/`-valuetext` on any node; on
macOS a drawn `progressbar`, `slider`, `meter`, `scrollbar` or `spinbutton` is one AppKit element with its role, value,
minimum and maximum) and `ab9110c3a` (a determinate `progress value=… max=…`, AppKit's own `NSProgressIndicator` bar).

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| PB-1 | Every progress UI is a drawn track and fill, `aria-hidden`, with an invisible indeterminate `progress` (a busy indicator) beside it named "<label>, <n>%" (`progress-value.contract` `ProgressValue`, PV-1 of [dialog-trap-and-progress-value](20261011-dialog-trap-and-progress-value.md)): the Antigravity runtime download (`providers-setup.contract`) and the context window meter (`composer-controls.contract`). VoiceOver hears a busy indicator, with no value. The Usage bars and any other drawn track and fill to be checked | The Antigravity download is `<progress aria-label="Antigravity download" value max>` (ProviderSetupSection.tsx); the context window a drawn `role="progressbar"` box with `aria-valuemin=0 aria-valuemax=100 aria-valuenow` (ContextWindowMeter.tsx); accessibility reads the value (AXValue/AXMinValue/AXMaxValue) |
| PB-2 | The X49/X55 workaround (`ProgressValue`, and the colour picker's slider values in `aria-description`, "Contract carries no aria-valuenow / aria-valuetext (X49)") is still in the clone; `EXACT2-GAPS.md` X49 and X55 say "the workaround stays until a follow-up task switches the bars" | — (retire the workaround; X49's main issue was fixed by `43f6d298c` and `ab9110c3a`) |

## Steps

1. Reference first (source under `target/t3-ref/src-1e2ecbd975`): list every `<progress>`, `role="progressbar"`,
   `aria-valuenow` and drawn track-and-fill of the web client, with its accessible markup, and the clone's counterpart.
2. PB-1: each clone bar becomes the reference's element: a `<progress value max>` is Contract's `progress value=… max=…`
   (AppKit's bar), a drawn `role="progressbar"` box with `aria-value*` stays drawn with the same attributes (AppKit's
   progress indicator element over the drawn fill); a bar the reference draws as something else (an image, a button, an
   `aria-hidden` decoration) keeps the reference's markup.
3. PB-2: delete `progress-value.contract` and its uses; the colour picker's stops and hue slider take the reference's
   `aria-valuemin`/`-valuemax`/`-valuenow`/`-valuetext`; `EXACT2-GAPS.md` X49 and X55 updated.
4. Tests; the AX tree (`tree --ax`) before and after; one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| PB-1 | reference markup listed; Bun tests; `tree --ax` (AXRole, AXValue, AXMinValue, AXMaxValue) on the bars before and after; agent drive | AX text before/after; images of each bar |
| PB-2 | `progress-value.contract` gone, no `aria-description` value left; `EXACT2-GAPS.md` X49, X55; `tree --ax` of the colour picker's sliders before and after | AX text before/after |
| VoiceOver on the bars | real-input steps for the coordinator's verification session | — |

## Reference

- `apps/web/src/components/settings/ProviderSetupSection.tsx` (Runtime row): `<div className="h-1">` holding, while
  `installation.phase === "downloading"` and `totalBytes > 0`, `<progress aria-label="Antigravity download"
  className="block h-1 w-full accent-foreground" value={downloadedBytes} max={totalBytes} />`.
- `apps/web/src/components/chat/ContextWindowMeter.tsx`: with `usage.maxTokens !== null`, `<div className="h-1.5 w-full
  overflow-hidden rounded-full bg-muted/60" role="progressbar" aria-valuemin={0} aria-valuemax={100}
  aria-valuenow={Math.round(normalizedPercentage)} aria-label="Context window usage">` around the coloured fill.
- `apps/web/src/components/usage/UsageLimits.tsx` WindowBar: `role="img"`, `tabIndex={0}`, `aria-label={summary}`
  ("<label>: N% left, M% of the window left, resets in …"), the fill and the hairline drawn inside;
  `UsageLimitsPooled.tsx` segment: a `<button>` named "<account>: N% left, …", its fill and hatch `aria-hidden`;
  `UsagePage.tsx` model breakdown bars: `aria-hidden`.
- `apps/web/src/components/ui/color-picker.tsx`: the plane's stops are `<input type="range" min=0 max=100>` with
  `aria-valuetext="N%"` and `aria-describedby` naming the plane's sr-only instructions ("Use arrow keys to adjust the
  focused value. Hold Shift for larger steps. Use Home and End for the minimum and maximum. Press Tab to move between
  saturation and brightness."); `ColorHueSlider` a `role="slider"` with `aria-valuemin={0} aria-valuemax={360}
  aria-valuenow={Math.round(value)}`. It has two users: `settings/ThemeColorPicker.tsx` (the theme editor's swatches)
  and `settings/ProviderAccentColorPicker.tsx` (a provider instance's accent: the plane `label="Accent color"`
  `variant="edge"`, the slider `label="Accent color hue"`; ProviderInstanceCard.tsx commits each change, 120 ms
  debounced).
- `apps/web/src/components/preview/PreviewChromeRow.tsx` `.preview-loading-progress`: `aria-hidden`.
- `apps/web/src/components/cloud/RelayClientInstallDialog.tsx` `<progress>` (T3 Connect: scope closed by the user) and
  `sidebar/DesktopUpdateStatusIcon.tsx` (the desktop update feed: scope closed) have no clone counterpart.

## What was built

**PB-1.** Every progress UI of the clone against the reference's markup (the reference's `<progress>`, `role="progressbar"`
and drawn tracks, listed in Reference):

- **The Antigravity runtime download** (`providers-setup.contract` ProviderRuntimeRow) is the reference's element:
  `progress value=runtime.progressValue max=runtime.progressMax aria-label="Antigravity download" display="block"
  width="100%" height="0.25rem" accent-color=…`, in the reference's `h-1` slot, shown while the download's size is
  known. On macOS it is AppKit's determinate `NSProgressIndicator` (small), an `AXProgressIndicator` with its own value,
  minimum and maximum (AppKit reports the value as a fraction, 0.3 of 1; VoiceOver speaks "30 percent"). The drawn
  track and fill, `progressPercent` and `progressLabel` are gone (`provider-install.ts`).
- **The context window bar** (`composer-controls.contract`, the context popover) is the reference's drawn
  `role="progressbar"` with `aria-valuemin=0 aria-valuemax=100 aria-valuenow` (the used share held to 0–100 and rounded,
  `composer-controls-view.ts` `contextValueNow`, as ContextWindowMeter.tsx's `Math.round(normalizedPercentage)`) and
  `aria-label="Context window usage"`; it keeps its drawn look (the red fill over 90%), and on macOS it is one
  `AXProgressIndicator` with value, minimum and maximum, its fill presentational (main's `43f6d298c`).
- **The Usage bars stay as the reference draws them**: UsageLimits.tsx's WindowBar is `role="img"` named with its
  figures ("5h limit: 58% left, 40% of the window left, resets in 3h"), UsageLimitsPooled.tsx's segment a button so named
  with its fill and hatch `aria-hidden`, UsagePage.tsx's model breakdown bars `aria-hidden`; the clone's are the same
  (an `AXImage` with that description on macOS, before and after). The specification named them; the reference makes
  none of them a progress element, and the user's decision of 2026-10-11 is to match the reference, so they are not
  changed. The browser surface's loading bar is `aria-hidden` in both. `progress-native-bars.test.ts` lists every drawn
  fill whose width is a percentage with its reference markup; a new one fails until it is classified.
- No other place puts a percentage in a busy indicator's name: the clone has no indeterminate `progress` left at all
  (the test checks every `progress`, `role="progressbar"`, `"slider"`, `"meter"`, `"spinbutton"` of the app).

**PB-2.** The X49/X55 workaround is retired: `progress-value.contract` (`ProgressValue`, the invisible busy indicator
named "<label>, <n>%") and `progress-value.test.ts` are deleted. The theme colour picker's other X49 workaround, the
saturation and brightness stops' and the hue slider's values in `aria-description` ("Contract carries no
aria-valuenow / aria-valuetext (X49)", `theme-color-picker.contract`), now carries color-picker.tsx's range values:
the stops `aria-valuemin=0 aria-valuemax=100 aria-valuenow=(s × 100) aria-valuetext="N%"` (the reference's `<input
type="range" min=0 max=100>` with `aria-valuetext`), the hue slider `aria-valuemin=0 aria-valuemax=360
aria-valuenow=round(h)`. Before, macOS exposed none of the three (a drawn `role="slider"` was no element before main's
`43f6d298c`); after, each is an `AXSlider` with value, minimum, maximum and, on the stops, the "N%" value description.
`EXACT2-GAPS.md` X49 (fixed on main, adopted; workaround: none, retired by this task) and X55 (the progress bars: none;
`status`, `alert` and the modal dialog still wait for main) are updated. The provider accent picker, color-picker.tsx's
second user, was missed in Step 1 and is built in review round 1 below.

**Declared difference (platform, LLP 1116 D8 as built):** AppKit gives a determinate bar no tint, so the Antigravity bar
draws in the system accent (grey while the window is inactive) and at AppKit's small bar's thickness, where Chrome draws
the reference's `<progress>` 4 px high in its own colours (green on grey in the reference app, which ignores
`accent-foreground` there); the clone writes the reference's `accent-color`, which iOS and the web honour. Recorded in
`EXACT2-GAPS.md` X49.

## Review round 1 (2026-10-12)

An independent review of #462 found two should-fix problems; both are fixed on `37ad3353f` and `201311707`.

1. **The provider accent picker, the second counterpart of color-picker.tsx** (`settings-b-accent.contract`
   ProviderAccentPicker, markup from the initial clone `b0a14280a`): its plane was a `role="group"` with no stops, no
   range values and no Tab stops, and its hue a `role="group"` named with its value (`Accent color hue ${floor(ch)}`),
   the value-in-the-name pattern this task retires. Now it is ProviderAccentColorPicker.tsx's: the plane (`role="group"`
   "Accent color saturation and brightness") holds two invisible, focusable stops, `role="slider"` "Accent color
   saturation" / "Accent color brightness" with `aria-valuemin=0 aria-valuemax=100 aria-valuenow=(s × 100)
   aria-valuetext="N%"`, the first autofocused as the popover opens (Base UI's initial focus, as the reference: its
   focus lands on "Accent color saturation"); the hue is a `role="slider"` "Accent color hue" with `aria-valuemin=0
   aria-valuemax=360 aria-valuenow=round(h)` and a Tab stop. The keys are color-picker.tsx's (stops: arrows 2 %, Shift
   10 %, Home/End; hue: arrows 1°, Shift 10°, wrapping), a drag focuses its control (the reference's pointerdown), and a
   keyboard focus draws the ring and the plane's "Saturation N%"/"Brightness N%" label (focus-visible): an opening by
   Enter or Space on the trigger shows it on the first stop, an opening by a press does not, as Chrome's focus-visible
   after Base UI's initial focus. The trigger hears its keys only (`201311707`): a `pointerdown` on it, the first
   version's, kept a press there from the window's outside-press count that closes the Usage page's pinned popover
   (`usage-pooled.test.ts` lists every pointer taker and failed on it). Each key
   commits its colour, as the reference commits each change; a colour the window drops because a command is pending
   (`providerAccent` takes none while `commandPending`) is sent when that command lands, through a busy tracker beside
   the trigger (2 wide while busy, its `resize` back to 1 sends the colour the picker reached; the theme picker's
   pattern, X22), so the last colour a key reached is the one saved. The claim "every range box says its value" now
   holds for groups too: `progress-native-bars.test.ts` fails on any `aria-label` that carries a value (`floor(`,
   `round(`, `%`), and checks the accent picker's markup.
2. **The stops' description** (`theme-color-picker.contract`, and the accent picker's new stops): each stop's
   `aria-describedby` names the plane's sr-only instructions, a `text` with the reference's sentence (1 px, clipped,
   transparent, as Tailwind's `sr-only`, the idiom of `settings-kit.contract` SettingsSrHeading), as color-picker.tsx's
   `aria-describedby={instructionsId}`. On macOS the stop's AXHelp is that text (`Accessibility.swift`
   `accessibleDescription`); in the reference it is the slider's description.

Live drive (agent mode, 1280×840): before on `t3-code-evidence-base` at `6db5cadea` (lane
`progress-native-bars-before`), after on this branch's bundle (lane `progress-native-bars`), reference T3 Code
1e2ecbd975 (lane `progress-native-bars`, CDP `Accessibility.getFullAXTree`). The after side ran on `37ad3353f`'s bundle
and, once more (the one retry), on `201311707`'s after the trigger fix, from the same state (the lane's accent reset to
none), with the same results plus the keyboard opening: Enter on the trigger focuses "Accent color saturation" with the
ring and "Saturation 94%", as the reference's. Opened, the clone's
picker reads as the reference's: the group, "Accent color saturation" 84.26 (focused), "Accent color brightness"
92.16, both with the instructions as their help, "Accent color hue" 221; after Shift+→, Tab, Shift+↓, Tab, → both read
94.26, 82.16 and 222, both draw the "Brightness 82%" label and the ring, and both saved `#0c47d2` (the lane's
settings). Before: two groups, "Accent color hue 221", nothing to Tab to. The theme picker's stops read the instructions
as their help after, as the reference's. The out-of-process AX reader reached no window this time (`windows: 0`; the
agent-mode window, as PV-1 found), so the AX values are the agent's `tree --ax`, the AppKit accessibility objects read
in process (role, name, value, help, focus); the range bounds are the markup's (tested) and were read out of process for
the theme picker's sliders in round 0.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| PB-1 | pass (out-of-process AX read, the client API VoiceOver uses; agent `tree`; reference over CDP): before, the Antigravity and context window bars are `AXBusyIndicator`s named "Antigravity download, 30%" / "Context window usage, 42%" with value 0; after, `AXProgressIndicator` "Antigravity download" value 0.3 of 0–1 and "Context window usage" value 42 (95) of 0–100, as the reference's `progressbar` 13680000 of 0–45600000 and 42 (95) of 0–100; the Usage WindowBar an `AXImage` named with its figures before and after (the reference's `role="img"`). Measured on a one-file probe per side (each bar's lines copied from the source, literals for the data) on the clone's own binaries: the lane can show neither bar live (a runtime download is a provider install, token usage a provider turn) | [pb1-bars-light.png](https://raw.githubusercontent.com/ccheever/exact2/0536f61abd7a4201708b1c75cca7f1734d0b7e06/progress-native-bars/pb1-bars-light.png), [pb1-bars-dark.png](https://raw.githubusercontent.com/ccheever/exact2/7af60a7844fef6a01978b7eb65602d96ba3d0f3f/progress-native-bars/pb1-bars-dark.png), [pnb-record.txt](https://raw.githubusercontent.com/ccheever/exact2/17071586f2ff54291ff630e3add36de44a984dbd/progress-native-bars/pnb-record.txt), [probe-before.contract.txt](https://raw.githubusercontent.com/ccheever/exact2/b11b04adc0d11ced8d9ace4e2ef5d89de4854398/progress-native-bars/probe-before.contract.txt), [probe-after.contract.txt](https://raw.githubusercontent.com/ccheever/exact2/a57b609cce832bfebe562be06457b7b2ef3dc1e1/progress-native-bars/probe-after.contract.txt) |
| PB-2 | pass (live agent drive, before and after, the same ops; out-of-process AX read; reference over CDP): before, the picker's stops and hue slider are no AX element (only the plane's group); after, three `AXSlider`s, "Background saturation" 10 of 0–100 ("10%"), "Background brightness" 0 of 0–100 ("0%"), "Background hue" 1 of 0–360 after Shift+→, Tab, Shift+↓, →, as the reference's sliders; the picker looks the same; `progress-value.contract` gone; `EXACT2-GAPS.md` X49, X55 | [pb2-picker.png](https://raw.githubusercontent.com/ccheever/exact2/03ab66c39466f63759789070d50b6f91a8fa0d74/progress-native-bars/pb2-picker.png), [pnb-record.txt](https://raw.githubusercontent.com/ccheever/exact2/17071586f2ff54291ff630e3add36de44a984dbd/progress-native-bars/pnb-record.txt), [drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/7413690b335128a8a1a4fa6c046fb0174d1b2ae7/progress-native-bars/drive.sh.txt) |
| R1-1 provider accent picker | pass (live agent drive before/after, the same opening; reference over CDP): before, two `AXGroup`s ("Accent color saturation and brightness", "Accent color hue 221"), no stop; after, the group with `AXSlider` "Accent color saturation" 84.26 (focused on open) and "Accent color brightness" 92.16, and `AXSlider` "Accent color hue" 221, as the reference's sliders (84.26, 92.16, 221; its focus on saturation); after Shift+→, Tab, Shift+↓, Tab, → the clone and the reference both read 94.26, 82.16, 222 and saved `#0c47d2`; opened by Enter on the trigger, both focus saturation with the ring and "Saturation 94%" | [r1-accent-open.png](https://raw.githubusercontent.com/ccheever/exact2/ab4dfb1fabe5f1fc39e94d70499543e6e3d61161/progress-native-bars/r1-accent-open.png), [r1-accent-keys.png](https://raw.githubusercontent.com/ccheever/exact2/375db47f5fc4cab9981872dd2dcb6395aadd218e/progress-native-bars/r1-accent-keys.png), [r1-accent-keyopen.png](https://raw.githubusercontent.com/ccheever/exact2/6de26c6d23ec099b81853f3c13cbdbc16ebb422a/progress-native-bars/r1-accent-keyopen.png), [r1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/7f1601bf6953eda4717171fadb79af752de87988/progress-native-bars/r1-record.txt), [r1-drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/fb4af11b737323a7935f41bc2c014d027801ac0e/progress-native-bars/r1-drive.sh.txt) |
| R1-2 the stops' description | pass (same drive): after, the theme picker's and the accent picker's four stops carry the instructions as their help ("Use arrow keys to adjust the focused value. …"), as the reference's sliders' description; before, none | [r1-theme-described.png](https://raw.githubusercontent.com/ccheever/exact2/a8c1ad03f1046e953b0c4c5dc6b7acab3dfa8796/progress-native-bars/r1-theme-described.png), [r1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/7f1601bf6953eda4717171fadb79af752de87988/progress-native-bars/r1-record.txt) |
| VoiceOver on the bars and sliders | implemented, not verified (real input is the coordinator's session): Real-input batch steps below | — |

Drives (agent mode, 1280×840): before on `t3-code-evidence-base` at `6db5cadea` (lane `progress-native-bars-before`),
after on this branch's bundle (`a69744d5b`'s app code, lane `progress-native-bars`), the clone's embedded server on
16722; the reference T3 Code 1e2ecbd975 on 16720/16721 (lane `progress-native-bars`): its colour picker opened in
Settings › Appearance › Create theme, and, for the bars, the reference's own markup injected into its renderer with its
stylesheet ([ref-inject.js.txt](https://raw.githubusercontent.com/ccheever/exact2/987dd152ccacc73f17602c014bc755784006b8d1/progress-native-bars/ref-inject.js.txt), AX by [ref-ax.mjs.txt](https://raw.githubusercontent.com/ccheever/exact2/e12db4a1eda989e9fd4f7e4341c14426c906b8af/progress-native-bars/ref-ax.mjs.txt)). The out-of-process reader: [axread.swift.txt](https://raw.githubusercontent.com/ccheever/exact2/455cbc84b3a7aceed006c84e43c69ffb6f21b99d/progress-native-bars/axread.swift.txt)
(PV-1 found an agent-mode window unreachable from another process; with this terminal trusted for accessibility it is
reachable). Images: [compose.py.txt](https://raw.githubusercontent.com/ccheever/exact2/a3571c820a69c84e9fe5021a1e7c8e397cb24f3c/progress-native-bars/compose.py.txt). The live drive of the after bundle ran once, on its first run.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own name and bundle id, the lane home outside any git checkout). Never
send a message. Turn VoiceOver on (⌘F5) only for these steps and off after.

1. **PB-2, the colour picker.** Settings › Appearance › Create theme › the Background swatch. VO on the plane's first stop
   reads "Background saturation, 0%, slider" (or the current percentage); Shift+→ changes it and VO speaks the new
   value ("10%"); VO-→ to "Background brightness, N%, slider"; then "Background hue, 0, slider", → speaks 1. Escape twice.
1a. **Review round 1, the accent picker.** Settings › Providers › Codex › the "+" beside Display name. The focus lands
   on the plane's first stop: VO reads "Accent color saturation, 84%, slider" and, after a pause, the instructions ("Use
   arrow keys to adjust the focused value. …"); Shift+→ speaks "94%"; Tab: "Accent color brightness, 92%"; Tab:
   "Accent color hue, 221, slider" (no number in the name), → speaks 222. Then press Clear color (the accent goes back to
   none). In the theme editor's picker (step 1) VO likewise reads the instructions on each stop.
2. **PB-1, the bars, on the probe.** The lane cannot download Antigravity or show a thread's token usage without a
   provider, so use the probe: `target/debug/contract build probe-after.contract -o probe-after.plan` (the file from the
   evidence above), then launch the lane copy's executable with `EXACT_PLAN=<abs>/probe-after.plan` (and the lane's
   `T3_LOCAL_HOME`/`T3_LOCAL_PORT` in the 16000–16999 range). VO on the bar under "Downloading 13.7 MB of 45.6 MB." reads
   "Antigravity download, 30%, progress indicator"; on the two context window bars, "Context window usage, 42%" and
   "…, 95%, progress indicator"; on the orange bar, "5h limit: 58% left, 40% of the window left, resets in 3h, image".
3. **PB-1 in the app, only if the session has them:** Settings › Providers › Antigravity while its runtime downloads (VO:
   "Antigravity download, N%, progress indicator"); a thread with token usage, the composer's context ring › the popover
   bar (VO: "Context window usage, N%, progress indicator"). Without either, leave the row to step 2.

## Tests

- `progress-native-bars.test.ts` (new, 8): the Antigravity row's value and max (bytes, nothing without a size, the
  workaround's fields gone); the context window's `contextValueNow` (42, 4.5 → 5, held to 100, 0 without a size); the
  Antigravity `progress value max` with the reference's label, block and `h-1` slot, not `aria-hidden`; the context
  window's drawn `role="progressbar"` with the reference's range values and its fill inside; the colour picker's range
  values and no `aria-description` value; no `ProgressValue`, no indeterminate `progress` named with a value, no range
  role without `aria-valuenow` anywhere in the app; every drawn percentage fill classified against the reference; the
  Usage bars' and the loading bar's reference markup. Two mutations fail it (the hue slider back on `aria-description`:
  2 fail; the context bar back to `aria-hidden`: 1 fail).
- `progress-value.test.ts` deleted with its contract; `provider-setup.test.ts` drops `progressPercent`.
- Review round 1: `progress-native-bars.test.ts` (9): the theme picker's stops with `aria-describedby` and the
  instructions' text; the provider accent picker's stops (inside the plane, Tab stops, the first autofocused, their keys,
  range values, description) and hue slider (`role="slider"` "Accent color hue", 0–360, `aria-valuenow`); the app-wide
  check also fails on a value in an `aria-label`. The hue back to `role="group"` named `Accent color hue ${floor(ch)}`
  fails 2. `settings-b-accent.test.ts` (+3): colorPickers.test.tsx's two ProviderAccentColorPicker key tests ported with
  their names (each key's commit and the reported values), and the Contract key rules checked against color-picker.tsx's.

## Checks

Review round 1, once each on `201311707` (round 0's base unchanged: `origin/feat(example)/t3-code` is still `6db5cadea`,
#458's head still `213149fe7`, so nothing to merge):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4661 pass, 1 skip, 0 fail (311 files); on `37ad3353f` it failed 1, `usage-pooled.test.ts`'s pointer takers (the trigger's `pointerdown`), fixed by `201311707` |
| strict `tsc` on `app.ts` (README command) | 0 | clean |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,398 lines (unchanged); `settings-b-accent.contract` 178 |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed: the same `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on main's `apps/color-gallery/app.contract:269`, the sweep's only failure (nothing in `examples/t3-code`) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/caps.mjs` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

The bundle was built for each after drive (`host/apple/build.mjs t3-code-macos --bundle`, exit 0: `37ad3353f`, then
`201311707`).

Round 0, once each, on `ca333b0f0` (this branch with the adoption branch's head `213149fe7`, which has not moved; #458 is open,
so nothing else to merge; `origin/feat(example)/t3-code` is still `6db5cadea`):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4657 pass, 1 skip, 0 fail (311 files) |
| strict `tsc` on `app.ts` (README command) | 0 | clean |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,398 lines (unchanged) |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed: `contract --test it` `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on main's `apps/color-gallery/app.contract:269` ("styled default button needs appearance=\"none\""), inherited from main through #458 (its record: "the gate is red from main's color-gallery"); nothing in `examples/t3-code` |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/caps.mjs` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

The bundle was built once (`host/apple/build.mjs t3-code-macos --bundle`, exit 0, from `a69744d5b`'s app code, which the
final head keeps) before the one live drive.

## Not done / not verified

- VoiceOver hearing the bars and the sliders: ran in `realinput-1011c` ("Real-input results" below).
- The accent sliders' AXMinValue/AXMaxValue on macOS: not read this round. The out-of-process reader reached no window
  (`windows: 0`), so the drive read the AppKit accessibility objects in process (`tree --ax`: role, name, value, help,
  focus), which does not report the bounds. The bounds are the markup's (`progress-native-bars.test.ts`) and the same
  host path served them for the theme picker's identical sliders out of process in round 0 (0–100, 0–360).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, PB-2 (the theme editor's sliders) | G5-D3 (B6, VoiceOver) | Partial: both apps speak the values on the arrows; the clone speaks the hue slider as a percentage ("0%"; reference "0") and gives the instructions as a delayed hint, not the description | [G5-D3-captions-pair.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/pairs/G5-D3-captions-pair.png) |
| 1a, the accent picker | PNB-1a (B6x, VoiceOver) | Partial: Clear color passes; the hue is spoken "61.4%" (reference "221"), Shift+→ does not speak the new saturation ("94%"), and the instructions come as a delayed hint | [PNB-1a-captions-pair.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/pairs/PNB-1a-captions-pair.png) |
| 2, PB-1 on the probe | PNB-2 (B6x, clone only) | Pass: all four bars and the limit image spoken as expected | [PNB-2-context.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/PNB-2-context.png) |
| 3, PB-1 in the app | G5-D1 (B6) | Pass: the context window bar is spoken as its value; the Antigravity half not run (no download in the session); recorded: VoiceOver's order puts the composer's "Message" between the popover's description and its bar | [G5-D1-caption-bar-pair.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/pairs/G5-D1-caption-bar-pair.png) |

Not passed: G5-D3 and PNB-1a (the hue's spoken value, the instructions' timing).

## Delivery

Merged on 2026-10-12 as `fe0e39c91` (#462, squash) into `feat(example)/t3-code`, after #458.

## Next action

None. Closed after `realinput-1011c`; the sliders' spoken hue and delayed instructions (G5-D3, PNB-1a) are in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
