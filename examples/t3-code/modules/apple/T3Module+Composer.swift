// T3Module's composer ops (T3Module.swift routes them): the native text view's
// edits and reads, picked attachments, and the send gesture.
import Foundation
import AppKit

extension T3Module {
    /// The composer's text view (T3Composer.swift), attached files (T3ComposerAttach.swift) and send intent (T3ComposerIntent.swift).
    func composerOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        // A skill chip's press, the newest of the composer's and the Settings prompt sample's, and the app's close (T3ComposerChipPress.swift).
        if let op = request["op"] as? String, op == "editorChip" || op == "editorChipClose" {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return reply.send(["ok": false, "generation": 0]) }
                let presses = [self.composer.editor.styler.press, self.promptPreview.styler.press]
                if op == "editorChipClose" { presses.forEach { $0.close(seq: request["seq"] as? Int ?? -1, escape: request["escape"] as? Bool == true) } }
                reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": T3ComposerChipPress.latest(presses)])
            }
            return
        }
        // A citation's comment, opened under the cited text after Cite, and the app's Save (⌘↵: then
        // Send, as the composer's Return presses it), Cancel, Escape and an outside press's commit (T3ComposerChipPress.swift).
        if let op = request["op"] as? String, op == "editorCiteOpen" || op == "editorChipFinish" {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return reply.send(["ok": false, "generation": 0]) }
                let press = self.composer.editor.styler.press, generation = request["generation"] as? Int ?? 0
                if op == "editorCiteOpen" {
                    let frame = (request["source"] as? [Any] ?? []).compactMap { ($0 as? NSNumber)?.doubleValue }
                    let opened = press.openCitation(href: request["href"] as? String ?? "", source: frame)
                    return reply.send(["ok": true, "generation": generation, "value": ["opened": opened]])
                }
                let applied = press.finishCitation(seq: request["seq"] as? Int ?? -1, text: request["text"] as? String, focus: request["focus"] as? Bool ?? true,
                                                   escape: request["escape"] as? Bool == true)
                let sent = applied && request["send"] as? Bool == true && self.composer.pressSend()
                reply.send(["ok": true, "generation": generation, "value": ["applied": applied, "sent": sent]])
            }
            return
        }
        if let op = request["op"] as? String, op.hasPrefix("editor") {
            DispatchQueue.main.async { [weak self] in reply.send(self?.composer.perform(request) ?? ["ok": false, "generation": 0]) }
            return
        }
        // The files a ⌘V took, staged as a picked file is (T3ComposerEditor.swift's pastedFiles).
        if request["op"] as? String == "composerPasteTake" {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return reply.send(["ok": false, "generation": 0]) }
                let fileLimit = request["fileLimit"] as? Int ?? 0
                let files = self.composer.editor.takePastedFiles { self.attach.stage($0, fileLimit: fileLimit) }
                reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["files": files]])
            }
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
