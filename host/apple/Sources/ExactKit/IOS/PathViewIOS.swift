#if os(iOS)
import UIKit
import QuartzCore

/// A `path` node's drawing (LLP 1065 D6): a `CAShapeLayer` over the node's
/// content box. The path is the kernel's, built once per `d`, and fitted by
/// the view box on every layout. Core Animation renders a shape layer's
/// path at the resolution it is displayed at, so a node `scale` above one
/// stays sharp (measured at 3×: pixel-identical with a backing scaled to
/// match, so none is kept). The engine's
/// `stroke-start`/`stroke-end` arrive as `present` ops and set the layer's
/// own `strokeStart`/`strokeEnd`, which measure the whole path's length in
/// subpath order, as the web's split strokes do. The layer backs this view,
/// so UIKit adds no implicit animation: the engine is the only clock.
final class PathView: UIView {
    override class var layerClass: AnyClass { CAShapeLayer.self }
    private var shape: CAShapeLayer { layer as! CAShapeLayer }
    private weak var owner: NodeView?
    private var boundID: UInt32?
    private var data: String?
    private var unit: CGPath?
    private var viewBox: CGRect?
    private var width: CGFloat = 1

    /// The node's path view, if it is a path.
    static func of(_ node: NodeView) -> PathView? {
        node.kind == "path" ? node.subviews.lazy.compactMap { $0 as? PathView }.first : nil
    }

    /// After a node's props or style change: build, restyle and place its
    /// path view. A view taken for a new node starts at identity strokes,
    /// as every presentation does (`NodePool.rebind`).
    static func sync(_ node: NodeView) {
        guard node.kind == "path" else { return }
        let view = of(node) ?? {
            let v = PathView(frame: .zero)
            v.owner = node
            v.isUserInteractionEnabled = false
            v.clipsToBounds = true // SVG clips a path to its viewport
            node.addSubview(v)
            return v
        }()
        if view.boundID != node.id {
            view.boundID = node.id
            view.shape.strokeStart = 0
            view.shape.strokeEnd = 1
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
    func present(_ property: String, _ value: CGFloat) {
        let v = min(max(value, 0), 1)
        if property == "stroke-start" { shape.strokeStart = v } else { shape.strokeEnd = v }
    }

    /// The node's content box, in the node's coordinates (CSS: padding and
    /// border sit outside the viewport).
    func place() {
        guard let owner else { return }
        let box = owner.bounds.width > 0 && owner.bounds.height > 0 ? owner.contentBox() : .zero
        let target = box.width > 0 && box.height > 0 ? box : .zero
        if frame != target { frame = target }
        fit()
    }

    private func restyle() {
        guard let owner else { return }
        if owner.props["pathData"] != data {
            data = owner.props["pathData"]
            unit = VectorPath.path(data)
        }
        viewBox = VectorPath.viewBox(owner.props["viewBox"])
        width = VectorPath.paint(shape, owner.style, dark: traitCollection.userInterfaceStyle == .dark)
        // Decorative unless labelled; labelled, it is an image.
        owner.isAccessibilityElement = owner.props["accessibilityLabel"] != nil
        if owner.isAccessibilityElement { owner.accessibilityTraits.insert(.image) }
        fit()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        fit()
    }

    private func fit() {
        VectorPath.place(shape, unit, viewBox: viewBox, width: width, in: bounds.size)
    }
}
#endif
