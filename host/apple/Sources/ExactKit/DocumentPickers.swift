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
// Store stays possible). A drive that shows UIKit's own presentations
// (`--touch platform`) keeps the hold and presents the real picker too: what
// the person chooses there answers the hold, and the agent's answer, or
// `back`, closes it.
import Foundation
import UniformTypeIdentifiers
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension ExactSession {
    /// This session's number in the library's document table.
    var documentOwner: UInt64 { UInt64(runtime.rt) }

    func documentCall(_ request: [String: Any]) -> [String: Any] {
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
        PastedImages.forget(self)
        for url in picker.scoped { url.stopAccessingSecurityScopedResource() }
        picker.scoped = []
    }
}

#if !os(macOS)
extension ExactSession {
    /// Nothing is kept across launches here (LLP 1069.010.000: iOS keeps
    /// nothing until an iOS document app asks).
    func restoreKeptDocuments() {}
}
#endif

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
        if !change(testId: "open-file", value: doc) { log("open-file: refused: \(changeRefusal ?? "nothing took it")") }
    }
}
#endif

extension Picker {
    /// `showOpenFilePicker(id[, multiple])`, `showDirectoryPicker(id)`,
    /// `showSaveFilePicker(id, suggestedName)`, from any action.
    func document(_ name: String, _ args: [Any]) {
        var request: [String: Any] = ["command": name, "agent": ExactEnv.agentMode]
        if ExactEnv.agentMode && !ExactEnv.agentPaints { request["present"] = true }
        if let id = args.first as? String { request["id"] = id }
        if args.count > 1 {
            if let multiple = args[1] as? Bool { request["multiple"] = multiple }
            if let suggested = args[1] as? String { request["suggestedName"] = suggested }
        }
        let ruling = session.runtime.command(request)
        let view = (ruling["view"] as? NSNumber)?.uint32Value
        if ruling["refused"] != nil { if let view { documentCancelled(view, name) }; return }
        guard ruling["present"] as? Bool == true, let view else { return }
        if let ticket = (ruling["ticket"] as? NSNumber)?.uint64Value { documentTickets[view] = ticket }
        presentDocument(name, view: view, multiple: ruling["multiple"] as? Bool ?? false,
                        suggestedName: ruling["suggestedName"] as? String ?? "")
    }

    /// `requestPermission(id, path, mode)` (LLP 1069.010.000 D7): the
    /// runner's rule, then `granted` at `id` for a document that resolves
    /// (a native host owns no permission a person could withhold), else `denied`.
    func permission(_ args: [Any]) {
        var request: [String: Any] = ["command": "requestPermission"]
        for (key, value) in zip(["id", "path", "mode"], args) { request[key] = value }
        let ruling = session.runtime.command(request)
        guard let view = (ruling["view"] as? NSNumber)?.uint32Value, let path = ruling["path"] as? String else { return }
        let granted = session.documentCall(["op": "documentPermission", "path": path])["granted"] as? Bool == true
        session.apply(session.runtime.change(view, granted ? "granted" : "denied", now: session.now()))
    }

    /// The agent's answer to a held picker: each path minted as a chosen
    /// file would be.
    func documentAnswered(_ reply: [String: Any], request: [String: Any]) {
        guard let capability = reply["capability"] as? String,
              let name = ["open-file": "showOpenFilePicker", "open-directory": "showDirectoryPicker",
                          "save-file": "showSaveFilePicker"][capability],
              let view = (reply["node"] as? NSNumber)?.uint32Value else { return }
        // The real picker shown beside the hold closes, its outcome the agent's.
        if documentTickets.removeValue(forKey: view) != nil { closeDocument(view) }
        if reply["answered"] as? String == "cancel" { documentCancelled(view, name); return }
        let paths = (request["text"] as? String ?? "").split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        documentsChosen(view, paths.map { URL(fileURLWithPath: $0) }, name)
    }

    /// A shown picker's outcome answers its hold, as `type @t <paths>` or
    /// `tap @t cancel` would (journalled, never echoed); nil is a cancel.
    private func answerHold(_ view: UInt32, _ urls: [URL]?) {
        guard let ticket = documentTickets.removeValue(forKey: view) else { return }
        var ask: [String: Any] = ["op": urls == nil ? "tap" : "type", "ticket": ticket]
        if let urls { ask["text"] = urls.map(\.path).joined(separator: "\n") } else { ask["choice"] = "cancel" }
        if let data = try? JSONSerialization.data(withJSONObject: ask) { _ = session.agent(String(decoding: data, as: UTF8.self)) }
    }

