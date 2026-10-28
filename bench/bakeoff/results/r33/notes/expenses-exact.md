# Expenses build notes

Finished and installed as `dev.bakeoff.expenses.b6cae7a` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` using `EXACT_SIM=… bun exact.mjs ios`. No other simulator was used.

## Implemented

- Bundled all 240 supplied transactions through the data module’s JSON import as the initial history. The original `data/transactions.json` is retained unchanged.
- Newest-first history grouped by date, with daily totals and a current-month spending summary.
- Native add/edit sheets with amount, merchant, all seven supplied categories, a date picker defaulting to the local current date, and an optional note.
- Positive amount validation, integer-cent arithmetic, USD formatting, native swipe deletion, and atomic local-file persistence.
- Categories sorted by current-month spending with percentage shares and navigable transaction lists. Zero-spending months produce zero shares and useful empty states.

## Decisions and assumptions

- A merchant is required; a note is optional. Amounts round to the nearest cent. Invalid, zero, negative, and unsafe integer-cent amounts cannot be saved.
- Future dates are allowed because the specification does not forbid them.
- Category detail lists show the same month summarized on Categories. The main history always includes every month.
- “This month” follows the device’s local calendar date. Date-only transactions are formatted without shifting them across time zones.

## Validation

- Seven behavioral tests passed on both web and the assigned iPhone simulator. They cover the seed history and totals, category ranking and drill-down, invalid input, add/edit/delete, moving an expense into another month, original edit values, zero-spending months, and persistence after relaunch.
- The final native host verification reported `verify: clean`: no light/dark findings and nothing spilling on nine screens at accessibility-extra-extra-extra-large text size.
- Web layout verification also reported clean at phone and wide widths, light/dark mode, and the largest text size.
- Native real-touch scrolling reported nothing for the app to fix. Machine load prevented judging timing; physics checks ran. Exact reported its own slower status-bar scroll-to-top behavior, outside app control.
- Inspected simulator screenshots of the transaction history, category detail, and editor.

## What was hard / looked up

- Read Exact’s start guide and used its dashboard pattern. Looked up tabs, edit sheets, grouped forms, native swipe actions, date handling, money, storage grants, bundled JSON, symbols, and responsive wrapping.
- The first toolkit compiler invocation needed sandbox escalation to write its build lock in the allowed Exact checkout. The first native build was a cold compile; later builds were quick.
- Navigation attributes must live on the form component’s root for its header to become the native modal navigation bar. The compiler warning made the fix clear.
- Largest-text verification exposed title crowding and narrow merchant columns. System title styles, wrapping headers, and text-relative row bases resolved it without capping Dynamic Type.
- Changing sources during native verification invalidates the signed build receipt. The final pass used frozen sources, followed by a final install.

## Remaining verification caveat

No requested app feature is left unimplemented. However, the additional `contract verify` semantic differential phase returns a nonzero exit after the successful host verification. It reports disagreements between its two engines on generated synthetic resource values (including different row IDs at boot), and a refused form handler in its oracle. It also cannot deliver reloads or some select payloads. I could not obtain a clean result from that separate phase. The actual iOS/web behavioral tests and host layout checks passed. A focused reproduction log is at `.exact/property-check.log`; the toolkit printed its own differential receipts and counterexample paths.

## What would make this faster

- A warm compiler/native-host cache and a clearer sandbox-lock diagnostic.
- A starter combining tabs, grouped dated history, and an edit-sheet component with navigation already on its root.
- More clearly separate host-test results from synthetic semantic-oracle differences in the verifier’s final exit status and report.
- Show the large-text layout being checked directly alongside each finding.
