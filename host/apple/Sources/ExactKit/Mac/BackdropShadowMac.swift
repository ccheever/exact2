// A backdrop box's outer shadows on macOS (LLP 1053.000 §3, LLP 1064 D2).
// AppKit filters a layer's backdrop only while the layer masks to its bounds
// (`Backdrop.swift`), and that mask clipped the outer shadows, which CSS
// draws past the border box, together with the caster at the bottom of the
// layer (`BoxShadow.swift`). The caster stands beside the layer instead: in a
// contentless layer directly under the node's in its superlayer, with the
// node layer's geometry, transform, opacity, visibility and depth, and the
// animations Core Animation plays on those (a lowered keyframe animation, a
// press's ease). The shadow keeps CSS's order, under the box and over what
// precedes it, and the blur keeps its layer and its measured parity. A box
// that also has a `clip-path` or a `mask-image` keeps the caster in its
// layer, which clips the shadow with the box as CSS does.
//
// AppKit attaches a new view's layer, and reorders its siblings' layers, at
// the window's next display, so the stand finds its place again as each
// batch ends and on the turns after, until the node's layer is there.
#if os(macOS)
import AppKit

final class ShadowStand: CALayer {
    /// The node layer's animations that also play here, by key.
    fileprivate var mirrored: [String: CAAnimation] = [:]

    override init() {
        super.init()
        let none = NSNull()
        actions = ["bounds": none, "position": none, "anchorPoint": none, "transform": none, "sublayerTransform": none,
                   "opacity": none, "hidden": none, "zPosition": none, "sublayers": none, "geometryFlipped": none,
                   "onOrderIn": none, "onOrderOut": none]
    }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    /// The nodes whose shadows stand beside them.
    static let standing = NSHashTable<NodeView>.weakObjects()
    private static var queued = false, waits = 0

    /// Every stand to its place, now and on the next turn, after AppKit has
    /// attached and ordered the layers of views this batch added or moved.
    static func followAll() {
        guard standing.count > 0 else { return }
        for node in standing.allObjects { _ = node.followShadowStand() }
        later()
    }

    /// Once more on the next turn; a node whose layer is still not attached
    /// (a window that has not displayed) is tried on a few more, then left
    /// to the next batch.
    fileprivate static func later() {
        guard !queued else { return }
        queued = true
        DispatchQueue.main.async {
            queued = false
            let unplaced = standing.allObjects.filter { !$0.followShadowStand() }
            waits = unplaced.isEmpty ? 0 : waits + 1
            if !unplaced.isEmpty, waits < 4 { later() } else { waits = 0 }
        }
    }

    /// Whether the node layer's animation `a` moves or shows the stand too.
    fileprivate static func follows(_ a: CAAnimation) -> Bool {
        guard let path = (a as? CAPropertyAnimation)?.keyPath, let root = path.split(separator: ".").first else { return false }
        return ["opacity", "transform", "position", "bounds", "anchorPoint", "sublayerTransform", "zPosition", "hidden"].contains(String(root))
    }
}

extension NodeView {
    private static var standKey = 0

