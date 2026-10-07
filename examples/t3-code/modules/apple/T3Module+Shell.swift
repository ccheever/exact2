// T3Module's desktop-shell ops (task desktop-shell-details; T3Module.swift routes them): the
// project icon picker's "Open in Finder" (T3Menus.pickProjectFavicon), and the page's window
// facts for the activity reporter (`activityFacts`: exactPage() visibilityState and hasFocus,
// exact2 #219; shell-notify.ts reportWindowFacts).
import Foundation
import AppKit

extension T3Module {
    func shellOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "activityFacts" {
            activity.facts(visible: request["visible"] as? Bool ?? false, focused: request["focused"] as? Bool ?? false)
            return reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": [:] as [String: Any]])
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