    func documentsChosen(_ view: UInt32, _ urls: [URL], _ name: String) {
        answerHold(view, urls.isEmpty ? nil : urls)
        #if os(macOS)
        let docs = urls.compactMap { session.keptChoice($0) ?? session.mintDocument($0) }
        #else
        let docs = urls.compactMap { session.mintDocument($0) }
        #endif
        guard !docs.isEmpty else { session.log("\(name): refused: nothing to open"); documentCancelled(view, name); return }
        session.log("\(name): chosen")
        #if os(macOS)
        // A file chosen in the app is a document opened, as one Finder or
        // ⌘O brings is (studio diary R18): it joins Open Recent, and the
        // window that shows it is no longer an empty one.
        ExactDocuments.chosen(urls, in: session)
        #endif
        session.apply(session.runtime.change(view, docs.joined(separator: "\n"), now: session.now()))
    }

    func documentCancelled(_ view: UInt32, _ name: String) {
        answerHold(view, nil)
        session.log("\(name): cancelled")
        session.apply(session.runtime.pickerCancel(view, now: session.now()))
    }

    /// The manifest's `file_handlers`, as the bake wrote them into
    /// `CFBundleDocumentTypes`. The web-page type a Mac browser declares
    /// (`ExactWebPage`, LLP 1030 D2) is Launch Services' alone: an HTML file
    /// it brings is a link (`isWebPageFile`), never a document.
    static func declarations(in metadata: [String: Any] = ExactEnv.appMetadata) -> [[String: Any]] {
        (metadata["CFBundleDocumentTypes"] as? [[String: Any]] ?? []).filter { $0["ExactWebPage"] as? Bool != true }
    }

    /// The declared types: the only ones a picker offers (D2).
    static var declaredTypes: [UTType] { documentTypes.filter { $0 != .folder } }

    /// Every declared type, and the type this device gives each declared
    /// extension: an app's own format whose extension another app's type
    /// already has (studio diary R13: Freeform has `.board`) is still offered,
    /// whichever type a file of it resolves to here.
    static var documentTypes: [UTType] { types(in: ExactEnv.appMetadata) }

    static func types(in metadata: [String: Any]) -> [UTType] {
        let declarations = declarations(in: metadata)
        let declared = declarations.flatMap { $0["LSItemContentTypes"] as? [String] ?? [] }.compactMap { UTType($0) }
        let byExtension = declarations.flatMap { $0["CFBundleTypeExtensions"] as? [String] ?? [] }.compactMap { UTType(filenameExtension: $0) }
        return (declared + byExtension).reduce(into: []) { types, t in if !types.contains(t) { types.append(t) } }
    }
}

#if os(tvOS)
extension Picker {
    func presentDocument(_ name: String, view: UInt32, multiple _: Bool, suggestedName _: String) {
        // tvOS has no document picker.
        session.log("\(name): refused: no document picker"); documentCancelled(view, name)
    }
    func closeDocument(_: UInt32) {}
}
#elseif canImport(UIKit)
extension Picker {
    func presentDocument(_ name: String, view: UInt32, multiple: Bool, suggestedName: String) {
        guard let controller = ViewPresentation.presenter(for: session.presenter.views[view] ?? session.presenter.root) else {
            session.log("\(name): refused: no window"); documentCancelled(view, name); return
        }
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
        if documentTickets[view] != nil { documentShown[view] = picker }
        controller.present(picker, animated: true)
    }

    /// Close the picker shown for real at `view`, its outcome said elsewhere.
    func closeDocument(_ view: UInt32) {
        guard let picker = documentShown.removeValue(forKey: view) else { return }
        documentRequests[ObjectIdentifier(picker)] = nil
        picker.dismiss(animated: !ExactEnv.agentFreezes)
    }

    /// `back` with a real picker up (`--touch platform`): the newest closes,
    /// cancelled, as its Cancel button closes it. False with none.
    func agentBackDocument() -> Bool {
        guard let (view, picker) = documentShown.max(by: { $0.key < $1.key }),
              let (_, name) = documentRequests[ObjectIdentifier(picker)] else { return false }
        closeDocument(view)
        documentCancelled(view, name)
        return true
    }

    /// Whether a picker shown for real has yet to finish arriving (ShareSheet.arriving).
    var documentArriving: Bool { documentShown.values.contains { $0.presentingViewController == nil || $0.isBeingPresented } }

    /// A document picker's outcome; `false` when `controller` is not one.
    func documentFinished(_ controller: UIDocumentPickerViewController, urls: [URL]?) -> Bool {
        guard let (view, name) = documentRequests.removeValue(forKey: ObjectIdentifier(controller)) else { return false }
        documentShown[view] = nil
        if let urls, !urls.isEmpty { documentsChosen(view, urls, name) } else { documentCancelled(view, name) }
        return true
    }
}
#else
extension Picker {
    /// AppKit's panels are never shown beside a hold: no drive shows them (`--touch platform` is iOS's).
    func closeDocument(_: UInt32) {}

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
