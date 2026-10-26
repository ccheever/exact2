# Quick review: deep-link slow lane + QUEUE watch entries, Astra (gpt-6-astra, max)

- **fa374a3b5d — RIGHT.** [QUEUE.md:3](/tmp/iosfix-rv-dl/QUEUE.md:3) correctly identifies the assertion at `press.test.mjs:550` and the box’s hold/fade behavior. The load measurements cannot be verified from code; the proposed cause is appropriately tentative. [QUEUE.md:4](/tmp/iosfix-rv-dl/QUEUE.md:4) matches both stylesheets, `inked`/`refresh`, and `symbol_marks`. Commit `317ddfea6a` made the two selectors identical, removing the marker’s rendering effect.

- **39436c5431 — NIT.** The comments at [deep-link.test.mjs:157](/tmp/iosfix-rv-dl/host/web/deep-link.test.mjs:157) need correction: wasm stream waits now allow 120 seconds, and “each edit … rebuilds those crates” overstates it—Contract edits use the resident producer; the explicit `r` triggers the wasm rebuild. Otherwise, no functional regression found: the warm-up has owned-process cleanup, holds producer replies, and leaves no admitted state carried into the next process. Its build outputs do not change the assertions. The abort race preserves `signal.reason` and handles losing rejections; status fetches are bounded. JS success behavior and assertions remain unchanged, with tighter failure handling.

Checked the race in memory under Bun 1.4.2: pending reads, prior aborts, cleanup aborts, and late reader rejections passed without unhandled rejections. No files changed; full integration/load tests were not run.

Taken: the comment's wait bounds and the rebuild wording.
