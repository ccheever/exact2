#if os(iOS)
import UIKit

extension UITraitCollection {
    /// Whether the system places a vertical bar here (the iPhone Duo):
    /// UIKit 27.1's `verticalBarEdge` other than `.unspecified` (raw 0),
    /// read by name. The repo's default Xcode 27.0 has no such symbol and the
    /// same Swift 6.4 as Xcode 27.1, so no `#if` tells the SDKs apart
    /// (ReservedRegions.swift); below iOS 27.1 there is none, and false.
    var placesVerticalBar: Bool {
        let edge = NSSelectorFromString("verticalBarEdge")
        guard responds(to: edge) else { return false }
        return ((value(forKey: "verticalBarEdge") as? NSNumber)?.intValue ?? 0) != 0
    }
}
#endif
