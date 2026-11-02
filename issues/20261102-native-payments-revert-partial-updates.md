# Native payment sheets revert accepted details on a partial update

**Status:** Closed
**Severity:** P2
**Systems:** packages/payments, Apple Pay, Google Pay
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, Apple and Android rebuild each payment update from the original checkout details. A later partial update can therefore restore an obsolete total or shipping options.

`packages/payments/apple/Payments.swift:275-283` merges into `s.args["details"]` without retaining the result. `packages/payments/android/Payments.kt:233-241` does the same. Apple's shipping-address completion at line 293 additionally supplies an empty shipping-method list when the update omits it. A timed-out Apple callback also returns original details.

Initialize a checkout at $10. Return total=$15 and new shipping options from the shipping-address handler, then return `{}` from the shipping-option or payment-method handler. The next native completion returns $10. Omitted fields should retain the last accepted values, as [Payment Request's update algorithm](https://www.w3.org/TR/payment-request/#update-a-paymentrequest-s-details-algorithm) specifies.

`swift /tmp/exact-review-payment-update.swift` executed the exact Apple `updated` method with minimal PassKit output stubs: first total=15.00, shipping=[new]; second total=10.00, shipping=[]. This proves the method's state loss; native UI and paid transactions were not exercised. Android was verified by source review.

Keep current details in each Sheet, merge accepted updates into them, and preserve them on omitted fields or timeout. Add a sequence covering successive partial updates.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

**Resolution.** Retain the accepted detail dictionary for each native payment sheet and merge successive partial updates into it. Apple callbacks without updates return the current summary and shipping methods; malformed amounts fail the sheet.

**Verification after the fix.** All changed Apple sources typechecked. A Swift/PassKit harness of the actual update function checked successive partial updates and no-update callbacks retaining the accepted total and shipping methods. Android’s sheet now shares the same retained dictionary; no payment-provider UI transaction was run.
