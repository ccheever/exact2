// @ref llp/1106.010-mobile-browser-devices.decision.md#native-lifetime
#if os(iOS)
import UIKit
import UniformTypeIdentifiers

/// PreviewStreamWebView's remote dialog/file chooser/download actions. URLs stay in this native owner.
final class T3MobileBrowserFiles: NSObject, UIDocumentPickerDelegate {
    private weak var view: UIView?
    private weak var owner: T3MobileBrowser?
    private let target: T3MobileBrowserTarget
    private var alive = true, allowed = false
    private var chooser: [String: Any]?, dialogKey = "", chooserKey = ""
    private var chooserVersion = 0, dialogVersion = 0, pickerVersion = 0
    private weak var picker: UIDocumentPickerViewController?
    private var presented: UIViewController?
    private var workers: [T3MobileBrowserFileWork] = []
    private var tasks: [URLSessionTask] = [], temporary: [URL] = []
    private let session = URLSession(configuration: .ephemeral)
    init(view: UIView, target: T3MobileBrowserTarget, owner: T3MobileBrowser) { self.view = view; self.target = target; self.owner = owner }
    private var presenter: UIViewController? {
        var next: UIResponder? = view
        while let item = next { if let controller = item as? UIViewController { return controller }; next = item.next }
        return nil
    }
    @discardableResult private func present(_ controller: UIViewController, valid: @escaping () -> Bool = { true }) -> Bool {
        guard alive, owner?.agent != true, let presenter else { return false }
        let old = presenter.presentedViewController
        if let old, old !== presented { return false }
        presented = controller
        let show = { [weak self, weak presenter, weak controller] in
            guard let self, self.alive, let controller, self.presented === controller, valid(), let presenter else { return }
            presenter.present(controller, animated: true)
        }
        if let old { old.dismiss(animated: false, completion: show) } else { show() }
        return true
    }
    func alert(_ title: String, _ message: String) {
        let controller = UIAlertController(title: title, message: message, preferredStyle: .alert)
        controller.addAction(UIAlertAction(title: "OK", style: .default)); present(controller)
    }
    func dialog(_ data: [String: Any]?, allowed: Bool, answer: @escaping ([String: Any]) -> Void) {
        self.allowed = allowed
        if !allowed { invalidateChooser() }
        guard allowed, let data else {
            if !dialogKey.isEmpty { dialogVersion += 1; dialogKey = ""; presented?.dismiss(animated: true); presented = nil }
            return
        }
        let key = T3MobileBrowserView.json(data); guard key != dialogKey else { return }
        dialogVersion += 1; let version = dialogVersion
        let type = data["type"] as? String ?? "", message = data["message"] as? String ?? ""
        let controller = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        let valid = { [weak self] in self?.alive == true && self?.allowed == true && self?.dialogVersion == version }
        if type == "prompt" { controller.addTextField { $0.text = data["defaultValue"] as? String ?? "" } }
        controller.addAction(UIAlertAction(title: "Dismiss", style: .cancel) { [weak self] _ in
            guard valid() else { return }; self?.dialogVersion += 1; answer(["type": "dialog", "accept": false, "_dialogKey": key, "_dialogVersion": version + 1])
        })
        controller.addAction(UIAlertAction(title: "Accept", style: .default) { [weak self] _ in
            guard valid() else { return }; self?.dialogVersion += 1
            var reply: [String: Any] = ["type": "dialog", "accept": true, "_dialogKey": key, "_dialogVersion": version + 1]
            if type == "prompt" { reply["promptText"] = controller.textFields?.first?.text ?? "" }; answer(reply)
        })
        if present(controller, valid: valid) { dialogKey = key }
    }
    func allowsDialogAnswer(_ input: [String: Any]) -> Bool {
        alive && allowed && input["_dialogKey"] as? String == dialogKey && input["_dialogVersion"] as? Int == dialogVersion
    }
    private func current(_ work: @escaping () -> Void) {
        owner?.check(target) { [weak self] valid in guard self?.alive == true, valid else { return }; work() }
    }
    private func url(_ text: String, endpoint: String) -> URL? {
        guard let url = URL(string: text), let origin = URL(string: target.origin), url.scheme == origin.scheme,
              url.host == origin.host, url.port == origin.port, url.user == nil, url.password == nil,
              url.path == "/api/preview-stream/\(endpoint)" else { return nil }
        return url
    }
    private func invalidateChooser() {
        chooserVersion += 1; chooser = nil; chooserKey = ""
        if let picker { picker.delegate = nil; picker.dismiss(animated: false) }
        picker = nil
    }
    func choose(_ data: [String: Any]?) {
        let key = data.map(T3MobileBrowserView.json) ?? ""
        if key == chooserKey { return }
        invalidateChooser()
        guard let data, allowed, url(data["uploadUrl"] as? String ?? "", endpoint: "upload") != nil else { return }
        chooser = data; let version = chooserVersion
        let valid = { [weak self] in self?.alive == true && self?.allowed == true && self?.chooserVersion == version }
        let controller = UIAlertController(title: data["multiple"] as? Bool == true ? "The page asks for files." : "The page asks for a file.", message: nil, preferredStyle: .alert)
        controller.addAction(UIAlertAction(title: "Cancel", style: .cancel) { [weak self] _ in if valid() { self?.upload([], version: version) } })
        controller.addAction(UIAlertAction(title: data["multiple"] as? Bool == true ? "Choose files" : "Choose file", style: .default) { [weak self] _ in if valid() { self?.pick(version: version) } })
        if present(controller, valid: valid) { chooserKey = key }
    }
    private func pick(version: Int) {
        guard alive, allowed, version == chooserVersion, let chooser else { return }
        let accept = (chooser["accept"] as? String ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
        let types = accept.compactMap { value in value.hasPrefix(".") ? UTType(filenameExtension: String(value.dropFirst())) : UTType(mimeType: value) }
        let controller = UIDocumentPickerViewController(forOpeningContentTypes: types.isEmpty ? [.item] : types, asCopy: true)
        controller.allowsMultipleSelection = chooser["multiple"] as? Bool == true; controller.delegate = self
        picker = controller; pickerVersion = version
        present(controller) { [weak self] in self?.alive == true && self?.allowed == true && self?.chooserVersion == version }
    }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard controller === picker else { return }; upload(urls, version: pickerVersion)
    }
    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        guard controller === picker else { return }; upload([], version: pickerVersion)
    }
    private func upload(_ files: [URL], version: Int) {
        guard version == chooserVersion, let data = chooser, let url = url(data["uploadUrl"] as? String ?? "", endpoint: "upload"), let owner else { return }
        chooser = nil; chooserKey = ""; picker = nil
        owner.check(target) { [weak self] valid in
            guard let self, alive, valid, allowed, chooserVersion == version else { return }
            let worker = T3MobileBrowserFileWork(); workers.append(worker)
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                do {
                    let multipart = try Self.multipart(files, worker: worker)
                    DispatchQueue.main.async { [weak self] in
                        guard let self, alive, allowed, chooserVersion == version else { try? FileManager.default.removeItem(at: multipart.directory); return }
                        owner.check(target) { [weak self] valid in
                            guard let self, alive, valid, allowed, chooserVersion == version else { try? FileManager.default.removeItem(at: multipart.directory); return }
                            temporary.append(multipart.directory)
                            var request = URLRequest(url: url); request.httpMethod = "POST"
                            request.setValue("multipart/form-data; boundary=\(multipart.boundary)", forHTTPHeaderField: "Content-Type")
                            let task = session.uploadTask(with: request, fromFile: multipart.file) { [weak self] _, response, error in
                                try? FileManager.default.removeItem(at: multipart.directory)
                                DispatchQueue.main.async { [weak self] in
                                    guard let self, alive else { return }
                                    if error != nil || !((response as? HTTPURLResponse).map { (200..<300).contains($0.statusCode) } ?? false) {
                                        current { [weak self] in self?.alert("Could not send the files to the page", "The upload failed.") }
                                    }
                                }
                            }; tasks.append(task); task.resume()
                        }
                    }
                } catch { DispatchQueue.main.async { [weak self] in self?.current { [weak self] in self?.alert("Could not send the files to the page", error.localizedDescription) } } }
            }
        }
    }
    /// Match URI-backed FormData without holding the complete files or body on the UI thread.
    private static func multipart(_ files: [URL], worker: T3MobileBrowserFileWork) throws -> (directory: URL, file: URL, boundary: String) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-browser-upload-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("body"), boundary = "t3-mobile-\(UUID().uuidString)"
        do {
            FileManager.default.createFile(atPath: file.path, contents: nil)
            let output = try FileHandle(forWritingTo: file); defer { try? output.close() }
            func write(_ text: String) throws { try output.write(contentsOf: Data(text.utf8)) }
            for source in files {
                try worker.check()
                let scoped = source.startAccessingSecurityScopedResource(); defer { if scoped { source.stopAccessingSecurityScopedResource() } }
                let name = source.lastPathComponent.replacingOccurrences(of: "\"", with: "_").replacingOccurrences(of: "\r", with: "_").replacingOccurrences(of: "\n", with: "_")
                try write("--\(boundary)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"\(name)\"\r\nContent-Type: \(UTType(filenameExtension: source.pathExtension)?.preferredMIMEType ?? "application/octet-stream")\r\n\r\n")
                let input = try FileHandle(forReadingFrom: source); defer { try? input.close() }
                while let chunk = try input.read(upToCount: 1 << 20), !chunk.isEmpty { try worker.check(); try output.write(contentsOf: chunk) }
                try write("\r\n")
            }
            try write("--\(boundary)--\r\n"); return (directory, file, boundary)
        } catch { try? FileManager.default.removeItem(at: directory); throw error }
    }
    func download(_ data: [String: Any]) {
        guard let url = url(data["url"] as? String ?? "", endpoint: "download") else { return }
        let name = (data["fileName"] as? String ?? "Download").replacingOccurrences(of: "/", with: "_")
        let controller = UIAlertController(title: "Downloaded \(name)", message: nil, preferredStyle: .alert)
        controller.addAction(UIAlertAction(title: "Not now", style: .cancel))
        controller.addAction(UIAlertAction(title: "Save or share", style: .default) { [weak self] _ in self?.save(url, name: name) }); present(controller)
    }
    private func save(_ url: URL, name: String) {
        owner?.check(target) { [weak self] valid in
            guard let self, alive, valid else { return }
            let task = session.downloadTask(with: url) { [weak self] file, response, error in
                guard let self, let file, error == nil, (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) == true else {
                    DispatchQueue.main.async { [weak self] in self?.current { [weak self] in self?.alert("Could not save the download", "The download failed.") } }; return
                }
                do {
                    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-browser-\(UUID().uuidString)", isDirectory: true)
                    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                    let destination = directory.appendingPathComponent(name.isEmpty || name == "." || name == ".." ? "Download" : name)
                    try FileManager.default.moveItem(at: file, to: destination)
                    DispatchQueue.main.async { [weak self] in
                        guard let self, alive else { try? FileManager.default.removeItem(at: directory); return }
                        temporary.append(directory)
                        owner?.check(target) { [weak self] valid in
                        guard let self, alive, valid else { try? FileManager.default.removeItem(at: directory); return }
                        let share = UIActivityViewController(activityItems: [destination], applicationActivities: nil)
                        share.popoverPresentationController?.sourceView = view; share.popoverPresentationController?.sourceRect = view?.bounds ?? .zero
                        share.completionWithItemsHandler = { _, _, _, _ in try? FileManager.default.removeItem(at: directory) }; present(share)
                        }
                    }
                } catch { DispatchQueue.main.async { [weak self] in self?.current { [weak self] in self?.alert("Could not save the download", error.localizedDescription) } } }
            }; tasks.append(task); task.resume()
        }
    }
    func destroy() { alive = false; workers.forEach { $0.cancel() }; workers.removeAll(); invalidateChooser(); dialogVersion += 1; tasks.forEach { $0.cancel() }; tasks.removeAll(); session.invalidateAndCancel(); presented?.dismiss(animated: false); presented = nil; temporary.forEach { try? FileManager.default.removeItem(at: $0) }; temporary.removeAll() }
}
private final class T3MobileBrowserFileWork: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    func cancel() { lock.lock(); cancelled = true; lock.unlock() }
    func check() throws { lock.lock(); let stopped = cancelled; lock.unlock(); if stopped { throw CancellationError() } }
}
#endif
