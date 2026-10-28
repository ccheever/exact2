# Recipe: selling a subscription or a tip

`@exact/purchases` sells through the store on iOS, macOS and Android (LLP
1133.015). Elsewhere `purchasesStatus().available` is false and the lists are
empty. A screen gates on **`listPurchases()`**, what the person holds;
delivery reads **`unfinishedPurchases()`**, the store's queue, and nothing
else.

```text
use getDetails, listPurchases, unfinishedPurchases, purchase, ItemDetails, PurchaseDetails from "@exact/purchases"

shape Delivery
  tips: number

component App
  resource owned = listPurchases() as shape PurchaseDetails[] else empty()
  resource queue = unfinishedPurchases() as shape PurchaseDetails[] else empty()
  resource plans = getDetails(itemIds=["plus.yearly", "tip.small"]) as shape ItemDetails[] else empty()
  mutation bought as shape PurchaseDetails
  mutation delivered as shape Delivery
  derive due = queue.filter(p => p.purchaseState == "purchased").map(p => p.purchaseToken).join(",")
  action buy(itemId: string)
    send bought = purchase(itemId=itemId)
  action deliver
    send delivered = deliverPurchases()
  task delivering when due != "" key=due
    after(1, deliver)
```

The loop, in data code, delivers once by token, then finishes:

```text
for (const p of await unfinishedPurchases()) {
  if (p.purchaseState !== 'purchased') continue;     // pending: nothing yet
  if (p.itemId === 'tip.small') {
    if (!kept.tokens.includes(p.purchaseToken)) {
      kept.tips += 1; kept.tokens.push(p.purchaseToken);
      await save(kept);                              // credit and token together
    }
    await consume({ purchaseToken: p.purchaseToken });
  } else await acknowledge({ purchaseToken: p.purchaseToken });
}
```

- Show `displayPrice`, the store's own string.
- `purchase`'s answer is for the screen ("Waiting for approval" when
  `pending`); never deliver from it.
- Finish everything: Apple redelivers at every launch, Play refunds after
  three days.
- Restore sits behind a button (Apple); `requestReview()` is never a button.

Under the agent each purchase is a `purchase` ticket (`buy`, `cancel`,
`pending`, `unavailable`, `fail`) over the catalogue `app.json` names
(`"purchases": {"catalog": "store/catalog.json"}`); `prefer extension
purchases.storeEvent={"op":"approve"|"renew"|"revoke"|"purchase",…}` is the
store acting alone. `apps/weatherlight` is the whole app.
