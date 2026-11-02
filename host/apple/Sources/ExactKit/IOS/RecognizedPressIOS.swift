#if os(iOS) || os(tvOS)
import UIKit

/// The held agent contact's feedback, cancelled by the same nodes as a real touch.
final class RecognizedPress {
    private let shown: HeldNode
    private(set) var cancelled = false
    init(_ node: NodeView) { shown = HeldNode(node) }
    func cancel() {
        guard !cancelled else { return }
        cancelled = true
        guard let node = shown.node, node.incarnation == shown.incarnation else { return }
        if !node.hasPressContact { node.pressed = node.overlayHoldsPress }
        (node.presenter?.controls.controls[node.id] as? UIButton)?.isHighlighted = false
    }
}
#endif
