**Apple host** (LLP 1008 §7): images; toggles; accessibility beyond `testId`,
`accessibilityLabel` and macOS's button/link/image/heading roles (iOS calls a
link a button and has no image or header trait); justified text; rubber-banding on inner
scroll nodes; a generated header. **iOS** (LLP 1008 §9): the agent API on a
phone (`build.mjs --device --run` installs over USB or Wi-Fi, but nothing drives
the app there); a
synthesized touch for the agent's `tap` (UIKit's hit-test and the responder-chain
rule today); a pan chaining out of a nested scroll view at its edge (UIKit's own;
the agent's wheel chains).

*Filed under “Declared gaps, by system (each spec's "Not in v1")”.*
