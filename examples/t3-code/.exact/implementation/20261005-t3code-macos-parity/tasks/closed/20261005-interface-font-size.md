---
name: 20261005-interface-font-size
plan: 20261005-t3code-macos-parity
implementation: complete
verification: verified-with-unverified-rows
delivery: pr-open
repository: https://github.com/ccheever/exact2
base_branch: 'feat(example)/t3-code'
branch: 'feat(example)/t3-code-interface-font-size'
pr_url: https://github.com/ccheever/exact2/pull/206
verified_commit: null
---

# Interface font size: root size, `rem` support, class map and shared styles

## Outcome

This ticket builds the foundation for Settings › Appearance › Interface font size (12 to 20 px, default 16).
The app sets the root font size from the setting, `rem` sizes work in the Contract places the app needs, a
written map says which sizes are `rem` and which are `px` in the reference, and the shared `style` definitions
that many screens use already follow the setting. After this ticket the shared pieces scale at once and survive a
relaunch, the `px` items (prompt, code, diff and terminal text, borders, the 52 pt top bar) do not, and at 16 the
interface is the same as today. The per-area conversion of the remaining screens is planned from the map when this
ticket is verified (see Next action).

## Scope and exclusions

Included:

1. **Root size**: after the framework fix for X3, the app sets the root font size from
   `clampInterfaceFontSize(fontSizeInterface)` at startup and on every change. Before the preferences load it is 16.
   The unused `look.fontSize` field (`settings-appearance-look.ts:55`) feeds this or is removed.
2. **Feasibility check** of `rem` in the Contract: for each property kind the app uses (`font-size`, `line-height`,
   `width`/`height`/`min-`/`max-`, `padding`, `margin`, `gap`, `border-radius`, positions, icon sizes), whether a `rem`
   value compiles and `layout` reports the scaled value; and whether a `number` component prop or a `derive` can carry
   `rem`. The result and the chosen pattern are written down with a compile-checked example.
3. **Class and size map**: from the reference desktop app run at 12, 16 and 20, a map of every Contract file and
   `style` definition to `rem` or `px` with the oracle evidence. Format: `font-size-map.json` in the example
   (file, selector or style name, property, unit, probe values at 12/16/20) plus a summary in `AGENT-HANDOFF.md`.
4. **Shared styles**: convert the `style` definitions (59 in 12 files in the mc-orch tree on 2026-10-05, used through
   `class=` in 283 places) to `rem` where the map says so. Views that set sizes inline are not converted here.
5. **Ported tests**: `clampInterfaceFontSize`, `clampPromptFontSize`, `clampCodeFontSize`
   (`describe("font size clamping")`, 2 tests, `appearanceFonts.test.ts:111-125`).

Excluded: converting inline sizes in views, native modules with fixed metrics, and JS layout constants such as the sidebar
minimum width and the chat lane limits. They become per-area conversion tickets created from the map after this ticket is
verified. Also excluded: the setting row, storage and ranges (done: `settings-core.ts:52,66`, `settings-appearance.ts:193`),
the prompt, code and terminal size settings (they stay px), the Settings page layout.

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/web/src/appearanceFonts.ts:95-134`
(`root.style.fontSize = <n>px`; prompt, code and diff sizes are written in px so that "they do not scale twice"),
`packages/contracts/src/settings.ts:116-128` (12–20, default 16), `apps/web/src/routes/__root.tsx:304-338`
(`FontAppearanceSync`), `SettingsPanels.tsx:1536-1556` (row in Simple and Advanced views), `index.css:116-131,178-185`
(a 52 px top bar and a 12 px glass blur in px; control sizes, content widths and the extra text sizes in rem). In the web tree
I count 86 arbitrary pixel classes (`[Npx]`) and 67 arbitrary rem classes (`[Nrem]`) in `.tsx` files; examples of px: `h-[22px]`,
`text-[11px]`, `min-w-[760px]`. Reference quirk to keep: the details card layout reserves 280 px
(`threadDetailsCardLayout.ts:30`) while the card's CSS width is `17.5rem` (`index.css:129`).
Clone state (mc-orch tree, 2026-10-05): the setting is stored and the row works. `look.fontSize` has no Contract reader. Only the sidebar
minimum width reads the value (`presentation.ts:127`, `r4-polish-sidebar-width.ts:68`, `app.contract:486`). The Contract files hold
1,622 `font-size=` attributes (`rg` count) and no `rem` value. The macOS host sends a root size of 16 today (`EXACT2-GAPS.md` X3).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (bounded layout, units), design (typography,
layout stability), testing-and-debugging (resolved geometry, not only pixels), platforms (resize). Whether Contract accepts `rem` for
each property, whether a `number` component prop can carry `rem`, and the host fact for the root size are unknown in the library.
Consumer framework revision: a `main` pin that contains the X3 fix.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, the computed-style probe
(`target/t3-ui-parity/rem-probe.mjs`, new), `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged (the oracle runs at 12, 16 and 20) | pending |
| resolved framework issue | [X3 root font size](../../issues/closed/20261005-x03-root-font-size.md) | none yet (local draft) | Fix merged into `main` upstream (this plan files the issue only) and the example pinned to it, or the user waives | resolved: #102 closed by main #185; the branch merged main `cff90b364` |
| recorded decision | Apparatus approval: the computed-style probe `rem-probe.mjs` | none | User approves | pending |
| scheduling preference | After `20261005-floating-device-player` and `20261005-terminal-drawer` | none | Not a prerequisite | — |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X3](../../issues/closed/20261005-x03-root-font-size.md) | App-settable root font size (`rem` base) | `EXACT2-GAPS.md` X3: the host always sends 16 | blocking if reproduced (no workaround; the agent `prefer page root-font-size` can preview only) | `issue-open` reproduces it on the pin and files it; the upstream fix lands; then this ticket |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Line cap | `app.contract` near 1,500 lines | nonblocking | Conversion must not add lines; check `caps.mjs` |
| [X10](../issues/20261005-x10-text-rendering-parity.md) | Chrome text rendering differences | Word-boundary ellipsis, weight | nonblocking | Judge scaling by `layout` geometry, not only pixel scores |
| new — record at prepare | A `number` prop or `derive` cannot carry `rem` | Unknown | unknown | The feasibility check decides; record the gap if it fails |

