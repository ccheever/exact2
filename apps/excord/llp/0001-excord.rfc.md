# Excord LLP 0001: a Discord client that makes the transcript perfect

**Type:** RFC
**Status:** Draft r2, 2026-10-22. The panel sat on r1 (GPT-6 Astra max, Claude Fable 5.1 xhigh, Grok 4.7 xhigh; Muse was unavailable); its reviews are in `reviews/`, and §R records what r2 took. Charlie ruled on the account, the store and the list the same day (§9).
**Systems:** `apps/excord` (new); the data seam's sockets (LLP 1016.000 D7); the virtualized `list` (LLP 1010 §6.2, `scroll-start`, `scrollFollowEnd`, `reachstart`); the composer (`chat-thread`); the hosts in the order iOS, macOS, web, Windows, Android
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-22
**Implementer:** Claude (Opus 5.5) orchestrating; code by GPT-6 Astra and GPT-6.1 Sol in lanes, reviewed by Grok 4.7 and Sol (`skills/orchestrate`). Stage 1 starts 2026-10-22.
**Corpus:** Excord's own, in `apps/excord/llp/`; numbers here are Excord's, and "LLP NNNN" with four digits above 1000 is exact2's.
**Related:** LLP 1140 (how Discord's own Rust clients are built: the comparison Excord invites); LLP 1114 (the Bluesky clone: a real service's client as the source of Exact's asks); `apps/messages`, `apps/messages-stress` (the transcript work this builds on); `docs/recipes/chat-thread.md`

## R. The panel, and what r2 changed

- **The account (§2).** All three seats said r1's rules protect a second
  account, not the main one, and Grok that an agent paging history is the
  scraping case. Charlie ruled to use his main account from
  the start, so §2 now treats every request to real Discord as made on it: no
  automated run touches it, acks and typing count as writes, and each host
  identifies as Discord's own client for that host (Fable). Identifies are
  budgeted, because a reconnect loop spends them (Grok).
- **The store (§4.2).** All three chose (b), a small version stream and window
  resources, without the measurement r1 made a condition: a stream settles every
  snapshot whole, uncoalesced, so (a) pays for the whole cache on every dispatch
  (Grok, from LLP 1027.004 D1's measurements). Charlie agreed.
- **The list (§3.6, §6).** Astra and Fable said a jump needs no discontiguous windows in the
  list: the cache holds the runs, the list shows one, and a jump is a new key set
  placed with `scrollIntoView` (LLP 1070.000). The ask is gone. Charlie agreed.
- **READY's size (§4.1, §6).** A main account's READY, and a large
  `GUILD_CREATE`, can pass 1 MiB uncompressed. Fable proposed raising the
  stream's `exactIndependentHttp.maxResponseBytes`. Grok found that is not enough:
  the receive queue ends the stream on any frame that would take it past 1 MiB,
  whatever the ceiling (`host/apple/src/executor_socket.rs:218`,
  `host/web/http-body.js:197`), though the reference says the bound is "1 MiB plus
  the per-message response ceiling". So the gateway speaks `zlib-stream` from
  stage 1, the fixture sends a frame and a login burst over 1 MiB, and the
  queue's bound is an ask of Exact (§6).
- **Proof on a device (§7).** The agent's clock is virtual, so 120 Hz, the keyboard,
  VoiceOver and Dynamic Type on iOS are judged by hand on a physical iPhone, as
  part of iOS's gate (Astra; Charlie: "we'll need to test manually for some things").
- **The reference (§3).** Discord's iOS app is the reference for the transcript's
  structure and behaviours; the platform wins on controls, motion, keyboard, menus
  and haptics (Fable).
- **Windows and Android (§5).** TypeScript's bidirectional socket is documented on
  the web, macOS, iOS and Linux only. Stage 1 runs a `wss:` echo on every host
  so stages 6 and 7 start from a known port (Fable).

## Summary

Excord is a Discord client in `apps/excord`, built with Exact for iOS, macOS, the
web, Windows and Android, in that order. It talks to the real Discord: the user
gateway over a WebSocket and the REST API the official clients use, both
undocumented for user accounts and so reverse engineered. It reads and writes text
in server channels and DMs. Voice, video and most attachment types are out.

One thing has to be excellent: the messages list and the composer under it, where
a person spends almost all their time in a chat app. §3 states that bar as
properties a drive or a person with a device can check, so "perfect" is something
that can be measured.

The app's second purpose is to push Exact. Where Exact makes the bar hard to meet,
the fix goes into Exact as an issue or an exact2 LLP, not into the app as a
workaround (§6).

## 1. Scope

**In:** sign-in; the server list, a server's channels (text and announcement, with
categories), the DM and group-DM list; reading a channel's history and following it
live; sending, replying, editing and deleting your own messages; reactions; typing
indicators; unread state and mentions; Discord's Markdown, mentions, custom emoji
and timestamps; images (attachments and embeds), shown inline at their declared
size; link embeds as cards; a quick switcher on wide windows.

**Out for v1:** voice and video; uploading anything but images; non-image
attachments beyond a file row that opens the URL; threads and forum channels beyond
a link that opens them read-only; stickers, soundboard, activities, Nitro features;
server settings and moderation; push notifications (a later stage, after LLP 1133.004).

## 2. The account

Excord signs in to Charlie's main account (his ruling, 2026-10-22). Discord's
terms forbid third-party clients and automating a user account, and accounts have
been disabled for it, mostly for automated behaviour: mass messaging, scraping,
joining, request rates no person makes. Losing the account would end the project,
so these rules bind every lane:

- **Nothing automated touches real Discord.** Tests, smokes and agent drives run
  against the fixture (§4.4). The app's Discord origins (REST, the gateway's
  socket and the CDN, each with its own grant) are one build setting, and the
  agent's builds point all three at the fixture; a build pointed at Discord is
  run by Charlie, by hand. The bake rewrites only a `net.fetch` grant from
  `EXACT_BACKEND_ORIGIN` today, so the setting switches REST alone until the
  wiring lane extends it.
