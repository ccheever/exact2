# Review round 1, LLP 1121 stage 9 H: an empty symbol: name is no symbol, Grok (xhigh)

Reviewed: d55ef0369e.

## Findings

### SHOULD-FIX — A grouped row still gives an empty `symbol:` a box
`kernel/src/grouped.rs:212`

`apple_symbol` keeps a name when it is non-empty, the role starts with `sf/`, or it is a drawn icon. `symbol:sf/` therefore becomes `Some("")` and is consumed as the leading face (`:414`). `symbol:` (role `""`) becomes `None`, so the image stays in the row and the row is marked custom (`:443`).

The lowerer already treats both as a leading symbol: `contract/lower/src/grouped.rs:650` returns `Some("")` for both, and `:1163` applies `symbol-width` / `symbol-height`. On a custom row those sizes are carried into the cell. On the `sf/` row, `FaceSymbol.image("")` is nil (`FaceSymbol.swift:107`) and UIKit reserves no image. A grouped `image "symbol:"` plus a title measures a symbol box; `image "symbol:sf/"` does not.

### SHOULD-FIX — Snapshots still refuse `symbol:`
`contract/cli/src/snapshot.rs:297`

`local_image` accepts any non-empty name after `symbol:`, so `symbol:sf/` is local and `symbol:` is not. The snapshot pass then refuses it at `:255` (`snapshot-request`, "cannot fetch"). Lowering accepts that same source.

### SHOULD-FIX — Inspect still reports an unknown role
The session log was updated. The agent reason was not, so `symbol:` is `"role"` and `symbol:sf/` is `"empty"`:

- `host/web/glue.js:1054`
- `host/web-js/agent.js:178`
- `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:495`
- `host/apple/Sources/ExactKit/Mac/AgentMac.swift:370`
- `host/linux/src/presenter.rs:1082` (Linux, and the Windows and Android presenters that share it)

`found` is false for both (empty path, empty `symbolName`). Only the reason differs. An agent reading the node still hears a bad role for `symbol:`.

### NIT — The test comment overclaims the host
`contract/cli/tests/it/vocab.rs:1125`

"draws nothing on any host" is not what this test checks. It checks that the literal compiles and that `apple_symbol_name("")` / `symbol_path("")` are empty. Those kernel helpers were already true before this commit.

## What checks out

**1. The path the ruling names.** `symbols::check` (`contract/lower/src/symbols.rs:215`) lets an empty name through, as it already let `sf/` through. Aliases stay silent for both (`:131` returns on an empty `sf/` name; `role_of` does not alias `""`). `literal` rewrites neither. `warnings/miswiring.rs` only warns on a bare word, not a `symbol:` source. `icons::named` matches neither. `tags.rs` `symbol_ink` treats both as symbols. A native button refuses both (`contract/lower/src/controls.rs:998`; `symbol:sf/` is locked by `native_buttons.rs:211`).

`runner/src/bridge.rs` `icon_source` expands neither (an empty `sf` is excluded at `:149`). `apple_symbol_name("")` and `apple_symbol_name("sf/")` are both `""`; `symbol_path` is `None` for both. `SymbolMeasure.size` (`SymbolMeasure.swift:38`) therefore measures the same em square for a plain image of either source, and draws no glyph.

The four log sites skip both: `glue.js:379`, `symbols.js:132`, `NodeViewIOS.swift:594`, `NodeSymbolMac.swift:27`. Linux/Android paint (`host/linux/src/paint/svg.rs:171`) and the top-layer symbol (`look.rs:240`) return when `symbol_path` is `None`. The Linux image loader reports a font-size box and no refusal for every `symbol:` source, including `symbol:sf/` (`image.rs:226`). Windows is that presenter. The terminal has no symbol painter: both fail decode and fall through to the same missing-image text (`host/terminal/src/paint.rs:735`); a native button face drops the symbol and paints the title (`measure.rs:333`).

**2. The silence is what the ruling asked for.** A literal `image "symbol:"` now builds with no warning, exactly as `image "symbol:sf/"` already did. That hides a typo. The ruling chose that over skipping checks in a dead `if`, because the skip would also hide a real bad value written in the arm.

**3. The repro test does prove the build.** `an_empty_symbol_name_is_no_symbol` compiles `Row(symbol="")` and `Row(symbol="close")` under `if symbol != ""`, boots, and asserts the only image is `symbol:close`. That compile fails without the `check` change. `corpus.rs` is right: `""` is no longer a refused role, the `star.fill` stand-in in the `sf/` hint is gone, and `symbol:sf/` with an empty name stays on the admit list (`corpus.rs:64`).

Untested, and it matters: nothing asserts a live `image "symbol:"` (one the `if` did not delete) against a host log, an agent reason, a grouped row, or a snapshot. The host edits and the three divergences above sit in that gap.

**4. The QUEUE entry is accurate.** Dead arms are still checked after a constant inline. Stage 9 H only covers the empty `symbol:` case. Skipping those checks would hide a real bad value when that arm is the only copy that gets checked (every caller leaves it dead, or the condition is a constant false). Waiting on how dead arms should be reported matches the ruling. The parenthetical "`symbol:` is refused" is the pre-fix example; the next sentence says stage 9 H covers it.

After round 1: the grouped row, snapshots, the agent reasons and the test comment are fixed in round 2's sha, 963e889b00, with tests.