## Implementation notes

- Feasibility first, on three slices (a sidebar row, a settings row, the composer): compile a `rem` value for each property kind; check that
  `layout` reports scaled values when the root changes; choose how component props carry units (a unit string, a derived `rem` string in a
  template, or a helper). Record the choice and a test before the conversion starts.
- Classify, do not guess. For each target screen, run the oracle at 12, 16 and 20 and dump computed `font-size`, `line-height`, sizes, spacing and
  radii per element with the probe. A property whose value scales with the root is `rem`; one that does not is `px`. Convert shared styles with a
  reviewed script that only proposes edits; a person approves each file. Never use a global replace.
- At 16 the converted values must equal today's values (`N px` ↔ `N/16 rem`). The round-11 matrix is the regression gate.
- Breakpoints and thresholds (sheet layout at 980 pt, narrow-composer rules): record in the map what the oracle shows at 12 and 20 (they may or
  may not move with the setting); the follow-up tickets use it.
- States. Disabled: none. Loading: before the preferences load, 16. Error: an invalid stored value (NaN, string, out of range) rounds and clamps to
  12–20, or 16 for non-numbers. Empty, hover, keyboard focus and permission: unchanged; they scale with their parents. The Settings slider keeps its
  `aria-label` and arrow-key steps (1 px); no dialog or menu is added, so there is no Escape case. Motion: none. A slider drag updates the interface at
  once; there is no animation, so reduced motion changes nothing.
- Risks: windows at the minimum 840×620 with 20 px for the converted shared pieces; text clipped by fixed heights; the probe must run on the same
  fixture state at all three sizes.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Clamp tests | — | `bun test examples/t3-code` | Ported `clamp*FontSize` tests pass | macOS | log |