- **Passive writes are writes.** Acks (read state), typing, member-list
  subscriptions (op 14) and presence updates are sent only as the official client
  sends them, for what a person is looking at, and never more often.
- **It looks like the official client of its host.** The gateway `IDENTIFY` and the
  REST headers (`User-Agent`, `X-Super-Properties`) match one recent build of
  Discord's iOS app on iOS, its desktop app on macOS and Windows, its web client on
  the web, and its Android app on Android, each pinned in `discord/client.ts`. The
  app makes only the requests that client makes for the same screen, in its order.
- **It respects rate limits.** Every REST call honours the `X-RateLimit-*` buckets
  and a 429's `retry_after`; history loads at most 50 messages a page, one page
  in flight per channel. A dropped connection resumes rather than identifies when
  it can, reconnects back off as the official client's do, and the app counts its
  identifies: past a few in an hour it stops and says so rather than retrying.
- **A new request kind is checked first.** A request the app has never made goes
  to Charlie with what the official client sends for it before it ships.

The token is kept with `secret.keep` and never logged, written to the journal,
or sent anywhere but `discord.com`.

## 3. The bar: what a perfect transcript does

Each property is checked by an `app.test.contract` test on the fixture, where a
clock and a script make it deterministic, and by a filmed drive
(`"screenshot … over 600 every 50"`) where motion is the point. What only real
time and a real device show (frame rate, the keyboard's motion, VoiceOver, Dynamic
Type) Charlie judges on a physical iPhone (§7). For the transcript's structure and
behaviours (grouping, dividers, the jump pill, the reply gesture) the reference is
Discord's own app on the same device and OS version; for controls, motion,
keyboard, menus and haptics it is the platform, as LLP 1115 has it. The composer's
growth and the keyboard follow Messages, where the two are the same kind of thing.

