#if os(iOS)
import UIKit
import QuartzCore

/// The app's UI updates on its screen and the work its main thread does, which the card measures while it reads, on
/// iOS 18 and later (LLP 1079.000 §7.4). A passive `UIUpdateLink` takes part in every UI update of the scene, Exact's
/// frames and UIKit's alike, and requests none, nor any rate. Two observers of the main run loop time each spell its
/// thread works without waiting. Stopped, or before iOS 18, it observes nothing.
///
/// Each update is a frame, late when its last Core Animation commit, as seen just after it, comes after UIKit's
/// completion deadline, which UIKit says delays its presentation. A passive observer cannot tell whether a refresh
/// without an update was one the app had nothing to draw for, so frames bound no frame time: the time between two taps'
/// updates is no hitch. Frames held up by work show as the main thread's long spells.
final class FrameHealthScreen {
    /// The main thread's work since it was last taken, leaving out the card's own.
    struct MainThread {
        /// Seconds: the longest spell
        var longest = 0.0
        /// The spells longer than one refresh at the display's highest rate, which would delay a frame at that rate
        var long: UInt32 = 0
    }

    /// The frames, each with its work, the main thread's time from when its update was scheduled until its last Core
    /// Animation commit less its waits between, against UIKit's completion deadline; without a schedule, they have no
    /// frame times
    let health = FrameHealth(policy: .always, source: FrameHealth.screenSource)
    /// The card's charts, which take each frame and each long spell
    weak var chart: FrameHealthChart?
    /// Runs in each update after its `CADisplayLink` callbacks, where an animation makes its changes, and before UIKit
    /// lays out, draws and commits them, so what the card changes goes out with the app's frame
    var willDraw: (() -> Void)?
    /// The app session's workload flags (`FrameHealth.workload`), which each frame records as its update is scheduled
    var workload: (() -> UInt64)?
    /// Seconds: one refresh at the display's highest rate
    private(set) var refresh = 1.0 / 60
    private(set) var running = false
    /// The scene it observes
    private(set) weak var scene: UIWindowScene?
    /// The passive `UIUpdateLink`
    private var link: AnyObject?
    private var observers: [CFRunLoopObserver] = []
    /// An update in progress
    private struct Update {
        /// Seconds: when it was scheduled, and its deadline
        let start: Double, deadline: Double
        /// Who asked for it (`askers`) and the card's state (`FrameHealth.cardFlags`)
        let flags: UInt64
        /// As it began, the nodes' layouts and the seconds of Exact's host phases (`HostPhases.total`)
        let layouts: Int, phases: Double
        /// Seconds: how long the main thread waited and the card worked within it
        var waited = 0.0, card = 0.0
        /// At its last Core Animation commit, as seen just after it, where its work ends: the time, and the host phases'
        /// seconds, the nodes' layouts and the card's seconds then
        var committed: (at: Double, phases: Double, layouts: Int, card: Double)?
    }
    private var update: Update?
    /// When the main thread began waiting, zero while it works
    private var slept = 0.0
    /// When UIKit expects the newest frame on screen
    private var newest = 0.0
    /// When the main thread last woke, zero while it waits
    private var woke = 0.0
    /// Seconds of this spell the card worked on its own, which are not the app's work
    private var cardTime = 0.0
    /// How deep the card's own work nests, so only the outermost is timed
    private var cardDepth = 0
    private var main = MainThread()

    /// Seconds, one chart column: the app draws while its newest frame is due on screen within this long of now
    static let idle = 0.1

