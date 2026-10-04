// LLP 1083.000 D4: tree order stays in subviews; Core Animation gets only
// the dense sibling rank. Every layer standing for a node takes this value.
#if os(macOS)
import AppKit
typealias PaintView = NSView
#else
import UIKit
typealias PaintView = UIView
#endif

private var paintRanksKey: UInt8 = 0
private var paintForegroundKey: UInt8 = 0
private final class DensePaintRanks {
    var positions: [Int64: CGFloat] = [0: 0]
    static func of(_ parent: PaintView) -> DensePaintRanks {
        if let ranks = objc_getAssociatedObject(parent, &paintRanksKey) as? DensePaintRanks { return ranks }
        let ranks = DensePaintRanks()
        objc_setAssociatedObject(parent, &paintRanksKey, ranks, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return ranks
    }
}

/// All callers run on the UI thread. Nested presenter applies share the
/// transaction; a standalone lift/ghost change is its own transaction.
enum PaintOrder {
    private static var depth = 0
    private static var dirty: [ObjectIdentifier: PaintView] = [:]

    static func begin() { depth += 1 }
    static func end() {
        precondition(depth > 0)
        depth -= 1
        guard depth == 0 else { return }
        flush()
    }
    /// A capture within an open batch must see every rank written so far.
    static func flush() {
        let parents = dirty.values
        dirty = [:]
        for parent in parents { NodeView.rankChildren(of: parent) }
    }
    static func changed(_ parent: PaintView) {
        if depth > 0 { dirty[ObjectIdentifier(parent)] = parent }
        else { NodeView.rankChildren(of: parent) }
    }
}

extension NodeView {
    var paintRank: Int64 {
        if paintLifted { return 4_294_967_294 }
        if paintGhost { return 4_294_967_292 }
        return paintGhosts > 0 && rank == 0 ? 1 : rank
    }

    var paintZPosition: CGFloat {
        #if os(macOS)
        return layer?.zPosition ?? 0
        #else
        return layer.zPosition
        #endif
    }

    func setRank(_ value: Int64) {
        guard rank != value else { return }
        rank = value
        refreshPaintOrder()
    }

    func setLifted(_ value: Bool) {
        guard paintLifted != value else { return }
        paintLifted = value
        refreshPaintOrder()
    }

    func setGhost(_ value: Bool) {
        guard paintGhost != value else { return }
        PaintOrder.begin()
        defer { PaintOrder.end() }
        paintGhost = value
        if value {
            var parent = superview
            while let view = parent, !(view is NodeView) { parent = view.superview }
            ghostParent = parent as? NodeView
            ghostParent?.paintGhosts += 1
        } else {
            ghostParent?.paintGhosts -= 1
        }
        ghostParent?.refreshPaintOrder()
        if !value { ghostParent = nil }
        refreshPaintOrder()
    }

    /// Called after a mount, removal or change of native content holder.
    func paintOrderMoved() {
        let old = paintParent
        paintParent = superview
        if old !== superview, let old { PaintOrder.changed(old) }
        refreshPaintOrder()
    }

    private func refreshPaintOrder() {
        if let parent = superview { PaintOrder.changed(parent) }
        else { setPaintPosition(0) }
    }

    fileprivate static func rankChildren(of parent: PaintView) {
        // Dense depth is an ordering key, never an animated property. This
        // also prevents cached canvas mirrors interpolating from the old order.
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let children = parent.subviews.filter { $0 is NodeView || $0.paintForeground }
        // Zero is the origin even when no child has rank zero. Native
        // decoration/content layers remain there as well.
        let distinct = Set(children.map(\.siblingPaintRank)).union([0])
        let dense = DensePaintRanks.of(parent)
        if distinct != Set(dense.positions.keys) {
            let ranks = distinct.sorted()
            let zero = ranks.firstIndex(of: 0)!
            dense.positions = Dictionary(uniqueKeysWithValues: ranks.enumerated().map { ($0.element, CGFloat($0.offset - zero) * 0.001) })
        }
        for child in children { child.setPaintPosition(dense.positions[child.siblingPaintRank]!) }
    }

    /// Live children may reorder around an exit, but the ghost keeps its
    /// existing slot rather than being moved to the end of the native tree.
    static func keepingGhosts(_ children: [NodeView], in parent: PaintView) -> [NodeView] {
        var result = children
        for (index, view) in parent.subviews.enumerated() {
            if let ghost = view as? NodeView, ghost.paintGhost {
                result.insert(ghost, at: min(index, result.count))
            }
        }
        return result
    }

