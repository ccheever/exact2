```prompt Use symbols for my app's icons
Make the icons in the Exact app in this folder the platform's symbols. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/symbols.md as the source for each step. The exact2 checkout has the same page at docs/recipes/symbols.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. List each icon the app draws, including emoji and characters used as icons (▦, ☏), with the symbol role you would use for it from `bun exact.mjs contract vocab symbol`. Show me the list and ask before you change anything.
3. In app.contract, draw each icon with `image "symbol:<role>"`; a role draws on every host. For an SF Symbol with no role, add an icon token to app.tamber's `icons` with `sf` and a one-colour `svg` under assets/, and draw `symbol:icon.<name>`. `symbol:sf/<name>` alone draws only on Apple.
4. A button with only a symbol needs an `aria-label`. A symbol beside text that says the same takes `aria-hidden=true`. Leave the symbol's colour unsaid, and size it with its font (`font="-exact-title1"`), not `width`. Fix each `lower-symbol-alias` warning by writing the role it names.
5. Keep the existing tests passing without changing what they check.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me each icon and its symbol, any icon left as it was and why, what the tests and checks reported, and the launch command for each target platform.
```