    /// Observe `scene`, from a new generation; before iOS 18, nothing.
    func start(_ scene: UIWindowScene) {
        guard #available(iOS 18.0, *), !running, health.enabled else { return }
        running = true
        self.scene = scene
        refresh = 1 / Double(max(1, scene.screen.maximumFramesPerSecond))
        health.reset()
        (update, newest, cardTime, slept, main) = (nil, 0, 0, 0, MainThread())
        woke = CACurrentMediaTime()
        // A late frame's work is split between Exact's host phases, the card and the rest
        HostPhases.shared.hold(true)
        let link = UIUpdateLink(windowScene: scene)
        link.addAction(to: .afterUpdateScheduled) { [weak self] _, info in
            self?.scheduled(presented: info.estimatedPresentationTime, deadline: info.completionDeadlineTime)
        }
        link.addAction(to: .afterCADisplayLinkDispatch) { [weak self] _, _ in self?.willDraw?() }
        for phase in [UIUpdateActionPhase.afterCATransactionCommit, .afterLowLatencyCATransactionCommit] {
            link.addAction(to: phase) { [weak self] _, _ in self?.committed(at: CACurrentMediaTime()) }
        }
        link.addAction(to: .afterUpdateComplete) { [weak self] _, info in self?.completed(presented: info.estimatedPresentationTime) }
        link.isEnabled = true
        self.link = link
        // The thread wakes before any other observer runs and waits after every one has
        let wake = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, .min) { [weak self] _, _ in
            self?.wake(at: CACurrentMediaTime())
        }
        let wait = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, .max) { [weak self] _, _ in
            self?.wait(at: CACurrentMediaTime())
        }
        observers = [wake, wait].compactMap { $0 }
        for observer in observers { CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes) }
    }

    func stop() {
        guard running else { return }
        running = false
        if #available(iOS 18.0, *) { (link as? UIUpdateLink)?.isEnabled = false }
        link = nil
        for observer in observers { CFRunLoopObserverInvalidate(observer) }
        observers = []
        update = nil
        health.stop()
        HostPhases.shared.hold(false)
    }

    deinit { stop() }

    /// Whether the app draws now: its newest frame is due on screen within a tenth of a second of now.
    var drawing: Bool { running && newest > 0 && CACurrentMediaTime() - newest < Self.idle }

    /// Do the card's own work, whose time is then no part of the main thread's work the card gives, so the card
    /// cannot plot itself or keep its charts moving with it, while app work in the same spell still counts.
    func card(_ work: () -> Void) {
        guard cardDepth == 0 else { work(); return }
        cardDepth += 1
        defer { cardDepth -= 1 }
        let begin = CACurrentMediaTime()
        work()
        card(took: CACurrentMediaTime() - begin)
    }

    /// The card worked on its own for `seconds`.
    func card(took seconds: Double) {
        guard running else { return }
        cardTime += seconds
        update?.card += seconds
    }

    /// The main thread's work since the last call.
    func takeMainThread() -> MainThread {
        defer { main = MainThread() }
        return main
    }

    func scheduled(presented: Double, deadline: Double, now: Double = CACurrentMediaTime()) {
        let phases = HostPhases.shared
        update = Update(start: now, deadline: deadline, flags: Self.askers(workload?() ?? 0) | health.cardFlags,
            layouts: phases.counted(.nodeLayouts), phases: phases.total)
    }

    /// The update's Core Animation transaction was committed, as seen at `now`, just after.
    func committed(at now: Double = CACurrentMediaTime()) {
        guard let card = update?.card else { return }
        let phases = HostPhases.shared
        update?.committed = (now, phases.total, phases.counted(.nodeLayouts), card)
    }

    /// The update completed at `now`, and UIKit expects it on screen at `presented`.
    func completed(presented: Double, now: Double = CACurrentMediaTime()) {
        // Its work ends at its last commit, or with none seen, at its completion
        if update?.committed == nil { committed(at: now) }
        guard let update, let end = update.committed else { return }
        self.update = nil
        let finished = end.at
        // Its work is the main thread's time in it, less its waits, taken to begin that much after the update was
        // scheduled and to end at the commit
        let start = min(finished, update.start + update.waited)
        // Its work split, for a late frame the capture keeps
        if health.captureID != 0, finished > update.deadline {
            health.split(at: start, waited: update.waited, exact: end.phases - update.phases, card: end.card,
                layouts: end.layouts - update.layouts)
        }
        health.record(arrival: start, target: update.deadline, period: 0, finished: finished,
            flags: health.captureID == 0 ? 0 : health.environmentFlags | update.flags)
        chart?.screenFrame(at: presented, work: finished - start, budget: update.deadline - start)
        newest = max(newest, presented)
    }

    /// What asked for an update as it was scheduled: the app session's `workload`, and, from bit 40, each of Exact's
    /// frame clock's users asking for frames (`FrameClock.Order`), for any session in the process. With none of
    /// Exact's, UIKit or the system asked.
    static func askers(_ workload: UInt64) -> UInt64 { workload | FrameClock.shared.asking << 40 }

    /// The main thread woke at `now`, as the main run loop's first observer sees.
    func wake(at now: Double) {
        // UIKit waits between an update's scheduling and its event dispatch (`UIUpdateActionPhase.h`)
        if slept > 0 { update?.waited += now - slept }
        (woke, slept, cardTime) = (now, 0, 0)
    }

    /// The main thread is about to wait at `now`, as the main run loop's last observer sees: the spell since it woke,
    /// less the card's own time, is the app's work.
    func wait(at now: Double) {
        slept = now
        let (start, length) = (woke, max(0, now - woke - cardTime))
        woke = 0
        guard start > 0 else { return }
        main.longest = max(main.longest, length)
        guard length > refresh else { return }
        main.long = main.long == .max ? .max : main.long + 1
        chart?.busy(start: now - length, end: now, budget: refresh)
    }
}
#endif
