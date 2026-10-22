// T3Module's composer ops (T3Module.swift routes them): the native text view's
// large-paste facts and attached files' staged copies.
import Foundation
import AppKit

extension T3Module {
    /// The composer's text view (T3Composer.swift) and attached files (T3ComposerAttach.swift).
    func composerOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("editor") {
            DispatchQueue.main.async { [weak self] in reply.send(self?.composer.perform(request) ?? ["ok": false, "generation": 0]) }
            return
        }
        if let op = request["op"] as? String, op.hasPrefix("composerAttach") { return attach.perform(request) { reply.send($0) } } // a video's first frame and typed link (T3ComposerAttach.swift)
        next()
    }
}
