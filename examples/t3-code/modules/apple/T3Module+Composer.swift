// T3Module's composer ops (T3Module.swift routes them).
import Foundation
import AppKit

extension T3Module {
    /// The composer's requests (T3Composer.swift) and Paste as Text armed from the menu (T3Menus.swift).
    func composerOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "composerPasteAsText" {
            let generation = request["generation"] as? Int ?? 0
            DispatchQueue.main.async { [weak self] in reply.send(["ok": true, "generation": generation, "value": ["armed": self?.menus.takePasteAsText() ?? false]]) }
            return
        }
        if let op = request["op"] as? String, op.hasPrefix("editor") {
            DispatchQueue.main.async { [weak self] in reply.send(self?.composer.perform(request) ?? ["ok": false, "generation": 0]) }
            return
        }
        next()
    }
}
