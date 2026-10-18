// T3Module's window ops (T3Module.swift routes them): the installed fonts, the
// appearance, send shortcut and quit mode, Open in Finder's folder picker,
// copying text, and the shell's menu for a link's click.
import Foundation
import AppKit

extension T3Module {
    /// The installed fonts, device presentation, the folder picker and the clipboard.
    func windowOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "installedFonts" {
            DispatchQueue.main.async {
                reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0,
                            "value": ["fonts": T3InstalledFonts.catalog()]])
            }
            return
        }
        if request["op"] as? String == "devicePresentation" {
            DispatchQueue.main.async { [weak self] in
                self?.chrome.setAppearance(request["appearanceMode"] as? String ?? "system")
                self?.composer.sendShortcut = request["sendShortcut"] as? String ?? "enter"
                self?.menus.quitMode = request["confirmQuit"] as? String ?? "hold"
                T3RootFont.set(request["rootFontSize"]) // Interface font size (T3RootFont.swift)
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
        if request["op"] as? String == "shellMenu" { // A link's `contextmenu` left its click (or ContextMenu key) to the shell (T3TextContextMenu)
            DispatchQueue.main.async { [weak self] in
                let shown = self?.textMenu.shellMenuForLastClick(href: request["href"] as? String ?? "") ?? false
                reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["shown": shown]])
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
