// Observations for timer admission, sampled after every applied batch (LLP 1147).
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

final class TimerAdmissions {
    struct Observation {
        var now: Double
        var lastUpdate = -Double.infinity
        var frame = false, update = false, hidden = false
        var scrolling = false, motion = false, tasks = false, canvas = false, applying = false
        var waits: [Double]
        var next: Double?
        var recentUpdate: Bool { now <= lastUpdate + 1000 / 60 }
        var activeScroll: Bool { !hidden && scrolling }
        var demand: Bool { motion || tasks || canvas || waits.first == -2 || activeScroll || recentUpdate }
        var kinds: UInt32 {
            var result: UInt32 = 0
            if frame || hidden { result |= 2 }
            if update || (frame && lastUpdate == -Double.infinity) || !demand || hidden { result |= 4 }
            if !activeScroll { result |= 8 }
            return result
        }
        var eligible: Bool {
            waits.enumerated().contains { index, due in
                (due >= 0 && due <= now) || (due == -2 && kinds & (2 << index) != 0)
            }
        }
        var idle: Bool {
            waits.count == 4 && waits[3] == -2 && !waits.prefix(3).contains(-2)
                && !activeScroll && !demand && !applying && (next.map { $0 > now + 1000 / 60 } ?? true)
        }
    }
    weak var session: ExactSession?
    private(set) var enabled = false
    private var waits: [Double] = []
    private var wake: Timer?
    private var observer: CFRunLoopObserver?
    private var generation = 0
    private var epoch: Int?
    private var checking = false
    private var lastUpdate = -Double.infinity
    private var pending: (cutoff: Double, update: Bool)?
    #if canImport(UIKit)
    private var updateLink: AnyObject?
    private weak var updateView: UIView?
    #endif

