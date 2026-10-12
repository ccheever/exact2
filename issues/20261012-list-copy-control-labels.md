# A copy across a virtualized list's rows holds a control's label the host leaves unselected

**Status:** Open
**Systems:** runner lists, host/apple macOS, host/web wasm, text selection
**Severity:** P3
**Author:** daehyeon-mun (X81 follow-up, PR #493 review)
**Date:** 2026-10-12
**Related:** issues/closed/20261011-macos-list-copy-unselectable-text.md (X81); PR #493; LLP 1010 §6; LLP 1115 D8

## Summary

X81 (PR #493) leaves a virtualized list's `user-select: none` text out of a copy. The runner reads that from the
plan (`may_select`, `runner/src/instance/text.rs:82`), so rows that are not mounted follow too. A host's own `auto`
rule can also make a text unselectable, and the runner cannot see it. Two shapes of row still go wrong on PR #493's
head (`107faa2a5`). In each, the label comes before the code.

- **A native button's label** (`button` > `text`, a `Control` of type `button`). The web hosts' stylesheet puts
  `user-select: none` on `button[data-button-style]` (`host/web/index.html:132`). So on the JS target, Chrome neither
  paints the label nor copies it. The wasm target leaves it out of the highlight (`selectable`,
  `host/web/list-selection.js:34`), but `list_text` still copies it. On macOS the label is the `NSButton`'s own title,
  and no view in the row is a paragraph for it. The host's row walk (`texts(in:)`, `TextSelectionMac.swift:72`) does not
  count it, while the runner's `collect` (`text.rs:96`) counts every Text node. So an endpoint names the wrong
  paragraph, as X81's V did before #493. A drag from row 1's code to the end of row 3's code copies `B1`, row 1, `B2`,
  row 2 and `B3`, and row 3's code is missing. #493 did not change this case.
- **A pressable's label** (`view role="button"`; by `textSelectable`'s rules, also a box with a press action, or a
  chrome label in a `header`, `nav` or `toolbar`, `TextSelectionMac.swift:475`). AppKit's rule makes it unselectable on
  macOS. So it is not painted, and the Mac's copy of plain rows leaves it out. It is a paragraph, so since #493 it counts
  toward the ordinals and the endpoints are right. But the runner copies it, so the Mac's list copy holds `R2` and `R3`,
  which it does not paint. Chrome on the JS target paints and copies this label (no stylesheet rule), and the wasm
  target does too. Only the Mac's list copy and its paint disagree.

## Reproduction

X81's scratch app with this contract (it compiles with no warning). B is a list whose rows put a native button first,
R is a list whose rows put a `role="button"` view first, and Q is B's rows in a plain `column`.

```text
component X81app
  view
    main testId="root" width="100%" height="100%" padding=20 box-sizing="border-box" display="flex" flex-direction="column" gap=6
      text "B: list virtualized=true, a button's label first" font-size=12 user-select="none"
      list virtualized=true testId="blist" height=150 width=360 estimated-item-height=36
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`b${n}` height=36 align-items="center" gap=12 padding-left=8
            button testId=`btn${n}`
              text `B${n}`
            text `bcode ${n} body` font-size=14
      text "R: list virtualized=true, a role=button view's label first" font-size=12 user-select="none"
      list virtualized=true testId="rlist" height=140 width=360 estimated-item-height=28
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`r${n}` height=28 align-items="center" gap=12 padding-left=8
            view role="button" testId=`rb${n}`
              text `R${n}` font-size=14
            text `rcode ${n} body` font-size=14
      text "Q: plain rows (no list), a button's label first" font-size=12 user-select="none"
      column testId="qlist" width=360
        each n in [1, 2, 3, 4, 5] key=n
          row testId=`q${n}` height=36 align-items="center" gap=12 padding-left=8
            button testId=`qb${n}`
              text `Q${n}`
            text `qcode ${n} body` font-size=14
```

Each scenario drags from the start of row 1's code to the end of row 3's code, then copies. On macOS:
`bun exact.mjs agent macos --size 520x620 "tap b1 drag 92 72 from 60 18" "type root key Meta+c"`, then `pbpaste`
(R: `tap r1 drag 88 56 from 36 14`; Q: `tap q1 drag 93 72 from 60 18`). On the web, Playwright's mouse drags in
Chrome 155.0.8059.39 over the JS build and the `--wasm` build, then Meta+C. In the table, `\n` is a newline.

| Scenario | Platform | Revision | Actual | Expected |
|---|---|---|---|---|
| B: a button's label first | macOS 26.6.2 (25G83), the agent's drag and ⌘C | #493 `107faa2a5` (and main `8c25f5520`, in the AppKit test below) | `B1\n\nbcode 1 body\n\nB2\n\nbcode 2 body\n\nB3`: every label, and row 3's code is missing. Painted: the three codes only | the three codes, as on the JS target |
| B | wasm web target, Chrome 155 | #493 `107faa2a5` | `bcode 1 body\n\nB2\n\nbcode 2 body\n\nB3\n\nbcode 3 body`. Painted (and `getSelection()`): the codes only | the three codes |
| B | Exact web, JS target, Chrome 155 | #493 `107faa2a5` (the JS target is unchanged by it) | `bcode 1 body\nbcode 2 body\nbcode 3 body` | (the reference) |
| R: a `role="button"` view's label first | macOS | #493 `107faa2a5` | `rcode 1 body\n\nR2\n\nrcode 2 body\n\nR3\n\nrcode 3 body`. Painted: the codes only | what is painted: the labels either painted and copied, or neither (the Mac's copy of plain rows leaves them out) |
| R | macOS, main `8c25f5520`'s `TextSelectionMac.swift` | main | `R1\n\nrcode 1 body\n\nR2\n\nrcode 2 body\n\nR3` (the endpoints off; #493 fixed them) | as above |
| R | JS target and wasm target, Chrome 155 | #493 `107faa2a5` | `rcode 1 body\nR2\nrcode 2 body\nR3\nrcode 3 body` (wasm: the same with blank lines). Painted: the codes and the labels | (the reference) |
| Q: plain rows | macOS, JS and wasm | #493 `107faa2a5` | the three codes on each (macOS with blank lines) | (unchanged) |

