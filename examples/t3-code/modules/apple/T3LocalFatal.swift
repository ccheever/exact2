// `handleFatalStartupError` (T3 Code, MIT, see LICENSE-T3; reference 1e2ecbd975
// apps/desktop/src/app/DesktopApp.ts:109-141): a start the app cannot recover from — the embedded
// server finds no port (`DesktopBackendPortUnavailableError`, stage "bootstrap") — logs, shows
// "T3 Code failed to start" with "Stage: <stage>" and the message, then quits. A server that
// crashes is not fatal: it backs off and the primary reads "Reconnecting: <reason>"
// (20261005-local-primary-environment item 7). Return and Escape both dismiss the alert.
import AppKit
import Foundation

enum T3LocalFatal {
    static let title = "T3 Code failed to start"
    /// The alert's two lines (`showErrorBox(title, "Stage: <stage>\n<message>")`).
    static func detail(stage: String, message: String) -> String { "Stage: \(stage)\n\(message)" }

    // Seams: the alert and the quit (an AppKit test records them).
    static var alert: (String, String) -> Void = { title, detail in present(title: title, detail: detail) }
    static var quit: () -> Void = { NSApp.terminate(nil) }
    private static let lock = NSLock()
    private static var quitting = false

    /// Once per app run (the reference's `state.quitting` guard): log, alert on the main thread, quit.
    static func handle(stage: String, message: String) {
        lock.lock(); let first = !quitting; quitting = true; lock.unlock()
        FileHandle.standardError.write(Data("t3.local: fatal startup error stage=\(stage) message=\(message)\n".utf8))
        guard first else { return }
        let run = { alert(title, detail(stage: stage, message: message)); quit() }
        if Thread.isMainThread { run() } else { DispatchQueue.main.async(execute: run) }
    }

    /// An NSAlert with one OK button (Return), which Escape also dismisses (`dialog.showErrorBox`).
    static func present(title: String, detail: String) {
        let alert = NSAlert()
        alert.alertStyle = .critical
        alert.messageText = title
        alert.informativeText = detail
        alert.addButton(withTitle: "OK")
        let monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            // Escape (53), Return (36) and the keypad's Enter (76) press OK, even when the alert is not key.
            guard [53, 36, 76].contains(event.keyCode), NSApp.modalWindow === alert.window else { return event }
            NSApp.stopModal(withCode: .alertFirstButtonReturn)
            return nil
        }
        defer { if let monitor { NSEvent.removeMonitor(monitor) } }
        alert.runModal()
    }

    /// Tests: forget that a fatal error was handled.
    static func reset() { lock.lock(); quitting = false; lock.unlock() }
}
