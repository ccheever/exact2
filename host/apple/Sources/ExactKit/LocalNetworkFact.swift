import CExact
import Foundation

/// The host's half of the local-network fact (LLP 1133.036 D8):
/// `@exact/local-network` posts `exact.localNetwork` `{state}` with its
/// evidence, and the runner's rule for each fetch hop and WebSocket open (D6)
/// reads the fact this hands to Rust.
enum LocalNetworkFact {
    private static var installed = false

    static func install() {
        guard !installed else { return }
        installed = true
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.localNetwork"), object: nil, queue: .main) { note in
            switch (note.object as? [String: Any])?["state"] as? String {
            case "granted": _ = exact_local_network_report(0)
            case "denied": _ = exact_local_network_report(2)
            default: break
            }
        }
    }
}
