// @ref LLP 1047.001 D4 — drags (height, transform and reorder handles) are a
// linked capability: their holds and gestures are the `ExactDrag` module,
// which an app's composition links only when its plan has a drag, and
// installs here. The core holds them through these protocols; without the
// module a node never takes a drag gesture, and a plan that has one is
// refused at boot (`Unlinked("drag")`). What a List's reorder shares with a
// grouped list's (`ReorderParts`) stays in the core.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// What the core asks of the capability.
package protocol DragCapability: AnyObject {
    #if os(macOS)
    /// A presenter's mouse drags.
    func mouseDrags(_ presenter: Presenter) -> MouseDrags
    #else
    /// A node's reorder handle, after its props changed.
    func updateReorderGesture(_ view: NodeView)
    /// A node's transform drag, after its binding changed.
    func updateTransformGesture(_ view: NodeView)
    /// A node's height drag, after its binding changed.
    func updateHeightGesture(_ view: NodeView)
    /// Whether a drag's recognizer on `view` begins; nil when `gesture` is
    /// none of the drags'.
    func shouldBegin(_ view: NodeView, _ gesture: UIGestureRecognizer) -> Bool?
    /// The agent's pinch on a transform handle: why it was refused, or nil.
    func recognizedPinch(_ handle: NodeView, scale: Double, focal: CGPoint) -> String?
    #endif
}

/// A drag the session holds while its input lasts.
package protocol DragInput: AnyObject {
    func cancel()
    func cancelIfInputIneligible()
    /// A transform hold's motion the runtime retired.
    func retire(runtime: UInt64, token: UInt64)
}

extension DragInput {
    package func retire(runtime: UInt64, token: UInt64) {}
}

/// The presenter's lifted List row.
package protocol ReorderLift: AnyObject {
    func observe(_ next: ReorderState?)
    func raiseLifted()
    func lifts(_ view: UInt32) -> Bool
    func abandon()
}

/// The Drag module, installed by the composition.
package enum DragLink {
    package static var installed: DragCapability?
}

#if os(macOS)
/// One of a presenter's mouse drags.
package protocol MouseDrag: MouseRecognizer {
    func retire(_ id: UInt32)
}

/// The mouse transform drag, which also takes a wheel and a magnify.
package protocol MouseTransform: MouseDrag {
    func scroll(_ node: NodeView, event: NSEvent) -> Bool
    func magnify(_ node: NodeView, event: NSEvent) -> Bool
}

/// A presenter's three mouse drags, in the chain's order.
package struct MouseDrags {
    package let reorder: MouseDrag
    package let transform: MouseTransform
    package let height: MouseDrag
    package init(reorder: MouseDrag, transform: MouseTransform, height: MouseDrag) {
        self.reorder = reorder; self.transform = transform; self.height = height
    }
    /// An app without the capability: no mouse drag arms.
    static let none = MouseDrags(reorder: NoMouseDrag(), transform: NoMouseDrag(), height: NoMouseDrag())
}

private final class NoMouseDrag: MouseTransform {
    var armed: NodeView? { nil }
    var engaged: Bool { false }
    func arm(_ node: NodeView, event: NSEvent) {}
    func drag(_ event: NSEvent) -> Bool { false }
    func up(_ event: NSEvent) -> Bool { false }
    func cancel() {}
    func retire(_ id: UInt32) {}
    func scroll(_ node: NodeView, event: NSEvent) -> Bool { false }
    func magnify(_ node: NodeView, event: NSEvent) -> Bool { false }
}
#endif
