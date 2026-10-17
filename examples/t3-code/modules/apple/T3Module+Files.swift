// T3Module's file ops (T3Module.swift routes them): context menus, saving,
// opening and fetching text, and attachment text and saves.
import Foundation
import AppKit

extension T3Module {
    /// The branch and chat link menus, text export/import/fetch (T3ContextMenu.swift) and attachment bodies (T3PanelsNative.swift).
    func fileOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "contextMenu" { // Context menus (T3ContextMenu.swift); exportsRoot is set exactly under the agent.
            DispatchQueue.main.async { [textMenu] in textMenu.pageClickAnswered() } // the app's own menu took the click
            return T3ContextMenu.perform(request, agent: exportsRoot != nil) { reply.send($0) }
        }
        if request["op"] as? String == "saveText" { return T3ContextMenu.saveText(request, exportsRoot: exportsRoot) { reply.send($0) } }
        if request["op"] as? String == "openText" { return T3ContextMenu.openText(request, importsRoot: exportsRoot.map { $0.deletingLastPathComponent().appendingPathComponent("imports", isDirectory: true) }) { reply.send($0) } }
        if request["op"] as? String == "fetchText" { return T3ContextMenu.fetchText(request) { reply.send($0) } }
        if request["op"] as? String == "attachmentText" || request["op"] as? String == "attachmentSave" { return T3AttachmentFiles.perform(request, exportsRoot: exportsRoot, stagedFile: attach.stagedFile) { reply.send($0) } } // T3PanelsNative.swift
        next()
    }
}
