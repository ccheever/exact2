// `openURL` beyond the web's, mail's and the phone's schemes: a scheme the
// app declares in app.json's `open_url.schemes` (an editor's `vscode:` or
// `zed:` link) goes to whatever Launch Services or UIKit has registered for
// it. An undeclared scheme stays refused, so a link out of content the app
// did not write still fails closed (LLP 0382). Under the agent the open is
// journaled and not performed, as `app-settings:` is.
import Foundation

extension ExactEnv {
    /// app.json `open_url.schemes`, lowercased (the bundle's `ExactOpenURLSchemes`).
    public static let openURLSchemes = Set((appMetadata["ExactOpenURLSchemes"] as? [String] ?? []).map { $0.lowercased() })
}

extension ExactSession {
    /// Whether `url`'s scheme is one the app declares. A declared one is
    /// journaled and handed to `open`, unless the agent drives the app.
    public func openDeclaredScheme(_ url: URL, open: (URL) -> Void) -> Bool {
        guard let scheme = url.scheme?.lowercased(), ExactEnv.openURLSchemes.contains(scheme) else { return false }
        log("openURL \(scheme): \(url.absoluteString.prefix(200))\(ExactEnv.agentMode ? " (journaled, not opened under the agent)" : "")")
        if !ExactEnv.agentMode { open(url) }
        return true
    }
}
