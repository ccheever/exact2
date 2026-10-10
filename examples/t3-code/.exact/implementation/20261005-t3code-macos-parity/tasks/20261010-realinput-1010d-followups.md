---
name: 20261010-realinput-1010d-followups
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

# New findings of the real-input session of 2026-10-10 (realinput-1010d)

## Outcome

The session `realinput-1010d` drove #349's fix round, the rest of #383 and #384, #296 U6 and two #346 rows on
lane copies with real keys and pointer. #349's acceptance rows passed and it merged (`70e2ddd4b`). These findings are
new. Each is compared with the reference first: a row that matches the reference is closed as such, not changed.

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RD-1 | #349 pill | The floating player's pill shows when the pointer rests on the toolbar dot. | When the pointer arrives on the dot and rests, the grab hand shows but no pill, until the pointer moves again (3 of 3). Once showing, the pill holds. | [pill](https://raw.githubusercontent.com/ccheever/exact2/4f2df998e1042f628051a603eff6e80fba76b23e/realinput-1010d/D4-349-pill.png) |
| RD-2 | #349 Float | To check first: where the reference puts the player when it floats again after being moved. | Floating again with Toggle right panel puts the player back in the default corner at 240 × 333, not where it was left. | [D4](https://raw.githubusercontent.com/ccheever/exact2/10c4009a2b19eb891713cd0fed56905057929b93/realinput-1010d/D4-349-fix-round.png) |
| RD-3 | #346 row 3 | The agent's cursor glides to each target and pings on it. | In the 1280 × 800 Responsive viewport the cursor reaches each target's x but sits about 270 pt below it (driven by #352's lane-only fake ACP agent, "cursor" scenario). | [D6](https://raw.githubusercontent.com/ccheever/exact2/9faadb38bd7f3c0e59354496ebe247b0d798e906/realinput-1010d/D6-346-r3-agent-cursor.png), [log](https://raw.githubusercontent.com/ccheever/exact2/a4b636d6dacfbf3da5e61c2cb14bc60044564296/realinput-1010d/D6-346-r3-automation-log.txt), [agent](https://raw.githubusercontent.com/ccheever/exact2/28736eb368a0d3e74dc58a3d9b213c339a3837fc/realinput-1010d/D6-fake-acp-cursor.mjs.txt) |
| RD-4 | #384 step 4 | Right-click on selected timeline text: Cut (disabled), Copy, Paste (disabled), Select All (`DesktopWindow.ts`, context-menu). | AppKit's text menu: Look Up, Copy, Speech, Services. | [D2](https://raw.githubusercontent.com/ccheever/exact2/a308565c52b88f44620c19ab3079e5888278fca4/realinput-1010d/D2-384-help-tags-menus-context.png) |
| RD-5 | #383 step 4 | The snooze menu's items fit inside the menu. | "Mon 9:00 AM" runs past the menu's right edge. | [D1](https://raw.githubusercontent.com/ccheever/exact2/7051ebd9ca964887ec425fe240ce19e889b24afa/realinput-1010d/D1-fx2-one-highlight.png) |

## Scope and exclusions

Included: RD-1 to RD-5. Where a row's cause is in the framework (for example, if RD-4's text menu cannot be replaced
from app code, or RD-1's hover needs a move the host never sends), record it in `EXACT2-GAPS.md` with a one-file repro
and leave the framework alone; the main issue is filed separately.

Excluded:
- #383 step 4's second Escape leaving Filters open: the same cause as RC-3, fixed in
  [realinput-1010c-fixes](20261010-realinput-1010c-fixes.md).
- #308's gutter drag: not run, because the GitHub lane's Code tab answered HTTP 503. It is a re-check for the next
  real-input session (STATUS), not a fix.
- #296 U6's race (pairing before the server registers itself as This machine): real input cannot time it; its unit
  test stands.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RD-1..RD-5 | reference comparison first; then a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Next action

Start now. Real-input checks join the next session with the RC rows.