    /// Front to back, stable among equals. The ordinary equal-depth case
    /// needs only one linear check, and no invalidation cache.
    static func hitOrder(_ views: [PaintView]) -> [PaintView] {
        func z(_ view: PaintView) -> CGFloat {
            #if os(macOS)
            return view.layer?.zPosition ?? 0
            #else
            return view.layer.zPosition
            #endif
        }
        guard let first = views.first else { return [] }
        let depth = z(first)
        guard views.contains(where: { z($0) != depth }) else { return views.reversed() }
        return views.enumerated().sorted {
            let a = z($0.element), b = z($1.element)
            return a != b ? a > b : $0.offset > $1.offset
        }.map(\.element)
    }
}

extension PaintView {
    fileprivate var paintForeground: Bool { (objc_getAssociatedObject(self, &paintForegroundKey) as? Bool) == true }
    fileprivate var siblingPaintRank: Int64 { paintForeground ? Int64.max : (self as? NodeView)?.paintRank ?? 0 }

    /// Native navigation replaces authored route holders, and a top-layer
    /// popover sits above the document. Their foreground plane must survive
    /// ranked authored siblings, just as their previous last-subview did.
    func setPaintForeground(_ on: Bool = true) {
        #if os(macOS)
        if on { wantsLayer = true }
        #endif
        objc_setAssociatedObject(self, &paintForegroundKey, on, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        if let superview { PaintOrder.changed(superview) }
        else { setPaintPosition(0) }
    }

    /// The one writer of a node's effective depth, including its stand-ins.
    func setPaintPosition(_ value: CGFloat) {
        #if os(macOS)
        if layer?.zPosition != value { layer?.zPosition = value }
        #else
        if layer.zPosition != value { layer.zPosition = value }
        Shadow.active?.rank(layer, value)
        #endif
        if let picture = (self as? NodeView)?.boxFilter?.picture {
            if picture.zPosition != value { picture.zPosition = value }
            #if os(iOS) || os(tvOS)
            Shadow.active?.rank(picture, value)
            #endif
        }
    }
}

#if os(macOS)
extension NSView {
    /// AppKit already walked the successful branch. Reuse that result and
    /// try only branches painting above it, including negative z.
    func raisedHit(_ hit: NSView?, _ point: NSPoint) -> NSView? {
        guard let hit, hit !== self, !isHidden else { return hit }
        var branch = hit
        while let parent = branch.superview, parent !== self { branch = parent }
        guard branch.superview === self else { return hit }
        let local = convert(point, from: superview)
        for view in NodeView.hitOrder(subviews) {
            if view === branch { return hit }
            if let found = view.hitTest(local) { return found }
        }
        return hit
    }
}
#endif

#if os(iOS) || os(tvOS)
/// A leaf has no UIView, but it still occludes siblings for input. Runs
/// retain each border box: the gaps between their shapes remain hittable.
final class FlatHit {
    weak var owner: NodeView?
    let frames: [CGRect]
    init(owner: NodeView, frames: [CGRect]) { self.owner = owner; self.frames = frames }
}
private var flatHitKey: UInt8 = 0
extension CALayer {
    var flatHit: FlatHit? {
        get { objc_getAssociatedObject(self, &flatHitKey) as? FlatHit }
        set { objc_setAssociatedObject(self, &flatHitKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}

extension NodeView {
    /// The same front-to-back order, with flat layers in their actual tree
    /// slots. Decoration layers never become input targets.
    static func hitChildren(in holder: UIView, at point: CGPoint, with event: UIEvent?,
                            visit: ((UIView) -> UIView?)? = nil) -> UIView? {
        let views = holder.subviews
        func hit(_ view: UIView) -> UIView? {
            if let visit { return visit(view) }
            return view.hitTest(holder.convert(point, to: view), with: event)
        }
        let layers = holder.layer.sublayers ?? []
        guard layers.contains(where: { $0.flatHit != nil }) else {
            for view in hitOrder(views) { if let found = hit(view) { return found } }
            return nil
        }
        let byLayer = Dictionary(uniqueKeysWithValues: views.map { (ObjectIdentifier($0.layer), $0) })
        let depth = layers.first?.zPosition ?? 0
        let ordered: [CALayer]
        if layers.allSatisfy({ $0.zPosition == depth }) { ordered = layers.reversed() }
        else {
            ordered = layers.enumerated().sorted {
                $0.element.zPosition != $1.element.zPosition ? $0.element.zPosition > $1.element.zPosition : $0.offset > $1.offset
            }.map(\.element)
        }
        for layer in ordered {
            if let view = byLayer[ObjectIdentifier(layer)] {
                if let found = hit(view) { return found }
            } else if !layer.isHidden, let flat = layer.flatHit, let owner = flat.owner,
                      !owner.inert, owner.style["pointer_events"]?.string != "none",
                      flat.frames.contains(where: { $0.contains(point) }) { return owner }
        }
        return nil
    }
}
#endif