    init(session: ExactSession) { self.session = session }
    deinit {
        wake?.invalidate()
        if let observer { CFRunLoopRemoveObserver(CFRunLoopGetMain(), observer, .defaultMode) }
    }
    #if DEBUG
    var hasPendingForTests: Bool { pending != nil }
    #endif
    var wantsFrame: Bool { waits.first == -2 && !PageFacts.hidden }
    func takeover() {
        pending = nil
        generation += 1; wake?.invalidate(); wake = nil; lastUpdate = -Double.infinity
        if let observer { CFRunLoopRemoveObserver(CFRunLoopGetMain(), observer, .defaultMode) }; observer = nil
        #if canImport(UIKit)
        if #available(iOS 18, tvOS 18, *) { (updateLink as? UIUpdateLink)?.isEnabled = false }
        updateLink = nil; updateView = nil
        #endif
    }
    func update(_ values: [Double]) {
        guard let s = session else { return }
        if epoch != s.generation { takeover(); epoch = s.generation }
        if values.isEmpty {
            if enabled { enabled = false; waits = []; takeover(); _ = s.runtime.timerWaits(kinds: 0, cutoff: s.now(), mode: 0) }
            return
        }
        waits = values
        if pending != nil { DispatchQueue.main.async { [weak self] in self?.check() } }
        if !enabled {
            enabled = true
            s.apply(s.runtime.timerWaits(kinds: 30, cutoff: s.now(), mode: 0))
        }
        if s.clock == nil, !ExactEnv.agentFreezes {
            let token = generation
            if observer == nil {
                observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, 0) { [weak self] _, _ in
                    guard let self, self.generation == token else { return }
                    self.check(idle: true)
                }
                CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .defaultMode)
            }
            #if canImport(UIKit)
            if #available(iOS 18, tvOS 18, *), let view = s.view, updateView !== view {
                (updateLink as? UIUpdateLink)?.isEnabled = false
                let link = UIUpdateLink(view: view)
                link.addAction(to: .afterUpdateScheduled) { [weak self] _, _ in
                    guard let self, self.generation == token, let session = self.session else { return }
                    self.lastUpdate = session.now()
                    self.check(update: true)
                }
                link.isEnabled = true; updateLink = link; updateView = view
            }
            #endif
        }
        #if canImport(UIKit)
        for node in s.presenter.views.values {
            guard let scroll = node.scrollView, !(scroll.gestureRecognizers ?? []).contains(where: { $0 is TimerScrollTouch }) else { continue }
            scroll.addGestureRecognizer(TimerScrollTouch { [weak self] in self?.check() })
        }
        #endif
        wake?.invalidate(); wake = nil
        guard s.clock == nil, !ExactEnv.agentFreezes, let due = waits.filter({ $0 >= 0 }).min() else { return }
        let token = generation
        wake = SessionClockTimer.schedule(after: max(0, due - s.now()) / 1000) { [weak self] _ in
            guard let self, self.generation == token else { return }
            self.check()
        }
    }
    private func scrolling(_ s: ExactSession) -> Bool {
        guard !PageFacts.hidden else { return false }
        return s.presenter.views.values.contains { node in
            #if canImport(UIKit)
            let scroll = node.scrollView
            let held = scroll?.gestureRecognizers?.contains { ($0 as? TimerScrollTouch)?.held == true } == true
            return held || scroll?.isTracking == true || scroll?.isDragging == true || scroll?.isDecelerating == true || node.scrollEnd?.owed == true
            #else
            return node.scrollEnd?.live == true || node.scrollEnd?.owed == true
            #endif
        }
    }
    func check(frame: Bool = false, update: Bool = false, idle: Bool = false, at: Double? = nil) {
        guard enabled, !checking, let s = session, s.state != .destroyed, s.clock == nil, !ExactEnv.agentFreezes,
              !s.applying, !s.ownerInFlight else { return }
        checking = true
        defer { checking = false }
        let cutoff = pending?.cutoff ?? at ?? s.now(), update = pending?.update ?? update
        guard pending != nil || waits.contains(-2) || waits.contains(where: { $0 >= 0 && $0 <= cutoff }) else { return }
        func observe() -> Observation {
            Observation(now: max(cutoff, s.now()), lastUpdate: lastUpdate, frame: frame, update: update, hidden: PageFacts.hidden,
                        scrolling: scrolling(s), motion: s.frames.motion, tasks: s.frames.tasks, canvas: s.canvases.wantsFrames,
                        applying: s.applying || s.ownerInFlight, waits: waits, next: [s.timerDue, s.taskTimers.nextDue].compactMap { $0 }.min())
        }
        if pending != nil || observe().eligible || s.timerDue.map({ $0 <= cutoff }) == true {
            // Post native UI admission without waiting on layout; Exact's own frame work keeps its ordering barrier
            if ExactSession.asyncFills, !frame, !idle, s.frames.link == nil {
                pending = (cutoff, update)
                s.advanceInFlight = true
                let token = generation, epoch = s.generation
                let kinds = observe().kinds
                DispatchQueue.main.async { [weak self, weak s] in
                    guard let s else { return }
                    guard let self, self.generation == token, s.generation == epoch, s.state != .destroyed else {
                        s.advanceInFlight = false
                        if let self, self.generation == token { self.pending = nil }
                        return
                    }
                    s.runtime.timerWaitsAsync(kinds: kinds, cutoff: cutoff) { [weak self, weak s] batch in
                        s?.publish(.advance, batch, epoch)
                        DispatchQueue.main.async { [weak self, weak s] in
                            guard let self, let s, self.generation == token, s.generation == epoch, s.state != .destroyed else { return }
                            if batch.waitContinue, !batch.busy, !batch.held { self.check() }
                            else { self.pending = nil; self.scheduleRecentWake() }
                        }
                    }
                }
                return
            }
            repeat {
                let batch = s.runtime.timerWaits(kinds: observe().kinds, cutoff: cutoff)
                s.applyUnlessEmpty(batch)
                if !batch.waitContinue || batch.busy || batch.held { break }
            } while true
            pending = nil
        }
        if idle, observe().idle {
            let batch = s.runtime.timerWaits(kinds: 16, cutoff: cutoff)
            s.applyUnlessEmpty(batch)
            if waits[3] == -2, batch.error == nil, !batch.busy, !batch.held {
                let token = generation
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.generation == token else { return }
                    self.check()
                }
            }
        }
        scheduleRecentWake()
    }
    private func scheduleRecentWake() {
        guard let s = session else { return }
        if waits.count == 4, (waits[1] == -2 || waits[3] == -2), lastUpdate.isFinite,
           s.now() <= lastUpdate + 1000 / 60, wake == nil {
            let token = generation
            wake = SessionClockTimer.schedule(after: max(0.000001, (lastUpdate + 1000 / 60 - s.now()) / 1000)) { [weak self] _ in
                guard let self, self.generation == token else { return }
                self.wake = nil; self.check()
            }
        }
    }
}

#if canImport(UIKit)
// Observe touches without recognizing, delaying, cancelling or preventing gestures
private final class TimerScrollTouch: UIGestureRecognizer, UIGestureRecognizerDelegate {
    private var touches: Set<ObjectIdentifier> = []
    private let changed: () -> Void
    var held: Bool { !touches.isEmpty }
    init(changed: @escaping () -> Void) {
        self.changed = changed
        super.init(target: nil, action: nil)
        delegate = self
        cancelsTouchesInView = false; delaysTouchesBegan = false; delaysTouchesEnded = false
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool { true }
    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) { for touch in touches { self.touches.insert(ObjectIdentifier(touch)) } }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { finish(touches) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { finish(touches) }
    private func finish(_ touches: Set<UITouch>) {
        for touch in touches { self.touches.remove(ObjectIdentifier(touch)) }
        if self.touches.isEmpty { DispatchQueue.main.async { [weak self] in self?.changed() } }
    }
    override func reset() { super.reset(); touches.removeAll(); DispatchQueue.main.async { [weak self] in self?.changed() } }
}
#endif
