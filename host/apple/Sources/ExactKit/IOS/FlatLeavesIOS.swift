// @ref LLP 1068 §6.1 — flat leaf boxes. An inert leaf box (no children,
// text, image, props, handlers; only a fixed background, corner radii and
// layout rows) is drawn as a bare `CALayer` in its parent's layer instead of
// a `NodeView`, as SwiftUI draws such shapes into its display list. The
// runner, kernel, batch and agent tree are unchanged: this is only how the
// presenter draws. Pixels are the view's: the same background, corner
// radius, curve and corner mask `applyBoxLayer` puts on a view's layer.
// Hits fall to the parent, where a web event with no handler on the leaf
// bubbles; the leaf was never an accessibility element. Anything else a node
// comes to need (a prop, a handler, a paint row, a child, motion other than
// opacity) promotes it: it gets its `NodeView` at the same place.
#if os(iOS) || os(tvOS)
import UIKit

/// No implicit animations on a flat leaf's layer.
private final class FlatAction: NSObject, CALayerDelegate {
    static let shared = FlatAction()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

/// No implicit animations on a run's shape layer.
private final class FlatRunAction: NSObject, CALayerDelegate {
    static let shared = FlatRunAction()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

final class FlatLeaf {
    let id: UInt32
    let layer = CALayer()
    var style: NodeStyle
    /// Its paint, read from `style` on the owner thread (LLP 1072 stage 4).
    var paint: FlatPaint
    var frame = CGRect.zero
    var opacity: Float = 1
    /// The node whose children op placed it.
    var parent: UInt32?
    init(id: UInt32, style: NodeStyle, paint: FlatPaint) {
        self.id = id; self.style = style; self.paint = paint
        layer.delegate = FlatAction.shared
        layer.anchorPoint = .zero
    }
}

final class FlatLeaves {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }
    private(set) var leaves: [UInt32: FlatLeaf] = [:]
    /// Each parent's children in order, for parents that hold a flat leaf.
    private var order: [UInt32: [UInt32]] = [:]
    /// Whether `parent` holds a flat leaf: an authored child that is a
    /// layer, not a view.
    func holdsLeaves(_ parent: UInt32) -> Bool { order[parent]?.isEmpty == false }
    /// Since launch: made flat, and promoted to views (`state`).
    private(set) var made = 0, promoted = 0
    private(set) var placements = 0
    /// Parents whose flat leaves are laid into layers when the batch ends.
    private var dirty = Set<UInt32>()
    private var remount = Set<UInt32>()
    private var reconsider = Set<UInt32>()
    /// Each parent's shape layers, one per run of alike adjacent leaves.
    private var runs: [UInt32: [CAShapeLayer]] = [:]
    /// Each run's leaves, in order (LLP 1080.001 D1).
    private var runLeaves: [ObjectIdentifier: [UInt32]] = [:]

    /// The kinds a flat leaf's parent may be: a plain box's container.
    private static let parents: Set<String> = ["view", "button"]

    /// Whether a created node can be a flat leaf: its paint was read
    /// (`FlatPaint`, on the owner) and nothing else about it needs a view.
    static func eligible(_ op: BatchOp) -> Bool {
        op.kind == "view" && op.handlers.isEmpty && onlyData(op) && (op.flat ?? FlatPaint(op.style)) != nil
    }

    /// Whether an op's props are at most `data-*` words, which never make a
    /// view (LLP 1075.003 §3.3): on iOS a hatch reads them only from a route.
    static func onlyData(_ op: BatchOp) -> Bool {
        op.props.keys.allSatisfy { $0 == "dataset" } && op.clear.allSatisfy { $0 == "dataset" }
    }

    /// The nodes this batch creates flat: eligible, and placed by the batch
    /// under a plain box (one it creates, or a view already there).
    private(set) var batchFlat = Set<UInt32>()
    func begin(_ batch: Batch) {
        batchFlat.removeAll()
        var candidates = Set<UInt32>(), kinds: [UInt32: String] = [:]
        for op in batch.ops where op.op == .create {
            kinds[op.id] = op.kind
            if Self.eligible(op) { candidates.insert(op.id) }
        }
        guard !candidates.isEmpty else { return }
        // A node with children is not a leaf; nor is one any op but the
        // box's own names, or a drag binding reaches.
        let own: Set<BatchOp.Kind> = [.create, .children, .frame, .content, .style, .destroy, .present, .rank]
        for op in batch.ops {
            if op.op == .children && !op.ids.isEmpty { candidates.remove(op.id) }
            if op.op == .rank, (op.payload["rank"] as? NSNumber)?.int64Value != 0 { candidates.remove(op.id) }
            if !own.contains(op.op) { candidates.remove(op.id) }
            if op.op == .transformDrag || op.op == .heightDrag {
                for key in ["target", "clip"] { if let v = (op.payload[key] as? Int).flatMap(UInt32.init(exactly:)) { candidates.remove(v) } }
            }
        }
        for op in batch.ops where op.op == .children {
            let parentKind = kinds[op.id] ?? presenter.views[op.id].flatMap { v in
                v.scroll == nil && v.overlay == nil && v.canvasAbove == nil ? v.kind : nil
            }
            guard let parentKind, Self.parents.contains(parentKind), !candidates.contains(op.id) else { continue }
            for child in op.ids where candidates.contains(child) { batchFlat.insert(child) }
        }
    }

