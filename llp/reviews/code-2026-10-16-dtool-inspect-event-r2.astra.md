# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `d382797d3`.
- **Method:** one brief (sha256 `03ad137b98f2a644cfae8ff9eefec5c5b04cfd33c874318be995a212b6771d94`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken, but Grok's first low in part.
  - A selection report names the inspect command it was made under. One from a command since replaced (Stop then Inspect, another client taking over) is refused, 409, and not published.
  - An off event says who took the builds (`to`), or null where inspect mode ended. A selection goes stale on every real end, `to: null`, whatever comes after it in the same answer, and on this tool's own loss. A takeover leaves other tools' selections alone.
  - A rename is another client: the tool reads what it inspects again from the records.
  - The first inspector test waits for the event.
  - The hello schema in docs/tamber-protocol.md lists `inspector`.
  - Tests: tamber-builds.test (`to` on each off), tamber-inspect.test (a replaced command's selection refused), contract (a rename, the lapse with the record).
  - In part (Grok low 1): after a missed poll, a build this tool owns that is away shows only when it registers again (hello lists builds with a stream). Until then Stop is not offered for it. Its ownership still ends when this tool's presence lapses.

---

- **Medium — A queued selection can still revive an ended inspect session.** [scripts/tamber-builds.mjs:280](/tmp/rv-dt-ievent-r2/scripts/tamber-builds.mjs:280). A click queued under command N arrives after Stop followed by Inspect, or after another client takes ownership. `inspectors.has()` passes, so the server publishes it and the tool clears its stale flag. The page’s sequence guard runs before queuing; non-null reports carry no command sequence. Include that sequence and reject reports from replaced commands. Test delayed delivery across both takeover and off/on.

- **Medium — Poll batching hides real stops from other clients.** [apps/design-tool/app.ts:1158](/tmp/rv-dt-ievent-r2/apps/design-tool/app.ts:1158). An agent selects a node, stops inspecting, then starts again before an observing tool’s next poll. The later `on` removes the build from `offNow`, leaving the old selection editable without another selection. A poll response is not an ownership transaction. Distinguish handovers from actual stops on the wire and latch staleness for actual stops, even when an `on` follows in the same response.

- **Medium — Renaming the client can leave Inspect permanently stuck on.** [apps/design-tool/app.ts:1157](/tmp/rv-dt-ievent-r2/apps/design-tool/app.ts:1157). Ada enables Inspect without pins, then saves the name Grace. `inspectOn` retains Ada’s builds; neither the client change nor Ada’s eventual off event removes them, because that event now belongs to another client. “Stop inspecting” and Outline remain enabled. Further Stop requests publish nothing once ownership is already gone, and replies no longer clear the set. Recompute ownership when the client changes, using the server’s records; test rename followed by lapse and Stop.

- **Low — The existing inspector test still assumes replies update the toggle.** [apps/design-tool/app.test.contract:789](/tmp/rv-dt-ievent-r2/apps/design-tool/app.test.contract:789). After Inspect it uses only `clock data` before expecting “Stop inspecting.” The stub answers polls immediately, and the next poll requires the `after(1)` timer; `clock data` does not fire timers. With reply-driven ownership removed, this assertion observes “Inspect.” Add `clock +1` followed by `clock data`, as the new test does, and correct the preceding comment.

The publisher covers all listed ownership mutations, suppresses unchanged ownership and refresh events, and publishes losses before acquisitions. Replaced page exits are now suppressed correctly. The page consumes the new 409 without retrying or blocking subsequent reports. The 1.5 bump is consistent; remaining 1.4 references are historical.

Tests were read, not run, per the read-only restriction. Ownership resync/new-epoch behavior, replacement/eviction publication, and Outline transitions still lack direct assertions.

**Verdict: request changes — three Medium findings and one Low finding remain.**