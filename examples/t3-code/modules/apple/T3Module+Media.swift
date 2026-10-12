// T3Module's media ops (T3Module.swift routes them): the media menu, copying a path, URL or
// image, and saving media (T3MediaActions.swift, task media-actions).
import Foundation
import AppKit

extension T3Module {
    /// MediaActions' native half; the agent's exports root keeps saves and the pasteboard off the person's.
    func mediaOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        guard let op = request["op"] as? String, T3MediaActions.ops.contains(op) else { return next() }
        T3MediaActions.perform(request, exportsRoot: exportsRoot) { reply.send($0) }
    }
}
