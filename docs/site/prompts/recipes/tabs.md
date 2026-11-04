```prompt Split my app into tabs
Split the Exact app in this folder into tabs, each with its own route stack. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/tabs.md as the source for each step. The exact2 checkout has the same page at docs/recipes/tabs.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. Tabs need at least two top-level sections; a one-tab bar is wrong. If I did not name the sections, propose them from what the app does (its current screens become the first tab) and ask me before you write code.
3. In app.contract, make each section a `tab` in `routes`, with its own routes under it, and add a `pick` action that runs `select(nav, name)`. Draw a `tabpanel` for each tab with its stack inside, and a root `tablist` as the root column's last child: one `role="tab"` button per tab, with a symbol over a label, `aria-controls` naming its panel and `aria-selected` when it is `nav.tab`.
4. To hide the tab bar, use `display="none"`, never `if`.
5. Keep the existing tests passing without changing what they check. A test that reads inside a tab other than the first must tap that tab first.
6. Add a test in app.test.contract that opens a screen in one tab, switches tabs and back, and expects the screen to still be there.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me the tabs and what each holds, what the tests and verify reported, and the command that opens the app: `bun exact.mjs web`. A web window 900 px or wider draws the tab bar as a sidebar.
```
