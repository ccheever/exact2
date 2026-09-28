// @ref LLP 1069.010 D1, D2 — documents in place. What the person chooses
// becomes a `doc:` handle the library mints for this session; the app reads
// and writes it under `fs.read doc:/` / `fs.write doc:/`. The File System
// Access API's three pickers are commands: the runner rules first (refused,
// or under the agent held as `open-file`, `open-directory`, `save-file` for
// `type @t <path>` / `tap @t cancel`), then macOS shows an NSOpenPanel or
// NSSavePanel sheet and iOS a document picker opening in place. The
// handle(s) arrive as `change` on the element the command names, one per
// line; a dismissed picker is HTML's `cancel` there. Each chosen URL stays
// under `startAccessingSecurityScopedResource` while the session lives: a
// no-op for today's unsandboxed builds, the call a sandboxed one needs, and
// what a security-scoped bookmark would be made from (ruled: the Mac App
// Store stays possible).
import Foundation
import UniformTypeIdentifiers
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension ExactSession {
    /// This session's number in the library's document table.
    var documentOwner: UInt64 { UInt64(UInt(bitPattern: ObjectIdentifier(self).hashValue)) }

    private func documentCall(_ request: [String: Any]) -> [String: Any] {
        var request = request
        request["owner"] = documentOwner
        let d = (try? JSONSerialization.data(withJSONObject: request)) ?? Data()
        return (try? JSONSerialization.jsonObject(with: Data(agent(String(decoding: d, as: UTF8.self)).utf8))) as? [String: Any] ?? [:]
    }

    /// A value arriving at the `open-file` field (LLP 1069.010 D1): a real
    /// path from any host route — Launch Services, the command line, ⌘O,
    /// Open Recent, a path typed there — becomes the document path, for an
    /// app granted `fs.read doc:/`.
    func documentValue(_ view: UInt32, _ value: String) -> String {
        guard value.hasPrefix("/"), presenter.views[view]?.props["testId"] == "open-file" else { return value }
        return documentCall(["op": "openDocument", "path": value])["path"] as? String ?? value
    }

    /// A URL the person chose, minted exactly: `doc:/<n>/<name>`.
    func mintDocument(_ url: URL) -> String? {
        if url.startAccessingSecurityScopedResource() { picker.scoped.append(url) }
        return documentCall(["op": "mintDocument", "path": url.standardizedFileURL.path])["doc"] as? String
    }

    /// The session ends: its handles go, and so does its access.
    func forgetDocuments() {
        _ = documentCall(["op": "forgetDocuments"])
        for url in picker.scoped { url.stopAccessingSecurityScopedResource() }
        picker.scoped = []
    }
}

#if canImport(UIKit)
extension ExactSession {
    /// A document from Files ("Open in", LLP 1069.010 slice 4): minted
    /// exactly, under its security scope, and delivered to the `open-file`
    /// node once the session has booted (a cold launch brings it first).
    public func openDocument(_ url: URL, tries: Int = 200) {
        guard state != .destroyed else { return }
        guard booted else {
            if tries > 0 { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.openDocument(url, tries: tries - 1) } }
            return
        }
        guard let doc = mintDocument(url) else { log("open-file: refused: \(url.lastPathComponent) is not a file"); return }
        if !change(testId: "open-file", value: doc) { log("open-file: refused: this app has no `open-file` field") }
    }
}
#endif

extension Picker {
    /// `showOpenFilePicker(id[, multiple])`, `showDirectoryPicker(id)`,
    /// `showSaveFilePicker(id, suggestedName)`, from any action.
    func document(_ name: String, _ args: [Any]) {
        var request: [String: Any] = ["command": name, "agent": ExactEnv.agentMode]
        if let id = args.first as? String { request["id"] = id }
        if args.count > 1 {
            if let multiple = args[1] as? Bool { request["multiple"] = multiple }
            if let suggested = args[1] as? String { request["suggestedName"] = suggested }
        }
        let ruling = session.runtime.command(request)
        let view = (ruling["view"] as? NSNumber)?.uint32Value
        if ruling["refused"] != nil { if let view { documentCancelled(view, name) }; return }
        guard ruling["present"] as? Bool == true, let view else { return }
        presentDocument(name, view: view, multiple: ruling["multiple"] as? Bool ?? false,
                        suggestedName: ruling["suggestedName"] as? String ?? "")
    }

