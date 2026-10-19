# The control-role table, measured (LLP 1121 D9, stage 8 C)

Each cell of `kernel/tables/schema.json`'s `controls`, with where its fidelity comes from.
`scripts/measure-controls.test.mjs` holds the table to this page: a cell that changes changes here, with its measurement.

**How:** `bun scripts/measure-controls.mjs <host>` builds `scripts/fixtures/control-roles` twice, as written
(an authored `accent-color` #e0218a, the design's `AccentColor` the same colour, which alone reaches the spinner, and `Field` #fff1a8, `FieldText` #0b8a3a, `placeholder` #d9480f) and with
neither, and compares each control's pixels, closed and at rest, the two builds pixel for pixel, leaving out
any pixel either build moved between two looks with the app's clock held (an animation the clock does not drive):

- **exact**: at least 4 pixels within ΔE00 3 of the authored colour where the plain build's are not;
- **best-effort** (theme2's ruling): none so close, but at least 4 pixels further than ΔE00 3 from the
  authored colour yet nearer it than the plain build's pixel there: moved by more than ΔE00 1, of the
  authored colour's hue (within 30°, chroma 10 or more), and not another colour the fixture asks for blended
  over the pixel or the control's ground (theme2's 08:42 ruling adds the hue; an authored grey, chroma under
  10, is judged by ΔE00 alone);
- **none**: otherwise;
- **unmeasured**: a frame with fewer than 16 pixels, frames that differ between the builds, or a frame the
  screenshot does not hold whole (the run fails).

An open menu (the reader fills a select's chosen row with the accent) and a focus ring are not judged:
the table's `select` is the closed button, as a person first sees it.

**Where (2026-10-18):**

- `ios`: iOS 27.0 simulator (iPhone 17 Pro, 24A434).
- `macos`: macOS 27.0, the agent's capture in a key window (`EXACT_AGENT_ACTIVATE=1`, which `measure-controls` sets for this run only).
- `web`: Chrome 155.0.8059.84 on macOS 27.0.
- `linux`: the Linux host's painter (caltrain-linux, host-dev), headless on macOS 27.0.
- `windows`: not measured.
- `terminal`: not measured.
- `android`: the Canvas reader (`agent.mjs android --reader`, the fixture built into an APK in a scratch
  checkout: `--scratch <worktree>`) on the Android 16 emulator (sdk_gphone64_arm64, density 420).

**Provenance:**

- `measured`: the run above gave it.
- `inferred: same painter, not measured`: Windows draws with the Linux painter (`host/windows` depends on `exact-linux`); Linux's row, until a Windows box exists.
- A `none` whose why says the host does not read a role yet is Exact's gap, not the platform's: each has a QUEUE line, and fixing the host flips the cell after a re-measure.
- `none by construction`: the terminal draws no colour and no control but progress as text.
- `measured, key window via EXACT_AGENT_ACTIVATE`: the Mac's accent cells. An agent's Mac app is never activated by design (an accessory app, ordered front, so a drive takes no focus), and AppKit draws an inactive app's controls grey; `measure-controls` activates the one run (stage 9 F2, 2026-10-18).

| control | parameter | host | fidelity | ΔE00 | pixels | provenance |
| --- | --- | --- | --- | --- | --- | --- |
| switch | accent-color | ios | exact | 0 | 6552 | measured |
| switch | accent-color | macos | none | (38.4) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| switch | accent-color | web | exact | 0 | 358 | measured |
| switch | accent-color | linux | exact | 0 | 6692 | measured |
| switch | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| switch | accent-color | terminal | none | - | - | none by construction |
| switch | accent-color | android | exact | 0 | 49364 | measured |
| switch:disabled | accent-color | ios | best-effort | 20.2 | 6934 | measured |
| switch:disabled | accent-color | macos | none | (43.4) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| switch:disabled | accent-color | web | best-effort | 19.6 | 408 | measured |
| switch:disabled | accent-color | linux | best-effort | 19.1 | 6752 | measured |
| switch:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| switch:disabled | accent-color | terminal | none | - | - | none by construction |
| switch:disabled | accent-color | android | best-effort | 19.4 | 51236 | measured |
| checkbox | accent-color | ios | exact | 0 | 2180 | measured |
| checkbox | accent-color | macos | exact | 0.7 | 166 | measured, key window via EXACT_AGENT_ACTIVATE |
| checkbox | accent-color | web | exact | 0 | 276 | measured |
| checkbox | accent-color | linux | exact | 0 | 6894 | measured |
| checkbox | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| checkbox | accent-color | terminal | none | - | - | none by construction |
| checkbox | accent-color | android | exact | 0 | 27553 | measured |
| checkbox:disabled | accent-color | ios | best-effort | 25 | 2333 | measured |
| checkbox:disabled | accent-color | macos | none | (33.5) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| checkbox:disabled | accent-color | web | best-effort | 19.1 | 295 | measured |
| checkbox:disabled | accent-color | linux | best-effort | 19 | 6917 | measured |
| checkbox:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| checkbox:disabled | accent-color | terminal | none | - | - | none by construction |
| checkbox:disabled | accent-color | android | best-effort | 19.4 | 27720 | measured |
| radio | accent-color | ios | exact | 0 | 1732 | measured |
| radio | accent-color | macos | exact | 0.9 | 160 | measured, key window via EXACT_AGENT_ACTIVATE |
| radio | accent-color | web | exact | 0 | 232 | measured |
| radio | accent-color | linux | exact | 0 | 6848 | measured |
| radio | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| radio | accent-color | terminal | none | - | - | none by construction |
| radio | accent-color | android | exact | 0 | 19618 | measured |
| radio:disabled | accent-color | ios | best-effort | 25 | 1994 | measured |
| radio:disabled | accent-color | macos | none | (33.5) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| radio:disabled | accent-color | web | best-effort | 20.2 | 256 | measured |
| radio:disabled | accent-color | linux | best-effort | 19 | 6892 | measured |
| radio:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| radio:disabled | accent-color | terminal | none | - | - | none by construction |
| radio:disabled | accent-color | android | best-effort | 14.2 | 27696 | measured |
| range | accent-color | ios | exact | 0 | 10726 | measured |
| range | accent-color | macos | none | (38.6) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| range | accent-color | web | exact | 0 | 180 | measured |
| range | accent-color | linux | exact | 0 | 180 | measured |
| range | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| range | accent-color | terminal | none | - | - | none by construction |
| range | accent-color | android | exact | 0 | 6371 | measured |
| range:disabled | accent-color | ios | best-effort | 19.8 | 10748 | measured |
| range:disabled | accent-color | macos | none | (39) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| range:disabled | accent-color | web | best-effort | 20.2 | 216 | measured |
| range:disabled | accent-color | linux | best-effort | 20.2 | 212 | measured |
| range:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| range:disabled | accent-color | terminal | none | - | - | none by construction |
| range:disabled | accent-color | android | best-effort | 7.7 | 7587 | measured |
| progress | accent-color | ios | exact | 0.6 | 7927 | measured |
| progress | accent-color | macos | none | (38.8) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| progress | accent-color | web | exact | 0 | 926 | measured |
| progress | accent-color | linux | exact | 0 | 924 | measured |
| progress | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| progress | accent-color | terminal | none | - | - | none by construction |
| progress | accent-color | android | exact | 0 | 5932 | measured |
| spinner | accent-color | ios | best-effort | 4.8 | 4217 | measured |
| spinner | accent-color | macos | none | (28.1) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| spinner | accent-color | web | exact | 0.1 | 36 | measured |
| spinner | accent-color | linux | exact | 0.4 | 49 | measured |
| spinner | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| spinner | accent-color | terminal | none | - | - | none by construction |
| spinner | accent-color | android | exact | 0 | 293 | measured |
| select | accent-color | ios | exact | 0 | 1312 | measured |
| select | accent-color | macos | none | (28.1) | 0 | measured, key window via EXACT_AGENT_ACTIVATE |
| select | accent-color | web | none | 33.3 | 0 | measured |
| select | accent-color | linux | none | 33.3 | 0 | measured |
| select | accent-color | windows | none | - | - | inferred: same painter, not measured |
| select | accent-color | terminal | none | - | - | none by construction |
| select | accent-color | android | none | 31.5 | 0 | measured |
| button-prominent | accent-color | ios | exact | 0 | 101731 | measured |
| button-prominent | accent-color | macos | exact | 0.2 | 8177 | measured, key window via EXACT_AGENT_ACTIVATE |
| button-prominent | accent-color | web | exact | 0 | 12639 | measured |
| button-prominent | accent-color | linux | exact | 0 | 12698 | measured |
| button-prominent | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| button-prominent | accent-color | terminal | none | - | - | none by construction |
| button-prominent | accent-color | android | exact | 0 | 76983 | measured |
| button-prominent:disabled | accent-color | ios | none | 35 | 0 | measured |
| button-prominent:disabled | accent-color | macos | best-effort | 11.6 | 8488 | measured, key window via EXACT_AGENT_ACTIVATE |
| button-prominent:disabled | accent-color | web | none | 28.1 | 0 | measured |
| button-prominent:disabled | accent-color | linux | none | 28.1 | 0 | measured |
| button-prominent:disabled | accent-color | windows | none | - | - | inferred: same painter, not measured |
| button-prominent:disabled | accent-color | terminal | none | - | - | none by construction |
| button-prominent:disabled | accent-color | android | exact | 0 | 77238 | measured |
| field | Field | ios | exact | 0 | 102933 | measured |
| field | Field | macos | none | 18.7 | 0 | measured |
| field | Field | web | exact | 0 | 12772 | measured |
| field | Field | linux | exact | 0 | 12817 | measured |
| field | Field | windows | exact | - | - | inferred: same painter, not measured |
| field | Field | terminal | none | - | - | none by construction |
| field | Field | android | exact | 0 | 138189 | measured |
| field | FieldText | ios | exact | 0 | 1308 | measured |
| field | FieldText | macos | exact | 0 | 54 | measured |
| field | FieldText | web | exact | 0 | 111 | measured |
| field | FieldText | linux | exact | 0 | 67 | measured |
| field | FieldText | windows | exact | - | - | inferred: same painter, not measured |
| field | FieldText | terminal | none | - | - | none by construction |
| field | FieldText | android | exact | 0 | 848 | measured |
| field | placeholder | ios | exact | 0 | 2405 | measured |
| field | placeholder | macos | exact | 0 | 117 | measured |
| field | placeholder | web | exact | 0 | 195 | measured |
| field | placeholder | linux | exact | 0 | 143 | measured |
| field | placeholder | windows | exact | - | - | inferred: same painter, not measured |
| field | placeholder | terminal | none | - | - | none by construction |
| field | placeholder | android | exact | 0 | 1633 | measured |
| textarea | Field | ios | exact | 0 | 189055 | measured |
| textarea | Field | macos | exact | 0 | 12142 | measured |
| textarea | Field | web | exact | 0 | 20462 | measured |
| textarea | Field | linux | exact | 0 | 20537 | measured |
| textarea | Field | windows | exact | - | - | inferred: same painter, not measured |
| textarea | Field | terminal | none | - | - | none by construction |
| textarea | Field | android | exact | 0 | 172841 | measured |
| textarea | FieldText | ios | exact | 0 | 1308 | measured |
| textarea | FieldText | macos | exact | 0 | 53 | measured |
| textarea | FieldText | web | exact | 0 | 111 | measured |
| textarea | FieldText | linux | exact | 0 | 67 | measured |
| textarea | FieldText | windows | exact | - | - | inferred: same painter, not measured |
| textarea | FieldText | terminal | none | - | - | none by construction |
| textarea | FieldText | android | exact | 0 | 848 | measured |
| textarea | placeholder | ios | exact | 0 | 2405 | measured |
| textarea | placeholder | macos | exact | 0 | 116 | measured |
| textarea | placeholder | web | exact | 0 | 195 | measured |
| textarea | placeholder | linux | exact | 0 | 143 | measured |
| textarea | placeholder | windows | exact | - | - | inferred: same painter, not measured |
| textarea | placeholder | terminal | none | - | - | none by construction |
| textarea | placeholder | android | exact | 0 | 1633 | measured |
