// @ref LLP 1068 §5.1, §5.2 — the macOS host's heavy-leaf hold, the AppKit twin
// of `IOS/HeavyLeavesIOS.swift` (Charlie's ruling of 2026-09-27, applied
// unchanged): a video, web view or native-module view in a collection's row
// is not made when the row is built one to three viewports ahead, but when
// its row comes within a quarter viewport of what shows, one a frame, and
// never while its list travels faster than a viewport a second once its
// kind has been measured to cost more than a frame. Its node exists, its box
// is laid out and painted, the rest of its row is built. A click on the box
// makes the view at once (a pointer-down is not motion), so does focus into
// it, so does the agent's settle. Far module views hide beyond that margin
// and show inside it.
//
// Measured before this (bones, the Extra Heavy fling, 2026-09-30): the macOS
// host made 31 MKMapViews to SwiftUI's 22 over the same travel, each loading
// to completion, and MapKit was 158 ms/s of the 264 ms/s process-CPU gap
// (tile decode and meshes off main 249 vs 124, its render on main 62 vs 29).
//
// One difference from iOS, where `NodePool.list(creating:)` says at a node's
// init which list is building it: here the node is not yet in its row when
// it is made, so every held kind waits at init and the batch's end sorts
// them — a leaf that turns out to be in no collection's row is made then,
// within the same apply, as it was made at init before.
#if os(macOS)
import AppKit

final class HeavyLeaves {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The kinds this may hold.
    static let held: Set<String> = ["video", "iframe", "native"]
    nonisolated(unsafe) private static var samples: [String: [TimeInterval]] = [:]
    nonisolated(unsafe) private static var cold = Set<String>()
    /// A creation of `kind` took `seconds`. The process's first of each kind
    /// pays for loading and is left out.
    static func record(_ kind: String, _ seconds: TimeInterval) {
        if cold.insert(kind).inserted { return }
        var s = samples[kind] ?? []
        s.append(seconds)
        if s.count > 5 { s.removeFirst(s.count - 5) }
        samples[kind] = s
    }
    /// The measured creation cost of `kind`, seconds, if measured: the median
    /// of its last five.
    static func cost(_ kind: String) -> TimeInterval? {
        guard let s = samples[kind]?.sorted(), !s.isEmpty else { return nil }
        return s[s.count / 2]
    }

    private struct Pending { weak var node: NodeView? }
    private var pending: [UInt32: Pending] = [:]
    private var link: CADisplayLink?
    private let target = Tick()
    /// Since launch: heavy leaves held, made after waiting, and retired
    /// before they were made (`observation`).
    private(set) var deferred = 0, released = 0, cancelled = 0

    /// A new node's embedded view (from `NodeView.init`): a video's player
    /// and an iframe's web view are made now, or held; a native module's box
    /// is registered (its view is made at its first props, `NativeViews`).
    func embed(_ node: NodeView) {
        if node.kind == "native", let natives = presenter.session?.natives, natives.holds == nil {
            natives.holds = { [weak self] owner in self?.hold(owner) ?? false }
            natives.measured = { kind, seconds in HeavyLeaves.record(kind, seconds) }
        }
        if node.kind == "video" || node.kind == "iframe", hold(node) { return }
        make(node)
    }

