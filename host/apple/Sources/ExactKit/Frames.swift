// A session's frames (LLP 1009 D4): the display link that runs only while
// motion or a canvas wants it. Split from Session.swift.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import Foundation
import QuartzCore

/// Frames come from the display link, only while motion runs or a canvas
/// has something to render (LLP 1009 D4), per session. iOS takes them from
/// the app's clock (`FrameClock`); macOS's link comes from the viewport, so
/// from its window's screen.
package final class Frames: NSObject {
    weak var session: ExactSession?
    #if canImport(UIKit)
    /// The app's link while this session takes frames from it.
    var link: CADisplayLink? { FrameClock.shared.wants(self) ? FrameClock.shared.link : nil }
    #else
    var link: CADisplayLink?
    #endif
    package var motion = false, spatial = false
    /// A 2D canvas asked for a frame (LLP 1056 D5): ticks run while it does.
    var canvas2d = false
    var timerSoon = false
    /// A frame task (LLP 1073 D5): each tick is the runtime's frame at the tick's target time.
    var tasks = false
    /// A hatch's frame ticket is live (LLP 1075.003.000.001 §2.4): each tick calls it, after the frame's tasks.
    var hatches = false
    /// Keep every existing app frame request when scheduling or resuming the session.
    var hasWork: Bool { motion || canvas2d || timerSoon || session?.canvases.wantsFrames == true }
    private var canvasRequested = false

    /// Input and reads ask for one frame; an agent-owned clock never self-reschedules.
    package func requestCanvas() {
        guard let s = session else { return }
        if s.clock == nil { run(true); return }
        guard !canvasRequested else { return }
        canvasRequested = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            canvasRequested = false
            guard let s = session, s.state != .destroyed else { return }
            s.canvases.settle(now: s.now())
        }
    }
    @objc func tick(_ link: CADisplayLink) {
        guard let s = session else { return }
        ticking = true
        let generation = s.generation
        let arrival = s.frameHealth.enabled && s.clock == nil ? CACurrentMediaTime() : 0
        if arrival != 0 { s.frameHealth.expireCapture(at: arrival) }
        let healthFlags = s.frameHealth.frameFlags(self, session: s)
        s.timerAdmissions.check(frame: true)
        s.canvases.lifecycleFrame()
        // Motion (below) and canvas frames target presentation.
        let frameNow = s.clock ?? (link.targetTimestamp - ExactEnv.t0) * 1000
        // ProMotion changes callback cadence (e.g. 120 → 80 Hz) while duration
        // can remain the nominal base interval. The target interval is actual;
        // canvases quantizes it and republishes this session’s stable class before rendering.
        s.canvases.period((link.targetTimestamp - link.timestamp) * 1000)
        let previous = s.canvases.frameNow
        s.canvases.frameNow = frameNow
        defer { s.canvases.frameNow = previous }
        // Explicit frame work advances here; ordinary task timers have independent native callbacks
        // A slice building on the owner holds this frame's timers and tick:
        // they run on the first frame after it lands (LLP 1072 §7.1).
        // Frame health: a callback that starts no frame work is not a frame, and a frame that sends
        // owner work finishes when that work lands (LLP 1079.000 §4.6)
        var held = false, ran = false, sent: UInt8 = 0
        if !s.fillInFlight {
            if timerSoon, !ExactEnv.agentFreezes, s.clock == nil {
                let now = s.now()
                s.followOffset()
                // Frame work that changes nothing shown skips the presenter's pass.
                // Posted where fills are (LLP 1072.000), one at a time: a
                // frame with one in flight skips its own, never caught up.
                let due = tasks || s.timerDue.map({ now >= $0 }) ?? true
                if ExactSession.asyncFills {
                    if due, !s.advanceInFlight {
                        if tasks, arrival != 0 {
                            s.frameHealth.sending(FrameHealth.advance, start: link.timestamp, deadline: link.targetTimestamp)
                        }
                        s.sendAdvance(now: now, frame: tasks ? frameNow : nil)
                        if tasks { sent |= FrameHealth.advance }
                    } else if tasks { held = true }
                }
                else if tasks { s.applyUnlessEmpty(s.runtime.frame(now: frameNow, wall: now, admitted: s.timerAdmissions.enabled)); ran = true }
                else if due { s.applyUnlessEmpty(s.runtime.advance(now: now)) }
            }
            // Motion samples at the frame's target, and starts there what
            // waits for it (LLP 1003.001 D5); the agent's clock has no frames.
            if motion || canvas2d {
                if arrival != 0, ExactSession.asyncFills, !s.tickInFlight {
                    s.frameHealth.sending(FrameHealth.tick, start: link.timestamp, deadline: link.targetTimestamp)
                }
                switch submitMotionTick(now: s.now(), frame: s.clock == nil ? frameNow : nil) {
                case .sent: sent |= FrameHealth.tick
                case .held: held = true
                case .applied: ran = true
                }
            }
        } else if motion || canvas2d || tasks { held = true }
        if hatches, s.clock == nil { s.natives.hatchClock.presented(frameNow); ran = true }
        // A canvas render that drew nothing, as without its drawable, draws when one lands, and the
        // first frame to see one waits until none does (`canvasDrawn`, or `canvasLost` when a waiting
        // canvas leaves). While it waits, a later callback's render of a waiting canvas changes only
        // which frame it draws, so a callback that drew no canvas is held, as is one whose canvas
        // wanted a frame but rendered nothing, as during AppKit's starvation backoff.
        let draws = s.canvases.drawn, wanted = s.canvases.wantsFrames
        let more = s.canvases.tick(now: frameNow)
        let waits = s.canvases.awaitingDrawable
        if waits, arrival != 0, !s.frameHealth.waits(on: FrameHealth.canvas) {
            s.frameHealth.sending(FrameHealth.canvas, start: link.timestamp, deadline: link.targetTimestamp)
            sent |= FrameHealth.canvas
        }
        if s.canvases.drawn != draws { ran = true } else if wanted || waits { held = true }
        if arrival != 0, s.generation == generation, s.clock == nil {
            s.frameHealth.record(arrival: arrival, target: link.targetTimestamp,
                period: link.targetTimestamp - link.timestamp, finished: CACurrentMediaTime(),
                held: held, ran: ran, sent: sent, flags: healthFlags)
            // The frame-health card's charts draw what the hook stored, after the hook returns
            s.frameHealth.chart?.present()
        }
        ticking = false
        run(motion || canvas2d || timerSoon || more || s.canvases.wantsFrames || s.canvases.lifecycleNeedsRetry)
    }

    /// Every canvas render that waited on its drawable has drawn.
    package func canvasDrawn() {
        session?.frameHealth.landed(FrameHealth.canvas)
        session?.frameHealth.chart?.present()
    }
    /// A canvas whose render waited on its drawable left before it drew.
    package func canvasLost() { session?.frameHealth.abandon(FrameHealth.canvas) }

    enum MotionTick { case sent, held, applied }

    /// The motion handoff follows the app's owner policy independently of other frame work.
    @discardableResult
    func submitMotionTick(now: Double, frame: Double?) -> MotionTick {
        guard let s = session else { return .applied }
        if ExactSession.asyncFills {
            guard !s.tickInFlight else { return .held }
            s.sendTick(now: now, frame: frame)
            return .sent
        }
        s.apply(s.runtime.tick(now: now, frame: frame))
        return .applied
    }

    static func requestedRange(maximum: Int, full: Bool, paint: Bool) -> CAFrameRateRange {
        let maximum = Float(maximum > 0 ? maximum : 60)
        if full { return CAFrameRateRange(minimum: min(80, maximum), maximum: maximum, preferred: maximum) }
        if paint { return CAFrameRateRange(minimum: min(30, maximum), maximum: min(60, maximum), preferred: min(60, maximum)) }
        return .default
    }

    /// Inside `tick`: work it applies can end motion, but the callback is recorded before the run ends.
    private var ticking = false

    func run(_ wanted: Bool) {
        if wanted, session?.clock != nil { requestCanvas() }
        let on = (wanted || hatches) && session?.clock == nil
        if !on, !ticking { session?.frameHealth.stop() }
        #if canImport(UIKit)
        guard on else { FrameClock.shared.drop(self); return }
        // Motion that changes place or size asks for the panel's full
        // rate while it runs, as a canvas does: at `.default` a ProMotion
        // iPhone presents a slide at 60 Hz. A fade or a colour change
        // reads the same at 60, so paint-only motion — a breathing loop
        // that runs for minutes — asks no more. The app's clock runs
        // only while something wants frames, so an idle app gets no
        // callbacks at all (LLP 1061 D4).
        let fullRate = (motion && spatial) || canvas2d || session?.canvases.wantsFrames == true
        let range = Self.requestedRange(maximum: session?.presenter.viewport.window?.screen.maximumFramesPerSecond ?? 60,
            full: fullRate, paint: motion)
        FrameClock.shared.want(self, .session, rate: range) { [weak self] in self?.tick($0) }
        // Inside `tick`, the request that governed this callback stands until it is recorded
        if !ticking { session?.frameHealth.requestedRate(range.preferred ?? range.maximum) }
        #else
        if on, link == nil {
            guard let viewport = session?.presenter.viewport else { return }
            let l = viewport.displayLink(target: self, selector: #selector(tick(_:)))
            l.add(to: .main, forMode: .common)
            link = l
        } else if !on, let l = link {
            l.invalidate()
            link = nil
        }
        if on, let link {
            let range = Self.requestedRange(maximum: session?.presenter.viewport.window?.screen?.maximumFramesPerSecond ?? 60,
                full: (motion && spatial) || canvas2d || session?.canvases.wantsFrames == true, paint: motion)
            if link.preferredFrameRateRange != range { link.preferredFrameRateRange = range }
            if !ticking { session?.frameHealth.requestedRate(range.preferred ?? range.maximum) }
        }
        #endif
    }
}