    /// The agent's answer to a held picker: each path minted as a chosen
    /// file would be.
    func documentAnswered(_ reply: [String: Any], request: [String: Any]) {
        guard let capability = reply["capability"] as? String,
              let name = ["open-file": "showOpenFilePicker", "open-directory": "showDirectoryPicker",
                          "save-file": "showSaveFilePicker"][capability],
              let view = (reply["node"] as? NSNumber)?.uint32Value else { return }
        if reply["answered"] as? String == "cancel" { documentCancelled(view, name); return }
        let paths = (request["text"] as? String ?? "").split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        documentsChosen(view, paths.map { URL(fileURLWithPath: $0) }, name)
    }

    func documentsChosen(_ view: UInt32, _ urls: [URL], _ name: String) {
        let docs = urls.compactMap { session.mintDocument($0) }
        guard !docs.isEmpty else { session.log("\(name): refused: nothing to open"); documentCancelled(view, name); return }
        session.log("\(name): chosen")
        session.apply(session.runtime.change(view, docs.joined(separator: "\n"), now: session.now()))
    }

    func documentCancelled(_ view: UInt32, _ name: String) {
        session.log("\(name): cancelled")
        session.apply(session.runtime.pickerCancel(view, now: session.now()))
    }

    /// The manifest's `file_handlers`, as the bake wrote them into
    /// `CFBundleDocumentTypes`: the only types a picker offers (D2).
    static var declaredTypes: [UTType] {
        let declarations = ExactEnv.appMetadata["CFBundleDocumentTypes"] as? [[String: Any]] ?? []
        return declarations.flatMap { $0["LSItemContentTypes"] as? [String] ?? [] }
            .compactMap { UTType($0) }.filter { $0 != .folder }
    }
}

#if canImport(UIKit)
extension Picker {
    func presentDocument(_ name: String, view: UInt32, multiple: Bool, suggestedName: String) {
        guard var controller = session.presenter.root.window?.rootViewController else {
            session.log("\(name): refused: no window"); documentCancelled(view, name); return
        }
        while let presented = controller.presentedViewController { controller = presented }
        let picker: UIDocumentPickerViewController
        switch name {
        case "showDirectoryPicker":
            picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder], asCopy: false)
        case "showSaveFilePicker":
            // iOS saves by moving a file where the person says: an empty one
            // under the suggested name, whose new location is the handle.
            let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-save-\(UUID().uuidString)", isDirectory: true)
            let staged = dir.appendingPathComponent(suggestedName)
            guard (try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)) != nil,
                  FileManager.default.createFile(atPath: staged.path, contents: Data()) else {
                session.log("\(name): refused: no scratch file"); documentCancelled(view, name); return
            }
            picker = UIDocumentPickerViewController(forExporting: [staged], asCopy: false)
        default:
            let types = Picker.declaredTypes
            picker = UIDocumentPickerViewController(forOpeningContentTypes: types.isEmpty ? [.item] : types, asCopy: false)
            picker.allowsMultipleSelection = multiple
        }
        picker.delegate = self
        documentRequests[ObjectIdentifier(picker)] = (view, name)
        controller.present(picker, animated: true)
    }

    /// A document picker's outcome; `false` when `controller` is not one.
    func documentFinished(_ controller: UIDocumentPickerViewController, urls: [URL]?) -> Bool {
        guard let (view, name) = documentRequests.removeValue(forKey: ObjectIdentifier(controller)) else { return false }
        if let urls, !urls.isEmpty { documentsChosen(view, urls, name) } else { documentCancelled(view, name) }
        return true
    }
}
#else
extension Picker {
    func presentDocument(_ name: String, view: UInt32, multiple: Bool, suggestedName: String) {
        let panel: NSSavePanel
        if name == "showSaveFilePicker" {
            panel = NSSavePanel()
            panel.nameFieldStringValue = suggestedName
            panel.canCreateDirectories = true
        } else {
            let open = NSOpenPanel()
            open.canChooseFiles = name == "showOpenFilePicker"
            open.canChooseDirectories = name == "showDirectoryPicker"
            open.allowsMultipleSelection = multiple
            open.treatsFilePackagesAsDirectories = false
            let types = Picker.declaredTypes
            if open.canChooseFiles && !types.isEmpty { open.allowedContentTypes = types }
            panel = open
        }
        let finish: (NSApplication.ModalResponse) -> Void = { [weak self, weak panel] response in
            guard let self, let panel else { return }
            let urls = (panel as? NSOpenPanel)?.urls ?? panel.url.map { [$0] } ?? []
            if response == .OK, !urls.isEmpty { self.documentsChosen(view, urls, name) } else { self.documentCancelled(view, name) }
        }
        if let window = session.presenter.root.window { panel.beginSheetModal(for: window, completionHandler: finish) }
        else { panel.begin(completionHandler: finish) }
    }
}
#endif