    /// A batch applied: rows are placed now, so a leaf in no collection's row
    /// is made at once, and leaves near the viewport are made in the same
    /// frame as their rows.
    func batchApplied(moved: Bool) {
        // An animation frame's batch makes and moves no row: nothing comes
        // near or goes far, and a waiting leaf is the tick's (`start`).
        guard moved else { return }
        for (id, entry) in pending where entry.node.map({ list(holding: $0) == nil }) ?? true { release(id) }
        hideFar()
        guard !pending.isEmpty else { return }
        releaseNear(limit: .max)
        if !pending.isEmpty { start() }
    }
    /// The list moved: a held leaf may have come near, a made one gone far.
    func scrolled() {
        hideFar()
        if !pending.isEmpty { start() }
    }
    /// A made module view in a collection's row hides beyond the margin a
    /// leaf is made within (a quarter viewport), and shows inside it. Its
    /// instance stays (macOS has no parked reuse, LLP 1068 §6: a released
    /// instance would be made again from nothing when the row comes back),
    /// so nothing here is released by distance. Not while VoiceOver or
    /// Switch Control runs, when nothing waits.
    private func hideFar() {
        guard let natives = presenter.session?.natives else { return }
        let gone = natives.recycleFar(hide: 0.25, release: .infinity) { [self] node in
            list(holding: node) != nil ? distance(node) : nil
        }
        for node in gone where !hold(node) { natives.release(node) }
    }
    /// How far `node`'s box is from what the window shows, in viewports
    /// (0 when they meet; the larger of the two axes).
    private func distance(_ node: NodeView) -> CGFloat {
        guard let window = node.window, let content = window.contentView else { return .infinity }
        let box = node.convert(node.bounds, to: nil), shown = content.convert(content.bounds, to: nil)
        let dx = max(0, box.minX - shown.maxX, shown.minX - box.maxX) / max(shown.width, 1)
        let dy = max(0, box.minY - shown.maxY, shown.minY - box.maxY) / max(shown.height, 1)
        return max(dx, dy)
    }
    private func make(_ node: NodeView) {
        guard node.kind == "video" || node.kind == "iframe" else { node.embedPlatformView(presenter); return }
        let started = CACurrentMediaTime()
        if node.kind == "video" { node.video = VideoView(owner: node) } else { node.embedPlatformView(presenter) }
        Self.record(node.kind, CACurrentMediaTime() - started)
    }

    /// Whether `node`'s platform view waits: a held kind, not yet waiting,
    /// while no assistive technology runs (then nothing waits). Whether it is
    /// in a collection's row is known once the batch has placed it
    /// (`batchApplied`).
    func hold(_ node: NodeView) -> Bool {
        guard Self.held.contains(node.kind), pending[node.id] == nil, !Self.assistive else { return false }
        pending[node.id] = Pending(node: node)
        deferred += 1
        start()
        return true
    }
    /// VoiceOver or Switch Control: every leaf is made as its row is.
    private static var assistive: Bool {
        NSWorkspace.shared.isVoiceOverEnabled || NSWorkspace.shared.isSwitchControlEnabled
    }
    /// After the node's create ops: a video whose box waits on its metadata
    /// is made now after all.
    func created(_ node: NodeView) {
        guard node.kind == "video", pending[node.id] != nil else { return }
        let definite = [node.style["width"], node.style["height"]].allSatisfy { v in
            v.map { $0 != .null && $0.string != "auto" } ?? false
        }
        if !definite { release(node.id) }
    }
    /// A click landed on a held leaf's box: it is made at once (a
    /// pointer-down is not motion).
    func pressed(_ node: NodeView) {
        if pending[node.id]?.node === node { release(node.id) }
    }
    func isPending(_ node: NodeView) -> Bool { pending[node.id]?.node === node }
    /// The frame interval, seconds.
    private var frame: TimeInterval {
        1 / Double(max(60, presenter.viewport.window?.screen?.maximumFramesPerSecond ?? 60))
    }
    private func list(holding node: NodeView) -> NodeView? {
        var v = node.superview
        while let current = v {
            if let n = current as? NodeView, n.kind == "list", presenter.collections.owns(n.id) { return n }
            v = current.superview
        }
        return nil
    }
    /// A leaf whose kind costs more than a frame to make waits while its
    /// list moves (§5.1).
    private func costly(_ node: NodeView) -> Bool {
        guard let cost = Self.cost(node.kind), cost > frame, let list = list(holding: node) else { return false }
        return moving(list)
    }
    /// Within a quarter viewport of what the window shows, and whether it shows.
    private func near(_ node: NodeView) -> (near: Bool, visible: Bool) {
        guard let window = node.window, let content = window.contentView else { return (false, false) }
        let box = node.convert(node.bounds, to: nil)
        let shown = content.convert(content.bounds, to: nil)
        return (box.intersects(shown.insetBy(dx: -shown.width / 4, dy: -shown.height / 4)), box.intersects(shown))
    }
    /// Makes the near leaves that need not wait, visible first, up to `limit`.
    private func releaseNear(limit: Int) {
        var ready: [(id: UInt32, visible: Bool)] = []
        for (id, entry) in pending {
            guard let node = entry.node, presenter.views[id] === node else { ready.append((id, false)); continue }
            let n = near(node)
            if n.near, !costly(node) { ready.append((id, n.visible)) }
        }
        ready.sort { ($0.visible ? 0 : 1, $0.id) < ($1.visible ? 0 : 1, $1.id) }
        for r in ready.prefix(limit) { release(r.id) }
    }
    /// A fling faster than a viewport a second: the list's travel as the
    /// presenter samples it at each scroll (`listVelocity`, 0 once still).
    private func moving(_ list: NodeView) -> Bool {
        let port = list.scroll?.contentView.bounds.height ?? list.bounds.height
        return abs(presenter.listVelocity(list.id)) >= Double(max(port, 1))
    }

