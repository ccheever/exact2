# Apple Pay reads a dot-decimal amount in the device locale

**Status:** Closed
**Systems:** Apple Pay (`packages/payments/apple/Payments.swift`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A Payment Request amount is a string with a dot as the decimal mark, such as `"10.99"`. On a device whose locale uses a comma, the sheet shows and charges a truncated amount: `"10.99"` becomes 10, and `"0.99"` becomes 0. An empty string or a non-numeric value becomes 0 on every locale, including `en_US`. The person authorizes that sheet.

**Cause.** `decimal` builds `NSDecimalNumber(string:)` with no locale. That scanner uses the device's decimal separator and stops at the first character it does not accept. A result of `notANumber` is replaced with zero. `summary`, `items` and `methods` all display this number.

**Reproduction.** On a German or French locale, present Apple Pay with `{ "currency": "EUR", "value": "10.99" }`. The summary item is 10. The same call with `"value": "0.99"` shows 0. On `en_US`, `"value": "nope"` shows 0.

**Fix.** Parse with the POSIX locale, and accept only the Payment Request spelling `^-?[0-9]+(\.[0-9]+)?$`. Refuse any other spelling. Do not substitute zero.

**Acceptance.** `"10.99"` is 10.99 on `de_DE` and `en_US`. `""`, `"nope"` and `"1.2.3"` fail the sheet with a type error. A pending line item still uses the parsed amount.

**Resolution.** Validate the complete Payment Request decimal spelling, parse with en_US_POSIX, and reject invalid amounts with TypeError. Initial and updated summary items and shipping methods use the same parser.

**Verification after the fix.** The actual parser and summary functions passed a Swift/PassKit harness for en_US and de_DE, 10.99 and 0.99, malformed and empty values, and pending items. All changed Apple payment sources typechecked; no live payment was authorized.
