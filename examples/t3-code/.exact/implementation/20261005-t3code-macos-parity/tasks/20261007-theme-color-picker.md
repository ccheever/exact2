---
name: 20261007-theme-color-picker
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-theme-color-picker
pr_url: https://github.com/ccheever/exact2/pull/252
verified_commit: null
---

# Choose custom theme colors with hue, saturation, brightness and RGB

## Outcome

The theme editor's color swatch opens the reference color picker: a saturation/brightness
plane, hue control, HEX field and RGB field. Users can choose arbitrary colors visually or
enter their RGB values, with every representation and the draft preview staying in sync.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.

1. Open Settings > Appearance > Create theme in each running app.
2. Open the Background color swatch.
3. Inspect the controls offered by the popup.

The reference shows the saturation/brightness plane, hue slider, `Background picker hex value`
and `Background picker RGB value`. The Exact popup offers 24 fixed color swatches and closes
when one is chosen. Its surrounding editor row permits manual HEX entry, but the popup has
no arbitrary visual color selection or RGB entry. This changes how a user can choose a color;
it is not only a layout discrepancy.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-appearance-create-color.{png,txt}`
- `target/desktop-audit/native/native-settings-theme-color.{png,json}`
- `target/desktop-audit/native/native-settings-theme-color-selected.{png,json}`

These captures are local artifacts and are not committed. Only the controls and preset-selection
path were compared here; the acceptance rows below require new verification after implementation.

## Scope and guidance

Reference `apps/web/src/components/settings/ThemeColorPicker.tsx` defines `ThemeColorPickerPanel`,
RGB parsing, HEX synchronization and preservation of an existing alpha suffix. It uses
`ui/color-picker.tsx` for `ColorSaturationValuePlane` and `ColorHueSlider`. Port the reference's
input commit/invalid-draft behavior rather than normalizing incomplete edits on every keystroke.

Clone `settings-appearance-editor.contract` `EditorPicker` renders `presets`; the fixed catalog
is `PRESETS` in `settings-appearance-editor.ts`. `EditorColorRow` separately renders the HEX
field. Keep the editor's current draft/save/cancel path and replace the popup's limited selection
controls. The provider accent control in `settings-b-accent.contract` and `settings-b-accent.ts`
already implements a hue control, a saturation/brightness plane and HSV conversion; inspect
and reuse suitable behavior while preserving theme-specific alpha and input rules.

Apply the picker to every existing theme color row in Create, Edit and Duplicate, simple and
advanced modes. Keep the row field, popup fields, swatch and draft preview synchronized. The
reference's accessible plane exposes separately adjustable saturation and brightness values;
retain keyboard controls, visible focus, Escape dismissal and trigger focus return.

## Dependencies and deduplication

- [Settings scopes and theme editor](closed/20261005-settings-scoped-controls-and-theme-editor.md)
  owns the floating editor's app-wide lifetime, drag, resize, minimize and save notices. Its D16
  scope does not include the color picker's missing controls.
- [X30](../issues/20261005-x30-ts-announce-readback-picker.md) tracks Inspect/pixel readback.
  Choosing a color in the editor does not require screen inspection.
- No framework blocker was demonstrated; the example already has hue/plane controls for
  provider accents. Pure spacing and color differences remain in the audit's shared visual task.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Arbitrary visual selection | Drag/click the plane and hue control to a non-preset color | Selection marker, swatch, HEX, RGB and app preview update together as in the reference | Paired live drive and captures |
| HEX and RGB input | Enter valid HEX and RGB values; leave partial/invalid values and blur | Reference parsing, commit and invalid-draft behavior; values never diverge | Focused input tests and UI drive |
| Alpha preservation | Edit a theme role whose current value has an alpha suffix | Hue, plane and RGB changes preserve the alpha behavior of `ThemeColorPickerPanel` | Conversion test and saved theme readback |
| Reopen and switch colors | Close/reopen the popup and move between color rows | Each control starts from the current row's value without leaking another row's draft | Live drive |
| Editor modes | Exercise Create, Edit and Duplicate in simple/advanced modes; Save or Cancel | Saved theme equals the chosen colors; Cancel restores the prior theme | Theme readback and relaunch |
| Keyboard | Adjust saturation/brightness/hue with arrows, Shift, Home/End; Tab and Escape | Same accessible control behavior and focus return as the reference | Bounded keyboard drive |

## Progress

Implemented on `feat(example)/t3-code-theme-color-picker` (2026-10-08) from `feat(example)/t3-code` `d82fb6a47`;
commits `454daaff3` (picker) and `b2074c7a1` (independent-review fixes). Reference `1e2ecbd975`.

| Scope item | Built | Where |
| --- | --- | --- |
| The reference panel | The swatch opens `ThemeColorPickerPanel`: header (label, "Choose a color", current swatch), the saturation/brightness plane over the hue (two `linear-gradient` layers), the hue slider, HEX (with its dot) and RGB fields; 18rem, the reference's sizes. The 24 presets and `PRESETS` are gone. Tooltip "Choose <label> color" on the swatch | `theme-color-picker.contract` `ThemeColorSwatch`, `settings-appearance-editor.contract` `EditorColorRow` |
| Pointer | `pointerdown`/`pointermove`/`pointerup` on the plane and the slider: a press jumps to the point and focuses saturation (plane) or the slider; a drag follows the held pointer, clamped; the thumbs animate 80 ms except while dragged; one pointer at a time | same |
| Keys | Plane stops (saturation, brightness: invisible, focusable, `role="slider"`): arrows ±2 % (Shift ±10 %), Home/End; slider: arrows ±1° (Shift ±10°), wrapping; the focus ring and the "Saturation/Brightness N%" label only for a keyboard focus; the popover opens with saturation focused (Base UI's initial focus); Escape closes only the popover and returns the focus to the swatch | same |
| Commits | Each change is a `themelocal:<part>` op with id `session-N|role` and a stamp rising strictly from the runner time, on its own mutation (`app.contract` `themeLive`, `coreThemeLive`); one op is in flight at a time, the op's landing sends the latest colour a drag has reached, a release or a key always sends; the draft drops an op at or below the role's last stamp or from another session and echoes the stamp (`row.seq`) | `theme-color-picker.contract`, `settings-appearance-editor.ts` `themeLocal`, `client-ops-lanes.ts`, `client.ts` (local op) |
| Display | The panel shows its own HSV while a drag is on, an op is in flight or the row shows its colour (a grey keeps its hue), otherwise the row's (another path changed it, a typed value landed, Light/Dark switched) | `theme-color-picker.contract` `own` |
| HEX and RGB | Every keystroke goes to the draft, which parses as `handleHexChange` (six digits, lowercased, drops the alpha) and `handleRgbChange` (three integers 0–255, `rgb()` or spaces, keeps the alpha); a partial value stays as typed and changes nothing; a blur shows the current colour | `theme-color-picker.ts` `pickerCommit`, `themeRgbToHex` |
| Alpha | `updateFamily` keeps an alpha suffix in the chosen role and composites it over the canvas (the sidebar for the selection) for the roles it derives, as `updateThemeColorFamily` does; the plane, the slider and RGB re-attach the suffix | `settings-appearance-editor.ts` |
| Placement | Below the swatch 10 apart, or above when the window has no room below (Base UI's flip); left-aligned and clamped into the window by the host (no `span-left`, no `position-try`: X17, #112) | `theme-color-picker.contract` `open` |
| Escape layering | The popover is `aria-modal`, so Settings' Back (an Escape shortcut) no longer takes the popover's Escape. The editor's own Escape shortcut stays (D16), **provisional, user decision pending**: the reference's panel has no Escape close (`ThemeEditorPanel.tsx:692` only cancels Inspect) | `settings-appearance-editor.contract` |
| Click pass-through | The popover box takes `press` and `retainFocus=true` so a click on the plane does not also reach the editor control under the popover (seen in the clone: the Dark toggle pressed, the name field focused) | X50 (local draft, unconfirmed: not reproduced in a one-file app) |

Accessibility gaps kept: a stop's value is its `aria-description` (`"53%"`), not `aria-valuenow`/`aria-valuetext` (X49);
the HEX/RGB labels are text beside the fields, not `<label>`s (a click on "HEX" does not focus the field).

### Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Arbitrary visual selection | **Pass (agent)**: plane down at (60,100) → `#382b2b`, two moves → `#563535`, `#743636`, release; hue drag over 400 ms → `#365a74` (s and v kept); marker, current swatch, HEX/RGB, the row and the app's palette read back equal at each step. Real-pointer drags: **deferred to the real-input batch — screen locked (user away)** | [plane](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/04-plane-drag.png), [hue](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/05-hue-drag.png), [record](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/agent-drives.txt) |
| HEX and RGB input | **Pass**: RGB `12, 34, 56` → `#0c2238`; HEX `#ABCDEF` → `#abcdef`; `#12` and `12, 34` change nothing and stay as typed; the blur shows `#abcdef` | [typed](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/07-typed.png), `theme-color-picker.test.ts` |
| Alpha preservation | **Pass**: row `#ff000080` (the base stored `#ff0000`) → plane `#34242480` → hue `#24342c80` → RGB `0, 0, 255` → `#0000ff80`; saved and read back in Edit as `#0000ff80`; HEX typing replaces it whole, as the reference | [alpha](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/08-alpha.png), [readback](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/09-saved-readback.png), tests |
| Reopen and switch colors | **Pass**: after Background → `#743636`, the Accent popover starts at `#1b4ed8` and Background reopens at `#743636`; each opening starts from the row (no leak between rows or sessions) | [record](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/agent-drives.txt) |
| Editor modes | **Pass (agent)**: Create simple (save "Picked", active, Edit readback), Edit advanced (Border keys → `#2200ce`, Cancel → Edit shows the saved `#e4e4e7`), Duplicate Grove (keys → `#061a0d`, saved, active); tests cover Create/Duplicate/Edit × simple/advanced Save and Cancel. Relaunch: **deferred to the real-input batch** (an agent session starts on a fresh store) | [duplicate](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/10-duplicate.png), [advanced](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/03-advanced-border.png), tests |
| Keyboard | **Pass (agent)**: Shift+← on saturation, Shift+↓/End on brightness, Home on saturation, Shift+→ on the slider; Tab from saturation to brightness shows the ring and "Brightness 35%"; Escape closes only the popover, the focus returns to the swatch and Settings stays; a second Escape closes Settings, the editor stays (the reference's `useEscapeToGoBack`). VoiceOver: not run | [keyboard](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/06-keyboard.png), [escape](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/02-escape.png) |
| Before/after | Base popover (24 presets) vs the picker; Escape; Advanced | [popover](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/01-background-popover.png), [escape](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/02-escape.png), [advanced](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/03-advanced-border.png) |

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `454daaff3` | Agent drives found: a hue drag highlighted the whole window's text (fixed with `user-select: none`); Escape with the popover open closed Settings instead (fixed with `aria-modal`); a click on the plane also pressed the Dark toggle or focused the name field under the popover (fixed with `press` + `retainFocus` on the popover; X50); the popover cut at the window's bottom (fixed: it flips above) | lane notes (not committed), X50 | — |
| 2 | `b2074c7a1` | Independent review (a separate agent): two blocking findings (the marker snapping back mid-drag; the editor's Escape removed against D16) and four medium ones (Light/Dark leak, equal stamps, release offsets, shared mutation), all fixed; drives re-run on the rebuilt bundle | [record](https://raw.githubusercontent.com/ccheever/exact2/7ba5e1b01365b57521150b27d7cad76a9b7998f9/theme-color-picker/agent-drives.txt) | real-input rows (screen locked), relaunch, user decision on the editor's Escape |

Checks on `b2074c7a1`: `bun test examples/t3-code` 2542 pass, 1 skip, 0 fail (205 files); strict `tsc` clean;
`contract build` 2662 slots, 46 resources, 2712 actions, 59474 nodes; `cargo test -p t3-code-macos --lib` 11 pass;
`git add -A && bun scripts/caps.mjs` within caps (`app.contract` 1500 lines); the five repository checks exit 0
(`cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests --no-fail-fast` 3383 passed / 0 failed /
33 ignored, clippy `-D warnings`, `fmt --check`, `boot`). No Swift changed, so no AppKit binary was run.

## Real-input batch steps

Deferred to the real-input batch — screen locked (user away). One session, real-input lock held:

1. Build: in this worktree, `export PATH="$HOME/.bun-1.4.2/bin:$PATH"`, `EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle`.
2. Lane copy: copy `target/clients/*/com.exact.t3code.macos/macos/T3 Code (Exact).app` to `target/lane-tcp/app/T3 Code (Lane TCP).app`, set `CFBundleIdentifier` `com.exact.t3code.lanetcp`, `CFBundleName`/`CFBundleDisplayName` "T3 Code (Lane TCP)", `codesign --force --deep -s -`.
3. Launch the copy's `Contents/MacOS/T3 Code (Exact)` with `T3_LOCAL_HOME=target/lane-tcp/real/t3-home T3_LOCAL_PORT=16052 T3CODE_TELEMETRY_ENABLED=false` and lane `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*`; record the pid; window 1280×840.
4. Welcome: Continue, Continue, Skip import. Settings (gear, bottom left), Appearance, Create theme; Background swatch (the popover opens above it).
5. `orca computer drag` on the plane from (60,100) to (140,70) in plane points (read the plane's window position from the capture): read back the HEX field (`#743636`), the swatch and the window background (the preview) from `screencapture -l`.
6. Pause mid-drag: press at one point, drag, hold still 1 s (no release), capture: the window's background must already show the colour under the marker; then release.
7. Drag on the hue slider from x 30 to x 150: HEX `#365a74`.
8. Click on the plane where the editor's Light/Dark toggle lies under it: the editor stays Light and the name field does not take the focus (X50 workaround).
9. Keys: Tab (ring + "Brightness N%"), ↑/↓ with and without Shift, Home/End, Shift+→ on the slider; Escape: the popover closes, the swatch shows the focus ring, Settings stays; Escape again: Settings closes, the editor stays.
10. Paste `12, 34, 56` into RGB (⌘A first): HEX `#0c2238`. Name "Real input", Create theme; quit (⌘Q hold), relaunch the same copy: the theme card is there and Edit theme shows Background `#0c2238`.
11. Release the lock; kill only the recorded pid if it is still running.

## Next action

Review the draft PR. User decisions: keep or drop the editor's Escape close (D16 vs the reference). Run the
real-input batch steps above when the screen is unlocked. X50 stays a local draft until reproduced outside the clone.
