// A launch URL (`openURL` before boot) the app cannot hear is said once,
// as a URL arriving while it runs is (`navigate refused: …`): an app with
// no navigation root otherwise drops it without a word (#104).
//
// @ref LLP 1038 D8
extension ExactSession {
    /// The view that hears `navigate`: the navigation root (LLP 1038 D8).
    var navigationRoot: NodeView? {
        presenter.views.values.first { $0.props["navigationBack"] != nil && $0.handlers.contains("navigate") }
    }

    /// After the first frame: a launch location that matched no route and
    /// has no navigation root to hear it is journaled, then forgotten.
    func refuseUnheardLaunch() {
        defer { launchLocation = nil }
        guard let location = launchLocation, navigationRoot == nil, !runtime.routeMatches(location) else { return }
        log("launch URL refused: no navigation root handler (\(location))")
    }
}
