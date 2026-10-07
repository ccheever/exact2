// @ref LLP 1069.010 D3 — export is a copy the host makes. `saveFile(id,
// from, suggestedName)`: the runner rules first (bad data or an ungranted
// `from` refused into the journal; under the agent the request held as
// `export`, answered by `type @t <path>` or `tap @t cancel`), then macOS
// shows an NSSavePanel sheet and copies the `app:/` file to the chosen URL,
// and iOS copies it into a scratch file and hands that to the exporting
// document picker. The chosen name arrives as `change` on the element `id`
// names; a dismissed panel is HTML's `cancel` there. `saveFile(id, text=…,
// suggestedName=…)` saves the text itself: the host writes it to a scratch
// file and goes on as for a copy (x2apps notes #4).
import Foundation
import UniformTypeIdentifiers
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension Picker {
    /// `saveFile(id, from, suggestedName)`, or `(id, none, suggestedName,
    /// text)` for `text=`, from any action.
    func save(_ args: [Any]) {
        var request: [String: Any] = ["command": "saveFile", "agent": ExactEnv.agentMode]
        let strings = args.map { $0 as? String }
        if args.count == 3, let id = strings[0], let from = strings[1], let name = strings[2] {
            request["id"] = id; request["from"] = from; request["suggestedName"] = name
        } else if args.count == 4, strings[1] == nil, let id = strings[0], let name = strings[2], let text = strings[3] {
            request["id"] = id; request["text"] = text; request["suggestedName"] = name
        }
        let ruling = session.runtime.command(request)
        let view = (ruling["view"] as? NSNumber)?.uint32Value
        if ruling["refused"] != nil { if let view { saveCancelled(view) }; return }
        guard ruling["present"] as? Bool == true, let view, let name = ruling["suggestedName"] as? String else { return }
        if let text = ruling["text"] as? String {
            let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-text-\(UUID().uuidString)", isDirectory: true)
            let file = dir.appendingPathComponent(name)
            do {
                try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                try Data(text.utf8).write(to: file)
            } catch {
                session.log("saveFile: refused: \(error.localizedDescription)"); saveCancelled(view); return
            }
            presentSave(view: view, file: file, name: name)
            return
        }
        guard let from = ruling["from"] as? String else { return }
        guard let file = appFile(from), FileManager.default.fileExists(atPath: file.path) else {
            session.log("saveFile: refused: \(from) does not exist"); saveCancelled(view); return
        }
        presentSave(view: view, file: file, name: name)
    }

    /// The agent's answer to a held export (LLP 1069.007 D4): the copy goes
    /// to the driver's path, as the panel's would go to the chosen URL.
    func saveAnswered(_ reply: [String: Any], request: [String: Any]) {
        guard reply["capability"] as? String == "export", let view = (reply["node"] as? NSNumber)?.uint32Value else { return }
        if reply["answered"] as? String == "cancel" { saveCancelled(view); return }
        let destination = URL(fileURLWithPath: (request["text"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines))
        if let text = (reply["request"] as? [String: Any])?["text"] as? String {
            do { try Data(text.utf8).write(to: destination) } catch {
                session.log("saveFile: refused: \(error.localizedDescription)"); saveCancelled(view); return
            }
            saved(view, name: destination.lastPathComponent)
            return
        }
        guard let from = (reply["request"] as? [String: Any])?["from"] as? String, let file = appFile(from) else {
            session.log("saveFile: refused: no app file to copy"); saveCancelled(view); return
        }
        copyOut(view: view, file: file, to: destination)
    }

    /// The file behind an `app:/` path, as the library's roots name it.
    func appFile(_ path: String) -> URL? {
        let d = (try? JSONSerialization.data(withJSONObject: ["op": "appFile", "path": path])) ?? Data()
        let r = (try? JSONSerialization.jsonObject(with: Data(session.agent(String(decoding: d, as: UTF8.self)).utf8))) as? [String: Any]
        return (r?["file"] as? String).map { URL(fileURLWithPath: $0) }
    }

    /// Replace whatever is at `destination` with a copy of `file`, then fire
    /// `change` with the chosen name.
    func copyOut(view: UInt32, file: URL, to destination: URL) {
        do {
            if FileManager.default.fileExists(atPath: destination.path) { try FileManager.default.removeItem(at: destination) }
            try FileManager.default.copyItem(at: file, to: destination)
        } catch {
            session.log("saveFile: refused: \(error.localizedDescription)"); saveCancelled(view); return
        }
        saved(view, name: destination.lastPathComponent)
    }

    func saved(_ view: UInt32, name: String) {
        session.log("saveFile: saved")
        session.apply(session.runtime.change(view, name, now: session.now()))
    }

    func saveCancelled(_ view: UInt32) {
        session.log("saveFile: cancelled")
        session.apply(session.runtime.pickerCancel(view, now: session.now()))
    }
}

#if os(tvOS)
extension Picker {
    func presentSave(view: UInt32, file _: URL, name _: String) {
        // tvOS has no document picker to export through.
        session.log("saveFile: refused: no document picker"); saveCancelled(view)
    }
}
#elseif canImport(UIKit)
extension Picker {
    /// iOS: a scratch copy under the suggested name, handed to the
    /// exporting document picker, which moves a copy where the person says.
    func presentSave(view: UInt32, file: URL, name: String) {
        guard var controller = session.presenter.root.window?.rootViewController else {
            session.log("saveFile: refused: no window"); saveCancelled(view); return
        }
        while let presented = controller.presentedViewController { controller = presented }
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-export-\(UUID().uuidString)", isDirectory: true)
        let staged = dir.appendingPathComponent(name)
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: file, to: staged)
        } catch {
            session.log("saveFile: refused: \(error.localizedDescription)"); saveCancelled(view); return
        }
        let picker = UIDocumentPickerViewController(forExporting: [staged], asCopy: true)
        picker.delegate = self
        exports[ObjectIdentifier(picker)] = (view, dir)
        controller.present(picker, animated: true)
    }

    /// The exporting picker's outcome; `false` when `controller` is not one.
    func exportFinished(_ controller: UIDocumentPickerViewController, urls: [URL]?) -> Bool {
        guard let (view, dir) = exports.removeValue(forKey: ObjectIdentifier(controller)) else { return false }
        try? FileManager.default.removeItem(at: dir)
        if let url = urls?.first { saved(view, name: url.lastPathComponent) } else { saveCancelled(view) }
        return true
    }
}
#else
extension Picker {
    /// macOS: an NSSavePanel sheet on the window, typed by the suggested
    /// name's extension; the copy replaces what the person agreed to replace.
    func presentSave(view: UInt32, file: URL, name: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        let ext = (name as NSString).pathExtension
        if !ext.isEmpty, let type = UTType(filenameExtension: ext) { panel.allowedContentTypes = [type] }
        let finish: (NSApplication.ModalResponse) -> Void = { [weak self, weak panel] response in
            guard let self else { return }
            guard response == .OK, let url = panel?.url else { self.saveCancelled(view); return }
            let scoped = url.startAccessingSecurityScopedResource()
            self.copyOut(view: view, file: file, to: url)
            if scoped { url.stopAccessingSecurityScopedResource() }
        }
        if let window = session.presenter.root.window { panel.beginSheetModal(for: window, completionHandler: finish) }
        else { panel.begin(completionHandler: finish) }
    }
}
#endif
