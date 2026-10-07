#if os(iOS)
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

/// Routes remain Exact-owned. A dirty editor uses its authored guarded Back action.
final class T3MobileScheduledNavigation {
    private var guarded = Set<String>()
    private var previousGesture: [ObjectIdentifier: Bool] = [:]

    func configure(_ route: ExactRoute, editor: Bool, backActionID: String) {
        let item = route.controller.navigationItem
        if editor {
            guarded.insert(route.key)
            route.controller.isModalInPresentation = true
            item.hidesBackButton = true
            item.leftBarButtonItem = UIBarButtonItem(image: UIImage(systemName: "chevron.left"), primaryAction: UIAction { [weak route] _ in
                route?.element(backActionID)?.click()
            })
            item.leftBarButtonItem?.accessibilityLabel = "Back"
        } else if guarded.remove(route.key) != nil {
            route.controller.isModalInPresentation = false
            item.hidesBackButton = false; item.leftBarButtonItem = nil
        }
        // Exact may call route() before assigning the native stack's new top controller.
        DispatchQueue.main.async { [weak self, weak route] in
            guard let self, let route, route.isLive, let navigation = route.navigation?.controller,
                  navigation.topViewController === route.controller,
                  let gesture = navigation.interactivePopGestureRecognizer else { return }
            let key = ObjectIdentifier(navigation)
            if editor {
                if self.previousGesture[key] == nil { self.previousGesture[key] = gesture.isEnabled }
                gesture.isEnabled = false
            } else if let previous = self.previousGesture.removeValue(forKey: key) {
                gesture.isEnabled = previous
            }
        }
    }
    func end(_ route: ExactRoute) { guarded.remove(route.key) }
}
#endif
