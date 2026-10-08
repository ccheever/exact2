// T3Module's ChatGPT sign-in ops (T3Module.swift routes them; 20261005-managed-codex-chatgpt): the
// loopback receiver is T3CodexAuth.swift. An op file of its own, as every T3Module+<area>.swift, so the
// module tests that define their own `exactModule` (composer, menus, r5-panels) leave it out.
import AppKit
import Foundation

extension T3Module {
    /// codexAuthStart / codexAuthCancel / codexAuthTake (codex-setup-ops.ts).
    func codexAuthOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        let op = request["op"] as? String ?? ""
        guard op.hasPrefix("codexAuth") else { return next() }
        let generation = request["generation"] as? Int ?? 0, url = request["authorizationUrl"] as? String ?? ""
        let auth = T3CodexAuth.shared, agent = context.agent, changed = context.changed, diagnostics = context.diagnostics
        auth.changed = { changed("t3.codexAuth") }
        auth.log = { line in diagnostics.log(line) }
        auth.reveal = {
            DispatchQueue.main.async {
                // DesktopWindow.reveal: show, restore and focus the main window. An agent run leaves the user's focus alone.
                if agent { diagnostics.log("codex-auth: reveal the window (agent run: recorded)"); return }
                NSApp.activate(ignoringOtherApps: true)
                let window = NSApp.mainWindow ?? NSApp.windows.first { $0.isVisible || $0.isMiniaturized }
                if window?.isMiniaturized == true { window?.deminiaturize(nil) }
                window?.makeKeyAndOrderFront(nil)
            }
        }
        let success = { (value: [String: Any]) in reply.send(["ok": true, "generation": generation, "value": value]) }
        DispatchQueue.global(qos: .userInitiated).async {
            switch op {
            case "codexAuthStart":
                let timeout = agent ? (Double(ProcessInfo.processInfo.environment["T3_CODEX_AUTH_TIMEOUT_MS"] ?? "") ?? 300_000) / 1000 : 300
                if let failure = auth.start(url, timeout: timeout, open: { T3RemoteEditors.openOrRecord($0, agent: agent) }) {
                    reply.send(["ok": false, "generation": generation, "error": ["kind": "CodexAuth", "message": failure, "uncertain": false]])
                } else { success(["listening": true]) }
            case "codexAuthCancel": auth.cancel(url); success([:])
            case "codexAuthTake": success(auth.take(url))
            default: reply.send(["ok": false, "generation": generation, "error": ["kind": "Arguments", "message": "Unknown op \(op).", "uncertain": false]])
            }
        }
    }
}
