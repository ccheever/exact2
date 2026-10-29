# Payments by a person (LLP 1133.034 §10 drive 12, §13 question 7)

What the agent cannot drive: a wallet's own sheet authorizing a payment. Each
step names what to run and what to see; a step that shows something else is a
finding. Use the wallets' test environments only: Apple's sandbox test cards
and Google's test card suite. Never a real card.

## What must exist first

None of these exist yet. Each step below names the one it waits for.

- **Apple Pay merchant ID** `merchant.com.exact.photoeditor`, on the team's
  App ID `com.exact.photoeditor` with the Apple Pay capability, and a
  development provisioning profile that carries it. A build signs
  `com.apple.developer.in-app-payments` only with such a profile, or on a
  simulator (`host/apple/build.mjs` `entitlements`).
- **Apple Pay Payment Processing Certificate** for that merchant ID: the key
  that decrypts `paymentData`. The print lab never decrypts (it records the
  token's length), so the processor's account holds it, never this repo.
- **Apple Pay Merchant Identity Certificate** for that merchant ID, for merchant
  validation on the web: `prints.ts --merchant-cert <pem>` posts to Apple's
  sandbox gateway with it. Keep the file outside the repo.
- **A Sandbox Apple Account** (App Store Connect › Users and Access › Sandbox),
  signed in on the test iPhone, with one of Apple's sandbox test cards added in
  Wallet.
- **A Google account in Google Pay's test card suite** (join the
  `googlepay-test-mode-stub-data` group), signed in on the Android phone and
  in the Chrome profile used below. The `TEST` environment needs no merchant
  account; production needs a Google Pay merchant ID and the business console's
  approval.
- **The public origin** of LLP 1133.020 §3.1, registered with Apple Pay for
  Safari (the domain verification file under `/.well-known/`).

Start the print lab without `--agent`, so a substitute's synthetic payment is
refused as a production server would: `bun apps/photo-editor/fixture/prints.ts`.
`GET http://127.0.0.1:4334/fixture/log` shows each checkout, quote and
payment it saw (the token's length, never the token).

## iPhone (needs the merchant ID, a profile, and a Sandbox Apple Account)

1. `bun host/apple/build.mjs --ios --device photo-editor-apple --run`. The
   build log's entitlements include `com.apple.developer.in-app-payments`
   with the merchant ID; without a profile that carries it, Order a Print
   is not shown.
2. Order a Print › 8×10: the price reads $15.00 plus shipping. The Apple Pay
   button reads "Order with Apple Pay" in the system's style.
3. Tap it: Apple's sheet names Photo Editor Prints and $15.00. Choose a
   shipping address in California: the sheet shows Standard $5.00 and
   Express $15.00 and the total $20.00. The lab's log shows a quote with a
   country, region, city and postal code, and no street or name.
4. Choose Express: the total reads $30.00. Pay with Face ID: the sheet shows
   Done, closes, and the app says "Ordered. Your print is on its way."
   Orders lists the 8×10 as paid with its order id. The lab's log shows one
   payment with a non-zero token length and the full address.
5. Start the lab with `--decline-first`; order again and pay: the sheet shows
   the payment failed and closes, and the app says the payment didn't go
   through. Tap the button again and pay: one charge, Orders shows it paid.
6. Start the lab with `--reject-postal`; pay: the sheet stays with "Check the
   ZIP code." under the address. Edit the address to a ZIP ending in 0 and
   pay again: one order.
7. Open the sheet and tap Close in the app behind it (or swipe the print
   sheet down): Apple's sheet goes away and no order is made.

## iOS simulator, real PassKit (needs nothing but Xcode)

The simulator has a simulated Wallet, so the real sheet shows without an
account; its payment's token is empty.

1. Boot a simulator of your own and build with the merchant entitlement:
   `bun host/apple/build.mjs --ios --sim <udid> photo-editor-apple --run`.
2. Run under the agent with the module bound:
   `bun scripts/agent.mjs ios --bind payments state` reads
   `resources.wallet.canMakePayment` from `PKPaymentAuthorizationController`.
3. By hand in the simulator: Order a Print › Pay shows Apple's sheet over the
   app's window with the simulated card. Changing the shipping address
   updates the total from the lab. Pay: the lab records a payment whose
   token length is that of the simulator's empty `paymentData` (the
   substitute's would be refused without `--agent`).

## Mac (needs a profile with the merchant ID)

An ad-hoc Mac build carries no merchant entitlement, so `wallet.available`
reads false and Order a Print is hidden (drive 10). With a profile: the
iPhone's steps 2 to 7, with Touch ID or a paired iPhone or Apple Watch.

## Android phone (needs the Google test card suite account)

1. Build the APK of the conformance fixture (`bun packages/payments/test/android.mjs`
   builds it and prints its path) and install it with `adb install`.
2. Order a print: Google's Pay button. Tap it: Google Pay's sheet in the
   `TEST` environment lists the suite's test cards. Choose an address: the
   lab's quote shows country, region, city and postal code only, and the
   sheet shows Standard and, in the US, Express.
3. Pay: the sheet shows success and closes; the lab records a token whose
   length is a `PAYMENT_GATEWAY` token's for the `example` gateway.
4. With `--decline-first`: the first payment shows Google's error and the
   sheet stays; pay again with another card: one charge.
5. Leave the app's print screen while Google's sheet is up, then pay: Google
   shows an error, the lab records no payment, and the app's journal says the
   request was abandoned (`payment …: authorized after its call ended`).

## Chrome, Google Pay `TEST` (needs the test card suite account in the profile)

1. `bun host/web/dev.mjs --app photo-editor`, open the page in Chrome on
   `http://localhost:<port>`. Payment Request with Google Pay works on
   `localhost` without HTTPS.
2. Order a Print › the button: Chrome's Google Pay sheet in `TEST`. The
   address and option steps update the total from the lab; pay: "Ordered".
3. In Firefox the wallet reads unavailable and Order a Print is hidden.

## Safari, Apple Pay on the web (needs the public origin and the merchant identity certificate)

1. Deploy the photo editor to the public origin, with the lab behind it and
   started with `--merchant-cert <pem>`.
2. Order a Print › Apple Pay: the lab's log shows a merchant session from
   `apple-pay-gateway-cert.apple.com` before the sheet shows; pay with a
   sandbox card: "Ordered".