    /// A create op: made flat (true), or the caller makes a view.
    func create(_ op: BatchOp) -> Bool {
        guard batchFlat.contains(op.id), let flat = op.flat ?? FlatPaint(op.style) else { return false }
        let leaf = FlatLeaf(id: op.id, style: op.style, paint: flat)
        leaves[op.id] = leaf
        paint(leaf)
        made += 1
        return true
    }
    func isFlat(_ id: UInt32) -> Bool { leaves[id] != nil }
    /// The node a flat leaf's children op placed it in.
    func parent(of id: UInt32) -> UInt32? { leaves[id]?.parent }
    /// `id`'s children moved to another container view (a material or a
    /// glass group came or went): its leaves follow at the next flush.
    func containerChanged(_ id: UInt32) { if order[id] != nil { dirty.insert(id) } }

    /// A style op for `id`: applied (true), or the leaf was promoted.
    func style(_ id: UInt32, _ op: BatchOp) -> Bool {
        guard let leaf = leaves[id] else { return false }
        let style = op.style
        guard let flat = op.flat ?? FlatPaint(style) else { promote(id, style: style); return false }
        leaf.style = style
        leaf.paint = flat
        paint(leaf)
        return true
    }
    /// Rank zero is the only flat plane. Defer demotion until all the
    /// batch's style/children/props ops have landed.
    func rank(_ id: UInt32, _ value: Int64) {
        if value != 0 { promote(id)?.setRank(value) }
        else {
            presenter.views[id]?.setRank(0)
            reconsider.insert(id)
        }
    }
    func styleChanged(_ id: UInt32) { reconsider.insert(id) }

    private func demote(_ id: UInt32) {
        guard let view = presenter.views[id], view.kind == "view", view.paintRank == 0,
              view.handlers.isEmpty, view.props.isEmpty, view.subviews.isEmpty, !holdsLeaves(id),
              view.surface == nil, view.boxFilter == nil,
              let flat = FlatPaint(view.style) else { return }
        var ancestor = view.superview
        while let v = ancestor, !(v is NodeView) { ancestor = v.superview }
        guard let parent = ancestor as? NodeView, Self.parents.contains(parent.kind),
              view.superview === parent.container, parent.scroll == nil, parent.overlay == nil,
              parent.canvasAbove == nil, parent.surface == nil else { return }
        if order[parent.id] == nil { order[parent.id] = parent.container.subviews.compactMap { ($0 as? NodeView)?.id } }
        let leaf = FlatLeaf(id: id, style: view.style, paint: flat)
        leaf.frame = view.frame
        leaf.opacity = Float(view.alpha)
        leaf.parent = parent.id
        presenter.elements.destroyed(id)
        presenter.release(id) { $0.forget() }
        view.removeFromSuperview()
        leaves[id] = leaf
        made += 1
        frame(id, leaf.frame)
        leaf.layer.opacity = leaf.opacity
    }

    func frame(_ id: UInt32, _ rect: CGRect) {
        guard let leaf = leaves[id] else { return }
        leaf.frame = rect
        CATransaction.begin(); CATransaction.setDisableActions(true)
        leaf.layer.bounds = CGRect(origin: .zero, size: rect.size)
        leaf.layer.position = rect.origin
        CATransaction.commit()
        paint(leaf)
    }
    private func touch(_ leaf: FlatLeaf) { if let p = leaf.parent { dirty.insert(p) } }
    func opacity(_ id: UInt32, _ value: Float) {
        guard let leaf = leaves[id] else { return }
        leaf.opacity = value
        if leaf.layer.opacity != value { leaf.layer.opacity = value; changed(leaf); touch(leaf) }
    }
    func destroy(_ id: UInt32) {
        guard let leaf = leaves.removeValue(forKey: id) else { return }
        changed(leaf)
        touch(leaf)
        leaf.layer.removeFromSuperlayer()
        if let parent = leaf.parent, var ids = order[parent] {
            ids.removeAll { $0 == id }
            order[parent] = ids.contains(where: { leaves[$0] != nil }) ? ids : nil
        }
        order[id] = nil
        for run in runs.removeValue(forKey: id) ?? [] { run.removeFromSuperlayer() }
    }

