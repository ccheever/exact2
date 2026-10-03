// @ref LLP 1069.002 — the file picker on Apple. `showPicker(id)` presents
// PHPicker on iOS (no permission, no usage string) or a document picker for
// the manifest's non-media types, and a filtered NSOpenPanel sheet on macOS
// (D8). Native hosts don't enforce user activation (ruled, Q2). Each chosen
// file is copied into `app:/tmp/picked/` before `change` fires (D4), a HEIC
// photo becoming a JPEG when `accept` names images but not HEIC (D6,
// Safari's rule), with its pixel size and a video's duration (D3). Under
// the agent the request is held for `type @t <path>…` / `tap @t cancel`
// (D9), whose files take the same path.
import Foundation
import ImageIO
import AVFoundation
import UniformTypeIdentifiers
#if canImport(UIKit)
import UIKit
import PhotosUI
#else
import AppKit
#endif

/// The app's directories behind `app:/`, told by the library with each
/// picked name, so an `image` source can read a picked file (D7).
enum AppFiles {
    private static let lock = NSLock()
    private static var roots: [String: URL] = [:]
    static func learn(_ reply: [String: Any]) {
        guard let r = reply["roots"] as? [String: String] else { return }
        lock.lock(); defer { lock.unlock() }
        for (name, path) in r { roots[name] = URL(fileURLWithPath: path, isDirectory: true) }
    }
    /// The file an `app:/data|cache|tmp/…` path names; nil for `..` or a root.
    static func url(_ path: String) -> URL? {
        guard path.hasPrefix("app:/") else { return nil }
        let parts = path.dropFirst(5).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 2, !parts.dropFirst().contains(where: { $0.isEmpty || $0 == "." || $0 == ".." }) else { return nil }
        lock.lock(); let root = roots[parts[0]]; lock.unlock()
        return parts.dropFirst().reduce(root) { $0?.appendingPathComponent($1) }
    }
}

final class Picker: NSObject {
    unowned let session: ExactSession
    /// Inputs whose picker is showing: a second `showPicker` does nothing.
    private var open: Set<UInt32> = []
    /// Chosen documents under `startAccessingSecurityScopedResource`, until
    /// the session ends (LLP 1069.010 D1).
    var scoped: [URL] = []
    #if canImport(UIKit)
    private var requests: [ObjectIdentifier: Request] = [:]
    /// Exporting document pickers showing (LLP 1069.010 D3): the element
    /// the outcome goes to, and the scratch directory to remove.
    var exports: [ObjectIdentifier: (UInt32, URL)] = [:]
    /// Document pickers showing (LLP 1069.010 D2): the element, the command.
    var documentRequests: [ObjectIdentifier: (UInt32, String)] = [:]
    #endif
    init(session: ExactSession) { self.session = session }

    struct Request { let view: UInt32; let id: String; let accept: [String]; let multiple: Bool }

