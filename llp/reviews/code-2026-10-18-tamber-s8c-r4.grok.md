# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r4, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `182f4d61c..7451b70bd` (C1 round 4, C2 round 3).
- **Method:** one brief (sha256 `5432ee8fd9cfc972d8bd04db0b0b43b6833fe05aea79cbd1ab0a55c0897288b5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 0357e064a (C1), 956e143cf and 7a88718ef (C2), folded.

---

## BLOCKING

**A timed `progress` (`start`/`end`, no `value`) is judged as the bar, so the spinner's `none` cells stay silent.** `contract/lower/src/control_roles.rs:256` treats `written("start")` as `progress`. The table's bar row is `exact` on iOS, Linux, Windows, and Android (`kernel/tables/schema.json:5920`); the spinner row is `none` on those hosts (`schema.json:5921`). The painters draw a spinner unless `value` is set:

- `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:25-26` — bar only when `props["value"] != nil`; the spinner uses `color`, not accent (`ProgressIOS.swift:44-47`).
- `host/apple/Sources/ExactKit/Mac/ProgressMac.swift:35-36` — same split. Both Mac cells are `none` with the same why, so Mac still warns.
- `host/linux/src/paint.rs:1162-1166` — bar only when `PropId::Value` is set; otherwise `control::progress`, which paints `text_color` (`host/linux/src/paint/control.rs:478`). Windows is that painter, marked inferred. The Android reader is this painter too (`apps/tamber-host/android/src/lib.rs` `canvas_jni!`; `host/linux/src/paint/look.rs:10-13` says the reader does not take the neutral look).

The web is the exception that must stay a bar: `kernel/src/control.rs:56-58` makes `start` a `ProgressBar`, `host/web/src/element.rs:349-351` emits `<progress>`, and `host/web-js/live-time.js:81-83` writes `e.value`. RFC C1 defines `progress` as with a value and `spinner` as `progress` without (`llp/1121-tamber.rfc.md:3487`). `contract/cli/tests/it/control_roles.rs:176-178` locks the iOS miss (`progress start=0 end=1000` expects no warnings).

## SHOULD-FIX

**`written()` ignores the node's own row.** `control_roles.rs:228-232` is true if any class or node attr of that name is not `false`. `values()` (`:216`) uses `rfind`, so the node's `buttonStyle`, `appearance`, and `type` win; `disabled`, `value`, and `start` do not. A class `disabled` plus the node's `disabled=false` selects `button-prominent:disabled`, which is `none` on iOS, the web, Linux, and Windows (`schema.json:5924`) for a button that is enabled. A class `value` plus the node's `value=none` selects the bar for a control the painters draw as a spinner.

**A class row counts as "always written" even when it is `none`.** `control_roles.rs:170-176` puts every class attr name into `sure`. `clears()` (`:179-183`) filters only the node's own names, so a class `color=none`, or a node `color=none` over a class `color`, still exempts `FieldText` (`:73-76`, `:122`). On the web, Linux, and Windows that role is `none` (`schema.json:5926`) and the warning is dropped. The comment at `control_roles.rs:150-153` says a sure row is one no branch clears with `none`.

**A conditional `type` uses only the last branch.** `str_attr` (`control_roles.rs:223`) keeps `values().last()`. The comment at `:211` says a conditional counts any branch, and `buttonStyle` does (`:260-261`). `type=(flag ? "range" : "text")` is only a field, so a macOS range (`none`: `NSSlider`, `schema.json:5919`) is not warned when that branch is taken.

**`role="switch"` is not a switch.** `control_roles.rs:239-244` requires the positional word. `role` lowers to `accessibilityRole` (`contract/lower/src/tags.rs:597`), and both `host/apple/Sources/ExactKit/ControlKinds.swift:24` and `host/web/src/element.rs:1146` draw that as a switch. `contract/lower/src/grouped/inputs.rs:29-36` already accepts both spellings. On macOS the switch cell is `none` and the checkbox cell is `best-effort`, so `input type="checkbox" role="switch"` is silent.

**`accent-color="auto"` only wins when the whole value is that string.** `control_roles.rs:66-68`. In `class=(flag ? Brand : Auto)` with `Auto`'s `accent-color="auto"`, `rfind` hits `auto` and drops the override, including while the `Brand` branch is active. A ternary that is not exactly `"auto"` or exactly `none` (`accent-color=(flag ? "auto" : none)`) is always an override.

## NIT

- `contract/cli/tests/it/control_roles.rs:92` accepts a prefix `FieldText on the web's fields: none: `, which the message never has (`control_roles.rs:137-141` puts the cell's why straight after the colon).
- `contract/lower/src/warnings.rs:165` pushes `tamber-unreachable`, then `:194` truncates to `MAX_WARNINGS` (20). Earlier warnings can drop it.

Round 1 and 2 hold where checked: `exact` has no why (`kernel/build/validate.rs:261-266`); the web spinner `none` is in the table and in `QUEUE.md:3`; Mac disabled checkbox and prominent-button cells are `best-effort` and match `contentTintColor` / `bezelColor`; Windows copies the Linux row; terminal, tvOS, and watchOS are not judged; a literal `accent-color="auto"`, `appearance="none"` on any branch, and node-over-class for `buttonStyle` / `appearance` behave as the tests say. Theme2's 08:42 hue rule and the animation mask were not re-reviewed.
