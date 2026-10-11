---
name: 20261011-progress-native-bars
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-progress-native-bars
pr_url:
verified_commit:
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
| PB-1 | Every progress UI is a drawn track and fill, `aria-hidden`, with an invisible indeterminate `progress` (a busy indicator) beside it named "<label>, <n>%" (`progress-value.contract` `ProgressValue`, PV-1 of [dialog-trap-and-progress-value](closed/20261011-dialog-trap-and-progress-value.md)): the Antigravity runtime download (`providers-setup.contract`) and the context window meter (`composer-controls.contract`). VoiceOver hears a busy indicator, with no value. The Usage bars and any other drawn track and fill to be checked | The Antigravity download is `<progress aria-label="Antigravity download" value max>` (ProviderSetupSection.tsx); the context window a drawn `role="progressbar"` box with `aria-valuemin=0 aria-valuemax=100 aria-valuenow` (ContextWindowMeter.tsx); accessibility reads the value (AXValue/AXMinValue/AXMaxValue) |
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
  `aria-valuetext="N%"`; `ColorHueSlider` a `role="slider"` with `aria-valuemin={0} aria-valuemax={360}
  aria-valuenow={Math.round(value)}`.
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
`status`, `alert` and the modal dialog still wait for main) are updated.

**Declared difference (platform, LLP 1116 D8 as built):** AppKit gives a determinate bar no tint, so the Antigravity bar
draws in the system accent (grey while the window is inactive) and at AppKit's small bar's thickness, where Chrome draws
the reference's `<progress>` 4 px high in its own colours (green on grey in the reference app, which ignores
`accent-foreground` there); the clone writes the reference's `accent-color`, which iOS and the web honour. Recorded in
`EXACT2-GAPS.md` X49.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| PB-1 | pass (out-of-process AX read, the client API VoiceOver uses; agent `tree`; reference over CDP): before, the Antigravity and context window bars are `AXBusyIndicator`s named "Antigravity download, 30%" / "Context window usage, 42%" with value 0; after, `AXProgressIndicator` "Antigravity download" value 0.3 of 0–1 and "Context window usage" value 42 (95) of 0–100, as the reference's `progressbar` 13680000 of 0–45600000 and 42 (95) of 0–100; the Usage WindowBar an `AXImage` named with its figures before and after (the reference's `role="img"`). Measured on a one-file probe per side (each bar's lines copied from the source, literals for the data) on the clone's own binaries: the lane can show neither bar live (a runtime download is a provider install, token usage a provider turn) | [pb1-bars-light.png](https://raw.githubusercontent.com/ccheever/exact2/0536f61abd7a4201708b1c75cca7f1734d0b7e06/progress-native-bars/pb1-bars-light.png), [pb1-bars-dark.png](https://raw.githubusercontent.com/ccheever/exact2/7af60a7844fef6a01978b7eb65602d96ba3d0f3f/progress-native-bars/pb1-bars-dark.png), [pnb-record.txt](https://raw.githubusercontent.com/ccheever/exact2/17071586f2ff54291ff630e3add36de44a984dbd/progress-native-bars/pnb-record.txt), [probe-before.contract.txt](https://raw.githubusercontent.com/ccheever/exact2/b11b04adc0d11ced8d9ace4e2ef5d89de4854398/progress-native-bars/probe-before.contract.txt), [probe-after.contract.txt](https://raw.githubusercontent.com/ccheever/exact2/a57b609cce832bfebe562be06457b7b2ef3dc1e1/progress-native-bars/probe-after.contract.txt) |
| PB-2 | pass (live agent drive, before and after, the same ops; out-of-process AX read; reference over CDP): before, the picker's stops and hue slider are no AX element (only the plane's group); after, three `AXSlider`s, "Background saturation" 10 of 0–100 ("10%"), "Background brightness" 0 of 0–100 ("0%"), "Background hue" 1 of 0–360 after Shift+→, Tab, Shift+↓, →, as the reference's sliders; the picker looks the same; `progress-value.contract` gone; `EXACT2-GAPS.md` X49, X55 | [pb2-picker.png](https://raw.githubusercontent.com/ccheever/exact2/03ab66c39466f63759789070d50b6f91a8fa0d74/progress-native-bars/pb2-picker.png), [pnb-record.txt](https://raw.githubusercontent.com/ccheever/exact2/17071586f2ff54291ff630e3add36de44a984dbd/progress-native-bars/pnb-record.txt), [drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/7413690b335128a8a1a4fa6c046fb0174d1b2ae7/progress-native-bars/drive.sh.txt) |
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

## Next action

Final checks on the final head, then the draft PR.