    private func object(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8))) as? [String: Any] ?? [:]
    }
    private func quoted(_ s: String) -> String {
        let d = (try? JSONSerialization.data(withJSONObject: [s])) ?? Data("[\"\"]".utf8)
        return String(String(decoding: d, as: UTF8.self).dropFirst().dropLast())
    }

    /// `showPicker(id)`, from any action (D2).
    func show(_ args: [Any]) {
        guard let id = args.first as? String else { session.log("picker: refused: showPicker names an element id"); return }
        if ExactEnv.agentMode {
            let r = object(session.agent("{\"op\":\"showPicker\",\"id\":\(quoted(id))}"))
            if let e = r["error"] as? String { fputs("exact: \(e)\n", stderr) }
            return
        }
        let r = object(session.agent("{\"op\":\"picker\",\"id\":\(quoted(id))}"))
        guard let view = (r["view"] as? NSNumber)?.uint32Value else {
            session.log("picker: refused: no file input with id \"\(id)\""); return
        }
        guard !open.contains(view) else { session.log("picker: \"\(id)\" is already open"); return }
        open.insert(view)
        present(Request(view: view, id: id, accept: r["accept"] as? [String] ?? [], multiple: r["multiple"] as? Bool ?? false))
    }

    /// The agent's answer, after the library consumed the hold (D9): `reply`
    /// is the library's, `request` the driver's (its paths, never journalled).
    func answered(_ reply: [String: Any], request: [String: Any]) {
        guard reply["capability"] as? String == "pick", let view = (reply["node"] as? NSNumber)?.uint32Value else { return }
        if reply["answered"] as? String == "cancel" { cancel(view); return }
        let text = request["text"] as? String ?? ""
        var paths = text.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        if paths.count == 1 { paths = paths[0].split(separator: " ").map(String.init) }
        let accept = (session.presenter.views[view]?.props["accept"] ?? "").split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces).lowercased() }.filter { !$0.isEmpty }
        deliver(view, accept: accept, files: paths.map { (URL(fileURLWithPath: $0), ($0 as NSString).lastPathComponent) })
    }

    func cancel(_ view: UInt32) {
        open.remove(view)
        session.apply(session.runtime.pickerCancel(view, now: session.now()))
    }

    /// Copy each file in, converting per D6, describe it (D3), fire `change`.
    func deliver(_ view: UInt32, accept: [String], files: [(url: URL, name: String)]) {
        open.remove(view)
        var lines: [String] = []
        for f in files {
            let ext = (f.name as NSString).pathExtension.lowercased()
            let mime = UTType(filenameExtension: ext)?.preferredMIMEType ?? "application/octet-stream"
            let heic = mime == "image/heic" || mime == "image/heif"
            let keeps = accept.contains { ["image/*", "image/heic", "image/heif", ".heic", ".heif"].contains($0) }
            let convert = heic && !keeps && accept.contains { $0.hasPrefix("image/") }
            let stored = convert ? (f.name as NSString).deletingPathExtension + ".jpg" : f.name
            let reply = object(session.agent("{\"op\":\"pickedPath\",\"name\":\(quoted(stored))}"))
            AppFiles.learn(reply)
            guard let path = reply["path"] as? String, let file = reply["file"] as? String else {
                session.log("picker: refused: no app:/tmp/picked/ for \(f.name)"); cancel(view); return
            }
            let target = URL(fileURLWithPath: file)
            do {
                try FileManager.default.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
                try? FileManager.default.removeItem(at: target)
                if convert { try Picker.jpeg(from: f.url, to: target) } else { try FileManager.default.copyItem(at: f.url, to: target) }
            } catch {
                session.log("picker: refused: \(f.name): \(error.localizedDescription)"); cancel(view); return
            }
            let type = convert ? "image/jpeg" : mime
            let bytes = (try? target.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            var size = ["", "", ""]
            if type.hasPrefix("image/"), let (w, h) = Picker.pixels(target) { size = ["\(w)", "\(h)", ""] }
            else if type.hasPrefix("video/") { size = Picker.video(target) }
            let name = f.name.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
            lines.append(([path, name, type, "\(bytes)"] + size).joined(separator: "\t"))
        }
        session.apply(session.runtime.picked(view, lines.joined(separator: "\n"), now: session.now()))
    }

    /// Pixels, orientation applied, from the header alone.
    static func pixels(_ url: URL) -> (Int, Int)? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let p = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let w = p[kCGImagePropertyPixelWidth] as? Int, let h = p[kCGImagePropertyPixelHeight] as? Int else { return nil }
        let orientation = p[kCGImagePropertyOrientation] as? Int ?? 1
        return (5...8).contains(orientation) ? (h, w) : (w, h)
    }

    /// A video's pixel size (the track's transform applied) and seconds.
    static func video(_ url: URL) -> [String] {
        let asset = AVURLAsset(url: url)
        var out = ["", "", ""]
        if let track = asset.tracks(withMediaType: .video).first {
            let s = track.naturalSize.applying(track.preferredTransform)
            out[0] = "\(Int(abs(s.width).rounded()))"; out[1] = "\(Int(abs(s.height).rounded()))"
        }
        let seconds = CMTimeGetSeconds(asset.duration)
        if seconds.isFinite { out[2] = "\(seconds)" }
        return out
    }

    /// D6: JPEG at quality 0.9, orientation applied, with no metadata copied
    /// (so no location).
    static func jpeg(from: URL, to: URL) throws {
        guard let source = CGImageSourceCreateWithURL(from as CFURL, nil),
              let (w, h) = pixels(from),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                  kCGImageSourceCreateThumbnailFromImageAlways: true,
                  kCGImageSourceCreateThumbnailWithTransform: true,
                  kCGImageSourceThumbnailMaxPixelSize: max(w, h),
              ] as CFDictionary),
              let destination = CGImageDestinationCreateWithURL(to as CFURL, UTType.jpeg.identifier as CFString, 1, nil)
        else { throw CocoaError(.fileReadCorruptFile) }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.9] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { throw CocoaError(.fileWriteUnknown) }
    }

    /// `accept` as Apple's types: `image/*` and `video/*` their families,
    /// a MIME type or extension its UTType.
    static func types(_ accept: [String]) -> [UTType] {
        accept.compactMap { token in
            switch token {
            case "image/*": return .image
            case "video/*": return .movie
            case let t where t.hasPrefix("."): return UTType(filenameExtension: String(t.dropFirst()))
            default: return UTType(mimeType: token)
            }
        }
    }
}

