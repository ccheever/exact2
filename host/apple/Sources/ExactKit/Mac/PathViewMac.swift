#if os(macOS)
import AppKit
import QuartzCore

/// A `path` node's drawing on AppKit (LLP 1065 D6): the UIKit view's twin —
/// a `CAShapeLayer` backing a flipped view over the node's content box, its
/// `strokeStart`/`strokeEnd` set by the engine's `present` ops with
/// implicit animation off.
final class PathView: NSView {
    private let shape = CAShapeLayer()
    private weak var owner: NodeView?
    private var boundID: UInt32?
    private var data: String?
    private var unit: CGPath?
    private var viewBox: CGRect?
    private var width: CGFloat = 1

    override var isFlipped: Bool { true }
    override func makeBackingLayer() -> CALayer { shape }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// The node's path view, if it is a path.
    static func of(_ node: NodeView) -> PathView? {
        node.kind == "path" ? node.subviews.lazy.compactMap { $0 as? PathView }.first : nil
    }

    /// After a node's props or style change: build, restyle and place.
    static func sync(_ node: NodeView) {
        guard node.kind == "path" else { return }
        let view = of(node) ?? {
            let v = PathView(frame: .zero)
            v.wantsLayer = true
            v.owner = node
            v.shape.masksToBounds = true // SVG clips a path to its viewport
            node.addSubview(v)
            return v
        }()
        view.still {
            if view.boundID != node.id {
                view.boundID = node.id
                view.shape.strokeStart = 0
                view.shape.strokeEnd = 1
            }
            view.restyle()
        }
        view.place()
    }

    private func still(_ change: () -> Void) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        change()
        CATransaction.commit()
    }

    /// A `present` op: the engine's value for this frame.
    func present(_ property: String, _ value: CGFloat) {
        let v = min(max(value, 0), 1)
        still { if property == "stroke-start" { shape.strokeStart = v } else { shape.strokeEnd = v } }
    }

    /// The node's content box, in the node's (flipped) coordinates.
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
        width = VectorPath.paint(shape, owner.style, dark: owner.drawsDark)
        let labelled = owner.props["accessibilityLabel"] != nil
        owner.setAccessibilityElement(labelled)
        if labelled { owner.setAccessibilityRole(.image) }
        fit()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        still { restyle() }
    }

    override func layout() {
        super.layout()
        still { fit() }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        still { fit() }
    }

    private func fit() {
        VectorPath.place(shape, unit, viewBox: viewBox, width: width, in: bounds.size)
    }
}
#endif
