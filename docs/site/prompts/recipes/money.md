```prompt Show money in my app
Make the Exact app in this folder keep and show amounts of money. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/money.md as the source for each step. The exact2 checkout has the same page at docs/recipes/money.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say the currency, ask me for its ISO 4217 code (USD, EUR, GBP, JPY) before you write code.
3. Keep every amount as a whole number of cents, in state and in the data the app sends: convert with `round(dollars * 100)` once, where text is parsed. Round each computed amount once, to a cent. Never add amounts as fractions of a dollar.
4. Format only where an amount is shown: write `fn money(cents: number): string = formatNumber(cents / 100, "currency", "<code>")` and use it in each `text`. Use `formatNumber(n, "percent")` for a rate and `formatDecimal(cents, 2)` for a field's value. An amount that changes in place takes `font-variant-numeric="tabular-nums"`.
5. If the person types an amount, follow docs/recipes/number-field.md for the field.
6. Add a test in app.test.contract that expects an amount grouped and rounded to the cent (`$1,748.13`), changes an amount with an action, and expects the new text.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me which amounts the app keeps in cents, the currency, what the tests and checks reported, and the launch command for each target platform.
```
