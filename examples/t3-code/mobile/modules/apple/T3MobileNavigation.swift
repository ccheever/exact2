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
