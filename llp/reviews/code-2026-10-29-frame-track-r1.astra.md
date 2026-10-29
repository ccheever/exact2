# Code review: the Apple agent's bounded scroll track, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `76d3b748c2`.
- **Method:** one brief (sha256 `4811315a0eefd110ffdb86b7d1664b6f7078ca90342432c3ed88d25676e0619c`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

No files edited. Driver tests: 23 passed; one failed because the sandbox forbids temporary files. Native execution was blocked, and build-info testing timed out.

VERDICT: CHANGES

1. **BLOCKER — Track requests still drain rendering work.** [Agent.swift:160](host/apple/Sources/ExactKit/Agent.swift:160)  
   `boundedFrameRead` recognizes report, health and capture, but omits track. Every carrier-level track request therefore calls `ExactSession.drainAll()`, which can block main on the owner and apply pending work. This violates measurement’s scheduling constraint. Add track to this exemption and test through the outer dispatcher; the new Swift tests bypass it.

2. **MAJOR — The documented CLI commands crash during text rendering.** [agent-inspect.mjs:653](scripts/agent-inspect.mjs:653)  
   `renderPerf` does not recognize `exact.frame-health.track/1`. Successful start/read/stop responses reach `r.sites.some` with no `sites`, throwing and terminating a normal CLI drive. Reproduced with the new start response. Add rendering for track responses, including refusals and expiry.

3. **MAJOR — Stop silently loses the final rows.** [agent-inspect.mjs:513](scripts/agent-inspect.mjs:513)  
   Scroll callbacks can append rows between `readTrack` finishing and the stop request. The host returns those rows in its stop response and releases the ring, but the driver discards that response. Reproduced with one intervening row. Consume the final response and handle any remaining-page indication.

4. **MAJOR — CLI reads neither preserve their cursor nor reliably report initial loss.** [agent-inspect.mjs:512](scripts/agent-inspect.mjs:512)  
   Every command starts `readTrack(s)` without a cursor. Consecutive reads repeat retained rows, and stop repeats them again. Moreover, the host interprets an omitted cursor as the oldest retained row: after 5,000 writes, the first CLI read silently omits 904 rows while reporting `dropped: 0`. Start at cursor `"0"` and retain the returned cursor per session/track, resetting after successful replacement.

5. **MAJOR — Invalid node IDs can terminate the host.** [AgentScrollProbe.swift:13](host/apple/Sources/ExactKit/AgentScrollProbe.swift:13)  
   `UInt32(id)` traps for a negative ID or an integer above `UInt32.max`. A raw track-start request can reach this conversion before lease validation or capture gating. Validate with an exact conversion and return a refusal; malformed input must not crash the application or destroy an existing track.

6. **MINOR — Explicit session teardown retains the ring.** [FrameHealth.swift:156](host/apple/Sources/ExactKit/FrameHealth.swift:156)  
   `destroy()` never clears `track`. A retained, destroyed session therefore retains its 64 KB ring until deallocation; subsequent agent requests are refused before they can expire it. Clear the track during destruction and test while retaining the destroyed session. Reload and clock-change paths do clear it.

7. **MINOR — Horizontal Mac tracks report the wrong offset.** [FrameHealthTrack.swift:132](host/apple/Sources/ExactKit/FrameHealthTrack.swift:132)  
   The Mac feed always records `clip.origin.y` and checks vertical bounds. A horizontal-only scroller produces constant offsets and incorrect bounce classification, contrary to the documented scrolling-axis behavior. Select the axis as the iOS probe does and cover horizontal wheel movement.

8. **MINOR — Mac resizing records rows without scrolling.** [NodeViewMac.swift:671](host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:671)  
   The existing bounds-change notification also fires for size changes. The new feed records every notification without checking whether the origin moved, producing rows—and potentially apparent missed intervals—while the scroller remains at rest. Filter unchanged origins and add a resize-at-rest test.

9. **MINOR — Large finite offsets can crash page encoding.** [FrameHealthTrack.swift:60](host/apple/Sources/ExactKit/FrameHealthTrack.swift:60)  
   Integer conversion is unchecked. An offset such as ±`1e17` remains finite in the stored `Float`, but converting its centipoints to `Int` traps. Validate the representable range before conversion and test both signs near the boundary. The current page-size test exercises only offsets 0–4999.
