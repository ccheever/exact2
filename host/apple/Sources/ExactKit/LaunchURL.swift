// A launch URL (`openURL` before boot) the app cannot hear is said once,
// as a URL arriving while it runs is (`navigate refused: …`): an app with
// no navigation root otherwise drops it without a word (#104).
//
// @ref LLP 1038 D8
import Foundation
import UniformTypeIdentifiers

extension ExactSession {
    /// The location the agent's `type <root> <text>` delivers: a path as
    /// written, and a URL with a scheme as the system's would arrive
    /// (`openURL`), so a drive opens a deep link without iOS's "Open in …?"
    /// confirmation that `simctl openurl` raises.
    func typedLocation(_ text: String) -> String {
        guard !text.hasPrefix("/"), let url = URL(string: text), url.scheme != nil else { return text }
        return incomingLocation(url)
    }

    func incomingLocation(_ url: URL) -> String {
        let href = url.absoluteString
        if isWebPageFile(url), let template = webTemplate, let location = runtime.protocolLocation(of: href, protocol: "file", template: template) {
            return location
        }
        return protocolHandlers.lazy.compactMap { handler in
            guard let name = handler["protocol"], let template = handler["url"] else { return nil }
            return self.runtime.protocolLocation(of: href, protocol: name, template: template)
        }.first ?? runtime.location(of: href)
    }

    /// The `https` handler's template, when the app claims the web's links
    /// (a Mac browser, LLP 1030 D2).
    var webTemplate: String? {
        protocolHandlers.first { $0["protocol"]?.lowercased() == "https" }?["url"]
    }

    /// Whether `url` is an HTML file Launch Services brought to a browser
    /// (Finder's Open With, a double-click once the app owns HTML): it
    /// arrives at the `https` template as its `file:` URL, as a page does in
    /// Safari, unless the app's own `file_handlers` take it.
    package func isWebPageFile(_ url: URL) -> Bool {
        guard url.isFileURL, webTemplate != nil else { return false }
        #if os(macOS)
        if ExactDocuments.accepts(url) { return false }
        #endif
        let type = (try? url.resourceValues(forKeys: [.contentTypeKey]).contentType) ?? UTType(filenameExtension: url.pathExtension)
        return type.map { t in [UTType.html, UTType("public.xhtml")].contains { $0.map(t.conforms(to:)) ?? false } } ?? false
    }

    /// A notification's location and a host URL share the cold and warm
    /// incoming-route path, without requiring a URL scheme.
    @discardableResult func openLocation(_ incoming: String) -> Bool {
        guard state != .destroyed else { return false }
        return openIncoming(runtime.location(of: incoming))
    }

    /// The manifest's `start_url` when it is not the root, which the Apple
    /// build bakes as `ExactStartURL` (W3C App Manifest).
    nonisolated(unsafe) static var startLocation = ExactEnv.appMetadata["ExactStartURL"] as? String

    /// A launch no link started opens at the start location, on either
    /// boot. A session already booted is not navigated: a restart (the dev
    /// menu's) boots again at the launch location the bridge keeps.
    func openStartLocation() {
        if !booted, launchLocation == nil, let start = Self.startLocation { _ = openIncoming(start) }
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

    /// Complete a native history jump without intermediate route commits.
    func hostBack(from source: String, to destination: String) {
        guard state != .destroyed, booted, let batch = runtime.hostBack(from: source, to: destination, now: now()) else { return }
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
/// A file is a document unless `webPage` says it is a page for a browser's
/// links (`isWebPageFile`).
package enum IncomingURLBatch {
    package static func deliver(_ urls: [URL], development: (URL) -> Bool, webPage: (URL) -> Bool = { _ in false },
                                document: (URL) -> Void, location: (URL) -> Void) {
        for url in urls {
            if url.isFileURL, !webPage(url) { document(url) }
            else if url.isFileURL || !development(url) { location(url) }
        }
    }
}
