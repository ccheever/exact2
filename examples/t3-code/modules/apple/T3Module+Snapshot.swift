// T3Module's SnapShot ops (T3Module.swift routes them): window capture, the
// draft captures it keeps, and its shortcut and feedback settings.
import Foundation
import AppKit

extension T3Module {
    /// SnapShot capture, drafts and settings (T3SnapShot.swift).
    func snapshotOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("snapshot") {
            DispatchQueue.main.async { [weak self] in self?.snapShot.perform(request) { reply.send($0) } }
            return
        }
        next()
    }
}
