---
name: 20261011-progress-native-bars
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
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

## Next action

Build PB-1 and PB-2.
