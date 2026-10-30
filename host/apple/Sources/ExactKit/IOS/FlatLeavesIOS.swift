// @ref LLP 1068 §6.1 — flat boxes. An inert box (no text, image, props or
// handlers; only a fixed background, corner radii, outer shadows and layout
// rows) whose children are all such boxes is drawn as a bare `CALayer`
// instead of a `NodeView`, as SwiftUI draws shapes into its display list and
// a hand-built UIKit screen draws a strip of cells as sublayers of one view.
// One drawn straight in a view's layer is a root; its children are its own
// sublayers, in their paint order. The runner, kernel, batch and agent tree
// are unchanged: this is only how the presenter draws. Pixels are the view's:
// the same background, corner radius, curve, corner mask and shadow casters
// `applyBoxLayer` puts on a view's layer. Hits fall to the view the boxes
// are drawn in, where a web event with no handler on them bubbles; none was
// an accessibility element. Anything else a box comes to need (a prop, a
// handler, a paint row, a child that is a view, motion other than opacity)
// promotes it: it gets its `NodeView` at the same place, and so do the flat
// boxes above it.
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
    /// Its frame drawn, on the device's pixel grid, in its parent's drawn
    /// coordinates, and as layout sent it (`PixelGridIOS.swift`).
    var frame = CGRect.zero, layout = CGRect.zero
    /// Its absolute origin on the grid, which its children are placed from.
    var origin = GridOrigin.zero
    /// Its content box and padding as layout sent them (`fieldContent`),
    /// kept for the view it becomes: a leaf paints only its fill.
    var content: (box: CGRect, padding: [CGFloat?]?)?
    var opacity: Float = 1
    /// Its paint rank among its siblings (LLP 1083.000): zero for a root.
    var rank: Int64 = 0
    /// The node whose children op placed it: a view, or a flat box.
    var parent: UInt32?
    /// The appearance its paint resolved in: its host view's.
    var dark = false
    var shadow: ShadowCaster?
    /// Its `faceBoxSize` (a header avatar's authored points), for the view it becomes.
    var faceBoxSize: String?
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
    /// Each parent's children in order: a view's when it holds a flat
    /// leaf, and a flat box's when it has any (all of them flat).
    private var order: [UInt32: [UInt32]] = [:]
    /// Whether `parent` holds a flat leaf: an authored child that is a
    /// layer, not a view.
    func holdsLeaves(_ parent: UInt32) -> Bool { order[parent]?.isEmpty == false }
    /// `parent`'s children in order, its flat leaves among them, when it holds one.
    func children(of parent: UInt32) -> [UInt32]? { order[parent] }
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

    /// The kinds a root's parent may be: a plain box's container.
    private static let parents: Set<String> = ["view", "button"]

    /// Whether a created node can be a flat leaf: its paint was read
    /// (`FlatPaint`, on the owner) and nothing else about it needs a view.
    static func eligible(_ op: BatchOp) -> Bool {
        op.kind == "view" && op.handlers.isEmpty && onlyData(op) && (op.flat ?? FlatPaint(op.style)) != nil
    }

    /// Whether an op's props are at most `data-*` words, which never make a
    /// view (LLP 1075.003 §3.3): on iOS a hatch reads them only from a route;
    /// and `faceBoxSize`, which only a box holding a text or a symbol (a
    /// face, `BoxFace`) is read for, and such a box is never flat.
    static func onlyData(_ op: BatchOp) -> Bool {
        let viewless: Set<String> = ["dataset", "faceBoxSize"]
        return op.props.keys.allSatisfy(viewless.contains) && op.clear.allSatisfy(viewless.contains)
    }
    /// A props op for `id`: kept (true), or the caller promotes it.
    func props(_ id: UInt32, _ op: BatchOp) -> Bool {
        guard let leaf = leaves[id], Self.onlyData(op) else { return false }
        if op.clear.contains("faceBoxSize") { leaf.faceBoxSize = nil }
        if let size = op.props["faceBoxSize"] { leaf.faceBoxSize = size }
        return true
    }

    /// Whether a leaf can be drawn straight in a view: a view's layer orders
    /// only rank zero among its subviews, in tree order.
    private func fitsRoot(_ leaf: FlatLeaf) -> Bool { leaf.rank == 0 && !leaf.paint.positioned }
    private func isRoot(_ leaf: FlatLeaf) -> Bool { leaf.parent.map { leaves[$0] == nil } ?? true }

    /// The id of the view a flat leaf is drawn in: its nearest ancestor that is not flat.
    func hostID(of id: UInt32) -> UInt32? {
        var at = leaves[id]?.parent
        while let p = at, let up = leaves[p] { at = up.parent }
        return at
    }
    /// The view a flat leaf is drawn in.
    func host(of id: UInt32) -> NodeView? { hostID(of: id).flatMap { presenter.views[$0] } }
    /// `id`, a view or a flat box, as the view it is drawn in.
    private func drawing(_ id: UInt32) -> NodeView? { leaves[id] != nil ? host(of: id) : presenter.views[id] }
    /// A leaf's drawn frame in its host view's coordinates.
    func frameInHost(_ id: UInt32) -> CGRect? {
        guard var leaf = leaves[id] else { return nil }
        var r = leaf.frame
        while let p = leaf.parent, let up = leaves[p] { r = r.offsetBy(dx: up.frame.minX, dy: up.frame.minY); leaf = up }
        return r
    }

    func documentOverflow(of host: NodeView, in document: UIView) -> CGRect? {
        var below = children(of: host.id) ?? [], reach: CGRect?
        while let id = below.popLast() {
            guard let leaf = leaves[id] else { continue }
            below.append(contentsOf: children(of: id) ?? [])
            guard leaf.frame.width > 0, leaf.frame.height > 0,
                  let frame = frameInHost(id) else { continue }
            let box = host.drawnRect(frame, in: document)
            reach = reach.map { $0.union(box) } ?? box
        }
        return reach
    }

    /// The nodes this batch creates flat: eligible, holding only nodes made
    /// flat, and placed by the batch under a flat box or, at rank zero and
    /// not positioned, under a plain box (one it creates, or a view already there).
    private(set) var batchFlat = Set<UInt32>()
    func begin(_ batch: Batch) {
        batchFlat.removeAll()
        var candidates = Set<UInt32>(), kinds: [UInt32: String] = [:], paints: [UInt32: FlatPaint] = [:]
        for op in batch.ops where op.op == .create {
            kinds[op.id] = op.kind
            if Self.eligible(op), let paint = op.flat ?? FlatPaint(op.style) { candidates.insert(op.id); paints[op.id] = paint }
        }
        guard !candidates.isEmpty else { return }
        // Nor is one flat that an op other than the box's own names, or a drag binding, reaches.
        let own: Set<BatchOp.Kind> = [.create, .children, .frame, .content, .fieldContent, .style, .destroy, .present, .rank]
        var parentOf: [UInt32: UInt32] = [:], kids: [UInt32: [UInt32]] = [:], ranks: [UInt32: Int64] = [:]
        for op in batch.ops {
            if op.op == .children { kids[op.id] = op.ids; for child in op.ids { parentOf[child] = op.id } }
            if op.op == .rank { ranks[op.id] = (op.payload["rank"] as? NSNumber)?.int64Value ?? 0 }
            if !own.contains(op.op) { candidates.remove(op.id) }
            if op.op == .transformDrag || op.op == .heightDrag {
                for key in ["target", "clip"] { if let v = (op.payload[key] as? Int).flatMap(UInt32.init(exactly:)) { candidates.remove(v) } }
            }
        }
        // A box holds children as a layer only when every one is flat too,
        // and it does not clip them.
        var pruned = true
        while pruned {
            pruned = false
            for id in Array(candidates) {
                guard let ids = kids[id], !ids.isEmpty else { continue }
                if paints[id]?.clips == true || ids.contains(where: { !candidates.contains($0) }) { candidates.remove(id); pruned = true }
            }
        }
        var decided: [UInt32: Bool] = [:]
        func flat(_ id: UInt32) -> Bool {
            if let known = decided[id] { return known }
            decided[id] = false
            var answer = false
            if candidates.contains(id), let p = parentOf[id], let paint = paints[id] {
                if candidates.contains(p), flat(p) { answer = true }
                else if let parent = leaves[p] { answer = !parent.paint.clips }
                else if let kind = kinds[p] ?? presenter.views[p].flatMap({ v in v.scroll == nil && v.overlay == nil && v.canvasAbove == nil ? v.kind : nil }),
                        Self.parents.contains(kind) {
                    answer = (ranks[id] ?? 0) == 0 && !paint.positioned
                }
            }
            decided[id] = answer
            return answer
        }
        for id in candidates where flat(id) { batchFlat.insert(id) }
    }

    /// A create op: made flat (true), or the caller makes a view.
    func create(_ op: BatchOp) -> Bool {
        guard batchFlat.contains(op.id), let flat = op.flat ?? FlatPaint(op.style) else { return false }
        let leaf = FlatLeaf(id: op.id, style: op.style, paint: flat)
        leaf.faceBoxSize = op.props["faceBoxSize"]
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
        guard let flat = op.flat ?? FlatPaint(style), !(flat.clips && holdsLeaves(id)),
              !isRoot(leaf) || (leaf.rank == 0 && !flat.positioned) else { promote(id, style: style); return false }
        leaf.style = style
        leaf.paint = flat
        paint(leaf)
        return true
    }
    /// Rank zero is the only flat plane in a view; a flat box orders its
    /// children by rank itself. Defer demotion until all the batch's
    /// style/children/props ops have landed.
    func rank(_ id: UInt32, _ value: Int64) {
        if let leaf = leaves[id], !isRoot(leaf) {
            guard leaf.rank != value else { return }
            leaf.rank = value
            touch(leaf)
        } else if value != 0 { promote(id)?.setRank(value) }
        else {
            presenter.views[id]?.setRank(0)
            reconsider.insert(id)
        }
    }
    func styleChanged(_ id: UInt32) { reconsider.insert(id) }

    /// `parent`'s appearance changed (its trait-change registration): its
    /// `light-dark()` leaves repaint in the new one.
    func appearanceChanged(_ parent: UInt32) {
        guard order[parent] != nil else { return }
        resolveScheme(under: parent)
        if !dirty.isEmpty { presenter.afterBatch { [weak self] in self?.flush() } }
    }
    /// Each `light-dark()` leaf under `parentID` (a view or a flat box) in
    /// its host view's appearance, and those under them. Paint motion hears
    /// every leaf's, as a view says its own (LLP 1062 D4), so a transition
    /// between pairs eases in the appearance it shows in.
    private func resolveScheme(under parentID: UInt32) {
        guard let ids = order[parentID], let host = drawing(parentID) else { return }
        let dark = host.drawsDark
        func resolve(_ ids: [UInt32]) {
            for id in ids {
                guard let leaf = leaves[id] else { continue }
                presenter.session?.noteAppearance(id, dark: dark)
                if leaf.paint.followsScheme, leaf.dark != dark {
                    leaf.dark = dark
                    paint(leaf)
                }
                if let kids = order[id] { resolve(kids) }
            }
        }
        resolve(ids)
    }
    /// Repaints the boxes drawn in `parent`, and those under them, when its tint is dimmed or comes
    /// back, greyed or not as it is now (TintDimmingIOS.swift); a run of them is drawn again
    func tintChanged(_ parent: UInt32) {
        guard let ids = order[parent] else { return }
        repaint(ids)
        if !dirty.isEmpty { presenter.afterBatch { [weak self] in self?.flush() } }
    }
    private func repaint(_ ids: [UInt32]) {
        for id in ids {
            guard let leaf = leaves[id] else { continue }
            paint(leaf)
            if let kids = order[id] { repaint(kids) }
        }
    }
    /// The appearance `id` paints in, when it is a leaf under a view.
    func appearance(of id: UInt32) -> Bool? { host(of: id)?.drawsDark }
    private func mountPromoted() {
        while !remount.isEmpty {
            let mounts = remount
            remount.removeAll()
            for id in mounts {
                if let parent = presenter.views[id], let ids = order[id] { presenter.placeChildren(parent, ids) }
            }
        }
    }

    private func demote(_ id: UInt32) {
        // A view's CSS animations are its layer's: one holding any stays a view.
        guard let view = presenter.views[id], view.kind == "view", view.paintRank == 0,
              view.handlers.isEmpty, view.props.isEmpty, view.subviews.isEmpty, !holdsLeaves(id),
              view.surface == nil, view.boxFilter == nil, !presenter.svg.animatesBox(id),
              let flat = FlatPaint(view.style), !flat.positioned else { return }
        var ancestor = view.superview
        while let v = ancestor, !(v is NodeView) { ancestor = v.superview }
        guard let parent = ancestor as? NodeView, Self.parents.contains(parent.kind),
              view.superview === parent.container, parent.scroll == nil, parent.overlay == nil,
              parent.canvasAbove == nil, parent.surface == nil else { return }
        if order[parent.id] == nil { order[parent.id] = parent.container.subviews.compactMap { ($0 as? NodeView)?.id } }
        let leaf = FlatLeaf(id: id, style: view.style, paint: flat)
        leaf.layout = view.grid?.layout ?? view.frame
        leaf.content = view.kernelContentBox.map { ($0, view.kernelPadding) }
        leaf.opacity = Float(view.alpha)
        leaf.parent = parent.id
        presenter.elements.destroyed(id)
        presenter.release(id) { $0.forget() }
        view.removeFromSuperview()
        leaves[id] = leaf
        made += 1
        frame(id, leaf.layout)
        leaf.layer.opacity = leaf.opacity
    }

    func content(_ id: UInt32, _ payload: [String: Any]) {
        let r = payload["rect"] as? [Double]
        leaves[id]?.content = r.flatMap { $0.count == 4 ? (CGRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]), NodeView.sentPadding(payload)) : nil }
    }

    func frame(_ id: UInt32, _ rect: CGRect) {
        guard let leaf = leaves[id] else { return }
        leaf.layout = rect
        reposition(leaf)
    }
    /// `leaf` drawn where its layout falls from its parent's origin, and its
    /// children from its own when that moved.
    private func reposition(_ leaf: FlatLeaf) {
        let placed = place(leaf)
        let moved = placed.origin != leaf.origin
        leaf.frame = placed.drawn
        leaf.origin = placed.origin
        CATransaction.begin(); CATransaction.setDisableActions(true)
        leaf.layer.bounds = CGRect(origin: .zero, size: placed.drawn.size)
        leaf.layer.position = placed.drawn.origin
        CATransaction.commit()
        paint(leaf)
        guard moved else { return }
        for id in order[leaf.id] ?? [] { if let kid = leaves[id] { reposition(kid) } }
    }
    private func place(_ leaf: FlatLeaf) -> GridPlace {
        let origin = leaf.parent.flatMap { leaves[$0]?.origin ?? presenter.views[$0]?.grid?.origin } ?? .zero
        return PixelGrid.place(leaf.layout, in: origin, scale: presenter.gridScale(under: origin))
    }
    /// Every parent's leaves drawn again (a new display's grid).
    func regridAll(where admits: (NodeView) -> Bool) {
        for parent in order.keys { if let v = presenter.views[parent], admits(v) { regrid(parent: v) } }
    }
    /// `parent`'s leaves drawn again from its origin, where that moves them.
    func regrid(parent: NodeView) {
        for id in order[parent.id] ?? [] {
            guard let leaf = leaves[id] else { continue }
            let placed = place(leaf)
            guard placed.drawn != leaf.frame || placed.origin != leaf.origin else { continue }
            reposition(leaf)
            touch(leaf)
        }
    }
    /// Its parent lays it in again, and its host, whose hit frames span it.
    private func touch(_ leaf: FlatLeaf) {
        guard let p = leaf.parent else { return }
        dirty.insert(p)
        if leaves[p] != nil, let host = hostID(of: leaf.id) { dirty.insert(host) }
    }
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

    /// A canvas that samples the leaf's host recaptures it, as it does for
    /// a view's change (LLP 1014).
    private func changed(_ leaf: FlatLeaf) {
        guard let host = host(of: leaf.id), let c = host.canvasAbove else { return }
        c.needsCapture = true
        host.canvases?.scheduleCapture()
    }

    /// The box as `applyBoxLayer` puts it on a view's layer: the fill, one
    /// radius over the corners that have one (CSS's reduction; a box
    /// needing more is promoted, as a view would draw it), and its outer
    /// shadows cast under it (`applyShadow`).
    private func paint(_ leaf: FlatLeaf) {
        let s = leaf.paint
        let bounds = CGRect(origin: .zero, size: leaf.frame.size)
        // Grey its colours on a dimmed control's face, as its host view greys its own (TintDimmingIOS.swift)
        let host = host(of: leaf.id)
        let fill = s.fill(dark: leaf.dark).map { f in host?.dimmedWithTint(f, key: "background_color") ?? f }
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
        // A bare layer's default curve is continuous; a view's box is circular (`applyBoxLayer`).
        if l.cornerCurve != .circular { l.cornerCurve = .circular }
        let hidden = s.hidden
        if l.isHidden != hidden { l.isHidden = hidden }
        var shadows = bounds.width > 0 && bounds.height > 0 ? BoxShadowSpec.list(s.shadow, dark: leaf.dark) : []
        if let host { for i in shadows.indices { shadows[i].color = host.dimmedWithTint(shadows[i].color, key: "box_shadow") } }
        if shadows.isEmpty {
            leaf.shadow?.removeFromSuperlayer(); leaf.shadow = nil
        } else {
            let caster = leaf.shadow ?? ShadowCaster()
            if caster.superlayer !== l { l.insertSublayer(caster, at: 0) }
            leaf.shadow = caster
            let sizes = radii.map { CGSize(width: $0, height: $0) }
            let outline = BorderPaint.roundedRect(bounds, sizes)
            caster.cast(box: bounds, outline: outline, shadows: shadows.map { spec in
                guard spec.spread != 0 else { return (outline, spec) }
                let grown = bounds.insetBy(dx: -spec.spread, dy: -spec.spread)
                let spread = sizes.map { CGSize(width: BoxShadowSpec.spreadRadius($0.width, spec.spread),
                                                height: BoxShadowSpec.spreadRadius($0.height, spec.spread)) }
                return (BorderPaint.roundedRect(grown, spread), spec)
            })
        }
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
        // Repaint the boxes whose new host dims its paint where their old one did not, or the other
        // way (TintDimmingIOS.swift); a new box painted with no host
        let dims = parent.dimsItsPaint
        let moved = flat.filter { leaves[$0]?.parent != parent.id && (host(of: $0)?.dimsItsPaint ?? false) != dims }
        for id in flat { leaves[id]?.parent = parent.id }
        if !moved.isEmpty { repaint(moved) }
        regrid(parent: parent)
        guard Self.parents.contains(parent.kind), parent.scroll == nil, parent.overlay == nil,
              parent.canvasAbove == nil, parent.surface == nil else { return flat }
        dirty.insert(parent.id)
        return flat.filter { leaves[$0].map { !fitsRoot($0) } ?? false }
    }

    /// A children op on flat box `id`: applied (true) when every child is a
    /// flat leaf it may hold, else the caller promotes it.
    func adopt(_ id: UInt32, _ ids: [UInt32]) -> Bool {
        guard let leaf = leaves[id], ids.allSatisfy({ leaves[$0] != nil }), ids.isEmpty || !leaf.paint.clips else { return false }
        placements += 1
        for old in order[id] ?? [] where !ids.contains(old) {
            if let gone = leaves[old], gone.parent == id { gone.layer.removeFromSuperlayer() }
        }
        order[id] = ids.isEmpty ? nil : ids
        let dims = host(of: id)?.dimsItsPaint ?? false
        for kid in ids.compactMap({ leaves[$0] }) {
            let repaints = kid.parent != id && (host(of: kid.id)?.dimsItsPaint ?? false) != dims
            kid.parent = id
            reposition(kid) // which paints it
            if repaints, leaves[kid.id] != nil, let nested = order[kid.id] { repaint(nested) }
        }
        dirty.insert(id)
        touch(leaf)
        return true
    }

    func mounted(_ parent: UInt32) { remount.remove(parent) }

    /// What a run shares: one fill, one radius on all four corners (or
    /// none), shown whole, and nothing else drawn. A leaf that is not alike
    /// stands alone, and so does a `light-dark()` leaf: its layer paints the
    /// pixels its view did, where a run's path antialiases the edges a layer
    /// leaves sharp at fractional pixel positions (a 5×5 LED glyph's cells went soft).
    /// Only an opaque fill runs: one path paints an overlap once, where two
    /// translucent boxes compose (two 50% fills over each other are 75%).
    private struct RunKey: Equatable { let rgba: [Double]; let radius: CGFloat; let rank: Int64 }
    private func runKey(_ leaf: FlatLeaf) -> RunKey? {
        let s = leaf.paint
        guard leaf.opacity == 1, !s.hidden, !s.followsScheme, s.shadow == nil, order[leaf.id] == nil,
              let c = s.light, c.count == 4, c[3] >= 1 else { return nil }
        let r = s.radii
        guard r.allSatisfy({ $0 == r[0] }) else { return nil }
        return RunKey(rgba: c, radius: r[0], rank: leaf.rank)
    }
    /// The frames a root's hits cover, in its host's coordinates: its own
    /// and every box under it, which the web hits where they overflow too.
    private func hitFrames(_ leaf: FlatLeaf, at offset: CGPoint = .zero) -> [CGRect] {
        let f = leaf.frame.offsetBy(dx: offset.x, dy: offset.y)
        return [f] + (order[leaf.id] ?? []).compactMap { leaves[$0] }.flatMap { hitFrames($0, at: f.origin) }
    }

    /// Each dirty parent's flat leaves into its layer, in paint order: a
    /// run of two or more alike adjacent leaves as one shape layer of their
    /// boxes (step 2), any other leaf as its own layer. Under a view, each
    /// directly above the child before it, or below the first view when
    /// none is before it; under a flat box, by rank, then tree order.
    func flush() {
        // Promotions create views while ops are applied; each affected
        // parent's latest child list is mounted once before drawing runs.
        mountPromoted()
        let candidates = reconsider
        reconsider.removeAll()
        for id in candidates { demote(id) }
        guard !dirty.isEmpty else { return }
        // A leaf's paint in its parent's appearance can need a view (a dark
        // fill on corners a layer cannot say): it is mounted before drawing too.
        for parentID in dirty { resolveScheme(under: parentID) }
        mountPromoted()
        let parents = dirty
        dirty.removeAll()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        for parentID in parents {
            for run in runs.removeValue(forKey: parentID) ?? [] { run.removeFromSuperlayer(); runLeaves[ObjectIdentifier(run)] = nil }
            guard var ids = order[parentID] else { continue }
            let box = leaves[parentID], parent = box == nil ? presenter.views[parentID] : nil
            guard let container = box?.layer ?? parent?.container.layer else { continue }
            if box != nil {
                ids = ids.enumerated().sorted {
                    let a = leaves[$0.element]?.rank ?? 0, b = leaves[$1.element]?.rank ?? 0
                    return a != b ? a < b : $0.offset < $1.offset
                }.map(\.element)
            }
            for id in ids { leaves[id]?.layer.removeFromSuperlayer() }
            var made: [CAShapeLayer] = []
            var previous: CALayer?
            func put(_ layer: CALayer, at i: Int, frames: [CGRect]) {
                layer.flatHit = parent.map { FlatHit(owner: $0, frames: frames) }
                if let previous {
                    container.insertSublayer(layer, above: previous)
                } else if parent != nil, let next = ids[(i + 1)...].lazy.compactMap({ self.presenter.views[$0] }).first(where: { $0.layer.superlayer === container }) {
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
                    put(leaf.layer, at: i, frames: hitFrames(leaf))
                }
                i = j
            }
            if !made.isEmpty { runs[parentID] = made }
            if let host = drawing(parentID), let c = host.canvasAbove { c.needsCapture = true; host.canvases?.scheduleCapture() }
        }
    }

    /// `id` needs a view after all: it gets one, with the leaf's style, frame
    /// and opacity, in the leaf's place among its parent's children. A view's
    /// parent is a view, so a flat box above it is promoted first; its own
    /// children stay flat in it where a view can hold them so.
    @discardableResult
    func promote(_ id: UInt32, style: NodeStyle? = nil) -> NodeView? {
        if let p = leaves[id]?.parent, leaves[p] != nil { promote(p) }
        guard let leaf = leaves.removeValue(forKey: id) else { return presenter.views[id] }
        promoted += 1
        leaf.layer.removeFromSuperlayer()
        touch(leaf)
        let view = NodeView(id: id, kind: "view", presenter: presenter)
        view.applyStyle(style ?? leaf.style)
        view.applyProps(set: leaf.faceBoxSize.map { ["faceBoxSize": $0] } ?? [:], clear: [])
        let placed = place(leaf)
        // A new view, so it takes its place at once inside an animation UIKit has open (`Presenter.moveOrPlace`)
        presenter.moveOrPlace(view) {
            view.frame = placed.drawn
            view.grid = placed
            if let content = leaf.content { view.applyKernelContentBox(content.box, padding: content.padding) }
            view.alpha = CGFloat(leaf.opacity)
        }
        presenter.views[id] = view
        if leaf.rank != 0 { view.setRank(leaf.rank) }
        if let parentID = leaf.parent { remount.insert(parentID) }
        if let kids = order[id] {
            for run in runs.removeValue(forKey: id) ?? [] { runLeaves[ObjectIdentifier(run)] = nil }
            for kid in kids.compactMap({ leaves[$0] }) where !fitsRoot(kid) { promote(kid.id) }
            remount.insert(id)
        }
        return view
    }

    func reset() {
        for leaf in leaves.values { leaf.layer.removeFromSuperlayer() }
        for run in runs.values.joined() { run.removeFromSuperlayer() }
        leaves.removeAll(); order.removeAll(); runs.removeAll(); runLeaves.removeAll(); dirty.removeAll(); remount.removeAll(); reconsider.removeAll()
    }

    /// The layers painting `parent`'s flat leaves, each with the leaves it
    /// paints: its runs, then the leaves that stand alone, each with the
    /// boxes under it (LLP 1080.001 D1).
    func inspectionLayers(under parent: UInt32) -> [(layer: CALayer, leaves: [UInt32])] {
        func subtree(_ id: UInt32) -> [UInt32] { [id] + (order[id] ?? []).filter { leaves[$0] != nil }.flatMap(subtree) }
        var out: [(layer: CALayer, leaves: [UInt32])] = (runs[parent] ?? []).map { ($0, runLeaves[ObjectIdentifier($0)] ?? []) }
        for id in order[parent] ?? [] {
            if let leaf = leaves[id], leaf.layer.superlayer != nil { out.append((leaf.layer, subtree(id))) }
        }
        return out
    }
    /// The view whose layer draws `id`, for `layout <id> native`.
    func inspectionParent(of id: UInt32) -> UInt32? { hostID(of: id) }

    var observation: [String: Any] { ["flatLeaves": leaves.count, "flatMade": made, "flatPromoted": promoted, "flatPlacements": placements] }
}
#endif
