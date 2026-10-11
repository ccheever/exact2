---
name: 20261011-dialog-initial-focus-and-model-prices
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

# Dialogs that open with nothing focused, and the Model prices dialog

## Outcome

The user asked on 2026-10-11 to fix these after `realinput-1011a` (STATUS "Known differences" and `QUEUE.md`).

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| DF-1 | Under real keys Add provider and Model prices open with nothing focused, and Tab / Shift+Tab reach none of their stops; New task took the focus only after a click in Name (`realinput-1011a` DT-5, DT-6, DT-8: [Add provider](https://raw.githubusercontent.com/ccheever/exact2/c3767307f0cf9e509c22e05c456d27426c1fc12b/realinput-1011a/DT5-add-provider-no-focus.png), [Model prices](https://raw.githubusercontent.com/ccheever/exact2/4848dce9c2fe436a7c5ded94cfaf9d2ed17ae3f7/realinput-1011a/DT8-model-prices-open.png), [New task](https://raw.githubusercontent.com/ccheever/exact2/d090b1fc5fe335a981fae0dce20308daa5a6e041/realinput-1011a/DT6-new-task-shift-tab-X.png)). #431's agent drives focused a field before pressing Tab, so they did not see it | Base UI's Dialog moves the focus into the popup when it opens (its `initialFocus`: find what each of these three gets, over CDP and in the source), so the first Tab is already inside the trap |
| DF-2 | The other modal dialogs may share DF-1 | Audit every modal dialog #431 classified (`dialog-trap.test.ts`): which element has the focus right after it opens, in the clone and the reference |
| MP-1 | Usage › the environment menu › Model prices opens nothing on the Limits view (`pages-usage.ts` `usageView`: `if (input.open && !limits) page.prices = presentPrices(client)`); on Cost and Tokens it opens (QUEUE.md) | `UsagePage.tsx` renders its price dialog whatever the metric (checked over CDP on Limits: it opens) |
| MP-2 | Custom model prices draws no X and no "Add model price" +: both are laid out (32×32 and 24×24) and take the focus, but their icons do not show in the agent's window screenshot (QUEUE.md) | The reference draws both |

## Steps

1. Reference first (CDP on the reference lane; source under `target/t3-ref/src-1e2ecbd975`): the focused element right
   after each modal dialog opens (DF-1's three, then DF-2's audit); MP-1 on Limits; MP-2's two icons, their size and
   colour.
2. DF-1: reproduce in agent mode without a click before the first key (open the dialog the way a person does: its
   button or menu, then `key Tab` and read `state`'s focus). If an agent key cannot show it because it reaches the
   window by another path than a real key, say which path and prove the fix at the closest level (an AppKit test that
   sends the key through `NSApplication.sendEvent`, for example). Fix it the way the reference does: the dialog takes
   the focus it gives on open, so the first Tab and Shift+Tab stay inside #431's guards. Keep #409's import wizard
   focus and #431's guards.
3. DF-2: apply the same fix to every dialog that opens with nothing focused; extend `dialog-trap.test.ts` so a modal
   dialog with no focus on open fails.
4. MP-1: present the prices on Limits too; report what Limits' view costs before and after. MP-2: find why the icons do
   not draw (compare with New task's X, which draws) and draw them as the reference does.
5. Tests; one agent drive of the three dialogs and the two Model prices rows; before / after / reference images. Remove
   the two `QUEUE.md` lines when they land.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DF-1, DF-2 | reference comparison first; tests; agent drive (no click before the first key) | before / after / reference images |
| MP-1, MP-2 | reference comparison; tests; agent drive | before / after / reference images, light and dark |
| DF-1 under real keys | a real-input step for the short final session | — |

## Next action

Start now.
