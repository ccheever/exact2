```prompt Give my app its colours
Give the Exact app in this folder its brand colours for light and dark mode. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/theme.md as the source for each step. The exact2 checkout has the same page at docs/recipes/theme.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not give the brand's colours for light and dark, ask me for them before you write code.
3. Use a role where the platform has one (`-exact-label`, `-exact-secondary-label`, `-exact-secondary-background`); write a token only for the brand. Write the tokens in app.tamber beside app.contract, each `light-dark(<light>, <dark>)`. A colour that is the same in both schemes on purpose (an ink on the brand's fill) says `scheme=fixed`. A text colour on a surface says `pair=<surface>`. If the app has a stylesheet, `bun exact.mjs tamber import css <file>` writes its tokens.
4. Set `AccentColor` for the app's tint, with both sides at 3:1 or more under white. Give each icon token an `sf` and a one-colour `svg` under assets/ for the other hosts.
5. In app.contract, name the tokens (`color="<token>"`, `background-color="<token>"`) in place of hex values. Run `bun exact.mjs tamber check .` and fix its warnings.
6. Add a test in app.test.contract that says `prefer prefers-color-scheme dark` before an `expect` or `screenshot`, then the same with `light`.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me the tokens, the roles you used instead of tokens, what tamber check, the tests and the checks reported, and the launch command for each target platform.
```
