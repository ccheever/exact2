#if os(macOS)
import AppKit
import AVFoundation
import UniformTypeIdentifiers

/// The composer's Attach files (ChatComposer's hidden file input): an
/// NSOpenPanel of any files. Supported images (GIF, HEIC, HEIF, JPEG, PNG,
/// WebP) become PNG draft images beside SnapShot's (the shelf and the upload
/// read them from there). Other files within the server's staging limit are
/// copied into staged/ for the draft's file chips (composer-editor-attach.ts)
/// and read back once, at send; the rest are reported so the client can say
/// why they were not attached. Under the agent the files are the isolated data
/// root's attach/ directory, so a drive supplies them without a panel.
final class T3ComposerAttach {
    static let maxImageBytes = 10 * 1024 * 1024
    /// PROVIDER_SEND_TURN_MAX_FILE_BYTES: no server limit stages more.
    static let maxFileBytes = 50 * 1024 * 1024
    private let drafts: URL
    private let staged: URL
    private let imports: URL?

    init(dataRoot: URL, agent: Bool) {
        drafts = dataRoot.appendingPathComponent("snapshots", isDirectory: true).appendingPathComponent("drafts", isDirectory: true)
        staged = dataRoot.appendingPathComponent("composer-files", isDirectory: true)
        imports = agent ? dataRoot.appendingPathComponent("attach", isDirectory: true) : nil
    }

    /// The MIME type a browser's File would carry for this name; a generic file otherwise.
    static func mimeType(of url: URL) -> String {
        UTType(filenameExtension: url.pathExtension.lowercased())?.preferredMIMEType ?? "application/octet-stream"
    }

    /// A plain file: copied into staged/ when the server takes files of its size (`fileLimit`, 0 when it takes none).
    func stageFile(_ url: URL, size: Int, fileLimit: Int) -> [String: Any] {
        let name = url.lastPathComponent
        var file: [String: Any] = ["kind": "file", "name": name, "sizeBytes": size, "mimeType": Self.mimeType(of: url)]
        guard size > 0, fileLimit > 0, size <= min(fileLimit, Self.maxFileBytes) else { return file }
        let id = UUID().uuidString.lowercased()
        do {
            try FileManager.default.createDirectory(at: staged, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: url, to: staged.appendingPathComponent(id))
        } catch { return ["kind": "unreadable", "name": name, "sizeBytes": size] }
        file["id"] = id
        // r4-composer: a video's tile shows its first frame and plays from a typed link (T3ComposerVideo.swift).
        if Self.isVideo(url) {
            let ext = url.pathExtension.lowercased()
            try? FileManager.default.linkItem(at: staged.appendingPathComponent(id), to: staged.appendingPathComponent("\(id).\(ext)"))
            try? FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
            // The frame at its natural (oriented) size names the preview's; the poster keeps at most 512 px.
            if let frame = Self.firstFrame(of: url), Self.writePNG(Self.scaled(frame, to: 512), to: drafts.appendingPathComponent("\(id).png")) {
                file["videoWidth"] = frame.width; file["videoHeight"] = frame.height
            }
        }
        return file
    }

    /// videoMimeType's extensions that AVFoundation can open.
    static func isVideo(_ url: URL) -> Bool {
        guard let type = UTType(filenameExtension: url.pathExtension.lowercased()) else { return false }
        return type.conforms(to: .movie)
    }

    /// prepareVideoFirstFrame: the frame near 0.1 s (the nearest one for a shorter video), oriented.
    static func firstFrame(of url: URL) -> CGImage? {
        let asset = AVURLAsset(url: url)
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.requestedTimeToleranceBefore = .positiveInfinity
        generator.requestedTimeToleranceAfter = .positiveInfinity
        let done = DispatchSemaphore(value: 0)
        var image: CGImage?
        generator.generateCGImagesAsynchronously(forTimes: [NSValue(time: CMTime(value: 1, timescale: 10))]) { _, frame, _, _, _ in
            image = frame; done.signal()
        }
        guard done.wait(timeout: .now() + 5) == .success else { generator.cancelAllCGImageGeneration(); return nil }
        return image
    }

