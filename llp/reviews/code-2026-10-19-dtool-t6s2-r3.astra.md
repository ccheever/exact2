# Code review: design tool T6 steps 1-2, round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `66ede92416`.
- **Method:** one brief (sha256 `fe619d7144d227b4e0c4c140ab3599efd9a0b0bcc386b6890c042538f7fc1080`), shared by both reviewers, blind to the other review, with a security pass; commit 66ede92416 (round 2's fixes) and the step whole. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (3 SHOULD-FIX, 1 NIT).
- **Disposition:** post-round, in the fix commit named in the sign-off request: the seeded selection is shown as the build reported it, not as current, so it offers no edit (the server keeps no cursor for it) (Astra 2); it is asked again after a failure and after the poll misses events, not once a server run (both); a design read with no axes counts as read (both); Cancel scrub is greyed below write (Grok 4); the pair test bounds what reaches the door after the revoke at the two requests in flight (Astra 1) and builds the tool without `--via`, whose 2 s probe of a dev loop times out under load (QUEUE). Not taken: a late 401 shown in the changes line for one poll (Grok 3: the wire keeps the session; the next poll clears it); the pair test's Origin refusal is scripts/tamber-remote.test.mjs's (Grok 5); the code is in state while it is typed and shown, as it must be (Astra 1).

**Nothing blocks.** I found three SHOULD-FIX items and one NIT.

The security pass found no remaining bearer-token leak or scope bypass. [wire.ts:108](/tmp/rv-dt-t6s2r3/apps/design-tool/wire.ts:108) rejects old handles and above-scope requests before sending; a current-session 401 stops subsequent sends. The late local-token JSON and old-pairing 401 fixes protect the active credential. `doorOf` restricts transport, and token reads, pairing and protocol calls refuse redirects. The stub’s [expiry check](/tmp/rv-dt-t6s2r3/apps/design-tool/stub/tamber-stub.ts:81) now matches the real server’s deadline check. I found no further enabled control that lets a reader or proposer draft above its scope.

1. **NIT — The integration test still overstates secrecy and cessation coverage.** [design-tool-pair.test.mjs:62](/tmp/rv-dt-t6s2r3/scripts/design-tool-pair.test.mjs:62) samples state only after submission clears `remoteCode`. A drive taken before submission exposes that [slot](/tmp/rv-dt-t6s2r3/apps/design-tool/app.contract:662), and the local panel’s `paircode` mutation contains the displayed code. These are intentional display values, but contradict an unqualified “code in no state” claim. Separately, [line 81](/tmp/rv-dt-t6s2r3/scripts/design-tool-pair.test.mjs:81) allows three post-revoke requests across three retry intervals; a client continuing to retry can satisfy that bound. Record request arrival and assert no count growth after the client observes its first 401. The real tunnel boundary, observed-method allowlist and server-output scan are now present.

2. **SHOULD-FIX — The seed manufactures current provenance and can be overwritten by older events.** [app.ts:1337](/tmp/rv-dt-t6s2r3/apps/design-tool/app.ts:1337) stamps a cached report with the *current* build key, axes and file digests. The server [retains the report](/tmp/rv-dt-t6s2r3/scripts/tamber-server.mjs:234) without its event cursor or build snapshot, and a later build-status change does not invalidate that cache. After a mode or design change, a newly paired client can therefore present an old selection as current and offer its editing provenance.

   Ordering remains unresolved too: an events response containing selection A can be delayed while refresh returns newer selection B; when A arrives, [app.ts:1065](/tmp/rv-dt-t6s2r3/apps/design-tool/app.ts:1065) unconditionally replaces B. Preserve and validate the cached report’s revision and event position, or show it as historical until freshness is established.

3. **SHOULD-FIX — The one-time seed gate prevents recovery.** [app.ts:1334](/tmp/rv-dt-t6s2r3/apps/design-tool/app.ts:1334) requires no existing selection and marks the epoch seeded before the request succeeds. Consequently:
   - A startup backlog selection recorded before the design finishes loading becomes stale, but prevents the seed from repairing it.
   - A later missed-events response [marks the existing selection stale](/tmp/rv-dt-t6s2r3/apps/design-tool/app.ts:1008), but cannot refresh it within that epoch.
   - One failed refresh suppresses further attempts for the server run.

   Readers and proposers cannot use Inspect to recover themselves. Track successful restoration separately and allow recovery after missing events or a failed request.

4. **SHOULD-FIX — Designs without axes never seed a selection.** The new `designAxes.length` condition at [app.ts:1334](/tmp/rv-dt-t6s2r3/apps/design-tool/app.ts:1334) treats an empty axes array as “not loaded.” Empty axes are valid—for example, [literals.json:6](/tmp/rv-dt-t6s2r3/tamber/tests/corpus/literals.json:6). A remote reader joining after the event history has rolled over therefore never asks for the cached selection. Use explicit design-read readiness; the inspector’s corresponding [axes-length check](/tmp/rv-dt-t6s2r3/apps/design-tool/inspector.ts:75) also needs to distinguish an empty design axis list from an unfinished read.

Read-only review: no files changed, no modifying git commands, and no builds or test suites run.
