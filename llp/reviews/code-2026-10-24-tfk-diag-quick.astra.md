# Quick review: TableFeedKeyMacTests held-click diagnostics (x2/tfk-diag feb70d7fe3), Astra (gpt-6-astra, max)

**RIGHT WITH NITS.** [TableFeedKeyMacTests.swift:63](/tmp/iosfix-rv-tfk/host/apple/tests/ExactKitTests/TableFeedKeyMacTests.swift:63) matches the conversions and applicable guards; omitting Control is correct. `row(at:)` and `pressing` are reads. Hit-testing may populate text caches, but I found no press/selection mutation or event pumping. The helper runs before mouse-up is posted.

Two diagnostic inaccuracies:

- [Line 71](/tmp/iosfix-rv-tfk/host/apple/tests/ExactKitTests/TableFeedKeyMacTests.swift:71): `pressing == nil` does **not** establish missing cell binding. [TableFeedMac.swift:794](/tmp/iosfix-rv-tfk/host/apple/Sources/ExactKit/Mac/TableFeedMac.swift:794) checks root existence and pressability, not cell attachment.
- [Line 74](/tmp/iosfix-rv-tfk/host/apple/tests/ExactKitTests/TableFeedKeyMacTests.swift:74) and [QUEUE.md:3](/tmp/iosfix-rv-tfk/QUEUE.md:3): guards passing later does not establish another monitor intercepted the event. A guard could have recovered; the helper also recomputes the current midpoint. Say “all guards pass now” and leave the cause open.

The cited log confirms five held-assert failures and no post-release failures. Static review; tests not run.

Both nits taken before landing: the messages and the QUEUE entry leave the cause open when every guard passes, and a nil pressing() says no mounted pressable root.