    /// A canvas that samples the leaf's parent recaptures it, as it does for
    /// a view's change (LLP 1014).
    private func changed(_ leaf: FlatLeaf) {
        guard let parent = leaf.parent.flatMap({ presenter.views[$0] }), let c = parent.canvasAbove else { return }
        c.needsCapture = true
        parent.canvases?.scheduleCapture()
    }

    /// The box as `applyBoxLayer` puts it on a view's layer: the fill, and
    /// one radius over the corners that have one (CSS's reduction; a box
    /// needing more is promoted, as a view would draw it).
    private func paint(_ leaf: FlatLeaf) {
        let s = leaf.paint
        let bounds = CGRect(origin: .zero, size: leaf.frame.size)
        let fill = s.fill
        let r = s.radii
        let sums = [r[0] + r[1], r[3] + r[2], r[0] + r[3], r[1] + r[2]]
        let edges = [bounds.width, bounds.width, bounds.height, bounds.height]
        var factor: CGFloat = 1
        for i in 0..<4 where sums[i] > 0 { factor = min(factor, edges[i] / sums[i]) }
        let radii = r.map { $0 * factor }
        let radius = radii.max() ?? 0
        let oneRadius = radii.allSatisfy { $0 == 0 || abs($0 - radius) < 0.01 }
            && radius <= min(bounds.width, bounds.height) / 2 + 0.01
        // An HDR fill needs its own layer's range (LLP 1100 D8): promote it.
        if fill != nil, !oneRadius || ColorRange.isHDR(fill) { promote(leaf.id, style: leaf.style); return }
        var corners: CACornerMask = []
        let masks: [CACornerMask] = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
        for (v, mask) in zip(radii, masks) where v > 0 { corners.insert(mask) }
        let l = leaf.layer
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if l.backgroundColor != fill { l.backgroundColor = fill }
        let cornerRadius = oneRadius ? radius : 0
        if l.cornerRadius != cornerRadius { l.cornerRadius = cornerRadius }
        if cornerRadius > 0, l.maskedCorners != corners { l.maskedCorners = corners }
        let hidden = s.hidden
        if l.isHidden != hidden { l.isHidden = hidden }
        changed(leaf)
        touch(leaf)
    }

    /// `parent`'s children op placed its views: its flat leaves are laid in
    /// when the batch ends (`flush`). Answers the flat leaves that cannot
    /// stay flat under this parent.
    func place(_ parent: NodeView, _ ids: [UInt32]) -> [UInt32] {
        placements += 1
        let flat = ids.filter { leaves[$0] != nil }
        guard !flat.isEmpty else {
            if order.removeValue(forKey: parent.id) != nil { dirty.insert(parent.id) }
            return []
        }
        order[parent.id] = ids
        for id in flat { leaves[id]?.parent = parent.id }
        guard Self.parents.contains(parent.kind), parent.scroll == nil, parent.overlay == nil,
              parent.canvasAbove == nil, parent.surface == nil else { return flat }
        dirty.insert(parent.id)
        return []
    }

    func mounted(_ parent: UInt32) { remount.remove(parent) }

    /// Every leaf is rank zero; promotion removes other ranks before runs
    /// form. What a run shares: one fill, one radius on all four corners (or none),
    /// shown whole. A leaf that is not alike stands alone.
    private struct RunKey: Equatable { let rgba: [Double]; let radius: CGFloat }
    private func runKey(_ leaf: FlatLeaf) -> RunKey? {
        let s = leaf.paint
        guard leaf.opacity == 1, !s.hidden, let c = s.rgba, c.count == 4, c[3] > 0 else { return nil }
        let r = s.radii
        guard r.allSatisfy({ $0 == r[0] }) else { return nil }
        return RunKey(rgba: c, radius: r[0])
    }

