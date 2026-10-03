// UIKit 27.1's fold, resolved through the Objective-C runtime (LLP 1078
// D5): `-[UIView reservedRegionsOfKind:options:]` with the division kind
// (the fold band and its margins, active while the hinge is partially
// open) and `UIHingeInteraction` (an update when the hinge moves, which
// changes a region's `isActive` without changing any bounds). Resolved by
// name, not compiled against the SDK: the repo's default Xcode 27.0 lacks
// the symbols and ships the same Swift 6.4 as the 27.1 beta, so no
// `#if compiler` or `canImport` tells the two SDKs apart; a 27.0 build
// finds neither class at run time and reports a flat viewport, a 27.1
// build on a Duo finds both. Below iOS 27.1 the classes do not exist.
#if os(iOS)
import UIKit
import ObjectiveC

enum ReservedRegions {
    private static let divisionKind: AnyObject? = {
        guard let kind = NSClassFromString("UIViewReservedRegionKind") as? NSObject.Type else { return nil }
        return kind.perform(NSSelectorFromString("divisionRegionKind"))?.takeUnretainedValue()
    }()
    private static let query = NSSelectorFromString("reservedRegionsOfKind:options:")
    /// Whether this UIKit has reserved regions at all (27.1 and later).
    static var available: Bool { divisionKind != nil && UIView.instancesRespond(to: query) }

    /// The view's division regions, active and inactive, in its own
    /// coordinates: each frame includes the margins. `nil` below 27.1.
    static func divisions(of view: UIView) -> [(frame: CGRect, active: Bool)]? {
        guard let kind = divisionKind, view.responds(to: query) else { return nil }
        typealias Query = @convention(c) (AnyObject, Selector, AnyObject, UInt) -> NSArray?
        let imp = view.method(for: query)
        let includeInactive: UInt = 1
        let regions = unsafeBitCast(imp, to: Query.self)(view, query, kind, includeInactive) ?? []
        return regions.compactMap { region in
            guard let object = region as? NSObject, let frame = (object.value(forKey: "frame") as? NSValue)?.cgRectValue else { return nil }
            return (frame, (object.value(forKey: "active") as? Bool) ?? false)
        }
    }

    /// Add a `UIHingeInteraction` whose updates call `changed` with the
    /// hinge's status (0 unknown, 1 closed, 2 partially open, 3 fully open;
    /// `nil` when the update carries no hinge). `false` below 27.1.
    @discardableResult
    static func observeHinge(on view: UIView, changed: @escaping (Int?) -> Void) -> Bool {
        guard let cls = NSClassFromString("UIHingeInteraction") as? NSObject.Type else { return false }
        let handler: @convention(block) (AnyObject, AnyObject) -> Void = { _, update in
            let hinge = (update as? NSObject)?.value(forKey: "hinge") as? NSObject
            changed(hinge.flatMap { ($0.value(forKey: "status") as? NSNumber)?.intValue })
        }
        // `alloc` hands over one reference; `init…` consumes it and returns the
        // same object, so the result is taken unretained.
        guard let allocated = cls.perform(NSSelectorFromString("alloc"))?.takeRetainedValue() as? NSObject,
              let interaction = allocated.perform(NSSelectorFromString("initWithUpdateHandler:"), with: unsafeBitCast(handler, to: AnyObject.self))?.takeUnretainedValue() as? UIInteraction
        else { return false }
        view.addInteraction(interaction)
        return true
    }
}
#endif
