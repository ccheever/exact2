// The session's clock: the wake for the runner's next deadline, and the
// advance of one already due when a batch lands.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension ExactSession {
    // @ref LLP 1043.000 §3 D8, §6 ruling 5 — one advance per display frame,
    // or one distant wake. Runner retains ordered catch-up and its 4096-commit cap.
    // After timer advances that showed nothing (`timersQuiet`), a near
    // deadline is a one-shot timer's too, not the display link's every
    // frame: a `sent` reader is answered at each MIDI message's time (LLP
    // 1125 D6), and with nothing on screen reading it a playing sequence
    // kept the link running for those answers. An advance that shows
    // something applies at once, as a distant timer's does, and Core
    // Animation presents it on the next frame; the deadlines after it are
    // the link's again, so a timer that animates (`every(16, step)`) is
    // paced by frames from its second step, and one whose steps alternate
    // between showing something and not keeps the link.
    func scheduleClock(due: Double?) {
        let agent = ExactEnv.agentFreezes || clock != nil
        // A frame task (LLP 1073 D5) keeps the link running; the agent's clock fires its virtual frames.
        let wake = SessionClockTimer.wake(due: due, now: now(), agent: agent, hidden: PageFacts.hidden,
                                          quiet: timersQuiet && !frames.tasks)
        frames.timerSoon = wake == .frame || (frames.tasks && !agent)
        // Keep a timer already armed for this deadline; most batches leave it unchanged.
        if case .timeout = wake, let armed = clockTimer, armed.isValid, clockDue == due {
            frames.run(frames.hasWork)
            return
        }
        clockTimer?.invalidate()
        clockTimer = nil
        clockDue = nil
        if case .timeout(let delay) = wake {
            clockDue = due
            clockTimer = SessionClockTimer.schedule(after: delay / 1000) { [weak self] _ in self?.fireClock() }
        }
        frames.run(frames.hasWork)
    }

    /// `clockTimer`'s advance. One the owner cannot take now (it waits on
    /// a callback on this thread, LLP 1072 T5) is asked again a frame
    /// later, as a frame's would be on the next: nothing else arms the
    /// clock while timers show nothing.
    private func fireClock() {
        guard state != .destroyed else { return }
        clockTimer = nil
        // Queued behind a slice, the driver may have taken the clock since
        whenIdle { [weak self] in
            guard let self, state != .destroyed, clock == nil, !ExactEnv.agentFreezes else { return }
            followOffset()
            // Posted, not waited for; one in flight re-arms the clock as it lands.
            if ExactSession.asyncFills { if !advanceInFlight { sendAdvance(now: now()) }; return }
            let batch = runtime.advance(now: now())
            guard batch.busy else { applyUnlessEmpty(batch, timer: true); return }
            guard clockTimer == nil else { return } // a batch since armed its own
            clockTimer = SessionClockTimer.schedule(after: 1.0 / 60) { [weak self] _ in self?.fireClock() }
        }
    }

    /// A deadline already due when a batch lands advances now, in the same
    /// main-thread work, before control goes back to UIKit or AppKit, which
    /// commit what the batch did: a `then` is armed at its answer's time and
    /// runs as its own commit at the next clock advance (LLP 1016.001 D3), so
    /// the frame waited for showed the answer without what its `then` did
    /// (Lexy's sign-in button read "Sign In" between two requests). Not while
    /// the driver owns the clock, a frame task's display link drives the
    /// session, a slice is in flight, or earlier slices are landing ahead of
    /// a batch (the batch, then the advance); those advance as before.
    /// Where timer advances are posted (LLP 1072.000 §4 B), main waits only
    /// for a `then`, a queue's `next` or an answer that is the earliest work
    /// due; what is due after it is posted.
    func advanceIfDue() {
        guard !advancingDue, !applying, !landing, !holdingReports, state != .destroyed, clock == nil, !ExactEnv.agentFreezes,
              !frames.tasks, !ownerInFlight, let due = timerDue, due <= now() else { return }
        advancingDue = true
        defer { advancingDue = false }
        followOffset()
        guard ExactSession.asyncFills else { applyUnlessEmpty(runtime.advance(now: now()), timer: true); return }
        let then = runtime.landThenFirst()
        guard !then.held else { return } // (over a reset: ResetGate replays it)
        applyUnlessEmpty(then, timer: true)
        if state != .destroyed, !ownerInFlight, let due = timerDue, due <= now() { sendAdvance(now: now()) }
    }

    /// A timer advance on the owner, not waited for (LLP 1072.000 §4 B):
    /// its batch lands in the owner's order, a main-queue turn after the
    /// owner made it, and until then a frame shows the last batch applied.
    /// One is in flight at a time. `frame`: a display frame's, with its
    /// frame tasks (LLP 1073 D5). The commit samples the live clock when the
    /// owner runs it, as one main waited for does.
    func sendAdvance(now: Double, frame: Double? = nil) {
        advanceInFlight = true
        let captured = generation
        runtime.advanceAsync(now: now, frame: frame, capture: frameHealth.captureID, measureFrame: frameHealth.enabled && clock == nil) { [weak self] batch in self?.publish(.advance, batch, captured) }
    }

    /// A frame's tick on the owner, not waited for: motion sampled at this
    /// frame lands with the next main-queue turn (LLP 1072 §7.1).
    func sendTick(now: Double, frame: Double? = nil) {
        tickInFlight = true
        let captured = generation
        runtime.tickAsync(now: now, frame: frame, capture: frameHealth.captureID, measureFrame: frameHealth.enabled && clock == nil) { [weak self] batch in self?.publish(.tick, batch, captured) }
    }
}
