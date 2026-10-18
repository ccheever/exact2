# Code review: stage 4 commit 5: the build channel, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD (the change commit).
- **Method:** one brief (sha256 `aadc29d61ab781fee9b7dab74cbdb4f534f4d08c3fbcc0bf027ce710759daf68`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (polled streams unswept; the relay stopping on a non-array body; Apple's plan digest; pins on refused modes, late iOS sessions and the web root font size; plan digests in web and JS status).
- **Disposition:** applied in the first-round commit, with theme2's rulings.

---

There are blocking issues.

[P1] Sweep polled build streams on a timer — scripts/tamber-builds.mjs:79

`poll` is the only walk that drops a stream whose `stale` check is older than 30 seconds (`openPolled` records that at line 74). `GET /__dev/hello` mints a stream on every request (`host/web/dev.mjs:1153`, `host/web-js/dev.mjs:240`) and nothing else ever expires them. A hello that is never polled keeps its build and its command queue for the life of the process. The gate compares `Origin` only on POST (`scripts/install-page.mjs:288`), so another origin can issue these GETs and grow the map with no cap. The same path keeps a device's main-loop stream after `--serve-as` stops polling: `relayChannel`'s `close` only clears `live` (`scripts/tamber-builds.mjs:171`). The protocol says a nonce unpolled for 30 seconds is forgotten (`docs/tamber-protocol.md:448`).

[P1] Keep the serve-as relay polling when the commands body is not a frame list — scripts/tamber-builds.mjs:165

`if (!Array.isArray(frames)) return` leaves the poll loop. A restarted main loop answers `GET /__dev/commands` with 403 `{message}`, which parses as an object. The `catch` under that line runs only when `fetch` throws, which is what the comment calls a restart. The device's SSE stays open and later pins are never written onto it until that device connects again.

[P1] Report Apple's plan digest for the generation the hello is about to apply — host/apple/Sources/ExactKit/PlanURL.swift:848

A hello with a new nonce calls `channel`, which registers and posts `status` immediately (`BuildChannel.swift:75`). `report` stamps `plan` from `currentGeneration()` (`PlanURL.swift:917`), and `connect` supplies `devGeneration` (`Session.swift:237`), which is still nil on first launch. That hello is also the generation announcement, and `subscribe` runs before the payloads are fetched (`PlanURL.swift:650`). After the generation applies, `liveBooted` (`LiveDesign.swift:31`) posts status only when a mode pin is held or the design string changes. A later generation push carries no hello, so the listed plan stays the previous digest. The protocol requires `status` on connect and on any change to carry the plan digest (`docs/tamber-protocol.md:453`).

[P2] Stop the serve-as relay when the device leaves during hello — host/web/dev.mjs:1215

`req.on('close', () => opened?.close())` is attached after `await relayChannel(...)`. If the device disconnects during that hello (up to 5 seconds), `close` has already fired and the listener never runs. `live` stays true, the relay keeps polling, and that nonce stays inside the 30 second window.

[P2] Refuse a pin's mode without leaving the previous pin's mode under the new facts — host/linux/src/presenter/build_channel.rs:81

`apply_pin` writes scheme, contrast, and root font size, then `check_modes`. On failure it skips `set_modes`, so the runner keeps the previous pin's mode while scheme and font already match the new pin. Status reports that mix. The same order is in `BuildChannel.swift:138` (`pinned` is stored before `pinModes` returns at line 159) and in `host/web/dev.js:312` and the JS page's `pin` (`host/web-js/dev.mjs:210`). The Linux test refuses `grey` only after a null pin has restored the default (`presenter/build_channel.rs:258`).

[P2] Apply a held pin's scheme on an iOS session that boots later — host/apple/Sources/ExactKit/BuildChannel.swift:166

`repinBooted` reapplies modes only. `pin` sets scheme and contrast as trait overrides on scenes that already exist (`BuildChannel.swift:140`, `AgentIOS.swift:1199`). A session that boots afterward reads scheme from that new scene (`Session.swift:1297`) and never receives `systemScheme` or `systemContrast`. Font does follow: `tellPage` pushes `PageFacts.rootFontSize` (`Session.swift:1313`). The contrast preference bit follows `agentContrast`. macOS scheme is process-global. The commit says a session that boots under a pin takes the pin again.

[P2] Pin the web root font size the way prefer does — host/web/dev.js:313

`prefer` sets `style.fontSize` and `--exact-host-font-size` (`host/web/navigation.js:860`). A percentage root size is `calc(var(--exact-host-font-size) * ratio)` (`navigation.js:875`), and the page fact is read with the app's `#exact-root-font-size` rule disabled (`navigation.js:916`). The pin sets `element.style.fontSize` only, then calls `exact_set_root_font_size` with `getComputedStyle` (`dev.js:314`), which includes that rule. An app root size masks the pin, and a percentage root size stays on the old host size. The protocol says the page sets the root font size as `prefer` does.

[P2] Send the running plan digest on web and JS status — host/web/dev.js:306

`status` leaves out `plan`. Registration sends `message.generation` only when the nonce changes (`dev.js:352`). A later generation is `push(announcement())` (`host/web/dev.mjs:420`) with no hello, so the server keeps the first hello's digest, or null when that hello was `{ready:false}`. The JS page registers `plan: null, design: null` and its status leaves both out (`host/web-js/dev.mjs:209`). Linux reports the new plan when a generation lands (`presenter/delivery.rs:42`).

Command frames are dispatched before generation handling on the web page, the JS page, Apple (`PlanURL.swift:847` returns), and Linux. A hello is both the registration and the generation announcement, which is how that frame is specified. Cross-site POST is refused by the existing gate. The new routes sit behind that same gate, including `--lan`, and they are outside the loopback-only installer path so a device can reach them. The nonce is not practically guessable. The lifetime problem is the one filed above.

A client's last named subscription clears its pins (`tamber-server.mjs:334`). Re-register replays a pin. A second stream that registers the same id takes it, and the old stream is detached. Pins for an id with no stream stay so a later register can replay them, and `clear` still drops them. Register is queued ahead of status on Apple, awaited ahead of status on the web and JS pages, and sent ahead of queued reports on Linux. An Apple report already in flight at reconnect still carries the old nonce. The server answers 403 or 409, and the new register follows on the main queue (`BoundedFetch` uses `delegateQueue: .main`).

Linux network work runs on the poller thread. The presenter applies frames. Apple's channel session uses the main queue, and `repinBooted` hops to main. Report bodies stop at 64 KiB. The tests match the claims in the commit message. They do not cover a 30 second sweep, the relay exiting on a non-array body, a refused mode after a pin that applied, Apple applying a pin, or a session booting under one. D4's journal sentence is absent from this commit's protocol section, which treats write scope as a subscription that names the client. `pin` is gated that way and writes no journal line. Touched sources stay within the 1,500 line cap. The largest is `Session.swift` at 1,499. That count is from the files themselves. `bun scripts/caps.mjs` was not run.
