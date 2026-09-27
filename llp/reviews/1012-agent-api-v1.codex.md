# Review: LLP 1012 Agent API v1 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only -C <capsule> --skip-git-repo-check --ephemeral`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export (the capsule), no network
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 eb4593d6df58863ce8fe7feeb1b446bcd3d8800da8473e8d63b5277d97c975e7; target r1 sha256 a20460f47130478a3f335833fcf77934e3f78688264fd8eaebea019d4fd9fb62; capsule sha256 02281bb288f90c9e12e07308471751add0647902953f5d0ba406bd3e5969acee. Verdict (round 1): NOT READY — binds to the r1 hash.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T12:54:47Z, verbatim)

## Overall assessment

Revision r1 (`a20460f47130478a3f335833fcf77934e3f78688264fd8eaebea019d4fd9fb62`) is not yet a reliable reference for the Linux host or shared carriers. The runner-owned reads, journal, ABIs, and present host paths are mostly transcribed accurately, but the central virtual-time guarantee is false on the web, and §1 conflates the public API, carrier-private messages, and host replies.

The transcript fixture recomputes exactly from the current renderer. Full smokes and metric reproduction were impossible from this capsule: it lacks `Cargo.toml`, built hosts, `scripts/metrics.mjs`, and captured run output.

## Strengths

- §1 lines 37–40 accurately describes `tree`, `state`, and the raw runner `logs` reply. `runner/src/agent.rs:42–228` confirms preorder nodes, schema-named props, typed records, and `null` for unit/none.

- §2 lines 54–67 is a good account of the journal itself. `runner/src/runner.rs:167–175,356–392` confirms a 4,096-line ring, oldest-line eviction, clock stamps, and outcome logging. `runner/src/agent.rs:162–183` confirms `from = max(since, journal_start)`.

- §3 lines 81–84 accurately describes the ABI buffer discipline and `exact_agent`; both host ABIs and `exact.h` agree.

- §5 lines 141–162 correctly identifies the important architectural choices: public taps and text changes do not call `Runner::dispatch` directly, and `layout` comes from the actual DOM/AppKit presentation rather than a reconstructed runner layout.

- §7 lines 178–185 correctly describes the current implementation as one lossy renderer whose output is not parsed back. The supplied JSON fixture renders byte-for-byte to `transcript.txt`.

- §8 lines 213–217 honestly admits the most important missing test: no transition is exercised through the web agent clock.

## Concerns

- **HIGH · Summary lines 15–17; §1 line 43; §3 lines 85–92 · the web does not freeze animation between calls.** `host/web/glue.js:125–128` records newly discovered animations in `starts`, but does not pause them. `pause()` is called only later by `seek` at lines 141–148. After an input, `scripts/agent.mjs:149` waits for frames, during which CSS transitions and `Element.animate` springs play on wall time; they can continue moving after the call returns or even disappear from `document.getAnimations()` after finishing. Thus “nothing moves between two calls” and “never played” are not true. In addition, `clock settle` computes its candidate before `exact_advance` (`glue.js:175–187`; `Agent.swift:123–132`), so a timer fired during that advance can create motion whose end was not included. Both hosts also return clock success even when the advance/tick batch reports an error. **Resolve:** pause and seek every animation when first registered; define whether settle is a snapshot or a fixed point over motion created while advancing; propagate batch errors; add tests that compare layout/screenshot before and after a real wall delay and cover timer-created motion.

- **HIGH · §1 lines 31–44 · the operation contract is not precise enough for Linux and mixes three interfaces.** “Requests and replies are JSON” is not uniformly true: web `tap`, `type`, and `screenshot` are CDP calls, not JSON requests to `exact.agent`; public `tap`/`type` add `target`, macOS wheel replies also include `wheel`, public `logs()` removes `next` and `from`, and macOS window screenshots omit `w`/`h`. A Linux author also cannot infer attached-versus-visible membership for `layout` (current hosts include attached views even when outside the viewport), node ordering, coordinate origin, units, transforms, rounding, zero-sized/offscreen input behavior, screenshot pixel scale, path ownership, or exact error semantics. `clock` lacks a normative order for setting host time, advancing timers, applying layout, and seeking motion. **Resolve:** separate and specify (1) eight public session methods, (2) the carrier interface, and (3) host-private/ABI messages; then give exact Linux acceptance semantics for the five host operations.

- **MEDIUM · §4 lines 116–130 · “what the smoke holds” overstates its assertions.** `scripts/smoke.mjs` checks only the first countdown, not every countdown; it does not assert that sixty timers fired; it checks a wheel of 300 moved content by more than zero, not by exactly 300; and it records an arbitrary nested-scroll limit greater than 100, not 652 or equality across hosts. The script runs one host at a time and performs no direct cross-host comparison. It does hold the exact 100-pixel nested wheel, no page movement before chaining, stopping at the observed limit, transcript equality, root width, rounded 96×36 logo, interaction state, GPU loading on web, and clean logs. **Resolve:** either strengthen the assertions—including pinning 652 on both hosts—or rewrite §4 to distinguish enforced properties from one-time observations.

- **MEDIUM · §2 lines 54–58 and §1 line 39 · the ring-loss signal does not reach the stated consumer.** The raw reply correctly exposes `from > since` when older lines were dropped, but `scripts/agent.mjs:242–245` returns only `{lines, host}` and discards both `from` and `next`. An agent using the documented session API therefore cannot tell that its cursor was passed. **Resolve:** preserve `from` and `next` in the public result, optionally adding an explicit `dropped` count, and specify cursor behavior across reload/reboot.

