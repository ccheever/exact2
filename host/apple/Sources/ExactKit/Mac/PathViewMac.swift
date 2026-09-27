#if os(macOS)
import AppKit
import QuartzCore

/// A `path` node's drawing on AppKit (LLP 1065 D6): the UIKit view's twin —
/// `VectorLayers` in a flipped view over the node's content box, the
/// strokes set by the engine's `present` ops with implicit animation off.
final class PathView: NSView {
    private let host = CALayer()
    private(set) lazy var layers = VectorLayers(in: host)
    private weak var owner: NodeView?
    private var boundID: UInt32?

    override var isFlipped: Bool { true }
    override func makeBackingLayer() -> CALayer { host }
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
            node.addSubview(v)
            return v
        }()
        view.still {
            if view.boundID != node.id {
                view.boundID = node.id
                view.layers.reset()
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
    func present(_ property: String, _ value: CGFloat) { layers.present(property, value) }

    /// The node's content box, in the node's (flipped) coordinates.
    func place() {
        guard let owner else { return }
        let box = owner.bounds.width > 0 && owner.bounds.height > 0 ? owner.contentBox() : .zero
        let target = box.width > 0 && box.height > 0 ? box : .zero
        if frame != target { frame = target }
        fit()
    }

    /// Paint again: the node's style, or a paint value moving over it.
    func restyle() {
        guard let owner else { return }
        still { layers.restyle(owner, dark: owner.drawsDark, props: owner.props) }
        let labelled = owner.props["accessibilityLabel"] != nil
        owner.setAccessibilityElement(labelled)
        if labelled { owner.setAccessibilityRole(.image) }
        fit()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        restyle()
    }

    override func layout() {
        super.layout()
        fit()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        fit()
    }

    private func fit() {
        still { layers.place(host, in: bounds.size) }
    }
}
#endif
