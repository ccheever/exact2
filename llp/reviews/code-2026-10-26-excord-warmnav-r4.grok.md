# Code review: Excord warm address-following, round 4, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `394e2889ef`.
- **Method:** one brief (sha256 `13bb847056b289c8ee5bc27e823f50003fb2aa1b20dfae8d5e5fe61e2c7d1ace`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check the warm-navigation delta at HEAD against `ac29430998` and verify each round-3 claim from the code, without changing the worktree.HEAD is `394e2889ef`. I'll pull the delta against `ac29430998` and check each claimed fix in the current files.The delta is four files. I'll read the routing and app contracts around the claimed fixes and check the new tests against them.The six round-3 claims match the code for the cases they name. Two of the new `stayOpen` call sites still treat `open` as the visit on screen after the list has replaced it.

**MAJOR** — `apps/excord/routing.contract:83`. On the narrow list, after Back or `/`, `open` is still the channel just left (`routing.e2e.test.ts:296` expects `open` to be general while `reading` is false; `open = ""` happens only in `toSignIn`, `app.contract:535`). `/nope` takes `stayOpen` (`routing.contract:82-84`). `stayOpen` (`68-72`) does `go(nav, path("channel", open))`, and `go` pushes when no stacked entry has that canonical URL (`route/src/router.rs:228-242`). The channel visit was popped on the way to the list (`routing.contract:91-94`), so the unknown address pushes it again and `viewChannel` rejoins it. The same `open` is left set when a wide `pickServer` sends the stack home (`app.contract:507-512`). The in-flight case is fixed: `/c/B` has already pushed B on top of A, `stayOpen` truncates back to A, and `routing.e2e.test.ts:237-249` checks the URL is `/c/<general>` and that help is never opened or kept. That test never leaves the channel first.

**MINOR** — `apps/excord/routing.contract:138`. A percent-encoded address of the channel `open` still names, typed after that visit has been popped, pushes the encoded entry (`askRoute`’s `go`; canonical URLs keep `%` escapes, `route/src/location.rs:7-8` and `102-112`). `takeDecoded` then `stayOpen`s, and `go` does not find `/c/<id>` on the stack, so it pushes a second visit. Back stops on the encoded entry (`askBack` sees the decoded id equal to `open`, `routing.contract:112-114`). While the canonical visit is still underneath, `go` truncates to it and the duplicate is dropped. That is the case the claim describes.

Checked, and holding:

- Picture: `openWant` clears `viewing` after `openChannel` (`routing.contract:156-158`). A channel address never calls `leaving` (`90-100`), and `openChannel` does not clear `viewing` (`app.contract:443-496`). `covered` follows `viewing` (`app.contract:197`). `routing.e2e.test.ts:272-276` expects help open, `viewing` empty, `covered` false. Removing that assignment leaves the picture up. Not re-run here.
- Beneath title: `openChannel` stores `# ${name}` or the conversation name in `visitNames` (`app.contract:494`); the covered header reads `draftOf(visitNames, …)` (`1220`).
- Sign-out: `owned` ignores `signedOutNow` (`routing.contract:37-38`); `ask` and `supersede` roll the server back on `owned` (`47-48`, `61-62`). Automatic sign-out reaches `supersede` through `toSignIn` (`app.contract:339-340`, `531-532`) only once `signedOutNow` is already true.
- Guild hold: `reset` without `keep` clears `holdGuildsMs` (`fixture/server.ts:778`); an IDENTIFY calls `reset(socket)` (`547`) and `login` still reads the hold (`470-471`). The control endpoint calls `reset()` (`937`).
- NITs: beneath-visit comment (`app.contract:1215-1216`); the unused `menu` helper is gone; the covered header prints `#` because that string is what `visitNames` stores (`494`, `1220`), same rule as the top header (`1226`).

REQUEST CHANGES
