#if os(iOS)
import UIKit

/// What VoiceOver reads for an empty compact picker in place of one of
/// UIKit's own elements for it. UIKit's element computes its value from the
/// picker's date, which an empty picker still holds, and ignores an assigned
/// one; so while the picker is empty it exposes these instead
/// (`UIAccessibilityContainer`, `BoundDatePicker.accessibilityElements`).
/// Each says what its UIKit element says, but for the value, which is the
/// author's placeholder or none, and activating it activates that element,
/// which opens the calendar or wheel; where that element leaves activation
/// to VoiceOver, VoiceOver taps its activation point, on UIKit's content.
final class EmptyDateElement: UIAccessibilityElement, AgentOwned {
    private(set) weak var native: NSObject?
    private weak var picker: BoundDatePicker?
    init(standingFor native: NSObject, in picker: BoundDatePicker) {
        self.native = native
        self.picker = picker
        super.init(accessibilityContainer: picker)
    }
    var agentViewId: UInt32? { picker.map { UInt32($0.tag) } }
    override var accessibilityLabel: String? { get { native?.accessibilityLabel } set {} }
    override var accessibilityHint: String? { get { native?.accessibilityHint } set {} }
    override var accessibilityValue: String? {
        get { picker.flatMap { $0.placeholder.isEmpty ? nil : $0.placeholder } }
        set {}
    }
    override var accessibilityTraits: UIAccessibilityTraits { get { native?.accessibilityTraits ?? .button } set {} }
    override var accessibilityFrame: CGRect { get { native?.accessibilityFrame ?? .zero } set {} }
    override var accessibilityActivationPoint: CGPoint {
        get { native.map(\.accessibilityActivationPoint) ?? CGPoint(x: accessibilityFrame.midX, y: accessibilityFrame.midY) }
        set {}
    }
    override var accessibilityIdentifier: String? { get { (native as? UIAccessibilityIdentification)?.accessibilityIdentifier } set {} }
    override var accessibilityCustomActions: [UIAccessibilityCustomAction]? { get { native?.accessibilityCustomActions } set {} }
    override func accessibilityActivate() -> Bool { native?.accessibilityActivate() ?? false }
}

extension BoundDatePicker {
    /// The elements UIKit exposes for this picker, found as VoiceOver finds
    /// them: its declared elements, else its subviews, down to each element.
    func nativeAccessibilityElements() -> [NSObject] {
        var found: [NSObject] = [], seen = Set<ObjectIdentifier>()
        func visit(_ object: NSObject) {
            guard seen.insert(ObjectIdentifier(object)).inserted else { return }
            if object.isAccessibilityElement { found.append(object); return }
            if let elements = object.accessibilityElements, !elements.isEmpty {
                for case let element as NSObject in elements { visit(element) }
                return
            }
            let count = object.accessibilityElementCount()
            if count != NSNotFound, count > 0 {
                for i in 0..<count { if let element = object.accessibilityElement(at: i) as? NSObject { visit(element) } }
            } else if let view = object as? UIView {
                for child in view.subviews { visit(child) }
            }
        }
        let own = (super.accessibilityElements ?? []).compactMap { $0 as? NSObject }
        for object in own.isEmpty ? subviews : own { visit(object) }
        return found
    }
}
#endif
