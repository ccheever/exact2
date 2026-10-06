// T3Module's composer ops (T3Module.swift routes them): the native text view's
// edits and reads, picked attachments, and the send gesture.
import Foundation
import AppKit

extension T3Module {
    /// The composer's text view (T3Composer.swift), attached files (T3ComposerAttach.swift) and send intent (T3ComposerIntent.swift).
    func composerOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("editor") {
            DispatchQueue.main.async { [weak self] in reply.send(self?.composer.perform(request) ?? ["ok": false, "generation": 0]) }
            return
        }
        if let op = request["op"] as? String, op.hasPrefix("composerAttach") { return attach.perform(request) { reply.send($0) } } // pick, read, remove (T3ComposerAttach.swift)
        if request["op"] as? String == "composerSendIntent" {
            DispatchQueue.main.async { [weak self] in reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": self?.intent.take() ?? [:]]) }
            return
        }
        next()
    }
}