**Opening**

1. A channel opens at its newest message on its first frame: no visible scroll,
   no row resizing after it shows. Images reserve their box from the
   `width`/`height` Discord sends with every attachment and embed image.
2. A channel with unread messages opens at the newest, with a "New" divider at the
   first unread one and a bar above the transcript to jump to it. Tapping it lands
   with the divider near the top of the screen.
3. Returning to a channel restores where its reader left it, to the row and offset,
   if it was read within the session; otherwise property 1 applies.

**Reading back**

4. Scrolling up loads older history before the reader reaches the top (`reachstart`
   with a page of lead), and what is on screen does not move when it lands. A
   reader flinging up through a year of history sees no stall and no jump.
5. Content that changes height above the viewport (an image decoding at a size
   other than declared, an embed arriving, an edit, a deletion, a reaction row
   appearing) never moves what the reader is looking at. The anchor is the first
   visible row and its offset (LLP 1010 §6.2).
6. Jumping to a message far back (a reply's quote, a search result, a pinned
   message) loads the history around it, shows it highlighted, and offers
   "Jump to present", which returns to a live tail. The cache holds history as
   separate runs, so the gap between them is never fetched; the list shows one run
   at a time, and a jump replaces its rows and places the target with
   `scrollIntoView` (LLP 1070.000).

**Following**

7. At the end, a new message follows: the transcript moves by the new row's
   height in the platform's motion, and a burst of messages lands as one move, not
   a stutter. Within one row's height of the end counts as at the end.
8. Away from the end, nothing moves. A pill counts the new messages and jumps to
   the end on tap.
9. Your own message appears the moment you send it, dimmed until Discord confirms
   it. The confirmation (matched by `nonce`) replaces it in place with no flash,
   reorder or height change. A failed send stays, marked, with Retry and Delete.

**The composer**

10. The composer grows with its text up to a limit, then scrolls inside itself. As it
    grows, a transcript at its end stays at its end; one away from the end keeps its
    rows where they are.
11. The keyboard rises and falls with the transcript's last message staying just
    above it. On iOS the keyboard dismisses interactively by dragging the transcript
    down, as in Messages.
12. Sending keeps the keyboard up. The draft survives switching channels and
    relaunching, per channel.
13. On a hardware keyboard: Return sends and Shift-Return breaks the line;
    Up arrow in an empty composer edits your last message; Escape cancels an edit
    or reply, and otherwise jumps to the end and marks the channel read; Page Up
    and Page Down scroll the transcript.

**Feel**

14. Scrolling holds the display's frame rate (120 Hz on ProMotion) with a
    thousand-message window mounted around the reader: no frame over its budget in
    `perf frames` during a fling, on the slowest device in each host's support range.
15. Messages from one author within seven minutes group under one name and avatar,
    as Discord groups them; a day change draws a date divider.
16. Long-pressing a message (right-clicking on the Mac, the web and Windows) opens the
    platform's menu: Reply, React, Copy Text, Copy Link, Edit and Delete for your own.
    Swiping a message left replies on iOS, as in Discord's app. Text is selectable on
    the desktop hosts.
17. VoiceOver reads a message as author, time and text, and moves message by message.
    Dynamic Type reflows every row, and the anchor holds through the reflow.

## 4. Architecture

### 4.1 The client in the data module

The Discord client is TypeScript in `apps/excord/discord/`, imported by `app.ts`.
It is pure where it can be: the protocol logic takes frames and replies and
returns state and the frames to send, so `bun test` exercises it without a
socket.

- `gateway.ts`: the gateway session. `HELLO` (op 10), `IDENTIFY` (2), heartbeats
  (1, ack 11), dispatches (0), `RESUME` (6), `RECONNECT` (7), `INVALID_SESSION` (9).
  JSON encoding over `zlib-stream`, as the official clients connect. Hermes has
  no `DecompressionStream`, so inflating is a pure-JavaScript inflater's
  (`fflate`). The mapper does the least a frame needs, so the receive queue drains
  during a login burst. Stage 1 measures, once and by hand, the READY Charlie's
  account receives: its compressed and inflated size, and its inflate and parse
  time on an iPhone.
