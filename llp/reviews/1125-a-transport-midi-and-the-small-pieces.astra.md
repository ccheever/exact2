# LLP 1125 r1, blind review: GPT-6 Astra, max

> **Review provenance.** Family: OpenAI GPT-6 Astra (max) · Provider and runtime: `codex exec -m gpt-6-astra -c model_reasoning_effort=max -s read-only` · Date: 2026-10-16 · Redacted: no · Method: blind (shown no other review), direction and fundamentals only, nits excluded; given LLP 1125 r1 at `404863b35` in a read-only detached worktree, with the euxy sources under `~/projects/euxy-src` and `~/projects/euxy-ports` readable. The text below is the reviewer's, unedited; the author's dispositions are in the RFC's §R.

RIGHT DIRECTION WITH CHANGES

The central direction is sound: declare timed output as a value, execute it in the host, and keep deterministic semantics in the runner. A `sequence` schedule on a gated task fits. Transport/MIDI integration, sampler controls, element-scoped wake locks, and file sharing warrant standard vocabulary. LLP 1124’s pull-based extensions do not cover the timed event stream. QR encoding and arbitrary element capture need separate justification.

1. **BLOCKING — D3, D5, D7, §4: External-clock playback is not reproducible from the proposed record.**  
   Different MIDI tick histories can produce the same reported tempo while producing different note times. The 500 ms anchor updates lose that history; automatically generating ticks at a preferred tempo tests an ideal clock, not the received one. Record timestamped clock inputs and transport transitions, potentially in batches without UI commits, and replay those through the same transport model. Include jitter, missing ticks, Stop/Continue and Song Position. Seek equivalence must hold for the same input history, including pattern revisions.

