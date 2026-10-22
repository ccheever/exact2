// T3Module's file ops (T3Module.swift routes them): the branch picker's menu,
// saving, opening and fetching text.
import Foundation
import AppKit

extension T3Module {
    /// The branch picker's menu and text export/import/fetch (T3ContextMenu.swift).
    func fileOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "contextMenu" { // exportsRoot is set exactly under the agent.
            return T3ContextMenu.perform(request, agent: exportsRoot != nil) { reply.send($0) }
        }
        if request["op"] as? String == "saveText" { return T3ContextMenu.saveText(request, exportsRoot: exportsRoot) { reply.send($0) } }
        if request["op"] as? String == "openText" { return T3ContextMenu.openText(request, importsRoot: exportsRoot.map { $0.deletingLastPathComponent().appendingPathComponent("imports", isDirectory: true) }) { reply.send($0) } }
        if request["op"] as? String == "fetchText" { return T3ContextMenu.fetchText(request) { reply.send($0) } }
        next()
    }
}
