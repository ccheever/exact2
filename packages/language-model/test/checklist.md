# @exact/language-model on real devices (LLP 1133.016 §10, drive 9)

What the suites cannot check: a real model's replies, its download, and
the platform's settings. Each step names what to do and what must show.
Agents is the app throughout; "On this device" is in New Conversation
(⌘N, or the compose button) and in a conversation's model menu.

## Chrome on a Mac that meets its requirements

1. Build and serve Agents: `bun host/web/build.mjs agents-web`, then
   `bun host/web/dev.mjs --app agents`; open the URL it prints in Chrome.
2. New Conversation: "On this device" reads "Downloaded once, then it runs
   on this device." (or "Private and offline…" when Chrome already has
   the model). Choose it; press **Download On-Device Model**. The line
   counts up "Downloading… N%"; when it ends, the conversation opens.
3. Send "What is a borrow in Rust?": the reply streams in a few words at
   a time. Press Stop mid-reply: the reply keeps its words.
4. The list row's title changes from the first words to a short title.
5. Ask "What did I ask about borrowing?": the reply shows "Searched your
   conversations for “…”" above it only if Chrome's availability reads
   tools (most builds do not; then no line, and the reply answers from
   the conversation alone).

## A Mac with Apple Intelligence on (macOS 26 or later)

1. `bun host/apple/build.mjs agents-apple --run`.
2. New Conversation: "On this device" reads "Private and offline: nothing
   leaves this device." Choose it and press Start.
3. Steps 3 to 5 above; step 5's line shows, since Apple's model runs
   tools.
4. Turn Apple Intelligence off in System Settings and return to Agents:
   "On this device" is greyed and reads "Turn on Apple Intelligence in
   Settings to use this model." Turn it back on: it is offered again.

## An iPhone with Apple Intelligence on (iOS 26 or later)

1. `bun host/apple/build.mjs --ios agents-apple --device --run` (a
   connected iPhone, signed with the team profile on this Mac).
2. The Mac's steps 2 to 4, with Settings › Apple Intelligence & Siri.
3. On an iPhone below iOS 26 the app opens and "On this device" is not
   offered.

## A Pixel 9 or later (Gemini Nano through AICore)

Agents has no Android build yet (stage 4). With it:

1. New Conversation › On this device; Download if offered; Start.
2. A reply streams. Press the Home button mid-reply and return: the
   reply has ended with the text it had.
3. No "Searched your conversations" line ever shows: Android runs no
   tools.
