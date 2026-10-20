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
    func scheduleClock(due: Double?) {
        let agent = ExactEnv.agentMode || clock != nil
        let wake = SessionClockTimer.wake(due: due, now: now(), agent: agent, hidden: PageFacts.hidden)
        // A frame task (LLP 1073 D5) keeps the link running; the agent's clock fires its virtual frames.
        frames.timerSoon = wake == .frame || (frames.tasks && !agent)
        // Keep a timer already armed for this deadline; most batches leave it unchanged.
        if case .timeout = wake, let armed = clockTimer, armed.isValid, clockDue == due {
            frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
            return
        }
        clockTimer?.invalidate()
        clockTimer = nil
        clockDue = nil
        if case .timeout(let delay) = wake {
            clockDue = due
            clockTimer = SessionClockTimer.schedule(after: delay / 1000) { [weak self] _ in
                guard let self, state != .destroyed else { return }
                clockTimer = nil
                // Queued behind a slice, the driver may have taken the clock since
                whenIdle { [weak self] in guard let self, state != .destroyed, clock == nil, !ExactEnv.agentMode else { return }; followOffset(); applyUnlessEmpty(runtime.advance(now: now())) }
            }
        }
        frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
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
    func advanceIfDue() {
        guard !advancingDue, !applying, !landing, !holdingReports, state != .destroyed, clock == nil, !ExactEnv.agentMode,
              !frames.tasks, !(fillInFlight || tickInFlight || canvasInFlight), let due = timerDue, due <= now() else { return }
        advancingDue = true
        defer { advancingDue = false }
        followOffset()
        applyUnlessEmpty(runtime.advance(now: now()))
    }
}
