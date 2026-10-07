#if os(iOS)
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
// T3 Code 365aa87982 Stack.tsx header presets and ConnectionsNewRouteScreen.
// Exact owns stacks, presentation/detents, route views and scroll geometry.
import UIKit

enum T3MobileNavigation {
    static func configure(_ navigation: ExactNavigation) {
        let bar = navigation.controller.navigationBar
        bar.prefersLargeTitles = false
        func appearance(_ existing: UINavigationBarAppearance?) -> UINavigationBarAppearance {
            let result = existing?.copy() as? UINavigationBarAppearance ?? UINavigationBarAppearance()
            result.titleTextAttributes[.font] = UIFont.systemFont(ofSize: 18, weight: .heavy)
            result.shadowColor = .clear
            return result
        }
        bar.standardAppearance = appearance(bar.standardAppearance)
        bar.scrollEdgeAppearance = appearance(bar.scrollEdgeAppearance ?? bar.standardAppearance)
        bar.compactAppearance = appearance(bar.compactAppearance ?? bar.standardAppearance)
    }

    static func configure(_ route: ExactRoute, formSheet: Bool, scanActionID: String?, scannerOpen: Bool, tint: UIColor) {
        let item = route.controller.navigationItem
        item.largeTitleDisplayMode = .never
        item.backButtonDisplayMode = .minimal
        // Custom symbol views paint the authored header; Exact's bar projection
        // accepts its built-in image vocabulary. Decorate the public bar items
        // by their authored accessibility labels, retaining their targets/actions.
        let symbols = ["Show sidebar": "sidebar.left", "Hide sidebar": "sidebar.left",
                       "New task": "square.and.pencil", "Terminal": "terminal",
                       "Review changes": "plus.forwardslash.minus", "Files": "folder",
                       "Agents": "person.2", "Terminal options": "terminal",
                       "New scheduled task": "plus", "Filter usage environments": "line.3.horizontal.decrease",
                       "Refresh usage": "arrow.clockwise"]
        for button in (item.leftBarButtonItems ?? []) + (item.rightBarButtonItems ?? []) {
            if let label = button.accessibilityLabel, let symbol = symbols[label] {
                button.image = UIImage(systemName: symbol)
                button.tintColor = tint
            }
        }
        // The authored native header remains the title source. A sheet's fractional
        // resting heights must be projected through navigationDetent by the root.
        guard let id = scanActionID, !id.isEmpty else { return }
        let name = scannerOpen ? "xmark" : "qrcode.viewfinder"
        let button = UIBarButtonItem(image: UIImage(systemName: name), primaryAction: UIAction { [weak route] _ in
            route?.element(id)?.click()
        })
        button.tintColor = tint
        button.accessibilityLabel = scannerOpen ? "Close scanner" : "Scan QR code"
        button.accessibilityIdentifier = "mobile-scan-qr"
        item.rightBarButtonItem = button
    }
}

#endif
