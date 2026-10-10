# Text rows: `text-wrap: balance`, an authored placeholder colour, `-webkit-font-smoothing` (rest of #128)

**Status:** Open
**Systems:** kernel text, Contract, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/266

## Current scope

Add selected balance and authored ::placeholder color, pinning bounded line breaking and Contract representation and amending LLP 1104. Defaults remain platform-native when absent; font smoothing stays deferred.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #128. #208 fixed its first part (macOS line breaks under `overflow-wrap: anywhere | break-word` now follow Chrome's opportunities). Three text rows that #128 asked for are still not in Contract on any host:

- **`text-wrap: balance`** (CSS Text 4, `text-wrap-style: balance`): short blocks such as tooltips, toasts and empty states get even line lengths instead of a short last line.
- **An authored placeholder colour** (CSS `::placeholder { color }`): an app cannot make a field's placeholder follow its theme's placeholder token.
- **`-webkit-font-smoothing: antialiased | auto`**: the property Chrome and Safari honour on macOS for thinner grayscale text, used by desktop apps that offer a "thin text" appearance setting.

Each is a property or pseudo-element an author writes on the web; an app has no way to draw host text itself, so there is no app-side workaround.

### Current and expected behavior

On main `0365ad1a4` (the paths involved are unchanged on `e200397ec`):

- `text-wrap="balance"` and `text-wrap-style="balance"` on a `text`: `lower-unknown-attr` ("`text` has no attribute `text-wrap`"). `kernel/tables/schema.json` has no `text_wrap` row.
- `-webkit-font-smoothing="antialiased"` on a `text` or `input`: `lower-unknown-attr`. The only admitted `-webkit-` rows are `-webkit-text-stroke*` (LLP 1077 D7).
- `placeholder-color="red"` on an `input`: `lower-unknown-attr`; `contract vocab ::placeholder`: "`::placeholder` is not a tag or an attribute". The default placeholder colour is now each platform's own (LLP 1104, "Placeholder"), which also says "An authored `::placeholder` colour is not in Contract and stays out of scope."

Expected (Chrome): `text-wrap: balance` balances a block's lines (Chromium limits it to short blocks); `::placeholder { color }` sets the placeholder's colour on `input` and `textarea`; `-webkit-font-smoothing: antialiased` draws macOS text with grayscale antialiasing (other platforms ignore it).

Proposals, not requirements: `text-wrap` / `text-wrap-style` with `balance` as an inherited text row on every host; a placeholder colour row on `input`/`textarea` as the Contract form of `::placeholder` (for example `placeholder-color`, or a `::placeholder` selector in a class), mapped to `placeholderAttributedString` on macOS and `attributedPlaceholder` on iOS; `-webkit-font-smoothing` as an inherited row that macOS honours and other hosts accept and ignore.

### Reproduction and evidence

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Balanced wrapping | `bun scripts/exact.mjs new <dir>`; a probe file `component Probe` / `view` / `main testId="root"` / `text "a b c" width=100 text-wrap="balance" testId="p"`; `bun exact.mjs contract build <probe> --json` | compiler (macOS 26.6.2) | main `0365ad1a4` | `lower-unknown-attr`: "text has no attribute text-wrap"; `text-wrap-style="balance"` the same | accepted; the box's lines balanced as Chrome's | compiler output |
| Font smoothing | the same probe with `-webkit-font-smoothing="antialiased"` on the `text` | compiler | main `0365ad1a4` | `lower-unknown-attr`: "text has no attribute -webkit-font-smoothing" | accepted; macOS text drawn with grayscale antialiasing | compiler output |
| Placeholder colour | `input placeholder="x" placeholder-color="red" testId="p"`; `bun exact.mjs contract vocab ::placeholder` | compiler | main `0365ad1a4` | `lower-unknown-attr`: "input has no attribute placeholder-color"; vocab: "::placeholder is not a tag or an attribute" | a way to set the placeholder colour; red on macOS, iOS and the web | compiler output |

