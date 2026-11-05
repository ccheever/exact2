```prompt Sell a subscription or a tip
Add in-app purchases (a subscription or a tip) to the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/purchases.md as the source for each step. The exact2 checkout has the same page at docs/recipes/purchases.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. Ask me which items to sell, with their store IDs, and which are consumable (a tip) and which are a subscription. Do not invent item IDs. The store sells on iOS, macOS and Android only; elsewhere the lists are empty.
3. Install the package: `bun <exact2>/scripts/exact.mjs add purchases .`. Name the test catalogue in app.json (`"purchases": {"catalog": "store/catalog.json"}`) and write it with my items.
4. In app.contract, gate each screen on `listPurchases()`, show each price with `displayPrice`, and send `purchase(itemId=...)` from a press. Use the purchase's answer only for the screen ("Waiting for approval" while pending); never deliver from it.
5. In app.ts, deliver from `unfinishedPurchases()` only, once per `purchaseToken`: keep the credit and the token together, then `consume` a consumable and `acknowledge` the rest. Finish every purchase.
6. Put Restore behind a button. Never put `requestReview()` behind a button.
7. Add tests in app.test.contract with the agent's `purchase` ticket: a purchase (`buy`), a cancelled one (`cancel`) and a pending one (`pending`), and that a delivered tip counts once.
8. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 9) and report it; do not keep trying.
9. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
10. Tell me what you changed, what the tests and verify reported, and what I must still set up in each store before a real purchase works. `apps/weatherlight` in the exact2 checkout is the whole app.
```
