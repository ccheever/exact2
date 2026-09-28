// UIKit moves the scrollport; row construction follows outside its layout pass.
// @ref LLP 1044.000 §6 S5; LLP 1010 §6 — visible-only rescue, bounded lead.
#if os(iOS)
import UIKit

final class ScrollPump: NSObject, UIScrollViewDelegate {
    private weak var presenter: Presenter?
    private var link: CADisplayLink?
    private lazy var target = ScrollPumpTarget(self)
    private var queued = false
    private var epoch = 0
    private var inScroll = false
    private var travel: [UInt32: Travel] = [:]
    private var costs: [UInt32: FillCost] = [:]
    private var textPending = false
    private var refreshInterval = 1.0 / 60
    private var lastScroll = -Double.infinity
    private var lastSlice = -Double.infinity

    /// When the main run loop last woke: what this turn has already spent
    /// (a long timer or display-link handler before the pump's slice).
    private var turnStarted = CACurrentMediaTime()
    private var turnObserver: CFRunLoopObserver?
    /// A slice left its rows to the next one; that one builds them.
    private var fillDeferred = false

    init(_ presenter: Presenter) {
        self.presenter = presenter
        super.init()
        let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, 0) { [weak self] _, _ in
            self?.turnStarted = CACurrentMediaTime()
        }
        CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
        turnObserver = observer
    }
    deinit {
        link?.invalidate()
        restTimer?.invalidate()
        RasterWorkers.shared.travelling(self, false)
        if let turnObserver { CFRunLoopRemoveObserver(CFRunLoopGetMain(), turnObserver, .commonModes) }
    }
    var sliceBudget: TimeInterval { min(0.004, max(0.001, refreshInterval * 0.24)) }
    /// Seconds a list must be still before what it cached for travel is
    /// let go (`ExactSession.rest`).
    static let restDelay = 2.0
    private var restTimer: Timer?
    /// Travel (points/s) past which images decode one at a time.
    static let fastTravel = 20_000.0

    private struct Travel { var top: CGFloat, time: TimeInterval, velocity = 0.0 }
    private struct FillCost {
        var perRow: TimeInterval?
        var lastRows = 1
        mutating func record(_ seconds: TimeInterval, rows: Int) {
            guard rows > 0 else { return }
            let sample = max(0.000001, seconds / Double(rows))
            perRow = max(sample, (perRow ?? sample) * 0.75 + sample * 0.25)
            lastRows = rows
        }
        func rows(in seconds: TimeInterval) -> UInt32 {
            guard seconds > 0 else { return 0 }
            guard let perRow else { return 1 }
            return UInt32(max(0, min(Double(lastRows) * 2, floor(seconds * 0.9 / perRow), Double(UInt32.max - 1))))
        }
    }
    func velocity(_ id: UInt32) -> Double {
        guard !ExactEnv.agentFreezes, let t = travel[id], CACurrentMediaTime() - t.time < 0.15 else { return 0 }
        return t.velocity
    }
    private func sample(_ node: NodeView, now: TimeInterval) {
        guard let scroll = node.scroll else { return }
        // Along the list's own axis (LLP 1070 H3): a row list travels on x.
        let horizontal = presenter?.collections.entries[node.id]?.snapshot.horizontal == true
        let top = horizontal ? scroll.contentOffset.x : scroll.contentOffset.y
        var t = travel[node.id] ?? Travel(top: top, time: now)
        let delta = top - t.top, elapsed = now - t.time
        // A step longer than the port is a jump, not travel: nothing to lead.
        if abs(delta) > (horizontal ? scroll.bounds.width : scroll.bounds.height) {
            travel[node.id] = Travel(top: top, time: now)
            return
        }
        if delta != 0, elapsed > 0 {
            let speed = Double(delta) / max(elapsed, refreshInterval / 2)
            t.velocity = elapsed > 0.15 || speed * t.velocity <= 0 ? speed : t.velocity * 0.5 + speed * 0.5
            t.top = top; t.time = now
        }
        travel[node.id] = t
    }
    /// A scroller moved: sample a collection's travel and schedule its text.
    func scrolled(_ node: NodeView?) {
        guard let p = presenter, !p.applying, !inScroll else { return }
        inScroll = true
        defer { inScroll = false }
        lastScroll = CACurrentMediaTime()
        armRest(after: Self.restDelay)
        // A correction's move is not the reader's travel (LLP 1070.000 §2.5).
        if let node, p.collections.owns(node.id), !p.collections.correcting { sample(node, now: lastScroll) }
        p.leaves.scrolled()
        // What this frame shows has its text before it commits.
        p.paintVisibleText()
        textPending = true
        scheduleAfterScroll()
        start()
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) { scrolled(nil) }

    func batchApplied() {
        // A list that never scrolls, or whose rows change in place (live
        // prices), leaves launch's pixels and replaced labels in the caches
        // just the same: rest after a change as after a scroll.
        armRest(after: Self.restDelay)
        textPending = true
        start()
    }
    func requestText() { textPending = true; start() }
    /// A collection owes a report (LLP 1050.000): the next slice builds it.
    func requestFill() { scheduleAfterScroll(); start() }
    /// Build each owed collection's rows for this slice: as many as its
    /// measured per-row cost fits, at least one, and at least what the next
    /// two frames of travel uncover — so the scroll callback that follows
    /// finds its rows built instead of building them itself. A turn that
    /// has already spent half its frame before the slice builds nothing
    /// while the next frame's travel is covered: its rows wait for the next
    /// slice, which builds them, rather than making this frame late.
    @discardableResult
    private func fillCollections(deadline: TimeInterval) -> Int {
        guard let p = presenter else { return 0 }
        var rows = 0
        let spent = !fillDeferred && CACurrentMediaTime() - turnStarted > refreshInterval * 0.5
        fillDeferred = false
        for id in p.collections.fillPending.sorted() {
            let started = CACurrentMediaTime()
            let fits = (costs[id] ?? FillCost()).rows(in: deadline - started)
            let needed = p.collections.rowsToCover(id, ahead: CGFloat(velocity(id) * refreshInterval * 2))
            if spent && p.collections.rowsToCover(id, ahead: CGFloat(velocity(id) * refreshInterval)) == 0 {
                fillDeferred = true; continue
            }
            let created = p.collections.fillSlice(id, limit: max(1, fits, needed))
            costs[id, default: FillCost()].record(CACurrentMediaTime() - started, rows: created)
            rows += created
        }
        return rows
    }
    private func start() {
        guard link == nil else { return }
        let value = CADisplayLink(target: target, selector: #selector(ScrollPumpTarget.tick(_:)))
        let maximum = Float(presenter?.viewport.window?.screen.maximumFramesPerSecond ?? 60)
        value.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, maximum), maximum: maximum, preferred: maximum)
        value.add(to: .main, forMode: .common)
        link = value
    }
    /// Once scrolling has been still for `restDelay` after a scroll or a
    /// batch, the session trims its caches to what shows, as a browser
    /// discards the decoded images of content it scrolled past; what comes
    /// back into view is made again.
    private func armRest(after delay: TimeInterval) {
        guard restTimer == nil else { return }
        restTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
            guard let self else { return }
            restTimer = nil
            let still = CACurrentMediaTime() - lastScroll
            if still < Self.restDelay { armRest(after: Self.restDelay - still); return }
            // Cold shaped text is held to the screens a scroll passes.
            if let p = presenter { p.session?.text.fitShaped(visibleParagraphs: p.textViews.values.lazy.filter { p.textIsVisible($0) }.count) }
            presenter?.session?.rest()
        }
    }
    private func stop() { link?.invalidate(); link = nil; RasterWorkers.shared.travelling(self, false) }
    private func scheduleAfterScroll() {
        guard !queued else { return }
        queued = true
        let generation = epoch
        DispatchQueue.main.async { [weak self] in
            guard let self, self.epoch == generation else { return }
            self.queued = false
            self.pump()
        }
    }
    fileprivate func tick(_ link: CADisplayLink) {
        let interval = link.targetTimestamp - link.timestamp
        if interval > 0 { refreshInterval = interval }
        // UIKit's offset callback runs in layout. During travel its queued
        // slice owns the work, after layout returns; the link is the idle wake.
        guard CACurrentMediaTime() - lastScroll >= refreshInterval * 1.5 else { return }
        pump()
    }
    private func pump() {
        guard let p = presenter, !p.applying else { return }
        let now = CACurrentMediaTime()
        guard now - lastSlice >= refreshInterval * 0.8 else { return }
        lastSlice = now
        let deadline = now + sliceBudget
        if !p.collections.fillPending.isEmpty {
            let post = Presenter.signposts.beginInterval("pump-collection")
            let rows = fillCollections(deadline: deadline)
            Presenter.signposts.endInterval("pump-collection", post, "rows=\(rows)")
        }
        if textPending {
            let post = Presenter.signposts.beginInterval("pump-text")
            // Lead the fastest list's travel.
            let fastest = p.listViews.keys.map { velocity($0) }.max { abs($0) < abs($1) } ?? 0
            textPending = p.refreshVisibleText(deadline: deadline, velocity: CGFloat(fastest), interval: refreshInterval)
            Presenter.signposts.endInterval("pump-text", post)
        }
        // One image decode at a time while a list travels fast.
        RasterWorkers.shared.travelling(self, p.listViews.keys.contains { abs(velocity($0)) > Self.fastTravel })
        if !textPending && p.collections.fillPending.isEmpty { stop() }
    }
    /// Agent reads keep their settled contract, outside the scroll callback.
    func settle() {
        for _ in 0..<8 {
            presenter?.collections.settle()
            guard let collections = presenter?.collections, !collections.fillPending.isEmpty else { break }
            for id in collections.fillPending.sorted() { collections.fillSlice(id, limit: UInt32.max - 1) }
        }
        presenter?.collections.settle()
        textPending = presenter?.refreshVisibleText() ?? false
        if !textPending { stop() }
    }
    func reset() {
        stop(); epoch += 1; queued = false
        restTimer?.invalidate(); restTimer = nil
        travel.removeAll(); costs.removeAll()
        textPending = false
        lastScroll = -.infinity; lastSlice = -.infinity
    }
    func forget(_ id: UInt32) {
        travel[id] = nil; costs[id] = nil
    }

}
private final class ScrollPumpTarget: NSObject {
    private weak var pump: ScrollPump?
    init(_ pump: ScrollPump) { self.pump = pump }
    @objc func tick(_ link: CADisplayLink) { pump?.tick(link) }
}
#endif