No runnable API exists for these rows, so the reproduction is the compiler's refusal; #128 has the original measurements on `4c893fef6`.

### Acceptance criteria

- `text-wrap="balance"` compiles, and a 180 px box holding "Settle every thread in this section and keep the newest one open" has Chrome's line widths on macOS and the web (a conformance case against Chrome).
- A placeholder colour set by the author draws in that colour on `input` and `textarea` on macOS, iOS and the web; without it each platform keeps its own default (LLP 1104).
- `-webkit-font-smoothing="antialiased"` compiles; on macOS a 13 px line's stroke darkness matches Chrome's with the same property at 2x; other hosts accept it and draw unchanged.

### Constraints and related work

- Related: #128 (closed by #208, which lists these three under "Open points" as needing a framework decision).
- LLP 1104 (Draft r10) puts an authored `::placeholder` colour out of scope; LLP 1077 D7 admitted `-webkit-text-stroke` because the Compat Standard specifies that prefixed name, which it does not for `-webkit-font-smoothing`.
- Not tested: iOS and Linux, and stroke weight with and without smoothing (no row exists to compare).
- Out of this issue: the generic `monospace` family mapping that #208 also lists (SF Mono on macOS, Menlo in Chrome).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:16:19Z

## Decision needed

**What blocks it (main `0365ad1a4`).**
- Placeholder colour: `llp/1104-the-platforms-controls-by-default.rfc.md:165`: "An authored `::placeholder` colour is not in Contract and stays out of scope." (the default is each platform's own, `:160-163`).
- Font smoothing: `llp/1077-css-visual-properties-native-draws-cheaply.rfc.md:127` admits `-webkit-text-stroke` because "the prefixed name is the only one CSS has, and the Compat Standard specifies it". `-webkit-font-smoothing` is not in the Compat Standard, so no rule yet admits a vendor-only name.
- Balance: nothing refuses it; `kernel/tables/schema.json` has no `text_wrap` row, and #208's open points list it with the other two as "need a framework decision".

**Options.**
- **A. All three.** `text-wrap`/`text-wrap-style: balance` (inherited text row, every host); a placeholder colour on `input`/`textarea` as Contract's form of `::placeholder` (amending LLP 1104's out-of-scope line); `-webkit-font-smoothing: antialiased | auto` (inherited, honoured on macOS, accepted and ignored elsewhere, as Chrome does off macOS).
- **B. Balance and placeholder only.** Leave `-webkit-font-smoothing` out as a non-standard name; apps keep the platform's default smoothing.
- **C. Balance only.** Keep LLP 1104's ruling: placeholders stay the platform's colour.

**Recommendation.** A. Balance is standard CSS with no decision needed. A placeholder colour is plain CSS (`::placeholder`) and the platform default stays when it is absent, so LLP 1104's native look is kept. `-webkit-font-smoothing` is what Chrome and Safari honour on macOS, and the web is the standard here.

**Cost.** Balance: a kernel text row plus each host's line breaker (CoreText has no balance mode; the shared walker can run a balancing pass over its opportunities), with a Chrome conformance case. Placeholder colour: one colour row, three host mappings (AppKit/UIKit attributed placeholders, the web's `::placeholder`). Smoothing: one inherited enum row; macOS only (CoreText/`CGContext` font smoothing), a no-op elsewhere.

### ccheever — 2026-10-08T08:07:37Z

**Decision: Choose balanced text and authored ::placeholder color; defer font smoothing.**

Keep open with the bounded scope below.

The first two are ordinary author-facing CSS needs. -webkit-font-smoothing is platform-specific and lacks enough measured benefit to justify another row now.

Split the decisions: balance needs a bounded line-breaking algorithm checked against Chrome; placeholder styling needs a chosen Contract representation and amendment of LLP 1104's out-of-scope line. Keep platform defaults when absent.
