#if os(macOS)
import AppKit
import AVFoundation
import UniformTypeIdentifiers

/// What Exact's storage cannot do for a staged copy of an attached file. TypeScript stages a drop's, a paste's and
/// Attach files' files (composer-attach-stage.ts): an image as a draft beside SnapShot's, any other file within the
/// server's limit in composer-files/, and reads and removes them (composer-staged-copies.ts). This makes a video's first
/// frame (AVFoundation) and the typed link a video or audio file plays from (`composerAttachMedia`), and names a
/// staged copy for a chip's preview (`stagedFile`).
final class T3ComposerAttach {
    private let drafts: URL
    private let staged: URL

    init(dataRoot: URL) {
        drafts = dataRoot.appendingPathComponent("snapshots", isDirectory: true).appendingPathComponent("drafts", isDirectory: true)
        staged = dataRoot.appendingPathComponent("composer-files", isDirectory: true)
    }

    /// A video or audio file plays from a typed link beside the copy, whose extension tells AVFoundation its type
    /// (the Contract players, composer-attachments.ts stagedMediaSrc); a video's tile shows its first frame, whose
    /// natural (oriented) size names the preview's; the poster keeps at most 512 px.
    func media(_ copy: URL, ext: String, video: Bool) -> [String: Any] {
        let id = copy.lastPathComponent, clean = ext.lowercased().filter { $0.isLetter || $0.isNumber }
        let typed = clean.isEmpty ? copy : staged.appendingPathComponent("\(id).\(clean)")
        if typed != copy { try? FileManager.default.linkItem(at: copy, to: typed) }
        guard video else { return [:] }
        try? FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
        guard let frame = Self.firstFrame(of: typed), Self.writePNG(Self.scaled(frame, to: 512), to: drafts.appendingPathComponent("\(id).png")) else { return [:] }
        return ["videoWidth": frame.width, "videoHeight": frame.height]
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

    /// A draft file's staged copy, when it is still there (a file chip's preview reads it: T3PanelsNative.swift, T3MediaPreview.swift).
    func stagedFile(_ id: String) -> URL? {
        guard let url = stagedURL(["id": id]), FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url
    }

    /// `composerAttachMedia`.
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let fail = { (message: String) in reply(["ok": false, "generation": generation, "error": ["kind": "Attach", "message": message]]) }
        guard request["op"] as? String == "composerAttachMedia" else { return fail("Unknown attach operation.") }
        guard let url = stagedURL(request), FileManager.default.fileExists(atPath: url.path) else { return fail("That attached file is unavailable.") }
        let ext = request["ext"] as? String ?? "", video = request["video"] as? Bool ?? false
        DispatchQueue.global(qos: .userInitiated).async {
            let value = self.media(url, ext: ext, video: video)
            DispatchQueue.main.async { reply(["ok": true, "generation": generation, "value": value]) }
        }
    }
}
#endif
