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
- `macos`: macOS 27.0 (the agent's capture: an inactive window).
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
- `inferred: agent capture is an inactive window`: the agent's macOS capture is an inactive window here (no frontmost session), which draws every control grey; the cell is read from AppKit's API as the host uses it (`ControlsMac.swift`, `ButtonConfigurationMac.swift`): `none` where the control takes no tint, `best-effort` (unverified) where it takes one, until a Mac with a console session measures it (QUEUE).

| control | parameter | host | fidelity | ΔE00 | pixels | provenance |
| --- | --- | --- | --- | --- | --- | --- |
| switch | accent-color | ios | exact | 0 | 6552 | measured |
| switch | accent-color | macos | none | (38.2) | 0 | inferred: agent capture is an inactive window |
| switch | accent-color | web | exact | 0 | 358 | measured |
| switch | accent-color | linux | exact | 0 | 6692 | measured |
| switch | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| switch | accent-color | terminal | none | - | - | none by construction |
| switch | accent-color | android | exact | 0 | 49364 | measured |
| switch:disabled | accent-color | ios | best-effort | 20.2 | 6934 | measured |
| switch:disabled | accent-color | macos | none | (40.7) | 0 | inferred: agent capture is an inactive window |
| switch:disabled | accent-color | web | best-effort | 19.6 | 408 | measured |
| switch:disabled | accent-color | linux | best-effort | 19.1 | 6752 | measured |
| switch:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| switch:disabled | accent-color | terminal | none | - | - | none by construction |
| switch:disabled | accent-color | android | best-effort | 19.4 | 51236 | measured |
| checkbox | accent-color | ios | exact | 0 | 2180 | measured |
| checkbox | accent-color | macos | best-effort | (28.1) | 0 | inferred: agent capture is an inactive window |
| checkbox | accent-color | web | exact | 0 | 276 | measured |
| checkbox | accent-color | linux | exact | 0 | 6894 | measured |
| checkbox | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| checkbox | accent-color | terminal | none | - | - | none by construction |
| checkbox | accent-color | android | exact | 0 | 27553 | measured |
| checkbox:disabled | accent-color | ios | best-effort | 25 | 2333 | measured |
| checkbox:disabled | accent-color | macos | best-effort | (33.5) | 0 | inferred: agent capture is an inactive window |
| checkbox:disabled | accent-color | web | best-effort | 19.1 | 295 | measured |
| checkbox:disabled | accent-color | linux | best-effort | 19 | 6917 | measured |
| checkbox:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| checkbox:disabled | accent-color | terminal | none | - | - | none by construction |
| checkbox:disabled | accent-color | android | best-effort | 19.4 | 27720 | measured |
| radio | accent-color | ios | exact | 0 | 1732 | measured |
| radio | accent-color | macos | best-effort | (29.2) | 0 | inferred: agent capture is an inactive window |
| radio | accent-color | web | exact | 0 | 232 | measured |
| radio | accent-color | linux | exact | 0 | 6848 | measured |
| radio | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| radio | accent-color | terminal | none | - | - | none by construction |
| radio | accent-color | android | exact | 0 | 19618 | measured |
| radio:disabled | accent-color | ios | best-effort | 25 | 1994 | measured |
| radio:disabled | accent-color | macos | best-effort | (33.5) | 0 | inferred: agent capture is an inactive window |
| radio:disabled | accent-color | web | best-effort | 20.2 | 256 | measured |
| radio:disabled | accent-color | linux | best-effort | 19 | 6892 | measured |
| radio:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| radio:disabled | accent-color | terminal | none | - | - | none by construction |
| radio:disabled | accent-color | android | best-effort | 14.2 | 27696 | measured |
| range | accent-color | ios | exact | 0 | 10726 | measured |
| range | accent-color | macos | none | (35.8) | 0 | inferred: agent capture is an inactive window |
| range | accent-color | web | exact | 0 | 180 | measured |
| range | accent-color | linux | exact | 0 | 180 | measured |
| range | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| range | accent-color | terminal | none | - | - | none by construction |
| range | accent-color | android | exact | 0 | 6371 | measured |
| range:disabled | accent-color | ios | best-effort | 19.8 | 10748 | measured |
| range:disabled | accent-color | macos | none | (38.2) | 0 | inferred: agent capture is an inactive window |
| range:disabled | accent-color | web | best-effort | 20.2 | 216 | measured |
| range:disabled | accent-color | linux | best-effort | 20.2 | 212 | measured |
| range:disabled | accent-color | windows | best-effort | - | - | inferred: same painter, not measured |
| range:disabled | accent-color | terminal | none | - | - | none by construction |
| range:disabled | accent-color | android | best-effort | 7.7 | 7587 | measured |
| progress | accent-color | ios | exact | 0.6 | 7927 | measured |
| progress | accent-color | macos | none | (32.8) | 0 | inferred: agent capture is an inactive window |
| progress | accent-color | web | exact | 0 | 926 | measured |
| progress | accent-color | linux | exact | 0 | 924 | measured |
| progress | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| progress | accent-color | terminal | none | - | - | none by construction |
| progress | accent-color | android | exact | 0 | 5932 | measured |
| spinner | accent-color | ios | best-effort | 4.8 | 4217 | measured |
| spinner | accent-color | macos | none | (33.1) | 0 | inferred: agent capture is an inactive window |
| spinner | accent-color | web | exact | 0.1 | 36 | measured |
| spinner | accent-color | linux | exact | 0.4 | 49 | measured |
| spinner | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| spinner | accent-color | terminal | none | - | - | none by construction |
| spinner | accent-color | android | exact | 0 | 293 | measured |
| select | accent-color | ios | exact | 0 | 1312 | measured |
| select | accent-color | macos | none | (28.1) | 0 | inferred: agent capture is an inactive window |
| select | accent-color | web | none | 33.3 | 0 | measured |
| select | accent-color | linux | none | 33.3 | 0 | measured |
| select | accent-color | windows | none | - | - | inferred: same painter, not measured |
| select | accent-color | terminal | none | - | - | none by construction |
| select | accent-color | android | none | 31.5 | 0 | measured |
| button-prominent | accent-color | ios | exact | 0 | 101731 | measured |
| button-prominent | accent-color | macos | best-effort | (28.1) | 0 | inferred: agent capture is an inactive window |
| button-prominent | accent-color | web | exact | 0 | 12639 | measured |
| button-prominent | accent-color | linux | exact | 0 | 12698 | measured |
| button-prominent | accent-color | windows | exact | - | - | inferred: same painter, not measured |
| button-prominent | accent-color | terminal | none | - | - | none by construction |
| button-prominent | accent-color | android | exact | 0 | 76983 | measured |
| button-prominent:disabled | accent-color | ios | none | 35 | 0 | measured |
| button-prominent:disabled | accent-color | macos | best-effort | (34.6) | 0 | inferred: agent capture is an inactive window |
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
