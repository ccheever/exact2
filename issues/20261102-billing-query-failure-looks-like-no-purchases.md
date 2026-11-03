# A failed Play Billing query is reported as an empty purchase list

**Status:** Closed
**Systems:** Play Billing (`packages/purchases/android/Purchases.kt`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** `listPurchases`, `restorePurchases` and `unfinishedPurchases` answer success with an empty list when the Billing query failed. `acknowledge` and `consume` then look the token up in that list and answer `NOT_OWNED`. Play refunds an unacknowledged one-time product after three days. A disconnected or unavailable service is enough.

**Cause.** `query` takes the purchase lists from both `queryPurchasesAsync` callbacks and ignores both `BillingResult`s. It always calls `done` with whatever rows arrived. The three list operations put that array in `exact:answer`. A non-OK result from Play is an empty list before any cached rows are returned.

**Reproduction.** Call `listPurchases` after `onBillingServiceDisconnected`, or while `queryPurchasesAsync` returns `SERVICE_UNAVAILABLE`, `NETWORK_ERROR` or `SERVICE_DISCONNECTED`. The answer is `{ "exact:answer": [] }`. `acknowledge` of a token the account holds answers `NOT_OWNED`.

**Fix.** Pass the `BillingResult` through `query`. A non-OK code answers the existing failure object and does not answer an empty list. Run `acknowledge` and `consume` only against a list that query reported as OK.

**Acceptance.** A disconnected query answers a failure, and a later successful query still lists the purchase. An account that owns nothing still answers an empty list when both results are OK.

**Resolution.** Carry the BillingResult through both purchase queries. A failed INAPP or SUBS query returns the billing failure, including for acknowledge and consume, without reporting an empty or partial ownership list.

**Verification after the fix.** The actual package compiled against Billing 9.1.0. A Kotlin harness of its query function passed INAPP failure, SUBS failure after INAPP success, and successful empty-query cases. Live Play Store operations were not exercised.
