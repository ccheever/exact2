#if os(iOS)
import UIKit
import ObjectiveC

/// The compact picker's exposed element computes its value and ignores
/// assignments. Override the public getter, preserving its class's activation
/// and delegating its nonempty value back to UIKit.
enum DateAccessibility {
    private final class Owner {
        weak var picker: BoundDatePicker?
        init(_ picker: BoundDatePicker) { self.picker = picker }
    }
    private static var ownerKey: UInt8 = 0
    private static var classes: [ObjectIdentifier: AnyClass] = [:]

    static func value(of element: NSObject, follows picker: BoundDatePicker) {
        if let owner = objc_getAssociatedObject(element, &ownerKey) as? Owner {
            owner.picker = picker
            return
        }
        guard let base = object_getClass(element) else { return }
        let key = ObjectIdentifier(base)
        let subclass: AnyClass
        if let cached = classes[key] { subclass = cached } else {
            let selector = NSSelectorFromString("accessibilityValue")
            guard let method = class_getInstanceMethod(base, selector),
                  let type = objc_allocateClassPair(base, "ExactEmptyDate_" + NSStringFromClass(base), 0) else { return }
            typealias Getter = @convention(c) (NSObject, Selector) -> Unmanaged<NSString>?
            let original = unsafeBitCast(method_getImplementation(method), to: Getter.self)
            let get: @convention(block) (NSObject) -> NSString? = { object in
                if let picker = (objc_getAssociatedObject(object, &ownerKey) as? Owner)?.picker, picker.empty {
                    return picker.placeholder as NSString
                }
                return original(object, selector)?.takeUnretainedValue()
            }
            class_addMethod(type, selector, imp_implementationWithBlock(get), method_getTypeEncoding(method))
            objc_registerClassPair(type)
            classes[key] = type
            subclass = type
        }
        objc_setAssociatedObject(element, &ownerKey, Owner(picker), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        object_setClass(element, subclass)
    }
}
#endif