2. **BLOCKING — D4, D7: “Stop now” does not cancel queued MIDI.**  
   A note-on already submitted for `t + 80 ms` can play after the immediate note-offs and All Notes Off. The same issue affects restart and destination changes. [CoreMIDI provides destination-wide flushing](https://developer.apple.com/documentation/coremidi/midiflushoutput%28_%3A%29), but [Chromium’s MIDIOutput interface still leaves `clear()` unimplemented](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/modules/webmidi/midi_output.idl). Define implementable cancellation and output ownership before accepting the stop contract. Ordinary transport stop must also avoid silencing unrelated notes on shared channels through unconditional CC 123.

3. **MATERIAL — D3–D5: Autonomous playback needs one owner of voice semantics.**  
   Today [the runner’s sound table](/Users/ccheever/projects/exact2-review-1125/runner/src/sound.rs:194) decides choke groups, the 32-voice limit, cancellations and IDs; the mixer receives the resulting `Play`/`End` operations. A transport expanding directly into the mixer must reconcile those decisions with concurrent `playSound` auditions and `stopSounds`. Specify one deterministic arbitration model shared by host execution and runner simulation. Otherwise both can expand the same pattern yet disagree about which voices survive.

4. **MATERIAL — D3, §9 Q3: The fixed edit horizon lacks a product justification.**  
   Determinism requires an explicit effective edit time; it does not require 100 ms. Existing audio operations can cancel scheduled voices, so “already submitted” does not universally mean immutable. An unconditional delay also makes a mute perceptibly sluggish. Separate scheduling lookahead from authored edit semantics, measure interactive latency, and choose a common, replayable edit policy. The suggested fallback of silently giving Apple a different horizon weakens the central cross-host record promise.

5. **MATERIAL — D3, D5, §7: The web implementation has a weaker execution guarantee.**  
   A Worker posting wake-ups to the page leaves scheduling dependent on the page’s main thread. D5 acknowledges this, while the summary promises playback independent of runner responsiveness. The Apple-only 80 ms blockage test does not qualify that web behavior. Likewise, main-queue haptics can be delayed by the entire blockage, not merely a few milliseconds. Either implement independent audio execution or explicitly declare and test the weaker web guarantee. Keep intended event times distinct from actual late, skipped or unavailable output.

6. **MATERIAL — D2, D4, §2: The transport lifecycle is still tailored to endless loops.**  
   Looping tracks serve metronomes and drum machines well. Finite rhythm-game charts and interval sessions need host-enforced endings; relying on a runner timer to close the gate restores the stall dependency. Background behavior also needs explicit stop/pause/continue and interruption semantics. [The original euxy deliberately stops when locked](/Users/ccheever/projects/euxy-src/euxy/src/core/engine.ts:184). LLP 1098’s existing media-session ownership is limited to `audio`/`video`, so lock-screen transport controls do not follow automatically. Resolve this minimal lifecycle with a second consumer, or narrow the advertised scope.

7. **MATERIAL — D8: The overflow policy contradicts the delivery guarantee.**  
   One dispatch per message is reasonable at demonstrated rates. Dropping message 257 arbitrarily is not: it can be a note-off, sustain release or transport Stop. Opening every input also makes unrelated devices consume the queue. Keep the proposed ports/send/event surface, but define input selection, ordered delivery under load, and explicit recovery after overflow. Batching can preserve every message without requiring a full UI settlement for each; that choice needs a semantic decision and measurements with dense controller traffic.

8. **MATERIAL — D9: On-demand decoding needs an application-visible readiness contract.**  
   `loadSounds` is asynchronous, but the proposed `state.sounds.loaded` is driver state, not an established application mechanism for awaiting readiness. An app therefore cannot reliably prepare before starting its supposedly accurate downbeat. Apple’s existing first voice also opens the engine synchronously, with measured cold-start delay in LLP 1096 D8. Rate and pan are appropriate additions. Before changing decoding defaults, define preparation completion and the policy for starting a sequence whose assets or output are not ready.

9. **MATERIAL — D1, D11: QR encoding belongs in reusable data-module code first.**  
   QR needs neither a host clock nor platform access. A data module can return dimensions and SVG paths using a pinned encoder, including euxy’s exact byte segmentation and drawing style. That achieves reuse without adding compiler vocabulary, runner implementation and a JS-target module. Bit identity with `qrcode@1.5.4` is a reasonable fidelity requirement for this port, but insufficient justification for a permanent framework primitive. Move it to a reusable library; reconsider a built-in only after demonstrating a limitation of that route.

10. **MATERIAL — D12: `foreignObject` does not deliver the promised picture contract.**  
    Serializing a subtree does not preserve its computed styling, canvas contents or current presentation automatically. [SVG image processing disables external resource references inside `foreignObject`](https://www.w3.org/TR/SVG/conform.html#processing-modes), including ordinary external fonts and images—not just problematic cross-origin images. Resource preparation and rasterization must also finish while [Web Share’s transient activation remains valid](https://www.w3.org/TR/web-share/#share-method). Admit `share(files=)` independently. For element pictures, prove a bounded static-card contract with embedded resources and explicit refusals, or defer the feature. Silently omitting visible content contradicts “as drawn now.”

11. **MATERIAL — §§7–9: Stage the architectural risks before promising deletion or admission.**  
    Ship independent wake-lock, sampler-parameter and file-share work separately. Prove transport arbitration, MIDI cancellation, external-clock replay and web execution before replacing euxy’s engine. Stage 2 cannot delete its scheduler while stage 3 still depends on that scheduler for MIDI. Keep QR outside the runtime and element capture behind its own qualification. The [recorded euxy waiver](/Users/ccheever/projects/exact2-review-1125/rules/DEFERRED.md:317) names chrome improvements; obtain an explicit admission entry covering the selected capabilities. Already-deferred Linux MIDI is a non-goal, not a take.

For **§9**, I disagree with these recommendations:

- **Q1:** Admit specific capabilities explicitly; do not infer blanket admission for stages 1 and 2 from the existing entry.
- **Q3:** Do not standardize 100 ms yet. Establish cancellation and edit semantics, then choose latency from evidence.
- **Q6:** Do not switch universally to demand loading until applications can establish readiness. A preload command without observable completion is insufficient.
- **Q7:** Prefer a reusable data-module encoder over either a runner built-in or a dedicated Rust/Wasm module. Keep exact `qrcode` compatibility and styling requirements in that library.

I agree with Q2’s gated-task surface and Q9’s declared platform limitation. Q8’s capability fact is useful, provided the host still validates the actual files being shared.