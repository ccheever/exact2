// `openURL` beyond the web's, mail's and the phone's schemes: a scheme the
// app declares in app.json's `open_url.schemes` (an editor's `vscode:` or
// `zed:` link) goes to whatever Launch Services or UIKit has registered for
// it. An undeclared scheme stays refused, so a link out of content the app
// did not write still fails closed (LLP 0382). Under the agent the open is
// journaled and not performed, as `app-settings:` is.
import Foundation

extension ExactEnv {
    /// app.json `open_url.schemes`, lowercased (the bundle's `ExactOpenURLSchemes`).
    /// The manifest refuses the schemes that run or read content; this host
    /// refuses them again, whatever a bundle carries.
    public static let openURLSchemes = Set((appMetadata["ExactOpenURLSchemes"] as? [String] ?? []).map { $0.lowercased() })
        .subtracting(["javascript", "vbscript", "data", "file", "filesystem", "blob", "about", "view-source", "jar", "intent", "content"])
}

extension ExactSession {
    /// Whether `url`'s scheme is one the app declares. A declared one is
    /// journaled and handed to `open`, unless the agent drives the app; an
    /// open the system could not perform (no app registered) is journaled too.
    public func openDeclaredScheme(_ url: URL, open: (URL, @escaping (Bool) -> Void) -> Void) -> Bool {
        guard let scheme = url.scheme?.lowercased(), ExactEnv.openURLSchemes.contains(scheme) else { return false }
        let shown = url.absoluteString.prefix(200)
        log("openURL \(scheme): \(shown)\(ExactEnv.agentMode ? " (journaled, not opened under the agent)" : "")")
        if !ExactEnv.agentMode {
            open(url) { [weak self] opened in
                if !opened { self?.log("openURL \(scheme): not opened, no app registered for \(scheme): (\(shown))") }
            }
        }
        return true
    }
}
