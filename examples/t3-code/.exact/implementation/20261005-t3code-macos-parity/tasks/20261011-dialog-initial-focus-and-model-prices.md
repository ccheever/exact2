---
name: 20261011-dialog-initial-focus-and-model-prices
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-dialog-initial-focus-and-model-prices
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

## What was built

**DF-1, DF-2: every modal dialog takes the focus as it opens.** Base UI's Dialog and AlertDialog move the focus into the
popup when it opens (FloatingFocusManager's `initialFocus`): by default the popup's first Tab stop, unless an element
inside autofocuses as it mounts or the dialog names its own `initialFocus`. The clone's dialogs relied on Contract's
`autofocus`, which (as HTML's) takes the focus only when nothing holds it: most dialogs have none, so under real keys
they opened with nothing focused and a Tab reached no stop at all (the trap's guards are only reached from inside), and
a dialog opened from a key left the focus on its opener behind the popup.

- `dialog-trap.contract` `DialogOpenFocus(target)`: an invisible 1×1 `aria-hidden` box at the top of each popup whose
  first `resize` (ResizeObserver's first entry, the layout that drew the popup, as Base UI focuses from a layout effect)
  moves the focus to `target` once. A popup hidden under `display: none` lays it out 0×0, which is no opening; it opens
  again when it shows. No framework change.
- Of the 50 modal popups #431 classified (34 guarded, the command palette and Link pull request counted apart, and 16
  keyed), 48 carry one `DialogOpenFocus`, right after the start guard where the popup has one, naming the element the
  reference focuses; the import wizard and the profile removal confirm keep #409's own focus task (`app.contract`
  `importWizardStepFocus`, `removalFocus`). #431's guards are unchanged.
- The targets, from the reference source and over CDP: the first Tab stop by default (Add provider: "1 Provider", or
  the X after an instance is created; New task: Runs on, or Name when editing; Custom model prices: Apply to, else +,
  else the X; the usage model dialog: Set price, else the X; the confirms: Cancel, their first stop); an element the
  reference autofocuses (the palette's field, CommandInput `autoFocus`; Add/Edit Action's Name; Add a CLIProxyAPI hub's
  Hub URL; Add device host's name; Add a theme's search; Create pairing link's Client label); the dialog's own
  `initialFocus` (the expanded image's X, `ExpandedImageDialog` `closeButtonRef`; the welcome wizard's pairing URL, else
  Continue while it is enabled, else its first stop). Where a busy dialog disables its first stop, the target is the
  next enabled one, as Base UI skips disabled elements.
- Settings › Projects › Add/Edit Action's Name field gets the element id `action-name` it was named by.

**MP-1: Model prices on Limits.** `pages-usage.ts` `usageView` presents the prices whatever the metric (`if
(input.open)`; it was `input.open && !limits`), as `UsagePage.tsx` renders its price dialog on every metric. Its cost
on Limits is one lookup while closed: 40.6 → 40.3 µs per `usagePage` with Model prices closed, 41.8 → 41.6 µs with it
open (the dialog now drawn), mean of 3000 runs ([mp1-limits-cost.txt](https://raw.githubusercontent.com/ccheever/exact2/77c115d34c57464a16b2a0a6c010aca93fcb88b3/dialog-initial-focus-and-model-prices/mp1-limits-cost.txt)).

**MP-2: the X, the + and the chevron.** `UsageSchemeIcon` (`pages-usage-prices.contract`) picked each glyph's colour by
the `scheme` fact alone. Where that fact and the appearance the window draws disagree, it drew `#f5f5f5` on the light
dialog: an agent launch on this Mac (system appearance Dark) reads `scheme: "dark"` while it draws the window light
(`state` at launch: `scheme "dark"`, `appearance "light"`), which is how the agent's screenshots lost the Custom model
prices X, "Add model price" + and the Apply to chevron. The glyph's colour is now `light-dark()`, resolved against the
appearance the window draws, as New task's X is; it still mounts again per scheme, because an SVG's `light-dark()` does
not follow an appearance flip.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| DF-1 | pass (agent drive, no click before the first key; reference over CDP). Before: Add provider, New task and Custom model prices open with nothing focused, and Tab and Shift+Tab ×2 leave it so (as `realinput-1011a` DT-5, DT-6, DT-8 under real keys: an agent key reaches the window by the same path here, so no AppKit-level test was needed). After: Add provider "1 Provider" → Tab "2 Identity" → Shift+Tab ×2 the X; New task Runs on → Tab Name → Shift+Tab ×2 the X; Custom model prices Apply to → Tab + → Shift+Tab ×2 the X; each as the reference's | [add-provider](https://raw.githubusercontent.com/ccheever/exact2/d72ca83004b2a4cf42a999c10c87aea805d13ad8/dialog-initial-focus-and-model-prices/df1-add-provider.png), [new-task](https://raw.githubusercontent.com/ccheever/exact2/465a0fb9367c9893ac573d138c768d62e295168f/dialog-initial-focus-and-model-prices/df1-new-task.png), [model prices](https://raw.githubusercontent.com/ccheever/exact2/705b24317427f9ada1ab25304124848a95410a61/dialog-initial-focus-and-model-prices/df1-mp2-model-prices-launch.png), [focus read-outs](https://raw.githubusercontent.com/ccheever/exact2/6beb167e17fd7ea1035ee0b2ad141c2c27341da1/dialog-initial-focus-and-model-prices/focus-before-after.txt) |
| DF-2 | pass: all 50 modal popups take the focus on opening (48 by `DialogOpenFocus`, 2 by #409's task), checked by `dialog-trap.test.ts` (50 per-dialog tests; removing one `DialogOpenFocus` fails its test). The agent drive's sample: Add a CLIProxyAPI hub opened with nothing focused before, in Hub URL after, as the reference. The reference over CDP, as the clone's targets: Add environment (the Remote link card), Add hub (Hub URL), Add a theme (its search), the command palette (its field) | [add-hub](https://raw.githubusercontent.com/ccheever/exact2/f33381f32c8a2a9ed8ab5e7b6c30adbb75d792b8/dialog-initial-focus-and-model-prices/df2-add-hub.png), [focus read-outs](https://raw.githubusercontent.com/ccheever/exact2/6beb167e17fd7ea1035ee0b2ad141c2c27341da1/dialog-initial-focus-and-model-prices/focus-before-after.txt) |
| MP-1 | pass: Model prices opens on Limits (light and dark), as on Cost and Tokens, and as the reference; Limits' view cost unchanged | [limits light](https://raw.githubusercontent.com/ccheever/exact2/51c659471a99a7dcc8cf3090fae5e0f1e02f58f5/dialog-initial-focus-and-model-prices/mp1-limits-light.png), [limits dark](https://raw.githubusercontent.com/ccheever/exact2/71e67755ec4c153ab74907cf90efeb3146d69bdd/dialog-initial-focus-and-model-prices/mp1-limits-dark.png), [cost](https://raw.githubusercontent.com/ccheever/exact2/77c115d34c57464a16b2a0a6c010aca93fcb88b3/dialog-initial-focus-and-model-prices/mp1-limits-cost.txt) |
| MP-2 | pass: the X, + and chevron draw at launch in the light window (before: none of the three); dark unchanged; as the reference | [model prices at launch](https://raw.githubusercontent.com/ccheever/exact2/705b24317427f9ada1ab25304124848a95410a61/dialog-initial-focus-and-model-prices/df1-mp2-model-prices-launch.png), [cost dark](https://raw.githubusercontent.com/ccheever/exact2/fb3db9bcad6dff280975204504791a7fe2c46bfc/dialog-initial-focus-and-model-prices/mp2-cost-dark.png), [limits dark](https://raw.githubusercontent.com/ccheever/exact2/71e67755ec4c153ab74907cf90efeb3146d69bdd/dialog-initial-focus-and-model-prices/mp1-limits-dark.png) |
| DF-1 under real keys | implemented, not verified (real input reserved for the coordinator's final session): Real-input batch steps below | — |

Drives: the evidence base `ff1eb4ceb` (`t3-code-evidence-base`, lane `dialog-initial-focus-and-model-prices-before`) and
this branch's bundle (lane `dialog-initial-focus-and-model-prices`), the same ops at 1280×840, clone server on 16302; the
reference T3 Code 1e2ecbd975 on 16300/16301 over CDP. The after drive ran three times: the first stopped at op 5 and the
second at op 34, both on the lane's two stacked provider-update toasts (the second takes taps only once the first has
gone, and until then covered New task); the third, with the dismissals 2.5 s apart, is the evidence. Tools: [drive.sh,
focusmap.py, compose.py, limits-bench.ts and the reference scripts](https://raw.githubusercontent.com/ccheever/exact2/265ede65bfb2e411c5dc43e09d5bb55f3ea76f16/dialog-initial-focus-and-model-prices/tools.txt).

## Known differences (after this task)

- The focus a dialog takes on opening shows no ring (a pointer opened it), as the reference's `:focus-visible`; under
  agent keys the clone draws no ring on the X after Shift+Tab either (the focus is there: `state`), which the real-input
  step checks.
- Found, not in this task: with Custom model prices open, an appearance flip to dark leaves its title "Custom model
  prices" drawn in the light colour on the dark dialog (base and branch alike; the other texts flip). A dialog opened in
  dark draws it right. Not investigated.

## Tests

- `dialog-trap.test.ts`, "every modal dialog takes the focus as it opens (Base UI's initialFocus)": the mechanism; one
  test per modal popup (50: one `DialogOpenFocus`, right inside the popup, every literal target an element id; the
  default target is the popup's first stop, the start guard's or key wrap's; the reference's named and autofocused
  targets; the import wizard and profile removal keep #409's task); DF-1's three targets.
- `usage-pooled.test.ts` "Model prices on every metric": opens on Limits, Cost and Tokens and closes on each.
- `pages-usage-r3.test.ts` "Model prices draws its X, + and chevron in the drawn appearance's colour".

## Real-input batch steps

On a lane copy of this branch's bundle, light appearance (System Settings › Appearance › Light) and then once in dark:

1. Settings › Providers › Add provider (click the button). Without clicking in the dialog: press Tab. Expect the ring on
   "2 Identity". Press Shift+Tab twice: the ring on the X. Escape.
2. Settings › Scheduled Tasks › New task (click). Press Tab: the caret in Name. Shift+Tab twice: the ring on the X.
   Escape.
3. Usage › the environment menu (breadcrumb) › Model prices, on Cost and then on Limits: the dialog opens on both, with
   its X, the + after "Cache write" and the chevron after "All environments" drawn. Press Tab: the ring on +. Shift+Tab
   twice: the ring on the X. Escape.
4. Settings › Providers › Add hub (click), then type a letter at once: it lands in Hub URL.

## Next action

Coordinator: review the draft PR; run the Real-input batch steps in the final session.
