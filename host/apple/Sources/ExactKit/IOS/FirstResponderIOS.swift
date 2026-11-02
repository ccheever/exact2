#if os(iOS) || os(tvOS)
import UIKit

/// The window's first responder, found the way UIKit offers: an action sent
/// to nil reaches it.
enum FirstResponder {
    private static weak var found: UIResponder?
    static var current: UIResponder? {
        found = nil
        UIApplication.shared.sendAction(#selector(UIResponder.exactReportFirstResponder), to: nil, from: nil, for: nil)
        return found
    }
    fileprivate static func report(_ responder: UIResponder) { found = responder }
}

extension UIResponder {
    @objc fileprivate func exactReportFirstResponder() { FirstResponder.report(self) }
}
#endif