#if os(tvOS)
extension Picker {
    func present(_ r: Request) {
        // tvOS has no photo or document picker.
        session.log("picker: refused: no picker"); cancel(r.view)
    }
}
#elseif canImport(UIKit)
extension Picker: PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
    func present(_ r: Request) {
        guard var controller = session.presenter.root.window?.rootViewController else {
            session.log("picker: refused: no window"); cancel(r.view); return
        }
        while let presented = controller.presentedViewController { controller = presented }
        let media = !r.accept.isEmpty && r.accept.allSatisfy { $0.hasPrefix("image/") || $0.hasPrefix("video/") }
        if media {
            var config = PHPickerConfiguration()
            config.selectionLimit = r.multiple ? 0 : 1
            let images = r.accept.contains { $0.hasPrefix("image/") }, videos = r.accept.contains { $0.hasPrefix("video/") }
            config.filter = images && videos ? .any(of: [.images, .videos]) : images ? .images : .videos
            // D6: HEVC/ProRes stays as is under `video/*`; otherwise H.264.
            config.preferredAssetRepresentationMode = r.accept.contains("video/*") || !videos ? .current : .compatible
            let picker = PHPickerViewController(configuration: config)
            picker.delegate = self
            requests[ObjectIdentifier(picker)] = r
            controller.present(picker, animated: true)
        } else {
            let picker = UIDocumentPickerViewController(forOpeningContentTypes: Picker.types(r.accept), asCopy: true)
            picker.allowsMultipleSelection = r.multiple
            picker.delegate = self
            requests[ObjectIdentifier(picker)] = r
            controller.present(picker, animated: true)
        }
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let r = requests.removeValue(forKey: ObjectIdentifier(picker)) else { return }
        if results.isEmpty { cancel(r.view); return }
        // The provider's URL is deleted when its callback returns: stage a
        // copy there, then copy into app:/tmp/picked/ on the main thread.
        let group = DispatchGroup(), lock = NSLock()
        var staged = [(url: URL, name: String)?](repeating: nil, count: results.count)
        for (i, result) in results.enumerated() {
            let provider = result.itemProvider
            guard let type = provider.registeredTypeIdentifiers.first(where: {
                UTType($0).map { $0.conforms(to: .image) || $0.conforms(to: .movie) } ?? false
            }) else { continue }
            group.enter()
            provider.loadFileRepresentation(forTypeIdentifier: type) { url, _ in
                defer { group.leave() }
                guard let url else { return }
                let stage = FileManager.default.temporaryDirectory.appendingPathComponent("exact-pick-\(UUID().uuidString).\(url.pathExtension)")
                guard (try? FileManager.default.copyItem(at: url, to: stage)) != nil else { return }
                let name = provider.suggestedName.map { $0 + "." + url.pathExtension } ?? url.lastPathComponent
                lock.lock(); staged[i] = (stage, name); lock.unlock()
            }
        }
        group.notify(queue: .main) { [weak self] in
            let files = staged.compactMap { $0 }
            if files.isEmpty { self?.cancel(r.view) } else { self?.deliver(r.view, accept: r.accept, files: files) }
            for f in files { try? FileManager.default.removeItem(at: f.url) }
        }
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        if exportFinished(controller, urls: urls) || documentFinished(controller, urls: urls) { return }
        guard let r = requests.removeValue(forKey: ObjectIdentifier(controller)) else { return }
        deliver(r.view, accept: r.accept, files: urls.map { ($0, $0.lastPathComponent) })
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        if exportFinished(controller, urls: nil) || documentFinished(controller, urls: nil) { return }
        guard let r = requests.removeValue(forKey: ObjectIdentifier(controller)) else { return }
        cancel(r.view)
    }
}
#else
extension Picker {
    func present(_ r: Request) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = r.multiple
        panel.allowedContentTypes = Picker.types(r.accept)
        let finish: (NSApplication.ModalResponse) -> Void = { [weak self, weak panel] response in
            guard let self else { return }
            guard response == .OK, let urls = panel?.urls, !urls.isEmpty else { self.cancel(r.view); return }
            let scoped = urls.map { $0.startAccessingSecurityScopedResource() }
            self.deliver(r.view, accept: r.accept, files: urls.map { ($0, $0.lastPathComponent) })
            for (url, started) in zip(urls, scoped) where started { url.stopAccessingSecurityScopedResource() }
        }
        if let window = session.presenter.root.window { panel.beginSheetModal(for: window, completionHandler: finish) }
        else { panel.begin(completionHandler: finish) }
    }
}
#endif