    /// Makes `id`'s view now, with its node's latest props.
    private func release(_ id: UInt32) {
        guard let entry = pending.removeValue(forKey: id) else { return }
        guard let node = entry.node, node.presenter === presenter, presenter.views[id] === node else { cancelled += 1; return }
        released += 1
        switch node.kind {
        case "native": presenter.session?.natives.release(node)
        case "video": make(node); node.video?.update(); node.video?.layout(); presenter.videoVisibility?.changed()
        default: make(node); node.updateEmbedded()
        }
    }
    /// The agent's settle, and a reset: every waiting leaf is made (or dropped).
    func settle() { for id in pending.keys.sorted() { release(id) } }
    func reset() {
        pending.removeAll(); link?.invalidate(); link = nil
    }

    private func start() {
        guard link == nil else { return }
        target.leaves = self
        let value = presenter.viewport.displayLink(target: target, selector: #selector(Tick.tick))
        value.add(to: .main, forMode: .common)
        link = value
    }
    /// One leaf a frame, visible first, once its list is still enough — or
    /// at once when focus moved into it or its row went.
    fileprivate func tick() {
        guard !presenter.isApplying else { return }
        for (id, entry) in pending.sorted(by: { $0.key < $1.key }) {
            guard let node = entry.node, presenter.views[id] === node else { release(id); continue }
            if let responder = node.window?.firstResponder as? NSView, responder === node || responder.isDescendant(of: node) { release(id) }
        }
        releaseNear(limit: 1)
        // Far leaves wait for the list to move (`scrolled`), not a frame each.
        let waiting = pending.values.contains { entry in
            guard let node = entry.node else { return false }
            return near(node).near || list(holding: node).map { presenter.listVelocity($0.id) != 0 } ?? false
        }
        if pending.isEmpty || !waiting { link?.invalidate(); link = nil }
    }

    /// `state`'s part: the leaves waiting, and the counts since launch.
    var observation: [String: Any] {
        ["pendingLeaves": pending.keys.sorted().map(Int.init), "deferred": deferred, "released": released, "cancelled": cancelled,
         "costMs": Self.samples.keys.reduce(into: [String: Double]()) { $0[$1] = Self.cost($1).map { ($0 * 10_000).rounded() / 10 } }]
    }
}

/// The display link's Objective-C target: `HeavyLeaves` is not an `NSObject`.
private final class Tick: NSObject {
    weak var leaves: HeavyLeaves?
    @objc func tick() { leaves?.tick() }
}
#endif
