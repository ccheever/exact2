# Code review: the design tool drops the canvas grant on C2's entry word, round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `af7cd46416`.
- **Method:** one brief (sha256 `39eaafb7ea9ab211ada4a39f4767b0650bb62f6876c40e55ccda858de0bc084b`), the plan inline, shared by both reviewers, blind to the other review, with a security pass; commit af7cd46416 (round 2's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (2 SHOULD-FIX).
- **Disposition:** post-round, in the commit named in the sign-off request: the word counts only from a direct frame of the window it came to (`source.parent` and `source.top` both that window, `event.currentTarget`), so a frame of a popup the canvas opened, or a window gone (whose parent and top read null), is refused (Astra 1, Grok 1); every ask, one for no canvas included, overtakes a mint in flight, so a mint a failed hello overtook is answered without its grant and kept nowhere (Astra 2); tests for a popup's frame, a window gone, and an ask for no canvas. A second direct canvas-origin iframe would pass; the remote view has one, and the door serves the tool with `frame-ancestors 'none'` (both note).

**Nothing blocks this round. Two SHOULD-FIX findings remain at `af7cd46416`.**

1. **SHOULD-FIX — The source check accepts a frame of another window.** [pairing.ts:61](/tmp/rv-dt-t6s4r3/apps/design-tool/pairing.ts:61) establishes that the sender is directly below *some* top-level window. It never establishes that its parent is this tool.

   Concrete scenario: the canvas opens a popup, permitted by [app.contract:1415](/tmp/rv-dt-t6s4r3/apps/design-tool/app.contract:1415). That popup retains a reference to the tool and creates a child frame on the canvas origin. Messages from that child satisfy both comparisons and can acknowledge a later Reload before its grant POST succeeds. The next poll substitutes `/`, potentially aborting entry. An isolated source probe confirmed acceptance of this topology. This permits entry disruption; it neither discloses the grant nor creates a gateway session.

   Bind the parent to the actual receiving window, available as the listener event’s `currentTarget`. If multiple direct frames become possible, also bind the sender to the mounted canvas.

   The other requested cases:
   - A second direct iframe on the canvas origin would pass; the current remote template has only one.
   - A live frame inside the canvas, a popup itself, and a missing source are rejected.
   - The framed app passes the topology check, but after genuine entry `minted.frame` is empty, so subsequent words are ignored.
   - If the tool were framed, its legitimate canvas would be rejected as a grandchild. The remote serving path prevents this with [`frame-ancestors 'none'`](/tmp/rv-dt-t6s4r3/scripts/tamber-remote.mjs:213).
   - Native `WindowProxy.parent` and `.top` are cross-origin readable; I found no standards-based exception path requiring a catch here. Detached windows can return `null` for both, which also passes this guard. The probe confirmed acceptance of that shape, without exercising browser detachment. [HTML Standard](https://html.spec.whatwg.org/multipage/nav-history-apis.html)

2. **SHOULD-FIX — An empty-client ask does not overtake a pending mint.** The early return at [pairing.ts:82](/tmp/rv-dt-t6s4r3/apps/design-tool/pairing.ts:82) occurs before `asks` advances.

   Concrete sequence: start mint A; a concurrent `hello` fails and [clears client and epoch](/tmp/rv-dt-t6s4r3/apps/design-tool/app.ts:617); `loadCanvas` consequently returns an empty canvas; then A succeeds. Its request number still looks current, so it installs `minted` and returns the grant-bearing answer. The source probe reproduced both the returned grant and its subsequent cache hit.

   Although the resource discards that obsolete reply, the [TypeScript conversion cache](/tmp/rv-dt-t6s4r3/host/web-js/ts-data.js:397) receives it first. If hello later recovers with the same key, the tool reuses this unshown grant—even if its 30-second lifetime has elapsed. Make empty/ineligible asks participate in supersession before returning.

The ordinary shared-mint paths checked out in isolated probes: simultaneous same-key asks issue one request; returned errors reach both callers and permit retry; an old epoch completing **after** the new epoch enters returns no grant and leaves the clean root intact. Normal entry clears module memory immediately; contract state, the answer cache, and iframe `src` become grant-free after the next successful poll. The [poll gate](/tmp/rv-dt-t6s4r3/apps/design-tool/app.contract:825) now waits for `canvasAt`, and the counter provides 40 polls per key.

The [tests](/tmp/rv-dt-t6s4r3/scripts/design-tool-scope.test.mjs:149) cover useful source cases, but their synthetic peers have no receiving-window identity. They miss both findings, and the overtaken-mint test still completes the old request before acknowledging entry. Contract polling and budget transitions remain unexercised by these tests.

Read-only review and isolated in-memory probes only. No files changed, builds, or test suites run.
