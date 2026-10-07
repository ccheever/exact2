#if os(iOS)
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
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
        if route.element("new-task-cancel") != nil {
            item.hidesBackButton = true
            item.leftItemsSupplementBackButton = false
        }
        // Custom symbol views paint the authored header; Exact's bar projection
        // accepts its built-in image vocabulary. Decorate the public bar items
        // by their authored accessibility labels, retaining their targets/actions.
        let symbols = ["Show inspector": "sidebar.right", "Hide inspector": "sidebar.right", "Git": "arrow.triangle.branch", "Show sidebar": "sidebar.left", "Hide sidebar": "sidebar.left",
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
        // The authored native header remains the title source.
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

// @ref llp/1107.005-composer-and-transcript.decision.md#settings-ownership
// GAP 009: no pre-presentation modal-wrapper hook; keep Exact's pageSheet style.
// Pinned Stack.tsx: Model uses one full-height detent; compact New Task
// uses 92% of UIKit's maximum detent. Exact owns presentation style and pops.
final class T3MobileSheets: NSObject, UINavigationControllerDelegate {
    private final class Entry {
        weak var route: ExactRoute?
        weak var configuredController: UIViewController?
        let marker: String
        var recheckQueued = false
        init(route: ExactRoute, marker: String) { self.route = route; self.marker = marker }
    }
    private var entries: [String: Entry] = [:]
    private var alive = true

    func configure(_ navigation: ExactNavigation) {
        // Respect another app coordinator's delegate if one is installed later.
        if navigation.delegate == nil { navigation.delegate = self }
    }

    func configure(_ route: ExactRoute) {
        let marker = route.data[.mobileFormSheet] ?? ""
        guard marker == "composer" || marker == "new-task" else {
            entries.removeValue(forKey: route.key)
            return
        }
        let entry: Entry
        if let current = entries[route.key], current.route === route, current.marker == marker {
            entry = current
        } else {
            entry = Entry(route: route, marker: marker)
            entries[route.key] = entry
        }
        schedule(entry, key: route.key)
    }

    private func schedule(_ entry: Entry, key: String) {
        guard !entry.recheckQueued else { return }
        entry.recheckQueued = true
        // The route callback may precede UIKit containment in the same render.
        // Recheck once on the next main turn, never poll for a presentation.
        DispatchQueue.main.async { [weak self, weak entry] in
            guard let self, let entry, self.alive,
                  self.entries[key] === entry else { return }
            entry.recheckQueued = false
            self.apply(entry)
        }
    }

    private func apply(_ entry: Entry) {
        guard alive, let route = entry.route, route.isLive else { return }
        // A newly prepared navigation can temporarily belong to the underlying
        // presenter. Resolve only its immediate owner after the render completes.
        guard let navigation = route.navigation?.controller,
              route.controller.navigationController === navigation,
              let controller = navigation.parent,
              controller.presentingViewController != nil,
              controller.modalPresentationStyle == .pageSheet || controller.modalPresentationStyle == .formSheet,
              entry.configuredController !== controller,
              let sheet = controller.sheetPresentationController else { return }
        if entry.marker == "new-task" {
            sheet.detents = [.custom(identifier: .init("t3-new-task")) { context in
                context.maximumDetentValue * 0.92
            }]
        } else {
            sheet.detents = [.large()]
        }
        sheet.selectedDetentIdentifier = sheet.detents.first?.identifier
        sheet.prefersGrabberVisible = true
        entry.configuredController = controller
    }

    func navigationController(_ navigationController: UINavigationController,
                              didShow viewController: UIViewController, animated: Bool) {
        for (key, entry) in entries where entry.route?.navigation?.controller === navigationController {
            schedule(entry, key: key)
        }
    }

    func end(_ route: ExactRoute) {
        if entries[route.key]?.route === route { entries.removeValue(forKey: route.key) }
    }

    func destroy() { alive = false; entries.removeAll() }
}

#endif
