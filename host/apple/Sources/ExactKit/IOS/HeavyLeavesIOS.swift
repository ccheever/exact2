// @ref LLP 1068 §5.1 (Q2, ruled 2026-09-27) — a costly heavy leaf is not
// created mid-fling: a declared deviation, narrower than LLP 1050.000 D3.
//
// During user motion (a drag or a fling faster than a viewport a second), a
// video, web view or native-module view whose measured creation cost
// exceeds the frame gets no platform view while its row is built. Its node
// exists, its box is laid out and painted, the rest of its row is built;
// when the list slows under that speed or comes to rest, the view is made,
// one a frame, with the node's latest props. Defined:
// - props: the latest at creation; the ones between are never applied, as a
//   batch coalesces them;
// - events: none before creation (`load`, `canplay`, a module's own come
//   later, as after a slow load);
// - hits: the box takes them as an empty box of its kind; a touch down on
//   it makes the view at once (a pointer-down is not motion);
// - focus: a focus move into the leaf makes it at once; while VoiceOver or
//   Switch Control runs nothing waits;
// - retirement: a row retired first makes nothing;
// - the agent: `state` lists the waiting leaves (`pool.pendingLeaves`, and
//   `native.pending` on the node); `clock settle` makes every one.
// Near the viewport only (2026-09-28, the Extra Heavy feed; LLP 1068
// §5.2): a heavy leaf in a collection's row is made when its row comes
// within a quarter viewport of what shows, as UIKit's collection view
// prepares a cell just before it appears, not when the collection's lead
// builds its row one to three viewports ahead. The row, its box and the
// rest of its content are built as before (LLP 1050.000's never-blank rule
// is about rows; this is a leaf, as above). Once made, a leaf stays until
// its row retires.
// A kind is measured at each creation (the median of its last five, the
// process's first of each kind, which pays for loading, left out); one not
// yet measured, a GPU canvas, a 2D canvas and an editor are never held, and a
// video only when its box does not wait on its metadata (both sides set).
#if os(iOS)
import UIKit

final class HeavyLeaves: NSObject, UIGestureRecognizerDelegate {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The kinds this may hold.
    static let held: Set<String> = ["video", "iframe", "native"]
    private static var samples: [String: [TimeInterval]] = [:]
    private static var cold = Set<String>()
    /// A creation of `kind` took `seconds`.
    static func record(_ kind: String, _ seconds: TimeInterval) {
        if cold.insert(kind).inserted { return }
        var s = samples[kind] ?? []
        s.append(seconds)
        if s.count > 5 { s.removeFirst(s.count - 5) }
        samples[kind] = s
    }
    /// The measured creation cost of `kind`, seconds, if measured.
    static func cost(_ kind: String) -> TimeInterval? {
        guard let s = samples[kind]?.sorted(), !s.isEmpty else { return nil }
        return s[s.count / 2]
    }

    private struct Pending { weak var node: NodeView?; let press: UILongPressGestureRecognizer }
    private var pending: [UInt32: Pending] = [:]
    private var link: CADisplayLink?
    /// Since launch: heavy leaves held, made after waiting, and retired
    /// before they were made (`state`).
    private(set) var deferred = 0, released = 0, cancelled = 0

    /// A new node's embedded view (from `NodeView.init`): a video's player
    /// and an iframe's web view are made now, or held; a native module's
    /// box is registered (its view is made at its first props, `NativeViews`).
    func embed(_ node: NodeView) {
        if node.kind == "native", let natives = presenter.session?.natives, natives.holds == nil {
            natives.holds = { [weak self] owner in self?.hold(owner) ?? false }
            natives.measured = { kind, seconds in HeavyLeaves.record(kind, seconds) }
        }
        if node.kind == "video" || node.kind == "iframe", hold(node) { return }
        make(node)
    }

