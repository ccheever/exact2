# Purchases and store review on a device (LLP 1133.015 §10, drive 13)

What the agent cannot drive: StoreKit's and Play's own sheets, their sandbox
and test-card payments, and the stores' rating prompts. Weatherlight+ is the
consumer. Each step names what to see; a step that shows something else is a
finding. A person runs them, never an agent on a phone (LLP 1133.020).

## What the accounts must have first

- **Apple:** an App Store Connect record for `com.exact.weatherlight` with the
  catalogue's products under exactly its ids (`apps/weatherlight/store/catalog.json`):
  `plus.monthly` and `plus.yearly`, auto-renewable subscriptions in one group,
  and `tip.small`, a consumable; the Paid Apps agreement active; a
  development profile for the explicit App ID; a Sandbox Apple Account
  signed in under Settings › Developer › Sandbox Apple Account (LLP 1133.020
  §3 item 5).
- **Google:** Weatherlight on a Play internal track with the same products
  published, the phone's account a license tester, and Weatherlight
  installed from the track.

## iPhone

1. `bun host/apple/build.mjs --ios --device weatherlight-apple --run`. Support:
   both plans at the sandbox's prices and the tip.
2. Buy the yearly plan: StoreKit's sheet, then "Welcome to Weatherlight+" and
   sixteen days. Quit and reopen: Weatherlight+ from the first frame.
3. Watch a sandbox renewal (a yearly plan renews hourly in the sandbox):
   still sixteen days, and the journal's `purchases: finished <id>` for the
   renewal.
4. Leave a tip: "Thank you", one tip counted; again, two.
5. On a second device with the same sandbox account, Restore Purchases:
   Weatherlight+.
6. After the fifth day opened, with a forecast showing: the system's rating
   prompt, once. No button asks for it.

## Android phone

7. Buy with "Test card, always approves": Play's sheet, then sixteen days.
8. With the app closed, buy the monthly plan elsewhere with "Slow test card,
   approves after a few minutes"; reopen after it approves: delivered by the
   loop and acknowledged (the journal), sixteen days.
9. Leave a tip; Play's sheet; one tip, consumed (buyable again).
10. After the fifth day opened: Play's review dialog, or none (Play's quota
    decides).

## The StoreKit configuration suite (owed)

`Weatherlight.storekit` beside this file is the catalogue as a StoreKit
configuration file. `SKTestSession` runs only in a hosted XCTest; on this
Mac's ad hoc signing the StoreKit service refused it (`SKInternalErrorDomain`
4), so the suite of LLP 1133.015 §12 stage 1 waits for a development-signed
test host: products, a purchase, a pending purchase approved, a purchase
outside the app, renewals at `timeRate`, an expiry, a refund before finishing,
and two sessions.
