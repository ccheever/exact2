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
/// has something to render (LLP 1009 D4), per session.
final class Frames: NSObject {
    weak var session: ExactSession?
    var link: CADisplayLink?
    var motion = false, spatial = false
    /// A 2D canvas asked for a frame (LLP 1056 D5): ticks run while it does.
    var canvas2d = false
    var timerSoon = false
    /// A frame task (LLP 1073 D5): each tick is the runtime's frame at the tick's target time.
    var tasks = false
    private var canvasRequested = false

    /// Input and reads ask for one frame; an agent-owned clock never self-reschedules.
    func requestCanvas() {
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
        s.canvases.lifecycle.frame()
        // Motion keeps its existing sampling clock; canvas frames target presentation.
        let frameNow = s.clock ?? (link.targetTimestamp - ExactEnv.t0) * 1000
        // ProMotion changes callback cadence (e.g. 120 → 80 Hz) while duration
        // can remain the nominal base interval. The target interval is actual;
        // canvases quantizes it and republishes this session’s stable class before rendering.
        s.canvases.period((link.targetTimestamp - link.timestamp) * 1000)
        let previous = s.canvases.frameNow
        s.canvases.frameNow = frameNow
        defer { s.canvases.frameNow = previous }
        // Frame-precise timers: the first frame at or past the deadline fires
        // them. A frame before it would advance to no timer, and its commit
        // and presenter pass cost a list in motion a report a frame.
        // A slice building on the owner holds this frame's timers and tick:
        // they run on the first frame after it lands (LLP 1072 §7.1).
        if !s.fillInFlight {
            if timerSoon, !ExactEnv.agentMode, s.clock == nil {
                let now = s.now()
                s.followOffset()
                if tasks { s.apply(s.runtime.frame(now: frameNow)) }
                else if s.timerDue.map({ now >= $0 }) ?? true { s.apply(s.runtime.advance(now: now)) }
            }
            if motion || canvas2d {
                if ExactSession.asyncFills {
                    if !s.tickInFlight { s.sendTick(now: s.now()) }
                } else {
                    s.apply(s.runtime.tick(now: s.now()))
                }
            }
        }
        let more = s.canvases.tick(now: frameNow)
        run(motion || canvas2d || timerSoon || more || s.canvases.wantsFrames || s.canvases.lifecycle.needsRetry)
    }

    func run(_ wanted: Bool) {
        if wanted, session?.clock != nil { requestCanvas() }
        let on = wanted && session?.clock == nil
        if on, link == nil {
            #if canImport(UIKit)
            let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
            #else
            guard let viewport = session?.presenter.viewport else { return }
            let l = viewport.displayLink(target: self, selector: #selector(tick(_:)))
            #endif
            l.add(to: .main, forMode: .common)
            link = l
        } else if !on, let l = link {
            l.invalidate()
            link = nil
        }
        #if canImport(UIKit)
        if let link {
            // Motion that changes place or size asks for the panel's full
            // rate while it runs, as a canvas does: at `.default` a ProMotion
            // iPhone presents a slide at 60 Hz. A fade or a colour change
            // reads the same at 60, so paint-only motion — a breathing loop
            // that runs for minutes — asks no more. The link exists only
            // while something wants frames, so an idle app drops to no link
            // at all (LLP 1061 D4).
            let fullRate = (motion && spatial) || session?.canvases.wantsFrames == true
            let rate = Float(min(120, session?.presenter.viewport.window?.screen.maximumFramesPerSecond ?? 60))
            link.preferredFrameRateRange = fullRate ? CAFrameRateRange(minimum: min(80, rate), maximum: rate, preferred: rate)
                : motion ? CAFrameRateRange(minimum: min(30, rate), maximum: min(60, rate), preferred: min(60, rate)) : .default
        }
        #endif
    }
}