- `rest.ts`: `https://discord.com/api/v9`, with the rate-limit buckets of §2.
- `store.ts`: the normalized cache: users, guilds, channels, read states, and per
  channel its history as separate sorted runs (§3.6), each knowing whether it
  reaches the channel's newest.
- `markdown.ts`: Discord's Markdown (bold, italics, underline, strikethrough,
  spoilers, inline code and blocks, quotes, headings, lists, masked links,
  `<@id>`, `<#id>`, `<@&id>`, `<:name:id>`, `<t:unix:style>`) parsed into runs the
  view draws.
- `client.ts`: the pinned client identity of each host (§2).

### 4.2 How the gateway reaches the view

The gateway is a stream resource over a socket (LLP 1016.000 D7). Its mapper
answers heartbeat acks itself (the amendment of 2026-10-16), and a `task every`
of the `HELLO` interval sends the heartbeat. Sends (typing, op 14 member
subscriptions, `RESUME`) are mutations on the socket handle.

The stream's snapshot is small: the connection's state and a version per
channel. Each list the view shows (the servers, a server's channels, the DMs, the
open channel's messages) is its own resource, answered from the store in module
state and taking its version as an argument, so a dispatch refreshes only what
it changed. The gateway stream's arguments never change, because changing them
ends its socket (LLP 1016.000 D2). A mutation (a history page, a send) cannot move
a version inside the stream's snapshot, so it names the resource it changes with
`refreshes`. The messages resource answers one contiguous run around the reader's
anchor, never the channel's whole history.

The alternative, the whole store as the stream's snapshot, settles a value the
size of the cache on every dispatch, on the thread the transcript waits on; its
cost grows with the account's traffic in channels nobody is reading.

### 4.3 Sign-in

Stage 2 signs in by pasting a token, kept with `secret.keep`. Stage 3 adds
Discord's QR sign-in (the remote-auth gateway, `wss://remote-auth-gateway.discord.gg`:
the phone's Discord app scans a code and the desktop gets a token). QR sign-in
needs RSA-OAEP, which the data module's `crypto.subtle` refuses today (reference,
"Web APIs in a data module"); that is an ask (§6). Email and password sign-in is
not built: it meets hCaptcha and MFA flows that a third-party client handles badly.

### 4.4 The fixture

`apps/excord/fixture/` is a Bun server that speaks the same gateway and REST
protocol for a scripted world: three servers, a DM list, channels with ten thousand
messages of real-shaped content (Markdown, images with sizes, embeds, edits,
reactions, replies), and a script that injects traffic on the drive's clock. Every
test and agent drive runs against it; it is what makes §3 checkable and §2's
first rule keepable. Captured real payloads, with names, ids and text replaced,
are its source for shapes.

### 4.5 Why TypeScript and not Rust

Exact's in-repo Rust sources are binary-bound and their streams are listeners only
(LLP 1016.000, "As built"); the socket protocol of D7 is TypeScript's on every host
D7 covers, including the web JS target. TypeScript is also what an app author
outside this repo writes, so the gaps Excord finds are gaps those authors would hit.

## 5. Hosts, in order

1. **iOS** (stages 1–3): one column, a stack: servers and DMs, a channel list, the
   transcript. The bar of §3 is met here first.
2. **macOS** (stage 4): three columns (servers, channels, transcript), a member list
   on wide windows, the hardware-keyboard properties of §3.13, hover actions on a
   message, a quick switcher on ⌘K.
3. **Web** (stage 5): the browser calls Discord directly. Its API reflects any
   `Origin` in `Access-Control-Allow-Origin` and allows `Authorization` and
   `X-Super-Properties` (checked 2026-10-22 against `/api/v9/gateway` from
   `http://127.0.0.1:8767`), so no relay holds the token. The web build identifies
   as Discord's web client, since a browser sets its own `User-Agent`.
