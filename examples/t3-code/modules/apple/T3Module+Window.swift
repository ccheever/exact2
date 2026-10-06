// T3Module's window ops (T3Module.swift routes them): drawn frames for
// window-level popups, the appearance, send shortcut and quit mode, Open in
// Finder's folder picker, and copying text.
import Foundation
import AppKit

extension T3Module {
    /// Window-level popup frames (R8KeysMeasure.swift), device presentation, the folder picker and the clipboard.
    func windowOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "r8MeasureFrame" { DispatchQueue.main.async { [weak self] in reply.send(self?.measure.perform(request) ?? ["ok": false, "generation": 0]) }; return } // lane r8-keys
        if request["op"] as? String == "devicePresentation" {
            DispatchQueue.main.async { [weak self] in
                self?.chrome.setAppearance(request["appearanceMode"] as? String ?? "system")
                self?.composer.sendShortcut = request["sendShortcut"] as? String ?? "enter"
                self?.menus.quitMode = request["confirmQuit"] as? String ?? "hold"
                reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": [:]])
            }
            return
        }
        if request["op"] as? String == "pickFolder" { // The palette's Open in Finder (T3Menus.swift).
            DispatchQueue.main.async { [weak self] in
                self?.menus.pickFolder(startingAt: request["path"] as? String ?? "") { path in
                    reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["path": path ?? ""]])
                }
            }
            return
        }
        if request["op"] as? String == "copyText", let text = request["text"] as? String {
            DispatchQueue.main.async {
                NSPasteboard.general.clearContents()
                let copied = NSPasteboard.general.setString(text, forType: .string)
                reply.send(["ok": copied, "generation": request["generation"] as? Int ?? 0,
                            "value": ["copied": copied],
                            "error": ["kind": "Clipboard", "message": "Could not copy the message.", "uncertain": false]])
            }
            return
        }
        next()
    }
}
