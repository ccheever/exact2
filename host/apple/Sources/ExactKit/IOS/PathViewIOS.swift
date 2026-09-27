#if os(iOS)
import UIKit
import QuartzCore

/// A `path` node's drawing (LLP 1065 D6): shape layers over the node's
/// content box (`VectorLayers`). The path is the kernel's, built once per
/// `d`, and fitted by the view box on every layout. Core Animation renders
/// a shape layer's path at the resolution it is displayed at, so a node
/// `scale` above one stays sharp (measured at 3×: pixel-identical with a
/// backing scaled to match, so none is kept). The engine's
/// `stroke-start`/`stroke-end` arrive as `present` ops. The layers are
/// still ones and this view's own layer takes no implicit animation from
/// UIKit: the engine is the only clock.
final class PathView: UIView {
    private(set) lazy var layers = VectorLayers(in: layer)
    private weak var owner: NodeView?
    private var boundID: UInt32?

    /// The node's path view, if it is a path.
    static func of(_ node: NodeView) -> PathView? {
        node.kind == "path" ? node.subviews.lazy.compactMap { $0 as? PathView }.first : nil
    }

    /// After a node's props or style change: build, restyle and place its
    /// path view. A view taken for a new node starts at identity strokes.
    static func sync(_ node: NodeView) {
        guard node.kind == "path" else { return }
        let view = of(node) ?? {
            let v = PathView(frame: .zero)
            v.owner = node
            v.isUserInteractionEnabled = false
            node.addSubview(v)
            return v
        }()
        if view.boundID != node.id {
            view.boundID = node.id
            view.layers.reset()
        }
        view.restyle()
        view.place()
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (view: PathView, _: UITraitCollection) in
            view.restyle()
        }
    }
    required init?(coder: NSCoder) { nil }

    /// A `present` op: the engine's value for this frame.
    func present(_ property: String, _ value: CGFloat) { layers.present(property, value) }

    /// The node's content box, in the node's coordinates (CSS: padding and
    /// border sit outside the viewport).
    func place() {
        guard let owner else { return }
        let box = owner.bounds.width > 0 && owner.bounds.height > 0 ? owner.contentBox() : .zero
        let target = box.width > 0 && box.height > 0 ? box : .zero
        if frame != target { frame = target }
        layers.place(layer, in: bounds.size)
    }

    /// Paint again: the node's style, or a paint value moving over it.
    func restyle() {
        guard let owner else { return }
        layers.restyle(owner, dark: traitCollection.userInterfaceStyle == .dark, props: owner.props)
        // Decorative unless labelled; labelled, it is an image.
        owner.isAccessibilityElement = owner.props["accessibilityLabel"] != nil
        if owner.isAccessibilityElement { owner.accessibilityTraits.insert(.image) }
        layers.place(layer, in: bounds.size)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        layers.place(layer, in: bounds.size)
    }
}
#endif
