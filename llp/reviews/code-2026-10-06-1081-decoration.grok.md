# Code review: LLP 1081 stage 1b, text-decoration-line (grok)

**Reviewer:** Grok 4.7 (xhigh, plan mode), 2026-10-06, commit 5ff665c0f over 463acda68, 58 turns, `stopReason: end_turn`, brief sha256 `385f065c8d870623ca659827fab0e3070aa90bf774923b6ecf75baa4614a697a` (the first brief's run cancelled at its first command; this one adds the one-command rule). Mutually blind to Astra. Transcription: the joined text events; the first two sentences are progress narration.

**Disposition (author, after the review):** findings 1–3 taken as in the Astra file (casing, the explicit hint, computed and reversed coverage). The schema-digest snapshot this review did not mention failed in the checks and is updated.

---

I'll audit commit `5ff665c0f` against stage 1b of the names RFC, starting with the commit itself and the sections that define the change.The diff is small. Next I’ll trace every reader and writer of this enum, including paths the commit does not touch.**LAND.** Stage 1b is in the right place: the schema string, a real `from_css` on the dynamic parse arm, and the web host writing that string with the old special case gone. A resolved `UnderlineLineThrough` still paints the same on every native host. The old token is refused, not aliased.

## What the commit does

`TextDecorationLine`’s last value in `kernel/tables/schema.json:1702` is now `underline line-through`. `pascal()` already treats a space as a word break (`kernel/build.rs:86`), so the variant stays `UnderlineLineThrough` and its wire byte stays 3 (`kernel/build.rs:509-513`, encode `kernel/build/codec.rs:312`). `name()` is the schema string (`kernel/build.rs:548-551`). Generated code is not in the tree and was not edited.

`StyleProps::set_dynamic` calls `TextDecorationLine::from_css` (`kernel/build.rs:1296-1298`). `emit_grid_seam` still `continue`s only for the grid rows (`kernel/build/codec.rs:17-37`), so this arm actually runs for decoration. `from_css` (`kernel/src/style.rs:1474-1481`) splits on ASCII whitespace, sorts, and accepts `line-through` + `underline` in either order; one word goes to `from_name`. `underline-line-through` is `None`.

## 1. Readers and writers

| Path | What it does with the text | After this commit |
|---|---|---|
| Contract literals, including a `style` class | `set_dynamic` (`contract/lower/src/values.rs:726`, classes via `contract/lower/src/class.rs:14-80`) | Either order compiles. The old token is `lower-attr-value`, and the message lists `"underline line-through"` (`values.rs:128-130`). |
| `text-decoration` shorthand | Flag scan, then one canonical string (`contract/lower/src/shorthands.rs:273-287`) | Writes `underline line-through`. Order and case do not matter. The hyphenated token was already one unknown word (`shorthands.rs:279`). |
| Runner, native and wasm | `set_style` → `set_dynamic` (`runner/src/bridge.rs:147-148`). A refusal clears the row (`runner/src/instance.rs:914-926`). | Same parser as a literal. |
| Wasm web CSS | `text-decoration-line:` plus the enum string (`host/web/src/css.rs:327-331`, `606`). The `underline-line-through` arm is gone. | Emits `underline line-through`. |
| JS target, static | Literals go through the same bridge, then `css_text` (`host/web-js/src/style.rs:158-160`). | Canonical CSS. |
| JS target, dynamic | No mapper. The author string is `setProperty` (`host/web-js/src/style.rs:566-569`, `host/web-js/rt.js:813-830`). An invalid value is removed first, then rejected, so the property ends unset. | Either order is valid CSS, so the browser draws both lines. The old token unsets. |
| Apple content-region wire | Match on the variant (`host/apple/src/content_region/wire.rs:91-96`) | Already hardcoded `underline line-through`. |
| Apple batch JSON | `RowValue::Enum` is the enum’s `name()` (`host/apple/src/style.rs:298-302`) | The string changes from `underline-line-through` to `underline line-through`. |
| Apple paint | `contains("underline")` and `contains("line-through")` (`host/apple/Sources/ExactKit/Text.swift:789-790`, `RegionTextSource.swift:75-76`) | Both the old and new strings set both styles. |
| Linux | `host/linux/src/paint/text_decoration.rs:32-39` matches the variant and paints both lines. `Run::from_style` (`host/linux/src/text.rs:113-126`) still does not copy a decoration. | Variant match, so a resolved pair paints as before. LLP 1081’s “Linux draws none” is stale. |
| Terminal | Variant match (`host/terminal/src/paint.rs:355-359`) | Same cells for a resolved pair. |
| Windows | No reader of this enum. | — |
| Wire | `u8` discriminant (`kernel/build/codec.rs:283-284`, `312`) | Unchanged. |
| Agent | Quotes `name()` (`runner/src/agent.rs:609-611`, `runner/src/agent/row.rs:109`) | Reports `underline line-through`. The field name stays `text_decoration_line`. |
| Motion / keyframes | `text-decoration-line` is not a `Property` (`motion/src/property.rs`). | No parse path. |
| `StyleId::enum_from_name` | Still `from_name`, exact (`kernel/build.rs:669-671`) | Reversed order is `None`. Callers are tests only (`kernel/tests/it/style_dynamic.rs:410-418`), and those pass the canonical name, which `from_name` still accepts. |

Nothing still emits `underline-line-through`. Native painters of an already-resolved `UnderlineLineThrough` do not change.

One native runtime result does change, and it is the point of the patch. `apps/harness/terminal.contract:38` already computes `"underline line-through"`. That string used to miss `from_name` and clear the row. It now parses and the terminal draws both lines.

## 2. Computed values

On the native path and on the wasm web host, a bound string and a literal both go through `from_css`. Either order becomes `UnderlineLineThrough` and the web host prints the canonical spelling. The old token unsets the row.

On the JS target a literal is canonicalized the same way. A bound value is not: the browser sees the author string. `underline line-through` and `line-through underline` both paint. `underline-line-through` unsets, because `rt.js:828-830` removes the property before `setProperty` and the browser rejects the token. For the spellings stage 1b names, the pixels match.

## 3. Tests and leftover spellings

The new tests cover the parser and the literal longhand through the runner: either order, extra whitespace, `name()`, and refusal of the old token, `overline`, `none underline`, and duplicates (`kernel/src/style/decoration_tests.rs:4-32`, `contract/cli/tests/it/styles.rs:1414-1424`). The existing shorthand test (`styles.rs:1337-1373`) boots a computed `text-decoration=(done ? "underline line-through" : "none")` and expects the pair, so the shorthand’s new string is locked in.

`underline-line-through` remains only as a refused sample in those two tests, and in LLP 1081 and its reviews, which describe the old spelling. No `.contract`, host, app, or script still authors it.

## Findings

1. **Low.** `from_css` is case-sensitive (`kernel/src/style.rs:1475-1479`). CSS keywords are not. The shorthand lowercases (`shorthands.rs:274`); the longhand does not. A literal `UNDERLINE LINE-THROUGH` is refused on every host. A computed one is cleared on native and drawn on the JS target. Fix: ASCII-lowercase the words inside `from_css` before the sort, and add that case next to the existing refusal list.

2. **Low.** Stage 1b’s last bullet, the D2 pre-pass hint, is not in this commit. There is no old-token pre-pass in `contract/lower`. Authors see the generic list `expected one of "none", "underline", "line-through", "underline line-through"` (`values.rs:128-130`). The new test only checks that the message contains `underline line-through` (`styles.rs:1423-1424`), which that list satisfies. Stage 1 owns the pre-pass; this commit already refuses the token with no alias.

3. **Low.** Nothing in the commit checks the CSS the web hosts emit, a computed longhand in reversed order, or the shorthand in reversed order. Reversed literals do go through the runner (`styles.rs:1415-1421`). `name()` is asserted (`decoration_tests.rs:23`). The Chrome draw and the native screenshot gate in LLP 1081 §4 are not in the commit.

## Not checked

I did not compile, run tests, or drive Chrome or a native host. The command limits were git, rg, and wc. The generated parser is what `build.rs` writes at compile time; I read that generator, not its output. `contract-difftest` was not run. File caps are fine: `kernel/src/style.rs` is 1486 lines, `kernel/build.rs` is 1492.