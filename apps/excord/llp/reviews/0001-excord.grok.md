# Review: Excord LLP 0001 r1 (grok)

- **Family:** xAI. `grok-4.7` via `grok -p` (grok 1.0.52 (0920c8169e27)), reasoning effort xhigh, sandbox read-only, no subagents; 2026-10-22.
- **Method:** the panel (`skills/panel`), one pass on r1 (then numbered LLP 1141, at `llp/1141-excord.rfc.md` on an uncommitted tree over `c789d84447`). One brief (sha256 `4d209a6a63932e103c72cc516ebf182085a6055ea65d03a3ea9aaa82ba923075`), shared verbatim by every seat, blind to the others, read-only. It asked for fundamental issues only, at most five, and a view with confidence on §4.2 and §2. Muse Wonder Full did not sit ("model does not exist or you lack access"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Disposition:** r2 takes 1 and 2. Finding 1 was checked and holds: the receive queue refuses a frame past 1 MiB whatever the ceiling (`host/apple/src/executor_socket.rs:218`, `host/web/http-body.js:197`), against the reference's wording; r2 speaks `zlib-stream` from stage 1 and files the bound as an ask (§6). Finding 3's rules are taken (the fixture for every automated run, identifies budgeted); its disposable account is overruled by Charlie's ruling for his main account.

---

I'll read the Excord RFC and the Exact rules it depends on, then check the chat and data-module claims against the repo.The RFC is in. Next I’ll check the socket, list, and chat documents it treats as settled.Bluesky already hit feed refreshes. I’ll read that seam and the socket snapshot rules before judging §4.2.## 1. A real login does not fit through the socket Exact has

The gateway is specified as uncompressed JSON, with `zlib-stream` deferred until READY's size "matters on cellular" (§4.1). On every host, a TypeScript socket's receive queue is 256 frames and 1 MiB, and one frame over that ends the stream (`socket receive queue exceeds 256 frames or 1 MiB` in `host/apple/src/executor_socket.rs` and `host/web/http-body.js`). The app can raise `exactIndependentHttp.maxResponseBytes` to 64 MiB; the queue cap stays 1 MiB, so a single accepted frame that large still kills the session. Worker-placed TypeScript sockets are receive-only (LLP 1016.000, as built), so this mapper runs on the main executor, and socket snapshots are never coalesced.

Discord's user gateway delivers READY, then a guild burst, as websocket frames. Uncompressed, one READY or one large `GUILD_CREATE` is often bigger than 1 MiB, and there is no session to resume because identify never finished. Compressed, the same burst still piles into that 1 MiB queue while the mapper is inside `JSON.parse` of the payload it already took. Stage 1 never sees this: the fixture's scripted world is not that burst, and §6 does not list the socket bound. Stage 2 is then the first time the client has to log in, and the session dies on the account §2 is trying to protect.

Speak `zlib-stream` in stage 1, in `gateway.ts` and in the fixture. The mapper turn should only append the frame and return a tiny snapshot; inflate and parse on a later turn so the queue can drain. Add one fixture case that sends a frame larger than 1 MiB and a login burst larger than 1 MiB. The Exact change, if that case ends the stream, is that one frame within the app's own ceiling must be deliverable. That belongs in §6.

## 2. §4.2 should be (b), and the 1 ms test should not be allowed to pick (a)

**Take (b). Confidence 0.86.**

(b) is already the chat design in this repo. LLP 1027.004 D1: the window is the resource, the answer is at most K rows (messages-stress uses 200), and the arguments carry the cursor. The same plan measured a fresh whole-history answer at about 78 ms median for 100,000 rows against about 0.3 ms for a page, on a desktop, with TypeScript crossing and host painting excluded. Reuse helped only when the whole answer was unchanged. A new message changes the root. LLP 1010 §6.2 still validates every supplied key, so virtualization does not make a large answer cheap.

§4.2 has the costs backwards. It says (b) costs a refresh per message and (a) costs nothing extra. A stream snapshot is a full settlement of whatever the mapper returns, once per frame, with no coalescing. (a) pays that on the entire cache, on the same main thread that has to drain the 1 MiB queue above. (b) pays it only for a resource whose arguments changed. Key the open transcript as `(channel, cursor, version)` and return the resident window from module state. A message in another channel does not refresh it. The sidebar is a second, small resource (unread, last preview, typing), not a version map of every channel and not the message bodies.

The proposed measurement cannot rescue (a). One dispatch of a 5,000-message cache on an iPhone 12 is not the guild burst, not the TypeScript crossing, not list-key validation, and not a fling. Stage 1 would lock the choice before the workload that distinguishes them exists.

## 3. §2 does not make a ban unlikely

**The account rules are not sufficient. Confidence 0.84.**

The section's own account of bans is automated behavior: mass messaging, scraping, joining. The rules then allow automated reads, and stage 2's gate is those reads ("read-only drives clean"). An agent that identifies and pages history is the scraping case. A gateway identify is the self-bot whether or not it sends a message. REST `X-RateLimit` handling does not cover identify and session limits, which reconnect loops during bring-up burn through.

"Looks like one official client" is not a safety control. The first live session is the iOS app (stages 2–3) using one pinned desktop `IDENTIFY` and `X-Super-Properties`. Stage 5 already admits the web build cannot do that, because the browser sets `User-Agent`. A desktop build number goes stale on a cadence of days, and sending that client's headers without being that client is the mismatch. A week of clean agent drives is the experiment that spends the second account, and it says nothing about a delayed ban.

Use the fixture for every agent and every test. Open `discord.com` only from a process a person is driving, including reads, on a disposable account. Treat impersonation as best-effort compatibility for that host, not as what keeps the account alive.
