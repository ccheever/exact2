surfaces: web, macos, linux, ios
agent: Claude Code, Claude Opus 5.5
task: add a checkout that pays with the device's wallet

## Rough

- A failed send runs no `then` (docs/reference.md), so Order a Print's `paid` action never saw a failed payment: its `failure(ordered)` branch was dead code and the screen said nothing when the send failed. The failure is now a `derive` read in the view (`payFailure`), as the camera's `shotError` already was. Found only because the web suite's fixture read the same branch and always showed an empty line; about half an hour. A build warning for a `then` that reads `failure()` of its own mutation would have caught it.
- On the web, after leaving the print sheet while a payment was authorizing, the next payment's result reached the screen 20 to 40 seconds late. The runtime's payment sheet retired a closed sheet's call without ending it, so that call stayed in flight and every `clock settle` waited out its limit. Fixed in `host/web-js/modules.js`. About half an hour in this session, after an earlier session's unmeasured time; the journal line `reply 15; wall 41154 ms` pointed at the wait.
- On Linux the wallet read available and Order a Print showed: the Linux host told the runner its target only when it composed a package, so the substitute answered as macOS, the machine the binary was built on. Fixed in `host/linux/src/presenter/requests.rs`.
- On the web a sheet presented right after another was dismissed slides in on the agent's clock, and `clock settle` does not wait for it, so a tap in it fails "outside the viewport"; the drive steps the clock by 500 ms and retries (QUEUE).
- The macOS build could not sign with the team identity (`errSecInternalComponent`: the keychain was not reachable from this session); `EXACT_IDENTITY=-` signs it ad hoc, which the agent's drives accept.

## Lean in

The runner's substitute sheet made every payment path drivable with no wallet: `type @t {"shippingAddress": …}` and `tap @t authorize` run the app's own handlers against the print lab, so the same drive script covered the web, macOS and Linux, and the fault switches (`--decline-first`, `--lose-answer`, `--slow`) reached paths a real wallet rarely shows.

## Needed

- payments (Apple Pay, Google Pay, Payment Request on the web): provided by `@exact/payments`, rough (see Rough); real wallets not tried (no accounts yet)
- image rendering for the print: provided (`transformImage`)

## Checkpoints

- web build: smooth
- macOS build: rough; ad hoc signing needed
- iOS simulator build: rough; the simulator service hung once, and the next try passed
- editing app.contract: smooth
- driving the app: rough; see Rough
