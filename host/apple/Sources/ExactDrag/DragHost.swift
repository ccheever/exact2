// The Drag capability (LLP 1047.001 D4): height, transform and reorder
// handles. An app's composition links this module only when its plan has a
// drag, and installs it before any session is made.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactKit

public enum ExactDrag {
    public static func install() { DragLink.installed = DragHost() }
}

final class DragHost: DragCapability {
    #if os(macOS)
    func mouseDrags(_ presenter: Presenter) -> MouseDrags {
        MouseDrags(reorder: MouseReorder(presenter), transform: MouseTransformDrag(presenter), height: MouseHeightDrag(presenter))
    }
    #else
    func updateReorderGesture(_ view: NodeView) { view.updateReorderGesture() }
    func updateTransformGesture(_ view: NodeView) { view.updateTransformDragGesture() }
    func updateHeightGesture(_ view: NodeView) { view.updateHeightDragGesture() }
    func shouldBegin(_ view: NodeView, _ gesture: UIGestureRecognizer) -> Bool? {
        view.reorderShouldBegin(gesture) ?? view.transformShouldBegin(gesture)
    }
    func recognizedPinch(_ handle: NodeView, scale: Double, focal: CGPoint) -> String? {
        TransformDragHold.recognizedPinch(handle, scale: scale, focal: focal)
    }
    #endif
}

extension HeightDragHold: DragInput {}
extension TransformDragHold: DragInput {}
extension ReorderHold: ReorderLift {}

#if os(macOS)
extension MouseReorder: MouseDrag {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil || grouped }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
extension MouseTransformDrag: MouseTransform {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
extension MouseHeightDrag: MouseDrag {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
#endif
