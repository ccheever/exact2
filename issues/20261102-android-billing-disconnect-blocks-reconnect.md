# Android purchases wait indefinitely after Billing disconnects

**Status:** Closed
**Severity:** P2
**Systems:** packages/purchases, Android, Play Billing
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `packages/purchases/android/Purchases.kt:199` sets `ready=false` on a Billing disconnect. Every later call then enters `afterConnect` at line 204, which queues its body. Nothing makes a Billing call or starts another connection to drain it. A failed initial setup leaves later callers in the same state.

The client enables automatic reconnection, but [Google's integration guide](https://developer.android.com/google/play/billing/integrate#automatically_re-establish_a_connection) says a Billing API call triggers reconnection while disconnected. The queue prevents that call. The additional ready guard at lines 206-208 would also block it.

To reproduce, complete setup, deliver `onBillingServiceDisconnected()`, then request `listPurchases`, `getDetails`, or `purchasesStatus`. Each body remains queued; entitlement and catalog reads can stay pending until the client is destroyed. Alternatively, finish initial setup with an error, then make another request.

Validation was source-level control-flow review against Google's API contract. No Android build, emulator, or real purchase was run on the local Mac.

Distinguish initial setup pending from failed/disconnected setup, and allow operations to reach the auto-reconnecting client. Settle callers after setup failure. Test setup failure and disconnect/recovery separately.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

**Resolution.** Separate initial connection setup from later disconnections. Calls after setup reach BillingClient, allowing its enabled automatic reconnection to run; stale setup callbacks cannot alter a replacement client. Purchase status reads the client’s current readiness after automatic reconnection.

**Verification after the fix.** Compiled the actual package against Play Billing 9.1.0 and the Android SDK. A Kotlin harness of its connection and status functions passed initial setup failure, disconnect followed by an auto-reconnecting call, status after reconnection, and a stale setup callback after client replacement. No Play Store device was available for a live disconnect/reconnect exercise.