    /// A batch applied: rows are placed now, so leaves near the viewport
    /// are made, in the same frame as their rows.
    func batchApplied(moved: Bool) {
        // An animation frame's batch makes and moves no row: nothing comes
        // near or goes far, and a waiting leaf is the tick's (`start`).
        guard moved else { return }
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
    /// leaf is made within, and shows inside it (`NativeViews.hideFar`).
    private func hideFar() {
        presenter.session?.natives.hideFar { [self] node in
            guard list(holding: node) != nil else { return nil }
            return !near(node).near
        }
    }
    private func make(_ node: NodeView) {
        guard node.kind == "video" || node.kind == "iframe" else { node.embedPlatformView(presenter); return }
        let started = CACurrentMediaTime()
        if node.kind == "video" { node.video = VideoView(owner: node) } else { node.embedPlatformView(presenter) }
        Self.record(node.kind, CACurrentMediaTime() - started)
    }

    /// Whether `node`'s platform view waits: a held kind measured over the
    /// frame, in a collection's row, while that list moves.
    func hold(_ node: NodeView) -> Bool {
        guard Self.held.contains(node.kind), pending[node.id] == nil, !presenter.swipeActions.assistive,
              presenter.pool.list(creating: node.id) ?? list(holding: node) != nil else { return false }
        let press = UILongPressGestureRecognizer(target: self, action: #selector(pressed(_:)))
        press.minimumPressDuration = 0
        press.cancelsTouchesInView = false
        press.delaysTouchesBegan = false
        press.delegate = self
        node.addGestureRecognizer(press)
        pending[node.id] = Pending(node: node, press: press)
        deferred += 1
        start()
        return true
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
    func isPending(_ node: NodeView) -> Bool { pending[node.id]?.node === node }
    /// The frame interval, seconds.
    private var frame: TimeInterval {
        1 / Double(max(60, presenter.viewport.window?.screen.maximumFramesPerSecond ?? 60))
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
        guard let window = node.window else { return (false, false) }
        let box = node.convert(node.bounds, to: nil)
        let shown = window.bounds
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
    /// A drag or a fling faster than a viewport a second.
    private func moving(_ list: NodeView) -> Bool {
        guard let scroll = list.scroll, scroll.isDragging || scroll.isDecelerating else { return false }
        return abs(presenter.listVelocity(list.id)) >= Double(max(scroll.bounds.height, 1))
    }

    /// Makes `id`'s view now, with its node's latest props.
    private func release(_ id: UInt32) {
        guard let entry = pending.removeValue(forKey: id) else { return }
        entry.press.view?.removeGestureRecognizer(entry.press)
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
        for entry in pending.values { entry.press.view?.removeGestureRecognizer(entry.press) }
        pending.removeAll(); link?.invalidate(); link = nil
    }

    private func start() {
        guard link == nil else { return }
        let value = CADisplayLink(target: Tick(self), selector: #selector(Tick.tick))
        value.add(to: .main, forMode: .common)
        link = value
    }
    /// One leaf a frame, visible first, once its list is still enough — or
    /// at once when focus moved into it or its row went.
    fileprivate func tick() {
        guard !presenter.applying else { return }
        for (id, entry) in pending.sorted(by: { $0.key < $1.key }) {
            guard let node = entry.node, presenter.views[id] === node else { release(id); continue }
            if presenter.pendingFocusNode === node || presenter.editing === node { release(id) }
        }
        releaseNear(limit: 1)
        // Far leaves wait for the list to move (`scrolled`), not a frame each.
        let waiting = pending.values.contains { entry in
            guard let node = entry.node else { return false }
            return near(node).near || list(holding: node).map { $0.scroll?.isDragging == true || $0.scroll?.isDecelerating == true } ?? false
        }
        if pending.isEmpty || !waiting { link?.invalidate(); link = nil }
    }
    @objc private func pressed(_ press: UILongPressGestureRecognizer) {
        guard press.state == .began, let id = pending.first(where: { $0.value.press === press })?.key else { return }
        release(id)
    }
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }

    /// `state`'s part of the pool section.
    var observation: [String: Any] {
        ["pendingLeaves": pending.keys.sorted().map(Int.init), "deferred": deferred, "released": released, "cancelled": cancelled,
         "costMs": Self.samples.keys.reduce(into: [String: Double]()) { $0[$1] = Self.cost($1).map { ($0 * 10_000).rounded() / 10 } }]
    }
}

private final class Tick: NSObject {
    weak var leaves: HeavyLeaves?
    init(_ leaves: HeavyLeaves) { self.leaves = leaves }
    @objc func tick() { leaves?.tick() }
}
#endif
