**Events, the rest** (the minimal set landed 2026-08-30: `hover`, `focus`,
`blur`, `key` on web, macOS, iOS — LLP 1005 §3). Left out on purpose: pointer
coordinates and moves (a drag), `keyup`, double-click, wheel offsets reaching
the runner (the windowed-List line below). The Linux carrier has `tap … hover`
and `type … key` now (2026-09-23); `smoke.mjs` step 4a still skips them on Linux;
`key` inside a text field on macOS/iOS sees editing commands only (LLP 1008
§5's declared deviation); and an action cannot branch on a key or hover
payload — actions have assignment and command statements, no `if` and no
string `match` — so `key=` today can only record the key.

*Filed under “Next, in order (2026-08-29)”, item 2.*
