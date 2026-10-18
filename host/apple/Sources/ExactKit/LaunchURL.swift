// A launch URL (`openURL` before boot) the app cannot hear is said once,
// as a URL arriving while it runs is (`navigate refused: …`): an app with
// no navigation root otherwise drops it without a word (#104).
//
// @ref LLP 1038 D8
import Foundation

extension ExactSession {
    func incomingLocation(_ url: URL) -> String {
        let href = url.absoluteString
        return protocolHandlers.lazy.compactMap { handler in
            guard let name = handler["protocol"], let template = handler["url"] else { return nil }
            return self.runtime.protocolLocation(of: href, protocol: name, template: template)
        }.first ?? runtime.location(of: href)
    }

    /// A notification's location and a host URL share the cold and warm
    /// incoming-route path, without requiring a URL scheme.
    @discardableResult func openLocation(_ incoming: String) -> Bool {
        guard state != .destroyed else { return false }
        return openIncoming(runtime.location(of: incoming))
    }

    /// Before boot the first incoming location is the launch location and
    /// later ones navigate in order after it; afterwards each is one event.
    func openIncoming(_ location: String) -> Bool {
        if !booted {
            if launchLocation == nil { runtime.launch(location); launchLocation = location }
            else { pendingLaunchLocations.append(location) }
            return true
        }
        return navigate(location)
    }

    /// The view that hears `navigate`: the navigation root (LLP 1038 D8).
    var navigationRoot: NodeView? {
        presenter.views.values.first { $0.props["navigationBack"] != nil && $0.handlers.contains("navigate") }
    }

    /// The platform's Back from a route with no authored Back control: the
    /// location beneath, to the root it came from (LLP 1115 D5), as a
    /// popstate is to the web's.
    func navigate(_ location: String, at root: NodeView) {
        guard state != .destroyed, booted, presenter.views[root.id] === root, root.handlers.contains("navigate") else { return }
        apply(runtime.navigate(root.id, location, now: now()))
    }

    /// The platform's Back from a route with no authored Back control under
    /// a root with no `navigate` handler: the runner's own `back` (LLP 1115 D5).
    func hostBack(_ key: String) {
        guard state != .destroyed, booted, let batch = runtime.hostBack(key, now: now()) else { return }
        apply(batch)
    }

    /// After the first frame: a launch location that matched no route and
    /// has no navigation root to hear it is journaled, then forgotten.
    func refuseUnheardLaunch() {
        defer { launchLocation = nil; deliverPendingLaunchLocations() }
        guard let location = launchLocation, navigationRoot == nil, !runtime.routeMatches(location) else { return }
        log("launch URL refused: no navigation root handler (\(location))")
    }

    func deliverPendingLaunchLocations() {
        let locations = pendingLaunchLocations
        pendingLaunchLocations = []
        for location in locations { _ = navigate(location) }
    }
}

/// The adapter's URL batch entry. Each callback runs before the next URL.
package enum IncomingURLBatch {
    package static func deliver(_ urls: [URL], development: (URL) -> Bool,
                                document: (URL) -> Void, location: (URL) -> Void) {
        for url in urls {
            if url.isFileURL { document(url) }
            else if !development(url) { location(url) }
        }
    }
}