    static func scaled(_ image: CGImage, to limit: Int) -> CGImage {
        let side = max(image.width, image.height)
        guard side > limit else { return image }
        let width = max(1, image.width * limit / side), height = max(1, image.height * limit / side)
        guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return image }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage() ?? image
    }

    static func writePNG(_ image: CGImage, to url: URL) -> Bool {
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return false }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination)
    }

    private func stagedURL(_ request: [String: Any]) -> URL? {
        guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { return nil }
        return staged.appendingPathComponent(id.lowercased())
    }

    /// `composerAttachPick`, `composerAttachRead` (a staged file's bytes, once, at send) and `composerAttachRemove`.
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let fail = { (message: String) in reply(["ok": false, "generation": generation, "error": ["kind": "Attach", "message": message]]) }
        switch request["op"] as? String {
        case "composerAttachPick": pick(request, reply: reply)
        case "composerAttachRead":
            guard let url = stagedURL(request) else { return fail("That attached file is unavailable.") }
            DispatchQueue.global(qos: .userInitiated).async {
                let data = try? Data(contentsOf: url)
                let value: [String: Any]
                if let data, data.count > 0, data.count <= Self.maxFileBytes {
                    value = ["ok": true, "generation": generation, "value": ["base64": data.base64EncodedString(), "sizeBytes": data.count]]
                } else {
                    value = ["ok": false, "generation": generation, "error": ["kind": "Attach", "message": "The attached file was not saved."]]
                }
                DispatchQueue.main.async { reply(value) }
            }
        case "composerAttachRemove":
            if let url = stagedURL(request) {
                try? FileManager.default.removeItem(at: url)
                // A video's typed link and first frame go with it.
                let id = url.lastPathComponent
                for link in (try? FileManager.default.contentsOfDirectory(at: staged, includingPropertiesForKeys: nil)) ?? [] where link.lastPathComponent.hasPrefix("\(id).") {
                    try? FileManager.default.removeItem(at: link)
                }
                try? FileManager.default.removeItem(at: drafts.appendingPathComponent("\(id).png"))
            }
            reply(["ok": true, "generation": generation, "value": ["removed": true]])
        default: fail("Unknown attach operation.")
        }
    }

    /// classifyComposerAttachmentFile: a supported image, an image type the provider cannot take, or a plain file.
    static func kind(of url: URL) -> String {
        guard let type = UTType(filenameExtension: url.pathExtension.lowercased()) else { return "file" }
        let supported: [UTType] = [.gif, .heic, .heif, .jpeg, .png, .webP]
        if supported.contains(where: { type.conforms(to: $0) }) { return "image" }
        return type.conforms(to: .image) ? "unsupported-image" : "file"
    }

    /// One picked file as the client sees it; images are re-encoded as PNG drafts, plain files staged.
    func stage(_ url: URL, fileLimit: Int = 0) -> [String: Any] {
        let name = url.lastPathComponent
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        let kind = Self.kind(of: url)
        if kind == "file" { return stageFile(url, size: size, fileLimit: fileLimit) }
        guard kind == "image" else { return ["kind": kind, "name": name, "sizeBytes": size] }
        // A PNG keeps its own bytes (and size: the chip and shelf report the file's); other types are re-encoded.
        let original = try? Data(contentsOf: url)
        let isPNG = original.map { $0.starts(with: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) && NSBitmapImageRep(data: $0) != nil } ?? false
        guard let png = isPNG ? original : NSImage(contentsOf: url).flatMap({ $0.tiffRepresentation }).flatMap({ NSBitmapImageRep(data: $0) })
                .flatMap({ $0.representation(using: .png, properties: [:]) }) else {
            return ["kind": "unreadable", "name": name, "sizeBytes": size]
        }
        guard png.count <= Self.maxImageBytes else { return ["kind": "too-large", "name": name, "sizeBytes": png.count] }
        let id = UUID().uuidString.lowercased()
        do {
            try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
            try png.write(to: drafts.appendingPathComponent("\(id).png"), options: .atomic)
        } catch { return ["kind": "unreadable", "name": name, "sizeBytes": size] }
        let base = (name as NSString).deletingPathExtension
        return ["kind": "image", "id": id, "name": "\(base.isEmpty ? "image" : base).png", "sizeBytes": png.count]
    }

    func pick(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let fileLimit = request["fileLimit"] as? Int ?? 0
        let finish = { (urls: [URL]) in
            let files = urls.prefix(100).map { self.stage($0, fileLimit: fileLimit) }
            reply(["ok": true, "generation": generation, "value": ["files": files]])
        }
        if let imports {
            let urls = (try? FileManager.default.contentsOfDirectory(at: imports, includingPropertiesForKeys: nil))?
                .filter { !$0.lastPathComponent.hasPrefix(".") }.sorted { $0.lastPathComponent < $1.lastPathComponent } ?? []
            // A drive's files are consumed once, like a panel's selection.
            let picked = urls.map { self.stage($0, fileLimit: fileLimit) }
            for url in urls { try? FileManager.default.removeItem(at: url) }
            return reply(["ok": true, "generation": generation, "value": ["files": picked]])
        }
        DispatchQueue.main.async {
            let panel = NSOpenPanel()
            panel.allowsMultipleSelection = true
            panel.canChooseDirectories = false
            panel.canChooseFiles = true
            panel.prompt = "Attach"
            let done: (NSApplication.ModalResponse) -> Void = { response in finish(response == .OK ? panel.urls : []) }
            if let window = NSApp.keyWindow ?? NSApp.mainWindow { panel.beginSheetModal(for: window, completionHandler: done) } else { done(panel.runModal()) }
        }
    }
}
#endif