    /// The layer its outer shadows cast from while they stand beside it.
    var shadowStand: ShadowStand? {
        get { objc_getAssociatedObject(self, &Self.standKey) as? ShadowStand }
        set { objc_setAssociatedObject(self, &Self.standKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }

    /// Whether its outer shadows cast beside its layer: the layer masks for
    /// a backdrop blur, and nothing else clips the shadow with the box.
    var castsBeside: Bool { backdropDrawn != nil && clipPath == nil && style["mask_image"] == nil }

    /// The layer the outer casters go into: its stand, made and placed, or
    /// its own (the stand gone).
    func outerShadowHost() -> CALayer? {
        guard castsBeside else { removeShadowStand(); return layer }
        if let stand = shadowStand { return stand }
        let stand = ShadowStand()
        shadowStand = stand
        ShadowStand.standing.add(self)
        if !followShadowStand() { ShadowStand.later() }
        return stand
    }

    func removeShadowStand() {
        guard let stand = shadowStand else { return }
        stand.removeFromSuperlayer()
        shadowStand = nil
        ShadowStand.standing.remove(self)
    }

    /// A backdrop came or went: the casters move to where they now cast.
    func syncShadowStand() {
        guard let caster = shadowCaster else { return removeShadowStand() }
        let beside = shadowStand != nil && caster.superlayer === shadowStand
        if beside != castsBeside { applyShadow() } else { _ = followShadowStand() }
    }

    /// The stand directly under the layer in its superlayer, as it stands
    /// and plays; whether it is placed (false while AppKit has yet to attach
    /// the layer).
    @discardableResult
    func followShadowStand() -> Bool {
        guard let stand = shadowStand else { return true }
        guard superview != nil, let l = layer, let parent = l.superlayer else {
            stand.removeFromSuperlayer()
            return superview == nil
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let siblings = parent.sublayers ?? []
        if let at = siblings.firstIndex(where: { $0 === l }), at == 0 || siblings[at - 1] !== stand {
            parent.insertSublayer(stand, below: l)
        }
        if stand.bounds != l.bounds { stand.bounds = l.bounds }
        if stand.anchorPoint != l.anchorPoint { stand.anchorPoint = l.anchorPoint }
        if stand.position != l.position { stand.position = l.position }
        if !CATransform3DEqualToTransform(stand.transform, l.transform) { stand.transform = l.transform }
        if !CATransform3DEqualToTransform(stand.sublayerTransform, l.sublayerTransform) { stand.sublayerTransform = l.sublayerTransform }
        if stand.isGeometryFlipped != l.isGeometryFlipped { stand.isGeometryFlipped = l.isGeometryFlipped }
        if stand.opacity != l.opacity { stand.opacity = l.opacity }
        if stand.isHidden != l.isHidden { stand.isHidden = l.isHidden }
        if stand.zPosition != l.zPosition { stand.zPosition = l.zPosition }
        // The animations playing on the layer, each added once (adding
        // copies one; a replaced one is another object).
        var playing: Set<String> = []
        for key in l.animationKeys() ?? [] {
            guard let a = l.animation(forKey: key), ShadowStand.follows(a) else { continue }
            playing.insert(key)
            if stand.mirrored[key] !== a { stand.add(a, forKey: key); stand.mirrored[key] = a }
        }
        for key in stand.mirrored.keys where !playing.contains(key) {
            stand.removeAnimation(forKey: key)
            stand.mirrored[key] = nil
        }
        return true
    }

    // A present op's opacity (an enter's, an exit's), the disabled look's.
    package override var alphaValue: CGFloat {
        get { super.alphaValue }
        set { super.alphaValue = newValue; if shadowStand != nil { followShadowStand() } }
    }
}

extension Capture {
    /// A capture draws views, and the node's own drawing is clipped to its
    /// box with its layer: for the capture, a view directly under each
    /// standing node draws its shadow there, as the node would
    /// (`drawCapturedShadow`). The closure returned takes them out.
    static func standShadows(in root: NSView) -> () -> Void {
        var shown: [NSView] = []
        for node in ShadowStand.standing.allObjects where node.isDescendant(of: root) && !node.isHiddenOrHasHiddenAncestor {
            guard let parent = node.superview, let caster = node.shadowCaster, caster.superlayer === node.shadowStand else { continue }
            let reach = node.boxShadows.filter { !$0.inset }.map(\.reach).max() ?? 0
            let view = CapturedShadow(node: node, reach: reach)
            view.frame = node.frame.insetBy(dx: -reach, dy: -reach)
            view.alphaValue = node.alphaValue
            parent.addSubview(view, positioned: .below, relativeTo: node)
            shown.append(view)
        }
        return { shown.forEach { $0.removeFromSuperview() } }
    }
}

/// A standing node's shadow in a capture, its box `reach` in from each edge.
private final class CapturedShadow: NSView {
    weak var node: NodeView?
    let reach: CGFloat
    init(node: NodeView, reach: CGFloat) {
        self.node = node
        self.reach = reach
        super.init(frame: .zero)
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func draw(_ dirtyRect: NSRect) {
        guard let node, let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.translateBy(x: reach, y: reach)
        node.drawCapturedShadow(ctx)
    }
}
#endif
