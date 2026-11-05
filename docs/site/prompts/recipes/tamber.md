```prompt Give my app its brand colors
Give the Exact app in this folder its own brand colors in app.tamber, with a dark style the person picks. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/tamber.md as the source for each step. The exact2 checkout has the same page at docs/recipes/tamber.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not give the brand's colors, ask me for them. Do not invent a brand. Ask me whether the person picks a dark style (for example Dim or Black) or the app follows the system only.
3. Write app.tamber beside app.contract: a `theme` of tokens with `light-dark()` values and, for a picked dark style, an `axes` line with `under scheme=dark` and a `when` line for each other mode. Mark a one-color token `scheme=fixed`.
4. In app.contract, name the tokens where a color is the brand's, and keep the platform's roles everywhere else. Set a mode with `setMode("<axis>", "<value>")`. A preview that always shows one mode is a pin, `-exact-mode="<axis>=<value>"`, with `color-scheme="dark"` when it is dark.
5. Run `bun exact.mjs tamber check .`, then `bun exact.mjs tamber eval . --mode <axis>=<value>` for each mode, and `bun exact.mjs tamber fmt .`.
6. Add a test in app.test.contract that taps each mode button and checks the screen, and one that uses `prefer mode <axis>=<value>` after `clock data`.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me the tokens and modes, what tamber check, the tests and verify reported, and the command that opens the app: `bun exact.mjs web`.
```