![B: Chrome on the JS target, the wasm target and macOS](https://raw.githubusercontent.com/ccheever/exact2/1a163b6b5ae714f25da862a579f7853f19f97393/fw/x81-list-copy-unselectable/list-copy-control-labels-b.png)

![R: the same](https://raw.githubusercontent.com/ccheever/exact2/1a163b6b5ae714f25da862a579f7853f19f97393/fw/x81-list-copy-unselectable/list-copy-control-labels-r.png)

Record (the commands, every copy and the AppKit test's runs on main's and #493's Swift):
https://raw.githubusercontent.com/ccheever/exact2/1a163b6b5ae714f25da862a579f7853f19f97393/fw/x81-list-copy-unselectable/list-copy-control-labels-record.txt

`TextSelectionMacTests.testACopyAcrossListRowsLeavesOutUnselectableText` (PR #493) covers R's endpoints. It does not
cover B, or R's labels in the copy.

Not run: iOS (it has no list-text copy), Linux, Windows, Firefox, WebKit, a hardware mouse, rows that are not mounted,
⌘A, and the other shapes `textSelectable` makes unselectable (a box with a press action, a `header`, `nav` or
`toolbar` label), which are read from code only.

## Constraints

- X81's constraints stand: the web is the parity oracle, a list copy holds what the host paints selected, the host and
  the runner count the same paragraphs, and a copy does not mount the selected range (LLP 1010 §6). For a row that is
  not mounted, what is unselectable has to come from the plan.
- LLP 1115 D8: on the Mac `auto` is AppKit's (`textSelectable`); the web host keeps the browser's own. So R's labels are
  selectable on the web and not on the Mac, and that difference is the platform's, not a tell.
- How the host and the runner agree on the `auto` rule is open (X81 named three ways: the runner applies the host's
  rule, the host tells the runner which paragraphs count and which are selectable, or the host numbers every text and
  the runner skips the unselectable ones). #493 took the third for authored `user-select`. That needs a decision,
  and it may need an LLP 1010 §6 amendment.
- For B, the Mac and the runner also disagree on what a paragraph is: the Mac has no paragraph for a native control's
  label. The wasm page counts it (`[data-exact-text]`), as the runner does.
- The paragraph separator (`\n\n` on the Mac and in the runner, `\n` in Chrome) stays outside this issue, as in X81.

## Acceptance criteria

- On macOS, B's drag copies the three codes with no label, and its endpoints land on the code.
- On macOS, R's list copy holds what is painted, as the Mac's copy of plain rows does.
- The wasm target's B copy gives the JS target's text.
- A runner test and an AppKit test cover a native control's label and a pressable's label in list rows, including rows
  that are not mounted.
