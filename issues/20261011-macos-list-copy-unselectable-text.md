# macOS: a copy across a virtualized list's rows includes `user-select: none` text

**Status:** Open
**Systems:** runner lists, host/apple macOS, text selection
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-11
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261011-x81-macos-list-copy-unselectable-text.md; LLP 1010 §6 (a list selection's logical endpoints); main PR #327

## Summary

On the web (the JS target, the build that ships), a copy leaves out `user-select: none` text, and that includes a
virtualized list's rows. Those rows are DOM, and `host/web-js/list.js` adds no copy handler, so Chrome serializes the
selection and skips that text.

On macOS, the runner copies a selection inside a `list virtualized=true`, not the host. ⌘C reaches `TextSelection.copy()`
(`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift:262-278`). That asks `presenter.onListText`
(`Session.swift:749` → `Bridge.swift:502` → `exact_list_text` → `host/apple/src/abi_collections.rs:98` →
`Runner::list_text`). The runner projects a row's text with `collect` (`runner/src/instance/text.rs:73-117`). There,
every Text node is a paragraph unless it is `display: none`, and `user-select` is never read. `Tree::list_text`
(`:126-190`) copies every paragraph between the two endpoints, so the unselectable texts between them are copied too.

The host and the runner also number paragraphs differently. A list endpoint is a row key, a paragraph ordinal and a
UTF-16 offset (LLP 1010 §6). The host numbers a row's paragraphs over its selectable texts only (`paragraphs`,
`:58-74`, filtered by `textSelectable`, `:401-420`; `position`, `:313-339`). The runner numbers them over every text.
So when an unselectable text comes before a selectable one in a row, an endpoint names the wrong paragraph. Take rows
that put the line number first, and drag from row 1's code to the end of row 3's code. The copy holds `L1`, row 1,
`L2`, row 2 and `L3`, and row 3's code is missing. The painted selection is right, because it uses the host's own
ordinals; only the copy is wrong. ⌘A then ⌘C in a list copies every text of every row.

Plain rows, outside a virtualized list, are copied by the host from its selectable paragraphs (`selectedText`). They
leave the label out, as the web does. So the gap is in the list path only.

The wasm web target (`--wasm`) copies a list selection through the same runner call
(`host/web/list-selection.js:110-116`), and it includes the labels too. Its paragraphs count every `[data-exact-text]`
(`:25-30`), as the runner's do, so its endpoints land right and only the labels between them come in.

## Why this arose

T3 Code's diff (@pierre/diffs) marks its line-number column `user-select: none` (`[data-column-number]`), so a copy
across diff lines holds only the code. The clone's diff rows are a virtualized list. T3 PR #417 (task
diff-gutter-selection-followups GS-1, open) made the gutter's number a text with `user-select="none"` (`DiffNumber`).
It placed the number after the code in each row, so the endpoints still land on the code. Even so, a copy across the
diff's rows on macOS still includes the line numbers between the endpoints. Before #417 the number was a selectable
paragraph. The clone has no workaround.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `c001a862bf41f371999e3c79a1964993ea0f6fe3`. Its
`app.ts` exports an empty `sources`. `contract build` compiles it with no warning. There are three groups of five
rows. Each row holds a label `L<n>` with `user-select="none"` and a code text. V and W are `list virtualized=true`:
V puts the label first, and W puts it after the code, as #417 does. P is V's rows in a plain `column`.

```text
component X81app
  view
    main testId="root" width="100%" height="100%" padding=20 box-sizing="border-box" background-color="#ffffff" color="#111111" display="flex" flex-direction="column" gap=6
      text "V: list virtualized=true, number first" font-size=12 color="#666666" user-select="none"
      list virtualized=true testId="vlist" height=140 width=360 estimated-item-height=28 background-color="#f3f4f6"
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`v${n}` height=28 align-items="center" gap=12 padding-left=8
            text `L${n}` user-select="none" width=24 color="#888888" font-size=14
            text `vcode ${n} body` font-size=14
      text "W: list virtualized=true, number after the code" font-size=12 color="#666666" user-select="none"
      list virtualized=true testId="wlist" height=140 width=360 estimated-item-height=28 background-color="#f3f4f6"
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`w${n}` height=28 align-items="center" gap=12 padding-left=8
            text `wcode ${n} body` font-size=14
            text `L${n}` user-select="none" width=24 color="#888888" font-size=14
      text "P: plain rows (no list), number first" font-size=12 color="#666666" user-select="none"
      column testId="plist" width=360 background-color="#f3f4f6"
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`p${n}` height=28 align-items="center" gap=12 padding-left=8
            text `L${n}` user-select="none" width=24 color="#888888" font-size=14
            text `pcode ${n} body` font-size=14
```