| Root size applies | Lane backend (`target/t3-ui-parity/lane-backend.sh`, ports 16000–16999); data folder reset | Agent `tap` the slider to 20, then 12; `state`, `layout` of a fixed list of test ids | The root fact equals the value; each converted element scales by value ÷ 16; each `px` element stays | macOS 1280×840 | `--json` transcript |
| Feasibility record | Minimal slices | Compile the examples; `layout` at 12/16/20 | Each property kind is `works` or `fails` with a compile-checked example; number-prop and derive result stated; failures become new gap entries | macOS | note, compile log |
| Class map | Oracle at 12, 16, 20 | Probe on all target screens | `font-size-map.json` lists every Contract file and `style` with `rem`/`px`/`pending-area` and evidence; no unclassified entry; shared styles are `converted` | macOS | map, probe output |
| Pixel pairs | Round-11 matrix scenes that use shared styles | Screenshots at 12, 16 and 20 next to `target/t3-ui-parity/electron-oracle.mjs` shots | Shared-style regions match the oracle at 1280×840 and 840×620, light and dark; unconverted regions are listed as `pending-area`, not as a mismatch | macOS | pairs |
| No change at 16 | Round-11 matrix | Re-shoot after the conversion | Scores within 0.3 of the pre-ticket run | macOS | matrix table |
| px items stay | Scene with prompt text, code block, diff, terminal | Set 20 | Prompt equals the prompt setting, code equals the code setting, the terminal cell size is unchanged, the top bar stays 52 pt, borders stay 1 pt | macOS | `layout`, shots |
| Minimum window | 840×620 at 20 | Open the screens that use shared styles | No overlap or clipped control that the oracle does not also show | macOS | shots, `layout` |
| Persistence | Set 20 | Relaunch (not the agent clock) | 20 applies at the first frame after the preferences load; `t3-code.json` holds 20 | macOS | `state`, file excerpt |
| Follow-up input | Map complete | Review | The map is detailed enough to write the per-area tickets: every `pending-area` entry names its Contract file and area | — | map |
| File cap | — | `bun scripts/caps.mjs` after `git add -A` | Pass | — | log |
| Slider feel `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch | Drag the slider across the range | The converted regions follow without flicker or layout jump, as in the oracle | macOS | notes, video |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `*.contract` files that define `style`, `settings-appearance-look.*`, `presentation.ts`, `font-size-map.json`, the probe and
proposal scripts under `target/t3-ui-parity/`, `AGENT-HANDOFF.md`, `EXACT2-GAPS.md` (X3 moves to "fixed"), the host command wiring in the app module.
Required environment: a `main` pin with the X3 fix, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Implemented 2026-10-07 on `feat(example)/t3-code-interface-font-size` together with
[20261005-interface-font-size-conversion](20261005-interface-font-size-conversion.md) (one PR, as the user asked),
after the branch merged exact2 main `cff90b364` (X3 fixed upstream by #185; #176 and #159 also on main).

- **Root size.** `appearance-fonts.ts` ports `clampInterfaceFontSize`, `clampPromptFontSize`, `clampCodeFontSize`
  (and the ranges and defaults). `look.fontSize` is now `clampInterfaceFontSize(fontSizeInterface)` (16 until the
  preferences load) and is the source of the call: app.contract's `task rootFont key=data.look.fontSize` runs
  `setRootFontSize(data.look.fontSize)` one millisecond after launch and after every change.
  `r12-sidebar-width.ts` uses the shared clamp.
- **Feasibility record** (`evidence/20261007-interface-font-size/rem-probe.contract`, `rem-probe-layout.txt`, a scratch
  app driven on macOS at 12/16/20): `rem` compiles and lays out scaled for font-size, line-height, letter-spacing,
  width/height and min/max, gap and column-gap, padding, margin, top/left, border-radius and an svg's width/height
  (`1rem` → 12/16/20). A `number` prop carries rem through a template (`` width=`${size / 16}rem` `` → 10.5/14/17.5),
  and a ternary of rem strings works. Fails: `translate` takes px or percent only, and `calc()` takes
  `<percent> ± <px>` only (`calc(1rem + 4px)` is refused), so a length that adds a layout px value to a rem one
  multiplies the root size in (the top bars' traffic-light inset). Neither needs a new issue for this app.
- **Map.** `font-size-map.json`: every Contract file with its rem lengths (the px values at 16 that became N/16 rem;
  ×0.75 at 12, ×1.25 at 20), every `style` definition (60, all `converted`), the px items with their reason, the JS
  layout converted, and what stays px with the reason (`pxKept`). The oracle probe at 12/16/20 was not run: the desktop oracle task is not
  to be done (user decision 2026-10-06); the classes come from the reference source (Tailwind rem utilities, its 86
  `[Npx]` classes and `index.css` px values).
- **Shared styles.** All 60 `style` definitions are converted (with every other literal length; see the conversion
  task).
- **Tests.** `appearance-fonts.test.ts`: the reference's `describe("font size clamping")` (2 tests) plus a stored
  non-number, and the clone's JS-layout scaling (chat width, lane metrics, composer menus).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | branch over `4f523ef5c` + main `cff90b364`, commit `bf35a2d49` | `bun test` 2259/0/1 skip; strict `tsc` clean; `contract build` OK; `cargo test -p t3-code-macos --lib` 11/0; caps OK; live macOS drive at 12/16/20 | `evidence/20261007-interface-font-size/` | the conversion's remaining areas |
| 2 (2026-10-07) | same branch, round 2 (the conversion finished) | `bun test` 2260 pass / 0 fail / 1 skip; strict `tsc` clean; `contract build` OK; `cargo test -p t3-code-macos --lib` 11/0; AppKit `menus` 44/0, `ssh` 15 (1 skip)/0, `r6-media` 5/0, `media-actions` 7/0, `r7-device` 13/0, `r8-keys` 4/0, `composer` 46/0; caps OK; live drives at 1280×840 and 840×620 at 12/16/20 with `layout … native`: every probed box ×0.75/×1/×1.25, the prompt text and the 52 px bars stay | `evidence/20261007-interface-font-size/round2/` | none in code; the rows below are unverified |

Rows not run, with the exact reason:
- **Persistence across a real relaunch (launch, set 20, quit, relaunch).** An agent session cannot show it: under the
  agent the transport keeps no preferences (`T3Module.swift`: `T3Transport(persistent: !context.agent, …)`), so a second
  launch on the same named store came back at 16 (`round2/persist-record.txt`). A person-style launch needs real input, and
  `orca computer` could not read or click any app on this Mac today (every call, Finder included: "no accessibility
  window … macOS Accessibility may need Orca Computer Use toggled off and on again in System Settings"; permissions report
  granted), which only a person can repair. A seeded normal launch (`t3-code.json` with `fontSizeInterface: 20` under a lane
  HOME) did not read the saved preferences in the unpaired state either (a seeded `sidebarWidth` did not apply), so it proves
  nothing about this change. Covered instead by `client.test.ts` "a saved Interface font size is the root font size after the
  preferences load" (saved 20 → `look.fontSize` 20 → `devicePresentation.rootFontSize` 20) and the `rootFont` task, which runs
  on every key, the first included.
- **Attended size change with real input.** The same accessibility block. (The reference's control is a Select, not a
  slider: `SettingsPanels.tsx` FontFamilySettingsRow `<Select>` 12–20; the agent drove it at 12, 16 and 20.)
- **Oracle pixel pairs and the round-11 matrix.** No desktop oracle (user decision 2026-10-06). The workspace card at 20 was
  checked against the reference code instead (below); no screenshot of T3 Code (Nightly) at 20 was taken, because that would
  change its setting.
- Palette and Usage changes made after the drive (palette list caps 26.25/28/34rem, the usage grid gap) are compile- and
  test-checked only.

### Evidence (before/after)

Before is the untouched feature-branch tip `4f523ef5c` (`t3-code-evidence-base`), after is this branch at the final
drive (round 2); the same drive (`evidence/20261007-interface-font-size/round2/drive2.mjs.txt`), light, an isolated lane
server (port 16521, lane homes) with project Alpha and two threads. One image per scenario:

- Settings › Appearance at 16 (default), 1280×840, unchanged: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/01-settings-appearance-16-before-after.png
- Settings › Appearance at 12: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/02-settings-appearance-12-before-after.png
- Settings › Appearance at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/03-settings-appearance-20-before-after.png
- A thread with the sidebar and composer at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/04-thread-20-before-after.png
- A thread with the sidebar and composer at 12: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/05-thread-12-before-after.png
- Settings › Keybindings with the native key recorder at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/06-keybindings-recorder-20-before-after.png
- A thread at 20 in the 840×620 window: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/07-thread-20-840-before-after.png
- Keybindings and the key recorder at 20 in the 840×620 window: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/08-keybindings-recorder-20-840-before-after.png

`layout` per size (`evidence/20261007-interface-font-size/round2/{before,after}-{1280,840}-record.txt`; boxes w×h):

| | before (12 / 16 / 20 alike) | after at 12 | after at 16 | after at 20 |
|---|---|---|---|---|
| `state` root font size | 16 | 12 | 16 | 20 |
| sidebar thread row | 239×78 | 243×58.5 | 239×78 | 235×97.5 |
| sidebar toggle / settings button | 28×28 / 32×32 | 21×21 / 24×24 | 28×28 / 32×32 | 35×35 / 40×40 |
| composer toolbar | 678×48 | 550×36 | 678×48 | 918×60 |
| composer text (`layout composer native`) | 14 px, line 22.75 px | 14 px, 22.75 px | 14 px, 22.75 px | 14 px, 22.75 px |
| native key recorder (`layout keybinding-recorder native`) | 156×24 | 116.5×18 | — | 195.5×30 (840×620: 195.5×30) |
| chat / settings header (52 px, px) | 52 | 52 | 52 | 52 |

At 16 every probed box equals the before build's. No `setRootFontSize … refused` line in any log.

## Real input (2026-10-07, real-input-checks)

The two rows above that waited for a real hand ran in [20261007-real-input-checks](../20261007-real-input-checks.md)
on a normally launched lane bundle copy (own bundle id, paired with a lane server): Settings › Appearance ›
Interface font size set to 20 with real clicks on the dropdown, the UI scaled at once; a real held ⌘Q (the
clone's default quit mode), a relaunch of the same copy: `t3-code.json` holds `fontSizeInterface: 20` and
the UI comes back at 20. On the base (`4f523ef5c`) the same steps save 20 and change only the preview line.
Seen once on this branch: the relaunch showed "Environment disconnected" for about 30 s and the 20 px layout
arrived with the connection (the snapshot answer waits for the transport's `status`); the base reconnected
within 8 s. Image: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/real-input-checks/01-interface-font-size-relaunch.png

## Next action

PR #206 review and merge.
