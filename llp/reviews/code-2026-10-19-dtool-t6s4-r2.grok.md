# Code review: the design tool drops the canvas grant on C2's entry word, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI (--max-turns 400 --no-plan), reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `28db96bfb5`.
- **Method:** one brief (sha256 `fdbc5d71d4f2844e3f2ddc9ff38bf7ba4ff10fbb44a9dd4235f02e5b902b9cf8`), the plan inline, shared by both reviewers, blind to the other review, with a security pass; commit 28db96bfb5 (round 1's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (1 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in `927178760a`: the source check no longer needs the DOM (hidden from the data module in the shipped build): the word counts only from a direct frame of this page (`source.parent === source.top`, `source !== source.top`), so a window the canvas opened (its own top) or a frame inside the canvas is refused, and no source is refused (Astra 1, Grok 2); one mint a key, a second ask sharing the first, and a mint a newer ask overtook answered without its grant and kept nowhere (Astra 2, 3; Grok 1); the poll waits for the canvas to settle before it spends the key's budget (Astra 4); the listener exported for tests, which now give it real sources (a popup, a nested frame, none).

Nothing blocks. The origin check is exact, a different window is rejected while the canvas iframe is in the document, and the word does not hand the grant back to the sender. One correctness hole is worth fixing.

## Security

The listener is `apps/design-tool/pairing.ts:51-57`. It accepts `{type:'exact-canvas-entered'}` only when `minted.frame` is still set and `event.origin` is `minted.origin` (the gateway origin stored from `made.root`: scheme, host, and port, compared with `!==`). The browser sets `origin`. A popup, nested frame, other tab, or opener on any other origin, including the tool’s own and `"null"`, is ignored.

While `iframe[data-testid="canvas-frame"]` is in the document (`app.contract:1414-1415`, one branch at a time), `event.source` must be that element’s `contentWindow`.

- A popup the framed page opened, and an opener on the canvas origin, have a different `source`. They are rejected for the whole time the entry page is showing, which is when a forged word would swap `src` to `/` and drop the POST.
- A nested frame on that origin is a different window from the iframe’s own window, so `top.postMessage` does not match.
- Another tab on the canvas origin has no handle to the tool window, so it cannot deliver the event.
- The entry page and, after the 303, the app share that iframe window. C2 posts only after the grant POST returns ok (`ENTRY_SCRIPT` in `7c9890c652`). A later word from the app finds `minted.frame` already cleared and is ignored.

On acceptance the listener stores the canvas key and sets `minted.frame` to `''` (`pairing.ts:56-57`). `entered` is `JSON.stringify([client, epoch, tick])`, not a URL. `root` and `origin` have no fragment. The contract copy in `canvasAt.frame` and the iframe `src` stay until the next `loadCanvas`, which is the poll, and that returns `minted.root`. The POST has already burned the grant.

The not-found branch is finding 2.

## Findings

1. **SHOULD-FIX** — A new canvas key whose previous key had entered mints two grants. `app.contract:691` passes `canvasIn.entered` into `loadCanvas`, and `pairing.ts:77-85` mints whenever `minted.key` is not that key yet. `canvasEntered` is async, so a Reload, a server-run change, or Continue while the previous key is entered starts `loadCanvas` for the new key while `canvasIn` still shows the previous `true`. That call is in flight when the new answer `false` arrives; the changed argument starts a second call for the same key. Both set `asked` to that key, so the “overtaken” guard keeps both, and both reach `canvas`. The runtime frames only the later reply (`host/web-js/rt.js:521`). If the earlier reply lands last, `pairing.ts:84` writes that unspent grant back into `minted.frame` even after the word cleared it. The following poll still returns the gateway root, because `entered` matches the key. The cost is a second live grant on every such open: three Reloads after a successful entry spend the six-a-minute mint budget (`scripts/tamber-canvas.mjs`, `MINT_RATE`). Join an in-flight mint for the same key to the call already running, and ignore a completion that is no longer the latest write. The new test overlaps two ticks, so it never starts two calls for one key.

2. **NIT** — If the canvas iframe is not in the document, any window on the minted origin is accepted. `pairing.ts:54-55` returns only when `shown` is found and `source` differs. During `pending(canvasAt)` the iframe is unmounted (`app.contract:1414`) while `minted.frame` can still be the previous grant. A popup that kept the tool window can post the word for that whole mint and mark the previous key entered. The in-flight mint then overwrites `minted` with the new key, so the canvas that gets shown is not acknowledged by that word. Reject when the element is missing. `scripts/design-tool-scope.test.mjs:156` and `:180` dispatch the word with no `source` and no iframe, so they pass only because of this branch and would still pass if the source check were removed.