4. **Windows** (stage 6), **Android** (stage 7): the same app; each stage's work is
   the host's gaps against §3, filed against the host. TypeScript's bidirectional
   socket is documented on the web, macOS, iOS and Linux only; stage 1's echo
   shows where Windows and Android stand.

## 6. What Excord will ask of Exact

Known before starting:

- **RSA-OAEP** in the data module's `crypto.subtle`, for QR sign-in (§4.3).
- **Interactive keyboard dismissal** on iOS (§3.11), unless the transcript list
  already has it. Stage 1 checks.
- **A frame within the ceiling is deliverable** (§4.1): the socket's receive
  queue refuses any frame that would take it past 1 MiB, though the reference
  documents "1 MiB plus the per-message response ceiling". A 3 MiB READY under a
  4 MiB ceiling should arrive. Stage 1 files it with the fixture's case as its test.
- **A keyed jump to a row never measured** (§3.6): `scrollIntoView` places
  unmeasured rows and corrects them (LLP 1070.000); stage 1 checks it holds when
  the list's whole key set is replaced in the same commit.
- **The socket on Windows and Android** (§5): stage 1 runs a `wss:` echo on each.
- **Inline runs** that are tappable (mentions, links), images inline in text
  (custom emoji, at the line's height), and a spoiler run hidden until tapped.

Each gap found in a stage becomes an issue in `issues/` and, when it changes a
contract, a sub-LLP of exact2's corpus, fixed in Exact with a consumer test there. The app
does not carry workarounds; where it must wait, the stage records the gap and moves
on.

## 7. Stages

Each gate is a count of checkable items; the orchestrator reports N of M.

| Stage | Gate |
|---|---|
| 1. The transcript on the fixture, iOS | §3.1–3.12 and 3.15 as passing tests or filmed drives on the simulator; a `wss:` echo on every host; READY measured once by hand |
| 2. Real Discord, read, iOS | token sign-in; servers, channels and DMs list; history and live messages, by hand on Charlie's account |
| 3. Real Discord, write, iOS | send, reply, edit, delete, react, ack, typing, by hand; QR sign-in if its ask has landed; §3.11, 3.14 and 3.17 judged by Charlie on a physical iPhone |
| 4. macOS | three columns; §3.13, 3.16 and 3.17 |
| 5. Web | §3 on Chrome, Safari and Firefox |
| 6. Windows | §3 on the Windows host |
| 7. Android | §3 on the Android host |

## 8. Process

The panel (`skills/panel`) sat once on r1, for direction. Implementation
runs as orchestrated lanes (`skills/orchestrate`), each in its own worktree: the
fixture and protocol client, the transcript, the composer and keyboard, and the
host gaps of §6. Astra implements the transcript and anything else subtle (it is
the stage's core, so two implementations and a diff are allowed if one stalls);
Sol implements the rest. Grok and Sol review each diff blind before it lands on
main. Excord's tests run with `bun test ./apps/excord` and its contract tests;
it joins the workspace members, not `default-members`, so the gate's time is
unchanged.

## 9. Rulings

**Admitting Excord.** `rules/DEFERRED.md`'s entry of 2026-10-22 records Charlie's
ask as the ruling admitting Excord, with the take waived, as for Exact Live.
Decided at ~85% because he asked for it by name; striking the entry vetoes it.

**The account:** Charlie's main account, from the start ("let's just not fuck it
up"); §2. **The store:** (b), §4.2. **Proof on a device:** by hand, §7 ("we'll
need to test manually for some things"). **The list:** one run at a time, §3.6.
All 2026-10-22.

"The scrolling text field" is the messages list and the composer
together, as §3 has it (Charlie, 2026-10-22). This plan lives in Excord's own
corpus, `apps/excord/llp/` (Charlie, 2026-10-22).