Every scenario drags the mouse from the start of row 1's code to the end of row 3's code, then copies. On macOS it is
`bun exact.mjs agent macos --size 520x620 "tap v1 drag 86 56 from 45 14" "type root key Meta+c"` (W: `tap w1 drag 89
56 from 9 14`; P: `tap p1 drag 87 56 from 45 14`), then `pbpaste`. The agent's ⌘C goes through the responder chain,
as the Edit menu sends it. The pasteboard held no item before the drives and was left empty after them. On the web,
Playwright's mouse drags in Chrome over the JS build. Then `document.execCommand('copy')` and the Meta+C chord each
copy, and `navigator.clipboard.readText()` reads the result back. In the table, `\n` is a newline.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| V: rows 1–3, the label first | macOS 26.6.2 (25G83), Apple Silicon; the agent's drag and ⌘C | main `c001a862b` | `L1\n\nvcode 1 body\n\nL2\n\nvcode 2 body\n\nL3`: every label, and row 3's code is missing. Painted: the three codes only | the three codes and no label, as on the web | image (right), record |
| W: the label after the code | macOS | main `c001a862b` | `wcode 1 body\n\nL1\n\nwcode 2 body\n\nL2\n\nwcode 3 body` | the three codes | record |
| P: plain rows | macOS | main `c001a862b` | `pcode 1 body\n\npcode 2 body\n\npcode 3 body` | (no label, as on the web) | record |
| ⌘A then ⌘C in V | macOS | main `c001a862b` | all five rows with every label, `L1\n\nvcode 1 body\n\nL2 …` | the five codes | record |
| V, W, P | Exact web (JS target) in Chrome 155.0.8059.39 | main `c001a862b` | the code only in each, e.g. `vcode 1 body\nvcode 2 body\nvcode 3 body` | (the reference) | image (left), record |
| V, W, P | Exact web, wasm target (`--wasm`), Chrome 155 | main `c001a862b` | V `vcode 1 body\n\nL2\n\nvcode 2 body\n\nL3\n\nvcode 3 body`; W as on macOS; P the code only | as the JS target | record |

Each macOS scenario reproduced on its first drive.

![X81: a copy across a virtualized list's rows, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/ef0069c611f4bc0837fbb439e8dc57d3bf4d17d2/fw-issues-20261011k/x81-list-copy-user-select-web-macos.png)

Record (the contract, the commands, every drive's clipboard, and the code read):
https://raw.githubusercontent.com/ccheever/exact2/ef0069c611f4bc0837fbb439e8dc57d3bf4d17d2/fw-issues-20261011k/x81-record.txt
(sources at the same commit: `x81-app.contract.txt`, `x81-chrome-copy-probe.mjs.txt`; screenshots of each
selection: `x81-macos-*-selected.png`, `x81-web-js-*-selected.png`, `x81-web-wasm-v-selected.png`).

Not run: iOS (it has no list-text copy), Linux, Windows, Firefox, WebKit, and a hardware mouse. Also not run: a
copy across rows that are not mounted (a long list, scrolled), ⌘A on the web, and a row with a button's label before
its text. AppKit's rule makes such a label unselectable on the Mac, while the runner counts it; that case is read from
code only.

## Constraints

- The web is the parity oracle (`CLAUDE.md`). On the JS target, `user-select: none` text is not copied, inside a
  virtualized list or outside one. Under LLP 1115 the author wins over the platform and the CSS default, and here the
  author wrote `user-select="none"`. The guide says it "prevents ordinary text selection" (`docs/contract-for-agents.md`),
  and on macOS the host already keeps that text out of the paint and out of a plain copy.
- A list copy holds what the host paints selected. The host and the runner count the same paragraphs, so an endpoint
  names the same text on both sides.
- On macOS, "selectable" also covers AppKit's `auto` rule (`textSelectable`: a control's label, window chrome). The
  runner cannot see that rule from `user-select` alone. How the two agree is open. The runner could apply the host's
  rule, the host could tell the runner which paragraphs count, or the host could number every text and let the runner
  skip the unselectable ones.
- A copy still does not mount the selected range (LLP 1010 §6). For a row that is not mounted, which texts are
  unselectable has to come from the plan (the authored `user-select` on the text or an ancestor), not from views.
- The wasm web target shares `list_text` and should follow. The JS target needs no change.
- The Mac host's copy of plain rows stays as it is.
- The paragraph separator is a separate question, outside this issue. Both Mac paths and the runner use a blank line
  (`\n\n`), and Chrome puts one newline between rows.
- No open issue covers what a list copy holds. `issues/20261009-text-selection-rectangles-commands.md` is about the
  selection's geometry and setting it, and `issues/20261010-macos-right-click-selects-no-word.md` is about a
  right-click.
- Main PR #327 (head `14493d253`) does not fix it (read from its diff, not run). It changes none of `runner/`,
  `TextSelectionMac.swift`, `Bridge.swift`, `abi_collections.rs` or `list-selection.js`, and its `Session.swift`
  hunks leave `onListText` alone.

## Acceptance criteria

- In the repro on macOS, V's drag copies `vcode 1 body`, `vcode 2 body` and `vcode 3 body` with no label, and W's
  drag copies the three `wcode` lines the same way. ⌘A then ⌘C in V copies the five codes with no label.
- A drag that ends inside row 3's code copies row 3's code up to the pointer, as it is painted.
- P's copy is unchanged.
- The wasm web target's V and W copies give the JS target's text, with no labels.
- A runner test gives `list_text` rows that hold a `user-select="none"` text, with endpoints as the host gives them,
  and gets no unselectable text back. A macOS test copies across list rows that put a label before their text.
- Partial copy, full copy and a copy across rows that are not mounted keep working (`host/web/tests/it/lists.rs`, the
  runner's collection tests).
