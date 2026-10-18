// T3Module's desktop-shell ops (T3Module.swift routes them): the
// project icon picker's "Open in Finder" (T3Menus.pickProjectFavicon), and when the reader last
// pointed, typed or scrolled (`activityLastInteraction`, T3ActivityInput.swift).
import Foundation
import AppKit

extension T3Module {
    func shellOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        // useLiveRefresh's last interaction (pages-pr-refresh.ts) and the activity reporter's: the pointer, key and wheel time, in Unix ms.
        if request["op"] as? String == "activityLastInteraction" {
            return reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["at": activityInput.lastInteractionAt()] as [String: Any]])
        }
        if request["op"] as? String == "pickProjectFavicon" {
            let imports = exportsRoot.map { $0.deletingLastPathComponent().appendingPathComponent("imports", isDirectory: true) }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return reply.send(["ok": false, "generation": request["generation"] as? Int ?? 0,
                                                         "error": ["kind": "Picker", "message": "The window is gone.", "uncertain": false]]) }
                self.menus.pickProjectFavicon(startingAt: request["path"] as? String ?? "", importsRoot: imports) { path in
                    reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["path": path ?? ""]])
                }
            }
            return
        }
        next()
    }
}
