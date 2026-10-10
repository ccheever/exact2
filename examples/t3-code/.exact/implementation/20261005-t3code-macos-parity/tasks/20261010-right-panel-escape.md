---
name: 20261010-right-panel-escape
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-right-panel-escape
pr_url: null
verified_commit: null
---

# Escape and the right panel

## Outcome

The clone's "Toggle right panel" button declares Escape (`r4-surfaces.contract`, `testId="panel-toggle-right"`,
`aria-keyshortcuts` … `"Escape"`) whenever a panel is open and no editor, URL field, device setup or annotate pick has
the key. So Escape closes the right panel (Diff, Files, Browser, …). This has been there since the clone's first commit
(`b0a14280a`), and no record says why. T3 Code (`1e2ecbd975`) binds no Escape to the right panel: its defaults are
`rightPanel.toggle` = mod+alt+b and `rightPanel.close` = mod+w when the terminal is not focused
(`packages/shared/src/keybindings.ts:26,31`). Its components handle Escape only inside their own fields (the Files
search, a device tab's rename, the PR detail panel's own controls, the PR page's "Let panel shortcuts consume Escape
before page navigation"). Found in the review of #399 (realinput-1010c-fixes, RC-4).

## Steps

1. Check the live reference first (`target/t3-audit/ref-app.sh`, CDP): with each of Diff, Files, Browser and the
   Pull Requests page's panel open and the focus in the page (not in a field), press Escape. Record whether the panel
   closes. Also check with the focus on the panel's own toggle button.
2. If the reference never closes the panel on Escape, remove the clone's Escape from the toggle. Check what relied on it:
   tests that press Escape to close the panel, the Browser annotate pick (`panel.browser.capture.pickActive`), the file
   editor menu (`0fc543028`, "dismiss the file editor menu before the panel"), and the chip details' Escape order
   (`ChipPopoverEscape`, #399). If the reference closes it in some case, match that case only.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Reference behaviour | CDP drive of the live reference, each panel kind | text record |
| Clone matches | unit test of the toggle's keys; one agent drive: a panel open, Escape, the panel stays (or matches the reference's case) | before / after image |

## Reference behaviour (live, 2026-10-10)

T3 Code Alpha 0.0.45 (`1e2ecbd975`) on lane `right-panel-escape` (backend 16600, CDP 16601), Escape by CDP, the panel's
state read from Toggle right panel's `aria-pressed`. Full record:
[escape-record.txt](https://raw.githubusercontent.com/ccheever/exact2/6929441df7a0e6b26106831c256ec29926a971ef/right-panel-escape/escape-record.txt).

- **Inline (1280 × 840): Escape never closes the panel.** Checked with the launcher focused, Files (focus on BODY, on the
  toggle, on a tree row, in the search field, in the file editor), Browser (BODY, URL field, toggle), Diff on the server
  thread (BODY, toggle) and the composer. The search and URL fields blur; the file editor blurs
  (`installFileEditorDismissal`).
- **As a sheet (a window 980 wide or less, `RightPanelSheet`, a Base UI Dialog): Escape closes the panel** from Browser
  (the focused tab close button, the URL field), Diff, the Files search and the launcher. The file editor's first Escape
  only blurs it (its capture-phase listener stops the key); the second closes the sheet. Base UI also leaves Escape to
  a nested menu or dialog, and the Browser pick focuses the page.
- **Pull Requests page:** the fixture has no pull request, so its toggle is disabled and its panel cannot open; Escape
  there went back to the thread (`useEscapeToGoBack`). The clone's page panel (`PrPanel`) never used this toggle.

## Cause and fix

The clone closed the inline panel on Escape two ways, both since its first commit (`b0a14280a`): the tab bar's toggle
declared `aria-keyshortcuts="Escape"`, and the launcher's `keys` handed every other key, Escape included, to `panelUi`,
whose `(what == "key" and id == "Escape")` hid the panel.

- `r4-surfaces.contract` `R4HeaderBar`: the toggle declares Escape only `when sheet`, except while a nested dialog
  (`panel.deviceSetup`), the file's editor menu (`panel.files.editorsOpen`, `0fc543028`), the file editor
  (`panel.files.editing`, new) or the Browser pick (`panel.browser.capture.pickActive`) holds it. `urlFocused` is no
  longer an exclusion: the reference's sheet closes from the URL field. The prop leaves `R4PanelHeader`/`R4HeaderBar`
  and their two callers (`shell-panels.contract`, `diff.contract`); `SurfacePanel` still passes it to the Browser body.
- `shell-panels.contract`: the launcher bar's toggle declares Escape only `when sheet`.
- `app.contract` and `app-window.contract` `panelUi`: the `key` Escape clause goes, so the launcher's Escape hides
  nothing; its letters still open surfaces.
- `r4-surfaces-files.contract` `R4FilePreview`: the editor's own `key` takes Escape and blurs, which ends the editing
  (it had no Escape of its own while the toggle took the key).
- What relied on the old key, checked: the annotate pick (`browser-capture.test.ts` row updated; the pick still holds it
  in a sheet), the editor menu (its backdrop has its own Escape; it still holds the sheet's), the diff scope menu's
  scoped Escape (`menu-keys.test.ts`, unchanged, it still matters in a sheet), and `ChipPopoverEscape`'s first-node
  order (#399; unchanged, the sheet's toggle still declares Escape). No test pressed Escape to close the panel.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Reference behaviour | Done: inline never closes; a sheet closes except where the focus keeps the key | [escape-record.txt](https://raw.githubusercontent.com/ccheever/exact2/6929441df7a0e6b26106831c256ec29926a971ef/right-panel-escape/escape-record.txt) |
| Clone matches: inline Files, Escape at the toggle | Pass: before closed, after stays | [inline-toggle-escape.png](https://raw.githubusercontent.com/ccheever/exact2/2438dcffd6361df2611f0e1cfff1940925fe6866/right-panel-escape/inline-toggle-escape.png) |
| Clone matches: inline file editor | Pass: before closed the panel, after the editor is left and the panel stays | [inline-editor-escape.png](https://raw.githubusercontent.com/ccheever/exact2/eaaec9b37f416561befb3c125a67f5e53f99c357/right-panel-escape/inline-editor-escape.png) |
| Clone matches: inline launcher | Pass: before closed, after stays | [inline-launcher-escape.png](https://raw.githubusercontent.com/ccheever/exact2/3de07f3ac9974f13c344ee320400ee61f40f8722/right-panel-escape/inline-launcher-escape.png) |
| Clone matches: the sheet closes | Pass: before and after close, as the reference (Files at the toggle; the launcher, text record) | [sheet-escape.png](https://raw.githubusercontent.com/ccheever/exact2/50500ec963af39b82659ec3e2899584731d0c049/right-panel-escape/sheet-escape.png) |
| Clone matches: the sheet's file editor | Pass: before closed the sheet, after the first Escape leaves the editor and the second closes | [sheet-editor-escape.png](https://raw.githubusercontent.com/ccheever/exact2/08ff724697f7f348b7e8e28630fb260a31ee9bc6/right-panel-escape/sheet-editor-escape.png) |
| Unit test of the toggle's keys | Pass: `right-panel-escape.test.ts` evaluates both toggles' `aria-keyshortcuts` over inline/sheet × each holder | Tests |

No real-input rows: inline nothing declares Escape any more, and the sheet's key is the same shortcut as before.

## Tests

- `right-panel-escape.test.ts` (new, 6): the tab bar toggle's and the launcher bar toggle's `aria-keyshortcuts`
  evaluated as expressions (inline: none, whatever holds the focus; sheet: Escape, none while each holder holds it); the
  `urlFocused` prop gone from the header; the Files editor's Escape; `panelUi` without the `key` Escape clause. Each
  fails on the base (the old expression reads `urlFocused`, declares Escape inline; `panelUi` hid on Escape).
- `browser-capture.test.ts`: the annotate pick row checks the new expression.

## Next action

Coordinator review of the draft PR.

