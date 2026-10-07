#if os(iOS)
// @ref llp/1106.005-composer-and-transcript.decision.md#new-task-ownership
// Native counterpart of pinned mobile composerImages.ts; files feed the existing shared upload contract.
import UIKit
import PhotosUI
import UniformTypeIdentifiers
import ImageIO

final class T3MobileAttachments: NSObject, PHPickerViewControllerDelegate, UIDocumentPickerDelegate,
    UIAdaptivePresentationControllerDelegate {
    static let maxImageBytes = 10 * 1024 * 1024
    static let maxFileBytes = 50 * 1024 * 1024
    private let drafts: URL
    private let staged: URL
    private let imports: URL?
    private var completion: (([String: Any]) -> Void)?
    private var picker: UIViewController?
    private var serial = 0
    private var generation = 0
    private var fileLimit = 0
    private var remaining = 100

    init(dataRoot: URL, agent: Bool) {
        drafts = dataRoot.appendingPathComponent("snapshots/drafts", isDirectory: true)
        staged = dataRoot.appendingPathComponent("composer-files", isDirectory: true)
        imports = agent ? dataRoot.appendingPathComponent("attach", isDirectory: true) : nil
        super.init()
    }

    private func failure(_ text: String, generation: Int) -> [String: Any] {
        ["ok": false, "generation": generation, "error": ["kind": "Attach", "message": text]]
    }
    private func url(_ request: [String: Any], image: Bool) -> URL? {
        guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { return nil }
        return (image ? drafts : staged).appendingPathComponent(id.lowercased())
    }
    private func activePresenter() -> UIViewController? {
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .filter { $0.activationState == .foregroundActive }.flatMap { $0.windows }
        guard var controller = windows.first(where: { $0.isKeyWindow })?.rootViewController else { return nil }
        while let next = controller.presentedViewController { controller = next }
        return controller
    }
    private func finish(_ files: [[String: Any]] = [], error: String? = nil) {
        let done = completion; completion = nil; picker = nil
        var value: [String: Any] = ["files": files]
        if let error { value["error"] = error }
        done?(["ok": true, "generation": generation, "value": value])
    }
    func destroy() {
        serial += 1
        picker?.dismiss(animated: false)
        finish(error: "The attachment picker was closed.")
    }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { finish() }

    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let gen = request["generation"] as? Int ?? 0
        switch request["op"] as? String {
        case "mobileAttachmentSource": chooseSource(request, reply: reply)
        case "composerAttachPick": pick(request, reply: reply)
        case "composerAttachRead", "snapshotDraftRead":
            let image = request["op"] as? String == "snapshotDraftRead"
            guard let file = url(request, image: image) else { reply(failure("That attachment is unavailable.", generation: gen)); return }
            DispatchQueue.global(qos: .userInitiated).async {
                let bytes = try? Data(contentsOf: file, options: .mappedIfSafe)
                let response: [String: Any]
                if let bytes, !bytes.isEmpty, bytes.count <= (image ? Self.maxImageBytes : Self.maxFileBytes) {
                    response = ["ok": true, "generation": gen, "value": ["base64": bytes.base64EncodedString(), "sizeBytes": bytes.count]]
                } else { response = self.failure("The attachment was not saved.", generation: gen) }
                DispatchQueue.main.async { reply(response) }
            }
        case "composerAttachRemove", "snapshotDraftRemove":
            let image = request["op"] as? String == "snapshotDraftRemove"
            guard let file = url(request, image: image) else { reply(failure("That attachment is unavailable.", generation: gen)); return }
            do {
                if FileManager.default.fileExists(atPath: file.path) { try FileManager.default.removeItem(at: file) }
                reply(["ok": true, "generation": gen, "value": ["removed": true]])
            } catch { reply(failure("The attachment could not be removed.", generation: gen)) }
        case "mobileAttachmentPreview":
            let image = request["image"] as? Bool ?? true
            guard let file = url(request, image: image) else { reply(failure("That attachment is unavailable.", generation: gen)); return }
            DispatchQueue.global(qos: .userInitiated).async {
                let data = Self.jpegThumbnail(file, edge: 512)
                let value: [String: Any] = ["dataUrl": data.map { "data:image/jpeg;base64,\($0.base64EncodedString())" } ?? ""]
                DispatchQueue.main.async { reply(["ok": true, "generation": gen, "value": value]) }
            }
        default: reply(failure("Unknown attachment operation.", generation: gen))
        }
    }

    /// Shared image uploads omit contentType (desktop PNGs). Mobile retains original supported bytes.
    /// Parent forwards this returned request to the same authenticated T3Transport upload method.
    static func uploadRequest(_ request: [String: Any]) -> [String: Any]? {
        if request["contentType"] as? String != nil { return request }
        guard let encoded = request["base64"] as? String, let bytes = Data(base64Encoded: encoded),
              !bytes.isEmpty, bytes.count <= maxImageBytes,
              let source = CGImageSourceCreateWithData(bytes as CFData, nil), let type = CGImageSourceGetType(source),
              let mime = supportedMime(type as String) else { return nil }
        var next = request; next["contentType"] = mime; return next
    }
    private static func supportedMime(_ identifier: String) -> String? {
        guard let type = UTType(identifier), [UTType.png, .jpeg, .gif, .webP].contains(where: { type.conforms(to: $0) }) else { return nil }
        return type.preferredMIMEType
    }
    private static func jpegThumbnail(_ url: URL, edge: Int) -> Data? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: edge, kCGImageSourceShouldCacheImmediately: true] as CFDictionary) else { return nil }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        return CGImageDestinationFinalize(destination) ? output as Data : nil
    }
    private func stage(_ source: URL, name suppliedName: String? = nil, photo: Bool, limit: Int) -> [String: Any] {
        let access = source.startAccessingSecurityScopedResource(); defer { if access { source.stopAccessingSecurityScopedResource() } }
        let name = suppliedName?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false ? suppliedName! : source.lastPathComponent
        let resource = try? source.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
        let size = resource?.fileSize ?? 0
        guard resource?.isRegularFile == true, size > 0 else { return ["kind": "unreadable", "name": name] }
        let id = UUID().uuidString.lowercased()
        do {
            if photo {
                var bytes: Data?, mime = "image/jpeg", outputName = (name as NSString).deletingPathExtension + ".jpg"
                if size <= Self.maxImageBytes, let sourceImage = CGImageSourceCreateWithURL(source as CFURL, nil),
                   let type = CGImageSourceGetType(sourceImage), let originalMime = Self.supportedMime(type as String) {
                    bytes = try Data(contentsOf: source); mime = originalMime; outputName = name
                } else { bytes = Self.jpegThumbnail(source, edge: 2048) }
                guard let bytes, !bytes.isEmpty else { return ["kind": "unreadable", "name": name] }
                guard bytes.count <= Self.maxImageBytes else { return ["kind": "too-large", "name": name, "sizeBytes": bytes.count] }
                try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
                try bytes.write(to: drafts.appendingPathComponent(id), options: .atomic)
                return ["kind": "image", "id": id, "name": outputName, "mimeType": mime, "sizeBytes": bytes.count]
            }
            let mime = UTType(filenameExtension: source.pathExtension.lowercased())?.preferredMIMEType ?? "application/octet-stream"
            var value: [String: Any] = ["kind": "file", "name": name, "mimeType": mime, "sizeBytes": size]
            guard limit > 0, size <= min(limit, Self.maxFileBytes) else { return value }
            try FileManager.default.createDirectory(at: staged, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: source, to: staged.appendingPathComponent(id))
            value["id"] = id; return value
        } catch { return ["kind": "unreadable", "name": name, "sizeBytes": size] }
    }

    private func chooseSource(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let gen = request["generation"] as? Int ?? 0
        guard completion == nil else { reply(failure("An attachment picker is already open.", generation: gen)); return }
        guard request["supportsFiles"] as? Bool == true else {
            reply(["ok": true, "generation": gen, "value": ["source": "photos"]]); return
        }
        guard imports == nil else {
            reply(failure("An agent drive must choose photos or files explicitly.", generation: gen)); return
        }
        guard let presenter = activePresenter() else { reply(failure("There is no active window for this picker.", generation: gen)); return }
        generation = gen; completion = reply
        let menu = UIAlertController(title: nil, message: nil, preferredStyle: .actionSheet)
        picker = menu
        for (source, title) in [("photos", "Photo Library"), ("files", "Choose Files"), ("", "Cancel")] {
            menu.addAction(UIAlertAction(title: title, style: source.isEmpty ? .cancel : .default) { [weak self] _ in
                guard let self else { return }
                let done = self.completion; self.completion = nil; self.picker = nil
                done?(["ok": true, "generation": gen, "value": ["source": source]])
            })
        }
        menu.popoverPresentationController?.sourceView = presenter.view
        menu.popoverPresentationController?.sourceRect = CGRect(x: presenter.view.bounds.midX, y: presenter.view.bounds.maxY - 44, width: 1, height: 1)
        presenter.present(menu, animated: true); menu.presentationController?.delegate = self
    }

    private func pick(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let gen = request["generation"] as? Int ?? 0
        guard completion == nil else { reply(failure("An attachment picker is already open.", generation: gen)); return }
        let source = request["source"] as? String ?? "photos"
        guard ["photos", "files"].contains(source) else { reply(failure("Choose Photo Library or Choose Files.", generation: gen)); return }
        remaining = max(0, min(100, request["remaining"] as? Int ?? 100))
        guard remaining > 0 else { reply(failure("You can attach up to 100 attachments per message.", generation: gen)); return }
        fileLimit = max(0, min(Self.maxFileBytes, request["fileLimit"] as? Int ?? 0))
        if let imports {
            let urls = ((try? FileManager.default.contentsOfDirectory(at: imports, includingPropertiesForKeys: nil)) ?? [])
                .filter { !$0.lastPathComponent.hasPrefix(".") }.sorted { $0.lastPathComponent < $1.lastPathComponent }
            let files = urls.prefix(remaining).map { stage($0, photo: source == "photos", limit: fileLimit) }
            for file in urls.prefix(remaining) { try? FileManager.default.removeItem(at: file) }
            reply(["ok": true, "generation": gen, "value": ["files": files]]); return
        }
        guard let presenter = activePresenter() else { reply(failure("There is no active window for this picker.", generation: gen)); return }
        serial += 1; generation = gen; completion = reply
        if source == "files" {
            let view = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
            view.allowsMultipleSelection = true; view.delegate = self; picker = view
            presenter.present(view, animated: true); view.presentationController?.delegate = self
        } else if source == "photos" {
            var config = PHPickerConfiguration(photoLibrary: .shared())
            config.selectionLimit = remaining; config.selection = .ordered
            config.filter = fileLimit > 0 ? .any(of: [.images, .videos]) : .images
            config.preferredAssetRepresentationMode = .current
            let view = PHPickerViewController(configuration: config); view.delegate = self; picker = view
            presenter.present(view, animated: true); view.presentationController?.delegate = self
        }
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { finish() }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        let epoch = serial, limit = fileLimit, count = remaining
        DispatchQueue.global(qos: .userInitiated).async {
            let files = urls.prefix(count).map { self.stage($0, photo: false, limit: limit) }
            DispatchQueue.main.async {
                if self.serial == epoch, self.completion != nil { self.finish(files, error: urls.count > count ? "You can attach up to 100 files per message." : nil) }
                else { self.discard(files) }
            }
        }
    }
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        let epoch = serial, limit = fileLimit
        func load(_ index: Int, _ files: [[String: Any]]) {
            guard index < min(results.count, self.remaining) else { self.finish(files); return }
            let provider = results[index].itemProvider, movie = provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier)
            let type = movie ? UTType.movie.identifier : UTType.image.identifier
            provider.loadFileRepresentation(forTypeIdentifier: type) { url, _ in
                let file = url.map { self.stage($0, name: provider.suggestedName, photo: !movie, limit: limit) }
                    ?? ["kind": "unreadable", "name": provider.suggestedName ?? "image"]
                DispatchQueue.main.async {
                    guard self.serial == epoch, self.completion != nil else { self.discard(files + [file]); return }
                    load(index + 1, files + [file])
                }
            }
        }
        if results.isEmpty { finish() } else { load(0, []) }
    }
    private func discard(_ files: [[String: Any]]) {
        for file in files {
            guard let id = file["id"] as? String, UUID(uuidString: id) != nil else { continue }
            try? FileManager.default.removeItem(at: (file["kind"] as? String == "image" ? drafts : staged).appendingPathComponent(id))
        }
    }
}
#endif
