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
    private var reporting = false
    private var inScroll = false
    private var batchPending = false
    private var pending = Set<UInt32>()
    private var geometry: [UInt32: [Double]] = [:]
    private var covers: [UInt32: Cover] = [:]
    private var travel: [UInt32: Travel] = [:]
    private var costs: [UInt32: FillCost] = [:]
    private var textPending = false
    private var refreshInterval = 1.0 / 60
    private var lastScroll = -Double.infinity
    private var lastSlice = -Double.infinity
    private var finalizationCost = 0.0005

    init(_ presenter: Presenter) { self.presenter = presenter }
    deinit { link?.invalidate() }
    var sliceBudget: TimeInterval { min(0.004, max(0.001, refreshInterval * 0.24)) }

    private struct Travel { var top: CGFloat, time: TimeInterval, velocity = 0.0 }
    private struct Cover {
        var first: CGFloat, last: CGFloat
        var atStart: Bool, atEnd: Bool
    }
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
        let top = scroll.contentOffset.y
        var t = travel[node.id] ?? Travel(top: top, time: now)
        let delta = top - t.top, elapsed = now - t.time
        if delta != 0, elapsed > 0 {
            let speed = Double(delta) / max(elapsed, refreshInterval / 2)
            t.velocity = elapsed > 0.15 || speed * t.velocity <= 0 ? speed : t.velocity * 0.5 + speed * 0.5
            t.top = top; t.time = now
        }
        travel[node.id] = t
    }
    private func isLegacy(_ node: NodeView) -> Bool {
        node.kind == "list" && (node.props["itemHeight"] != nil || node.props["estimatedItemHeight"] != nil)
            && presenter?.collections.owns(node.id) == false
    }

    /// Constant work for this scroller while its lead covers the viewport.
    /// Even the rescue report admits zero overscan rows (ABI limit 1).
    func scrolled(_ node: NodeView?) {
        guard let p = presenter, !p.applying, !inScroll else { return }
        inScroll = true
        defer { inScroll = false }
        lastScroll = CACurrentMediaTime()
        if let node, p.collections.owns(node.id) { sample(node, now: lastScroll) }
        if let node, isLegacy(node), let scroll = node.scroll {
            sample(node, now: lastScroll)
            let port = scroll.bounds
            let cover = covers[node.id]
            if cover == nil || (!cover!.atStart && port.minY < cover!.first)
                || (!cover!.atEnd && port.maxY > cover!.last) {
                syncLists(limit: 1, only: node.id)
            } else if let cover {
                let bias = CGFloat(max(-Double(port.height) * 0.75, min(Double(port.height) * 0.75, velocity(node.id) * 0.1)))
                if (!cover.atStart && port.minY - port.height + bias - 1 < cover.first)
                    || (!cover.atEnd && port.maxY + port.height + bias + 1 > cover.last) { pending.insert(node.id) }
            }
        }
        textPending = true
        scheduleAfterScroll()
        start()
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) { scrolled(nil) }

    func batchApplied() {
        batchPending = true
        guard !reporting else { return }
        syncLists(limit: ExactEnv.agentFreezes ? 0 : 1)
    }
    private func finishBatch() {
        guard let p = presenter else { return }
        batchPending = false
        covers.removeAll(keepingCapacity: true)
        for node in p.listViews.values where isLegacy(node) {
            guard let content = node.container.subviews.first as? NodeView else { continue }
            let rows = content.container.subviews.compactMap { $0 as? NodeView }
            let positions = rows.compactMap { Int($0.props["accessibilityPosInSet"] ?? "") }.sorted()
            // A distant focus/interaction pin cannot certify the gap it spans.
            guard !positions.isEmpty, positions.count == rows.count,
                  zip(positions, positions.dropFirst()).allSatisfy({ $1 == $0 + 1 }) else { continue }
            guard let first = rows.min(by: { $0.frame.minY < $1.frame.minY }),
                  let last = rows.max(by: { $0.frame.maxY < $1.frame.maxY }) else { continue }
            covers[node.id] = Cover(first: first.frame.minY + content.frame.minY,
                last: last.frame.maxY + content.frame.minY,
                atStart: first.props["accessibilityPosInSet"] == "1",
                atEnd: last.props["accessibilityPosInSet"] == last.props["accessibilitySetSize"])
        }
        textPending = true
        start()
    }
    func requestText() { textPending = true; start() }
    /// A collection owes a report (LLP 1050.000): the next slice builds it.
    func requestFill() { scheduleAfterScroll(); start() }
    /// Build each owed collection's rows for this slice, as many as its
    /// measured per-row cost fits, and at least one.
    private func fillCollections(deadline: TimeInterval) {
        guard let p = presenter else { return }
        for id in p.collections.fillPending.sorted() {
            let started = CACurrentMediaTime()
            let fits = (costs[id] ?? FillCost()).rows(in: deadline - started)
            let created = p.collections.fillSlice(id, limit: max(1, fits))
            costs[id, default: FillCost()].record(CACurrentMediaTime() - started, rows: created)
        }
    }
    private func start() {
        guard link == nil else { return }
        let value = CADisplayLink(target: target, selector: #selector(ScrollPumpTarget.tick(_:)))
        let maximum = Float(presenter?.viewport.window?.screen.maximumFramesPerSecond ?? 60)
        value.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, maximum), maximum: maximum, preferred: maximum)
        value.add(to: .main, forMode: .common)
        link = value
    }
    private func stop() { link?.invalidate(); link = nil }
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
        guard let p = presenter, !p.applying, !reporting else { return }
        let now = CACurrentMediaTime()
        guard now - lastSlice >= refreshInterval * 0.8 else { return }
        lastSlice = now
        let deadline = now + sliceBudget
        if !p.collections.fillPending.isEmpty {
            let post = Presenter.signposts.beginInterval("pump-collection")
            fillCollections(deadline: deadline)
            Presenter.signposts.endInterval("pump-collection", post)
        }
        if !pending.isEmpty {
            let post = Presenter.signposts.beginInterval("pump-list")
            syncLists(limit: 2, deadline: deadline)
            Presenter.signposts.endInterval("pump-list", post)
        }
        if textPending {
            let post = Presenter.signposts.beginInterval("pump-text")
            textPending = p.refreshVisibleText(deadline: deadline)
            Presenter.signposts.endInterval("pump-text", post)
        }
        if pending.isEmpty && !textPending && p.collections.fillPending.isEmpty { stop() }
    }
    /// Agent reads keep their settled contract, outside the scroll callback.
    func settle() {
        for _ in 0..<8 {
            syncLists()
            if let collections = presenter?.collections {
                for id in collections.fillPending.sorted() { collections.fillSlice(id, limit: UInt32.max - 1) }
            }
            if pending.isEmpty && presenter?.collections.fillPending.isEmpty != false { break }
        }
        textPending = presenter?.refreshVisibleText() ?? false
        if pending.isEmpty && !textPending { stop() }
    }
    func reset() {
        stop(); epoch += 1; queued = false
        pending.removeAll(); geometry.removeAll(); covers.removeAll(); travel.removeAll(); costs.removeAll()
        textPending = false; batchPending = false
        lastScroll = -.infinity; lastSlice = -.infinity; finalizationCost = 0.0005
    }
    func forget(_ id: UInt32) {
        pending.remove(id); geometry[id] = nil; covers[id] = nil; travel[id] = nil; costs[id] = nil
    }

    func syncLists(limit: UInt32 = 0, only: UInt32? = nil, deadline: TimeInterval? = nil) {
        guard let p = presenter, !p.applying, !reporting else { return }
        let post = Presenter.signposts.beginInterval("syncLists")
        reporting = true
        defer {
            reporting = false
            if batchPending {
                let started = CACurrentMediaTime()
                finishBatch()
                if deadline != nil {
                    let elapsed = CACurrentMediaTime() - started
                    finalizationCost = max(elapsed, finalizationCost * 0.75 + elapsed * 0.25)
                }
            }
            if !pending.isEmpty { start() }
            Presenter.signposts.endInterval("syncLists", post)
        }
        let lists = only.map { p.listViews[$0].map { [$0] } ?? [] }
            ?? p.listViews.values.sorted { $0.id < $1.id }
        var admitted = false
        for list in lists {
            guard isLegacy(list), p.views[list.id] === list, let scroll = list.scroll,
                  let content = list.container.subviews.first as? NodeView else { forget(list.id); continue }
            let focus = p.editing?.isDescendant(of: list) == true ? p.editing!.id : 0
            let interaction = p.views[p.interacting]?.isDescendant(of: list) == true ? p.interacting : 0
            var reportLimit = limit
            for attempt in 0..<8 {
                let started = CACurrentMediaTime()
                if let deadline {
                    let count = (costs[list.id] ?? FillCost()).rows(in: deadline - started - finalizationCost)
                    if started >= deadline || (count == 0 && admitted) { pending.insert(list.id); break }
                    if reportLimit > 1 { reportLimit = max(1, count) + 1 }
                }
                let top = Double(scroll.contentOffset.y), height = Double(scroll.bounds.height)
                let width = Double(content.frame.width), origin = Double(content.frame.minY)
                let rows = content.container.subviews.compactMap { $0 as? NodeView }
                let stamp = [top, height, width, origin, Double(focus), Double(interaction), velocity(list.id)]
                    + rows.flatMap { [Double($0.id), Double($0.frame.height)] }
                if geometry[list.id] == stamp && !pending.contains(list.id) { break }
                geometry[list.id] = stamp
                let previous = Set(rows.map(\.id))
                admitted = true
                let more = p.onList?(list.id, top, height, width, origin, focus, interaction, reportLimit) ?? false
                guard p.views[list.id] === list, isLegacy(list) else { forget(list.id); break }
                let created = content.container.subviews.compactMap { $0 as? NodeView }.filter { !previous.contains($0.id) }
                p.scrollCreatedRows += created.count
                p.scrollOffscreenRows += created.filter {
                    !$0.frame.offsetBy(dx: content.frame.minX, dy: content.frame.minY).intersects(scroll.bounds)
                }.count
                costs[list.id, default: FillCost()].record(CACurrentMediaTime() - started, rows: created.count)
                if more { pending.insert(list.id) } else { pending.remove(list.id) }
                let changed = top != Double(scroll.contentOffset.y) || height != Double(scroll.bounds.height)
                    || width != Double(content.frame.width) || origin != Double(content.frame.minY)
                guard changed, !showsViewport(scroll, content: content) else { break }
                if attempt == 7 { pending.insert(list.id) }
                reportLimit = limit == 0 ? 0 : 1
            }
        }
    }
    private func showsViewport(_ scroll: UIScrollView, content: NodeView) -> Bool {
        let port = scroll.bounds
        var reached = max(0, port.minY - content.frame.minY)
        let end = min(content.frame.height, port.maxY - content.frame.minY)
        guard port.height > 0, end > reached else { return true }
        let rows = content.container.subviews.compactMap { $0 as? NodeView }.sorted { $0.frame.minY < $1.frame.minY }
        for row in rows {
            if row.frame.maxY <= reached { continue }
            if row.frame.minY > reached { return false }
            reached = row.frame.maxY
            if reached >= end { return true }
        }
        return false
    }
}
private final class ScrollPumpTarget: NSObject {
    private weak var pump: ScrollPump?
    init(_ pump: ScrollPump) { self.pump = pump }
    @objc func tick(_ link: CADisplayLink) { pump?.tick(link) }
}
#endif