    /// Each dirty parent's flat leaves into its container's layer, in tree
    /// order among its views: a run of two or more alike adjacent leaves as
    /// one shape layer of their boxes (step 2), any other leaf as its own
    /// layer, each directly above the child before it, or below the first
    /// view when none is before it.
    func flush() {
        // Promotions create views while ops are applied; each affected
        // parent's latest child list is mounted once before drawing runs.
        let mounts = remount
        remount.removeAll()
        for id in mounts {
            if let parent = presenter.views[id], let ids = order[id] { presenter.placeChildren(parent, ids) }
        }
        let candidates = reconsider
        reconsider.removeAll()
        for id in candidates { demote(id) }
        guard !dirty.isEmpty else { return }
        let parents = dirty
        dirty.removeAll()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        for parentID in parents {
            for run in runs.removeValue(forKey: parentID) ?? [] { run.removeFromSuperlayer(); runLeaves[ObjectIdentifier(run)] = nil }
            guard let parent = presenter.views[parentID], let ids = order[parentID] else { continue }
            let container = parent.container.layer
            for id in ids { leaves[id]?.layer.removeFromSuperlayer() }
            var made: [CAShapeLayer] = []
            var previous: CALayer?
            func put(_ layer: CALayer, at i: Int, frames: [CGRect]) {
                layer.flatHit = FlatHit(owner: parent, frames: frames)
                if let previous {
                    container.insertSublayer(layer, above: previous)
                } else if let next = ids[(i + 1)...].lazy.compactMap({ self.presenter.views[$0] }).first(where: { $0.layer.superlayer === container }) {
                    container.insertSublayer(layer, below: next.layer)
                } else {
                    container.addSublayer(layer)
                }
                previous = layer
            }
            var i = 0
            while i < ids.count {
                let id = ids[i]
                guard let leaf = leaves[id] else {
                    if let view = presenter.views[id], view.layer.superlayer === container { previous = view.layer }
                    i += 1; continue
                }
                var j = i + 1
                if let key = runKey(leaf) {
                    while j < ids.count, let next = leaves[ids[j]], runKey(next) == key { j += 1 }
                }
                if j - i >= 2 {
                    let run = CAShapeLayer()
                    run.delegate = FlatRunAction.shared
                    let path = CGMutablePath()
                    for id in ids[i..<j] {
                        guard let f = leaves[id]?.frame, f.width > 0, f.height > 0 else { continue }
                        let r = min(leaf.paint.radii[0], f.width / 2, f.height / 2)
                        if r > 0 { path.addRoundedRect(in: f, cornerWidth: r, cornerHeight: r) } else { path.addRect(f) }
                    }
                    run.path = path
                    run.fillColor = leaf.layer.backgroundColor
                    put(run, at: j - 1, frames: ids[i..<j].compactMap { leaves[$0]?.frame })
                    made.append(run)
                    runLeaves[ObjectIdentifier(run)] = Array(ids[i..<j])
                } else {
                    put(leaf.layer, at: i, frames: [leaf.frame])
                }
                i = j
            }
            if !made.isEmpty { runs[parentID] = made }
            if let c = parent.canvasAbove { c.needsCapture = true; parent.canvases?.scheduleCapture() }
        }
    }

    /// `id` needs a view after all: it gets one, with the leaf's style, frame
    /// and opacity, in the leaf's place among its parent's children.
    @discardableResult
    func promote(_ id: UInt32, style: NodeStyle? = nil) -> NodeView? {
        guard let leaf = leaves.removeValue(forKey: id) else { return presenter.views[id] }
        promoted += 1
        leaf.layer.removeFromSuperlayer()
        touch(leaf)
        let view = NodeView(id: id, kind: "view", presenter: presenter)
        view.applyStyle(style ?? leaf.style)
        view.applyProps(set: [:], clear: [])
        view.frame = leaf.frame
        view.alpha = CGFloat(leaf.opacity)
        presenter.views[id] = view
        if let parentID = leaf.parent { remount.insert(parentID) }
        return view
    }

    func reset() {
        for leaf in leaves.values { leaf.layer.removeFromSuperlayer() }
        for run in runs.values.joined() { run.removeFromSuperlayer() }
        leaves.removeAll(); order.removeAll(); runs.removeAll(); runLeaves.removeAll(); dirty.removeAll(); remount.removeAll(); reconsider.removeAll()
    }

    /// The layers painting `parent`'s flat leaves, each with the leaves it
    /// paints: its runs, then the leaves that stand alone (LLP 1080.001 D1).
    func inspectionLayers(under parent: UInt32) -> [(layer: CALayer, leaves: [UInt32])] {
        var out: [(layer: CALayer, leaves: [UInt32])] = (runs[parent] ?? []).map { ($0, runLeaves[ObjectIdentifier($0)] ?? []) }
        for id in order[parent] ?? [] {
            if let leaf = leaves[id], leaf.layer.superlayer != nil { out.append((leaf.layer, [id])) }
        }
        return out
    }
    /// The flat leaf whose layer or run paints `id`, for `layout <id> native`.
    func inspectionParent(of id: UInt32) -> UInt32? { leaves[id]?.parent }

    var observation: [String: Any] { ["flatLeaves": leaves.count, "flatMade": made, "flatPromoted": promoted, "flatPlacements": placements] }
}
#endif