- **MEDIUM · §7 lines 187–205 · the transcript form is not tight enough to be the claimed fixture contract.** The actual predicates are inconsistent: `text` and `value` use `!= null`, while `testId`, `type`, and `accessibilityLabel` use truthiness; handlers require a nonempty array; scroll appears when `sx != null` even if `sy` is absent. Contrary to line 198, `testId`, type, and handlers are not JSON-quoted, so arbitrary test IDs can make lines ambiguous. The outer fixture grammar—`--- name`, entry order, blank lines, final newline, and the special mapping from fixture key `empty` to op `logs`—is also absent. The fixture enforces current rendering, but cannot enforce “never parsed” or uniqueness of renderers by itself. **Resolve:** transcribe the exact predicates and separators, quote arbitrary strings, specify the empty-log case and outer fixture construction, and add falsey/escaped/missing-field cases.

- **MEDIUM · §1 lines 46–50 and §5 lines 141–164 · “eight operations” and “one delivery path” need an explicit scope.** `focus` and `settle` are additional internal JSON op names; `focus` directly focuses/selects the DOM before CDP inserts text. `exact.reload` is carrier setup, and `Runner::act` remains an unexported semantic test seam—despite `runner.rs:537–552` and LLP 1005 lines 136–140 still saying “tests, agents.” These are honest internal exceptions, not alternate public input paths, but without a public/private boundary the comparison to exact1 wire-name counts is inconsistent. **Resolve:** state that the budget counts public semantic operations, inventory private carrier helpers, say that reload is session setup, and make `act` explicitly test-only unless it is intentionally exposed.

- **MEDIUM · §4 lines 132–137 · the measurements are not traceable enough, and one unit is wrong.** The smoke code prints total time, boot time, and observed scroll values, but the capsule contains no captured output. `scripts/metrics.mjs` is absent. The 0.46-second launch and all wasm/gzip values occur only in the spec. Moreover, `431,782 − 418,002 = 13,780` bytes, which is 13.46 KiB, not 13.8 KiB. “Against HEAD built in a scratch worktree” is candid but not reproducible or immutable. **Resolve:** record exact commit hashes, commands, profile/toolchain, compressor and raw/gzip before-and-after bytes—or cut the measurements.

- **LOW · Related line 9; §1 lines 49–50; §5 lines 143–157; §7 lines 180–181 · exact1 claims are unsupported by this capsule.** The capsule cannot substantiate six primitives/80 wire names, the three input exports, receipt fields, the seven-token identity envelope, the 50 ms/500 ms/10 s settle loop, or five transcript modes. **Resolve:** include and cite the referenced exact1 sources as research evidence, qualify these as reports from unavailable predecessor documents, or cut them.

## Suggestions

- Put “a ninth public operation replaces one of the eight” in both places: `DEFERRED.md` as the binding budget rule, and this spec as the closed v1 interface. Keep wheel-as-`tap` primarily here as operation semantics.

- Replace “cross-host parity check for free” with “the same assertions run on every host” until a harness actually compares host results or pins shared values.

- Cut the approximate line counts in the Summary and §3, the precise exact1 implementation history, §6 “Found on the way,” the parenthetical quotes, and volatile performance numbers without evidence. None governs the Linux host or shared carrier.

- Reconcile LLP 1005’s “tests, agents” wording for `Runner::act` with this API’s real public boundary.

## Open questions

- If advancing to the current settle candidate fires a repeating timer that starts another transition, should `clock settle` continue to a fixed point, settle only motion already in flight, or refuse an unbounded settle?

- Does the eight-operation budget count public semantic operations only, or every accepted JSON `op` name? The answer determines whether `focus` and `settle` are acceptable private helpers.

- Is `layout.nodes` ordered, and does “on-screen” mean attached, intersecting the viewport, or nonzero-box?

- Are screenshot dimensions CSS/point dimensions or PNG pixel dimensions, and must every screenshot reply include them?

## Recommended next step

Revise and keep the document in Draft: fix the web animation freeze, define settle/error behavior, split public/carrier/host contracts, align §4 with actual assertions, and tighten the transcript grammar. Then add the admitted transition fixture—including a wall-delay invariance check and timer-created motion—run both host smokes, capture metric evidence if the numbers remain, and review the new SHA.

## Verdict: NOT READY
---

## Disposition (orchestrator, 2026-08-29)

Folded into r2 (sha256 7e0f6d7d3cfd345275efbc4493ddd8634bf976119123c775918a0b6af9d2d1b1, unreviewed): the clock made true on both hosts (animations frozen at every batch; `Runner::advance_timed` with per-commit times, web `at` markers and a `clock` trailer, Apple per-commit engine seeks; the clock lands where the runner says, refusal and all; `settle` as a bounded fixed point; the GPU module on the agent's clock); §1 rewritten as the contract a host implements (space, membership, order, rounding, transforms, monotonic clock, wheel sign and delivery, `type` on inputs only, replies, errors, private messages); §5 walked back to what `check(...)` asserts, with the smoke made exact (300, 652, every countdown) and its wall-clock retry removed; the journal's FIFO drop and passed-cursor behavior stated and `from`/`dropped` surfaced by the driver; §7 tightened (predicates, quoting, the outer fixture grammar, `empty`/`dropped`); exact1 inventory cut to one research sentence; line counts and §6 cut; the wasm delta restated as 13 780 B = 13.5 KiB with commit and method; LLP 1005's "tests, agents" reworded. Not folded: a separate normal/agent wasm artifact (open for Charlie, §8); the "ninth replaces one" line (proposed for DEFERRED, not written here as law); a transition fixture under `clock` in the smoke (§8, not in v1). The verdict above binds to r1; r2 is unreviewed.
