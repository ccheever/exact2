# usage-pooled-view: live drive record (2026-10-08 KST, 11:27-11:46 UTC+9)

Host: this Mac, screen locked (agent-mode app; no real input). Each session is one launch of the
agent-mode app through `scripts/agent.mjs` `open()` (size 1280x840, `--epoch` 1791426420000 =
2026-10-08T02:27:00Z, the fixture's clock), driven op by op (`target/upv/drive.mjs`, uncommitted).
Pairing links were typed from 0600 files and redacted from every transcript (`token=<redacted>`).

| Item | Value |
| --- | --- |
| Lane T3 servers | staged release `t3` 0.0.46-nightly.20261005.2667; "Build box" pid 23385 on 127.0.0.1:16420, "Studio" pid 23383 (restarted as 43942) on 16410; isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME each; telemetry off |
| Fixture proxies | Bun HTTP+WebSocket proxies 16421 -> 16420 (pid 23386) and 16411 -> 16410 (pid 23384), `target/upv/lane/fixture.mjs` (uncommitted). They overlay each environment's label, the Codex snapshots (Build box: Codex `shared@` 50%/75% used, a second Codex "Work" `work@` 22%/48% used with 1 credit and ChatGPT `externalUsage`, a ready Cursor; Studio: Codex `shared@` 38%/71% used, fresher, 2 credits), Studio's hub sources (`usageLimitSourcesUpdated`, only for a subscription that asked for them: "Team hub" with a Claude account `ops@`, "Old hub" failing "token expired"), the usage summary (codex buckets per day, Build box's Cursor `enableCursorKeychain` source until enabled); they answer `provider.consumeResetCredit` (reset, then a typed error, after 1.5 s), `server.refreshUsageRates` (Studio ok, Build box failure, after 0.8 s; 4 s for the stop test) and the Cursor `server.updateSettings` (scripted: no Keychain read), and log every RPC (`rpc-log.txt`, emails and client ids redacted) |
| AFTER build | session 1: `58eac6668` + `b5c7847be` (`target/clients/5b8c08ec3c851d65b84617c2`); session 2 (the one retry): merge `ba541958d` with the Escape fix |
| BEFORE build | `t3-code-evidence-base` at `757d9517a`, its existing bundle (`target/clients/0d02a3550eba6edce4b12b59`), same lanes, same epoch, sizes and schemes |

## Session 1 (AFTER, Build box focused, Studio background)

- Pairing: wizard "Add a computer" twice; both "Connected". Studio's background transport subscribed
  `subscribeServerConfig {"usageLimitSources":true}` (rpc-log a 02:30:23).
- Limits opened: one `server.getUsageSummary` and one `server.refreshProviders` per environment
  (02:30:50). Read back (`state` usage): environments `Build box`, `Studio` (both checked);
  pools codex: "5h limit" 70% left with segments Codex 62% (Signed in "Build box, Studio") and Work 78%
  (Signed in "Build box"); claudeAgent: Session 36% with the hub account (chip "OL", Via
  "Studio · Team hub"); notices ["Studio · Old hub: token expired"]; links ["ChatGPT usage"]; Cursor
  offer for Build box.
- Hover on segment 0-0-0: popover rows Plan ChatGPT Pro, Signed in Build box, Studio, Left 62%, Resets
  "1:39 PM · in 2h 12m", Restores "+19% of pool", "2 banked · expires in 27d 23h", Use reset.
- Use reset: `pooled.confirm` = "0-0-0", focus on `reset-credit-cancel`. Escape: confirm closed, focus
  back on the segment (view 751), no RPC. Use reset -> Use credit: at +300 ms the segment is busy
  ("Using…"); `provider.consumeResetCredit {"instanceId":"codex"}` went to **Studio** (rpc-log a
  02:31:31, the background environment whose snapshot showed the credits); at +2.5 s the status
  "Reset applied. Your windows have cleared." under the bar (`statuses` row 2).
- Narrow (840x620): legend rows, position numbers; pressing legend row 0-0-1 opened Work's popover
  (end-aligned, below the first card).
- Filter: Studio off -> label "Build box", only Build box's accounts, the hub, its notice and the
  shared account's Studio membership gone; back on -> all. No `refreshProviders` for the toggle
  (inside the five-minute window).
- Refresh (Limits): one `refreshProviders` per environment (02:32:54). Tokens -> Limits inside five
  minutes: none. Agent clock +6 min, Tokens -> Limits: one per environment again (02:33:14) and the
  countdown read "in 2h 6m" (it had stayed "in 2h 12m" while the clock ran).
- Cost: $117.00 from both environments (gpt-5.5 from Studio, gpt-5.5-mini from Build box), Cursor row
  "Cursor · Build box" after Codex. Refresh: `refreshUsageRates` once per environment (02:33:25),
  Studio ok, Build box failed, and both refetched `getUsageSummary` (02:33:26). `refreshing` true at
  +300 ms, false after.
- Cursor Enable (cost row): `server.updateSettings {"patch":{"cursorKeychainUsageEnabled":true}}` on
  Build box only, then rates + provider checks on both (afterPending); the row disappeared.
- Stop mid-refresh: rates delayed 4 s, refresh pressed, Studio's server (pid 23383) stopped 0.8 s later.
  Studio listed disabled "Reconnecting…", no `getUsageSummary` from it after its rates request; Build
  box refetched at 02:33:56; `refreshing` false; total $33.00 (Build box alone). Studio reconnected
  after its server restarted (pid 43942) and pooled again.
- Tab from segment 0-0-0 -> 0-0-1 -> 0-1-0 (hidden popovers are inert). Segment names:
  "Codex: 62% left, resets in 2h 4m, 1 reset credit banked".
- Reduced motion: 375 ms after a hover the popover is opaque under `prefers-reduced-motion: reduce`
  and still fading without it (pixel samples equal to the 875 ms frame vs not).
- Found: Escape on a segment that a mouse click had not pinned went back from the page (see session 2).

## Session 2 (AFTER retry, Studio focused, Build box background; pairing order differed)

- Build box added through Settings > Connections > Add environment (pasted link), both "Connected".
- Escape with a popover pinned by Enter: closed it, focus on the segment, still on Usage (journal
  `press view 2595 (escape#7027)`, `focus("usage-seg-0-0-0")`).
- Found X56: a mouse click on an unfocused segment journals `focus view … (focusChanged)` and no press,
  so it never pinned; Enter presses (`press view 2872 (pin#7027)`), and the LegendRow (no focus
  handler) presses on the first click (`press view 3200 (pin#7027)`). Fixed after the session by
  dropping the segment's focus-handler ring (the host draws the ring, X47 / main #189): the segment now
  presses as the LegendRow does. Not re-driven (session budget).
- Found: the second card's popover opened above and the scroll area clipped its top (16). Fixed after
  the session (`popoverSides`, unit test); not re-driven.

## BEFORE (base)

- Limits: one per-driver list from the focused environment (Build box): "5h limit 50% left", "Weekly
  limit 25% left", plain bars; no Work account, no Studio, no hub account, no notices, no links, no
  Cursor offer. The filter lists "Build box" only. Studio's background transport subscribed
  `subscribeServerConfig {}` (rpc-log a 02:41:14): no hub sources reach it. Opening Limits refreshed
  only the focused environment (rpc-log b 02:41:33; none on Studio).
- Cost: Build box alone ($33.00).
