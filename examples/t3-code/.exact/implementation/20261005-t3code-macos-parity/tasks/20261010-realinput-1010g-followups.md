---
name: 20261010-realinput-1010g-followups
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Findings of the real-input session realinput-1010g (the shell context menu)

## Outcome

The session `realinput-1010g` ran [shell-context-menu](closed/20261010-shell-context-menu.md)'s (#407) real-input steps
1–8 on the bundle of `6bac646cc`. Passed: steps 1, 3, 4, 6, 7 and 8. This task takes what did not pass. Each row is
compared with the reference first (`DesktopWindow.ts` `installContextMenu`, CDP plus the main-process inspector, as
#407 measured it); a row that matches it closes as such. Session notes:
[G0](https://raw.githubusercontent.com/ccheever/exact2/eb35a6a4fa4d21f56032512841efaccfadb772ac/realinput-1010g/G0-1010g-notes.txt).

Start after [realinput-1010f-followups](20261010-realinput-1010f-followups.md) merges: its RF-1 takes "Services ›" out
of the shell's menu in the same files.

## Findings

| Id | From | Clone under real input | Evidence |
| --- | --- | --- | --- |
| RG-1 | #407 step 2 | A right-click on a tool row's icon (the blue square left of "Ran 2 commands…") shows only Cut/Copy/Paste/Select All, with no Copy Image (3 tries, two layouts). The input log shows the click on a plain view with no id, and the AX tree has no image element there. #407's after drive did not cover the icon either (its fix landed after the drive). | [G2](https://raw.githubusercontent.com/ccheever/exact2/a371438dcba003b57d9cca271518c553f0266362/realinput-1010g/G2-FAIL-tool-icon-shell-menu-no-copy-image.png) |
| RG-2 | #407 step 5 | In Settings' search field, after typing `theme`, the right-click itself changed the field to "Theme". Cut and Copy were enabled; picking Cut (2 tries) closed the menu and left "Theme" in the field and the clipboard empty. The empty field's menu (all four disabled) passed. | [menu](https://raw.githubusercontent.com/ccheever/exact2/dee2bd39efc48478bb328c2f1b8363439f7314c1/realinput-1010g/G5-2-theme-menu.png), [after Cut](https://raw.githubusercontent.com/ccheever/exact2/9e259fa84efe3e90a34473d5b609c991e869c506/realinput-1010g/G5-3-FAIL-after-cut-unchanged.png) |
| RG-3 | #407 step 4 | The composer's spelling menu has "AutoFill" (and "Services ›", RF-1) after Cut/Copy/Paste/Select All. The reference's menu has neither. | [G4](https://raw.githubusercontent.com/ccheever/exact2/d39ff8da68d014c55d085790a4858eb881c0317b/realinput-1010g/G4-1-spelling-menu.png) |
| RG-4 | X78 check (#412) | A reply's paragraph links are drawn word by word as `text` nodes with an `href` and a `press` (FlowRuns). A `text` node with its own `href` is not a link in the accessibility tree on either host, so these links may not read as links at all; the reference's are `<a>` elements (AXLink with AXURL in Chrome). | [X78 record](https://github.com/ccheever/exact2/pull/412) |

## Steps

- RG-1: find what the reference's tool icon is (an `<img>`, an inline SVG or a CSS box) and what its right-click shows
  (`mediaType === "image"` gives Copy Image). If it is an image there, make the clone's icon carry what the shell menu
  needs (an image node, or the module's hit test over it), and copy the same image data. If it is not an image there,
  the clone's menu already matches: close the row.
- RG-2: find why a right-click capitalizes the word (the selection of the word under the pointer, then a text
  substitution or autocorrection replacing it; `NSSpellChecker` or the field's automatic text replacement) and why Cut
  does nothing in this field (the action's target, the field editor, or a field that is not the first responder when
  the menu acts). Compare with the reference (CDP: the same field, right-click, Cut). Fix.
- RG-4: check the clone's reply links in the accessibility tree on both hosts (`agent.mjs web|macos tree --ax`, and from another process on macOS) against the reference's (`<a>`, CDP `Accessibility.getFullAXTree`). If they are not links, make them links where Contract allows it (a `link` node, or `role="link"` with the href) without changing their look or menus; a host part is X78 (main #412).
- RG-3: AppKit adds AutoFill (and Services) to a menu it pops for a text view. Take AutoFill out where the shell's menu
  is built, with RF-1's change. Check whether "Writing Tools" or other system items appear on macOS 26 and take them out
  too.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RG-1..RG-4 | reference comparison first; an AppKit test of the menu builder and of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Next action

Start after realinput-1010f-followups merges.
