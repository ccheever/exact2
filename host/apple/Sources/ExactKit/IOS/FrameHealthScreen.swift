#if os(iOS)
import UIKit
import QuartzCore

/// The app's UI updates on its screen and the work its main thread does, which the card measures while it reads, on
/// iOS 18 and later (LLP 1079.000 §7.4). A passive `UIUpdateLink` takes part in every UI update of the scene, Exact's
/// frames and UIKit's alike, and requests none, nor any rate. Two observers of the main run loop time each spell its
/// thread works without waiting. Stopped, or before iOS 18, it observes nothing.
///
/// Each update is a frame, late when its changes reach the render server after UIKit's completion deadline. A passive
/// observer cannot tell whether a refresh without an update was one the app had nothing to draw for, so frames bound
/// no frame time: the time between two taps' updates is no hitch. Frames held up by work show as the main thread's
/// long spells.
final class FrameHealthScreen {
    /// The main thread's work since it was last taken, leaving out the card's own.
    struct MainThread {
        /// Seconds: the longest spell
        var longest = 0.0
        /// The spells longer than one refresh at the display's highest rate, which would delay a frame at that rate
        var long: UInt32 = 0
    }

    /// The frames, each with its work, from when its update began until its last Core Animation transaction was
    /// committed, against UIKit's completion deadline; without a schedule, they have no frame times
    let health = FrameHealth(policy: .always, source: "ui-update")
    /// The card's charts, which take each frame and each long spell
    weak var chart: FrameHealthChart?
    /// Runs in each update before its Core Animation transaction is committed, so what the card changes goes out with
    /// the app's frame
    var willCommit: (() -> Void)?
    /// Seconds: one refresh at the display's highest rate
    private(set) var refresh = 1.0 / 60
    private(set) var running = false
    /// The scene it observes
    private(set) weak var scene: UIWindowScene?
    /// The passive `UIUpdateLink`
    private var link: AnyObject?
    private var observers: [CFRunLoopObserver] = []
    /// The update in progress: when it began, its deadline, and when its last transaction was committed
    private var update: (start: Double, deadline: Double, committed: Double)?
    /// When UIKit expects the newest frame on screen
    private var newest = 0.0
    /// When the main thread last woke, zero while it waits
    private var woke = 0.0
    /// When the card last changed on its own, outside any update, so the update that sends the change may be the
    /// card's alone; zero once an update has begun since
    private var own = 0.0
    /// Seconds of this spell the card worked on its own, which are not the app's work, and when the update in progress
    /// began if it is the card's alone
    private var cardTime = 0.0, cardUpdate = 0.0
    /// How deep the card's own work nests, so only the outermost is timed
    private var cardDepth = 0
    private var main = MainThread()

    /// Seconds: an update this long after the app's newest frame, following a change of the card's own, sends only that
    /// change; a chart column
    static let idle = 0.1

    /// Observe `scene`, from a new generation; before iOS 18, nothing.
    func start(_ scene: UIWindowScene) {
        guard #available(iOS 18.0, *), !running, health.enabled else { return }
        running = true
        self.scene = scene
        refresh = 1 / Double(max(1, scene.screen.maximumFramesPerSecond))
        health.reset()
        (update, newest, own, cardTime, cardUpdate, main) = (nil, 0, 0, 0, 0, MainThread())
        woke = CACurrentMediaTime()
        let link = UIUpdateLink(windowScene: scene)
        link.addAction(to: .afterUpdateScheduled) { [weak self] _, info in
            self?.scheduled(presented: info.estimatedPresentationTime, deadline: info.completionDeadlineTime)
        }
        link.addAction(to: .beforeCATransactionCommit) { [weak self] _, _ in self?.willCommit?() }
        for phase in [UIUpdateActionPhase.afterCATransactionCommit, .afterLowLatencyCATransactionCommit] {
            link.addAction(to: phase) { [weak self] _, _ in self?.update?.committed = CACurrentMediaTime() }
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

    /// The card worked on its own for `seconds`, outside an update of its own alone, which counts as the card's whole.
    func card(took seconds: Double) { if running, cardUpdate == 0 { cardTime += seconds } }

    /// The card changed on its own, outside any update. The next update, while the app does not draw, sends only the
    /// card's change, so it is not one of the app's frames, and its time is the card's. A change that sends no update
    /// within a tenth of a second claims none.
    func ownChange() { own = CACurrentMediaTime() }

    /// The main thread's work since the last call.
    func takeMainThread() -> MainThread {
        defer { main = MainThread() }
        return main
    }

    private func scheduled(presented: Double, deadline: Double) {
        let now = CACurrentMediaTime()
        let cardOnly = own > 0 && now - own < Self.idle && (newest == 0 || presented - newest >= Self.idle)
        own = 0
        if cardOnly { (update, cardUpdate) = (nil, now) } else { update = (now, deadline, 0) }
    }

    /// The update completed, and UIKit expects it on screen at `presented`.
    private func completed(presented: Double) {
        if cardUpdate > 0 { (cardTime, cardUpdate) = (cardTime + CACurrentMediaTime() - cardUpdate, 0) }
        guard let update else { return }
        self.update = nil
        let finished = update.committed > 0 ? update.committed : CACurrentMediaTime()
        health.record(arrival: update.start, target: update.deadline, period: 0, finished: finished)
        chart?.screenFrame(at: presented, work: finished - update.start, budget: update.deadline - update.start)
        newest = max(newest, presented)
    }

    /// The main thread woke at `now`, as the main run loop's first observer sees.
    func wake(at now: Double) {
        // An update begins and completes within one spell
        (woke, cardTime, cardUpdate) = (now, 0, 0)
    }

    /// The main thread is about to wait at `now`, as the main run loop's last observer sees: the spell since it woke,
    /// less the card's own time, is the app's work.
    func wait(at now: Double) {
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
