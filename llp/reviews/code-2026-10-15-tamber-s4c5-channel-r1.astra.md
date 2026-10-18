# Code review: stage 4 commit 5: the build channel, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD (the change commit).
- **Method:** one brief (sha256 `aadc29d61ab781fee9b7dab74cbdb4f534f4d08c3fbcc0bf027ce710759daf68`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (production builds reaching the channel; polled streams never expiring; a cleared pin surviving reconnection; weak nonces; relay leaks; wasm pins lost on a new generation).
- **Disposition:** applied in the first-round commit, with theme2's rulings.

---

**I found blocking issues in HEAD (`63594611a`). I would not land it as-is.**

1. **P1 — Production builds can activate the channel.** [fetch.rs:127](/tmp/rv-s4-5/host/linux/src/fetch.rs:127), [BuildChannel.swift:74](/tmp/rv-s4-5/host/apple/Sources/ExactKit/BuildChannel.swift:74). Linux creates the link for any development URL; Apple checks only for a connection. Neither checks production trust, and production launches still accept `EXACT_DEV_PLAN`. The new registration and pin path therefore remains reachable in production bakes, contrary to D4/D13.

2. **P1 — Polled streams do not actually expire independently, and retained state is unbounded.** [tamber-builds.mjs:68](/tmp/rv-s4-5/scripts/tamber-builds.mjs:68). Expiration runs only inside `poll()`. Repeated unauthenticated `/__dev/hello` requests allocate streams indefinitely when nobody polls; `report()` also accepts an expired nonce until that sweep runs. Closed builds remain in `builds` permanently. I reproduced 1,000 retained streams after advancing time 60 seconds, and an expired report returning 204. This exposes memory exhaustion, including to LAN peers with `--lan`.

3. **P1 — A pin cleared while its build is offline survives reconnection.** [tamber-builds.mjs:99](/tmp/rv-s4-5/scripts/tamber-builds.mjs:99). `clear()` deletes the stored pin, but registration replays only existing pins. Reproduced: pin dark → disconnect build → close owning client → reconnect build. The reconnect receives no clearing frame, so the running host retains dark while the server advertises `pin: null`.

4. **P2 — Authentication nonces use insufficient, noncryptographic randomness.** [tamber-builds.mjs:51](/tmp/rv-s4-5/scripts/tamber-builds.mjs:51). The unpredictable portion is at most eight base-36 characters—about 41 bits—from `Math.random()`. Timestamp and counter are predictable, and the public hello route exposes further samples. This bearer credential should use cryptographic randomness, as the Tamber token already does.

5. **P2 — Disconnecting during relay setup leaks the relay.** [dev.mjs:1215](/tmp/rv-s4-5/host/web/dev.mjs:1215). The close listener is installed after awaiting `relayChannel()`. A disconnect during that await is missed; polling continues and the dead response enters `clients`. I reproduced continued polling after that disconnect. The JS proxy also needs to propagate downstream closure to its upstream request.

6. **P2 — An expired relay nonce permanently stops command delivery.** [tamber-builds.mjs:164](/tmp/rv-s4-5/scripts/tamber-builds.mjs:164). A 403 produces a non-array response and silently ends the polling loop. It neither obtains a new nonce nor closes the device’s SSE connection. After a sufficiently long interruption, the device remains connected with an unusable nonce and receives no further commands.

7. **P2 — Wasm pins disappear on generation replacement.** [dev.js:297](/tmp/rv-s4-5/host/web/dev.js:297). The client keeps the baseline and revision but not the held pin. Generation completion only synchronizes design; it never reapplies modes. The new runner resets them without reconnecting the stream. Reproduced: pinned `compact` becomes `comfortable`, while the last reported status still says `compact`, revision 1.

8. **P2 — Apple restore state is not maintained per session.** [BuildChannel.swift:135](/tmp/rv-s4-5/host/apple/Sources/ExactKit/BuildChannel.swift:135), [BuildChannel.swift:166](/tmp/rv-s4-5/host/apple/Sources/ExactKit/BuildChannel.swift:166). Only the first session’s modes are saved, so clearing a pin overwrites other sessions’ distinct original modes. Scene restoration covers only scenes present at the first pin, and `repinBooted()` reapplies only modes. A new iOS scene therefore misses held scheme and native contrast overrides.

9. **P2 — Web font pins bypass the host’s preference state.** [dev.js:313](/tmp/rv-s4-5/host/web/dev.js:313). Directly assigning `style.fontSize` leaves `pageReporter`’s substituted value and `--exact-host-font-size` unchanged. Reproduced under agent mode: pinning 20 sets the runner to 20, then a page-facts refresh resets it to 16 while CSS remains 20px. The JS client uses the same direct assignment; authored percentage sizing also needs the host-size property.

10. **P2 — Registration is not a barrier for all browser reports.** [dev.js:348](/tmp/rv-s4-5/host/web/dev.js:348). The nonce becomes usable before registration completes, while live batches run through a separate promise chain. Their status and unsupported reports can arrive first and receive 409; responses are ignored. I reproduced this ordering. All reports need to wait for successful registration.

11. **P2 — Artifact identity fields do not satisfy the protocol.** [web-js/dev.mjs:211](/tmp/rv-s4-5/host/web-js/dev.mjs:211), [dev.js:352](/tmp/rv-s4-5/host/web/dev.js:352). The JS page registers `plan: null, design: null` and never fills either. Wasm reports the generation identity instead of the plan digest already supplied as `digest`; Apple and Linux similarly use generation fingerprints. Apple and wasm initialize the reported design only through live batches, leaving fresh builds unidentified.

12. **P2 — Status does not follow ordinary fact changes.** [web-js/dev.mjs:209](/tmp/rv-s4-5/host/web-js/dev.mjs:209), [LiveDesign.swift:31](/tmp/rv-s4-5/host/apple/Sources/ExactKit/LiveDesign.swift:31). Reporting is tied to selected channel operations rather than changes in effective facts. App mode changes and OS preference changes leave status stale. Apple can also register before generation application and never report its completed boot when there is no mode pin or design snapshot.

13. **P2 — Apple reports the host font size instead of the effective size.** [BuildChannel.swift:100](/tmp/rv-s4-5/host/apple/Sources/ExactKit/BuildChannel.swift:100). `PageFacts.rootFontSize` is the size beneath the app’s `setRootFontSize`. For a 20px host pin and an authored 125% size, the runner renders at 25px but status reports 20px.

14. **P2 — Linux discards reports after failed POSTs.** [build_channel.rs:193](/tmp/rv-s4-5/host/linux/src/build_channel.rs:193). Every outcome except 403 removes the report, including transport failure and HTTP 500. A transient failure can permanently lose an unsupported report or applied-pin acknowledgement. Retain reports until a successful response.

15. **P2 — Duplicated browser tabs claim the same build ID.** [dev.js:298](/tmp/rv-s4-5/host/web/dev.js:298), [web-js/dev.mjs:208](/tmp/rv-s4-5/host/web-js/dev.mjs:208). Duplicating a tab copies its `sessionStorage` ID. Registration then transfers ownership to the second stream, leaving the first tab’s reports rejected and its existing pins without a clearing recipient. IDs need to distinguish simultaneously live tabs while surviving reloads.

The two in-memory channel tests passed. The socket relay test could not bind in this sandbox; native builds and app runs were not performed. Additional in-memory reproductions confirmed the lifecycle and browser failures above. The added Apple tests exercise parsing rather than pin application.

The new HTTP routes follow the development gate, report bodies are capped at 64 KiB, network work stays off Linux’s presentation thread, and all changed source files meet the 1,500-line cap. No files were changed.
