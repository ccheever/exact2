// media-actions: the native half of MediaActions (T3 Code 1e2ecbd975, MIT; see LICENSE-T3:
// apps/web/src/components/media/MediaActions.tsx and mediaContent.ts; the desktop shell's context
// menu apps/desktop/src/electron/ElectronMenu.ts; Electron's default download handling).
//
// - `mediaMenu` {items, anchor?}: the native menu at the pointer, or, for the Menu key or
//   Shift+F10 (`anchor: "bottom-left"`), at the focused media's bottom-left corner. Answers the
//   picked id or null. Under the agent no menu tracks (the window is never key): it answers the
//   next id of `T3_AGENT_MEDIA_PICKS` (comma separated, so a drive can pick), else null, with the
//   items and where a keyboard menu would open.
// - `mediaCopyText` {text}: the general pasteboard (the agent's: a private named pasteboard).
// - `mediaSave` {url, name}: readMediaBlob's fetch (HTTP failures and a web page are refused with
//   the reference's sentences), then, as Electron does for a page download with no save path, the
//   Save panel as a sheet on the window, the media's name filled in, opening in the folder last
//   saved to (Downloads at first). It answers `started` once the panel is up, as the reference's
//   anchor click returns before the dialog closes; the bytes move when the person saves. Under the
//   agent the file goes to the isolated data root's exports/ instead.
// - `mediaCopyImage` {url}: readMediaPng: PNG bytes as they are, anything else decoded (SVG too)
//   and re-encoded as PNG, refused over 64,000,000 pixels or without usable dimensions; written
//   as PNG to the pasteboard.
import AppKit
import ImageIO
import UniformTypeIdentifiers

enum T3MediaActions {
    static let ops: Set<String> = ["mediaMenu", "mediaCopyText", "mediaSave", "mediaCopyImage"]
    static let pixelLimit = 64_000_000
    /// The pasteboard an agent drive writes instead of the person's (read it back by this name).
    static let agentPasteboard = NSPasteboard.Name("com.exact.t3code.agent-media")

    static let fetchFailed = "The file could not be fetched. The host may block browser access (CORS), or the connection may be unavailable."
    static let webPage = "This link returned a web page instead of media. Open the original URL."
    static let tooLarge = "This image is too large or has no usable dimensions. Try saving it instead."
    static let undecodable = "The browser could not decode this image for copying. Try saving it instead."
    static let notPng = "The image could not be converted to PNG."

    /// The folder the Save panel opened last (Electron keeps the session's last save directory).
    private static var lastDirectory: URL?
    private static var agentPicks: [String] = (ProcessInfo.processInfo.environment["T3_AGENT_MEDIA_PICKS"] ?? "")
        .split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        return URLSession(configuration: configuration)
    }()

    /// One media op; `exportsRoot` is set only under the agent.
    static func perform(_ request: [String: Any], exportsRoot: URL?, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) in reply(["ok": true, "generation": generation, "value": value]) }
        let fail = { (message: String) in answer(["ok": false, "message": message]) }
        let agent = exportsRoot != nil
        switch request["op"] as? String {
        case "mediaMenu":
            DispatchQueue.main.async { answer(menu(request["items"] as? [[String: Any]] ?? [], anchor: request["anchor"] as? String, agent: agent)) }
        case "mediaCopyText":
            guard let text = request["text"] as? String else { return fail("The media action failed.") }
            DispatchQueue.main.async {
                let board = pasteboard(agent: agent)
                board.clearContents()
                board.setString(text, forType: .string) ? answer(["ok": true]) : fail("The media action failed.")
            }
        case "mediaSave":
            guard let url = mediaURL(request["url"]) else { return fail("This media is unavailable. Try reopening the preview.") }
            save(url, name: fileName(request["name"] as? String), exportsRoot: exportsRoot, answer: answer, fail: fail)
        case "mediaCopyImage":
            guard let url = mediaURL(request["url"]) else { return fail("This media is unavailable. Try reopening the preview.") }
            read(url) { result in
                switch result {
                case .failure(let message): fail(message)
                case .success(let (data, mimeType)):
                    do {
                        let png = try pngData(data, mimeType: mimeType)
                        DispatchQueue.main.async {
                            let board = pasteboard(agent: agent)
                            board.clearContents()
                            board.setData(png.data, forType: .png) ? answer(["ok": true, "width": png.width, "height": png.height]) : fail(notPng)
                        }
                    } catch let error as MediaError { fail(error.message) } catch { fail(notPng) }
                }
            }
        default:
            reply(["ok": false, "generation": generation, "error": ["kind": "Media", "message": "Unknown media operation.", "uncertain": false]])
        }
    }

    static func pasteboard(agent: Bool) -> NSPasteboard { agent ? NSPasteboard(name: agentPasteboard) : .general }

    /// An http(s) or data: URL (asset URLs, authored media); never a file URL.
    static func mediaURL(_ raw: Any?) -> URL? {
        guard let raw = raw as? String, let url = URL(string: raw), let scheme = url.scheme?.lowercased() else { return nil }
        if scheme == "data" { return url }
        guard scheme == "http" || scheme == "https", url.host != nil else { return nil }
        return url
    }

    /// The download's file name: the media's name without path separators, else "media".
    static func fileName(_ raw: String?) -> String {
        let cleaned = (raw ?? "").replacingOccurrences(of: "/", with: "-").replacingOccurrences(of: ":", with: "-").trimmingCharacters(in: .whitespacesAndNewlines)
        return cleaned.isEmpty ? "media" : String(cleaned.prefix(200))
    }

    // MARK: Menu

    static func menu(_ items: [[String: Any]], anchor: String?, agent: Bool) -> [String: Any] {
        let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { ($0.firstResponder as? NSView).map { $0 !== $0.window?.contentView } ?? false }
        let anchored = anchor.flatMap { anchor in
            window.flatMap { window in window.contentView.flatMap { T3Sidebar.anchorPoint(anchor, focus: window.firstResponder as? NSView, in: $0) } }
        }
        let labels = items.compactMap { item -> String? in
            guard let label = item["label"] as? String else { return nil }
            return item["disabled"] as? Bool == true ? "\(label) (disabled)" : label
        }
        if agent || window?.contentView == nil {
            let enabled = Set(items.filter { $0["disabled"] as? Bool != true }.compactMap { $0["id"] as? String })
            var pick: String?
            if agent, !agentPicks.isEmpty { let next = agentPicks.removeFirst(); pick = enabled.contains(next) ? next : nil }
            return ["id": pick.map { $0 as Any } ?? NSNull(), "shown": false, "items": labels,
                    "anchor": anchored.map { [$0.topLeft.x, $0.topLeft.y] as Any } ?? NSNull()]
        }
        guard let window, let view = window.contentView else { return ["id": NSNull(), "shown": false] }
        // NSMenuItem.target is weak: the owner lives across the modal popUp.
        let owner = T3ContextMenu()
        let menu = owner.menu(for: items)
        let point = anchored?.point ?? view.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        menu.popUp(positioning: nil, at: point, in: view)
        let picked = withExtendedLifetime(owner) { owner.picked }
        // Focus returns to the media the menu was opened from (the window keeps its first responder).
        return ["id": picked.map { $0 as Any } ?? NSNull(), "shown": true]
    }

    // MARK: Bytes

    struct MediaError: Error { let message: String }
    enum Read { case success((Data, String)), failure(String) }

    /// readMediaBlob: the bytes and their MIME type, or the reference's sentence for the failure.
    static func read(_ url: URL, completion: @escaping (Read) -> Void) {
        session.dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 60)) { data, response, error in
            if let refusal = refusal(response: response, error: error) { return completion(.failure(refusal)) }
            completion(.success((data ?? Data(), mimeType(response))))
        }.resume()
    }

    static func mimeType(_ response: URLResponse?) -> String {
        ((response?.mimeType ?? "").split(separator: ";").first.map(String.init) ?? "").trimmingCharacters(in: .whitespaces).lowercased()
    }

    /// The fetch failed, the server refused (HTTP N), or it answered with a web page.
    static func refusal(response: URLResponse?, error: Error?) -> String? {
        if error != nil || response == nil { return fetchFailed }
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) { return "The file could not be fetched (HTTP \(http.statusCode))." }
        return mimeType(response) == "text/html" ? webPage : nil
    }

    static func save(_ url: URL, name: String, exportsRoot: URL?, answer: @escaping ([String: Any]) -> Void, fail: @escaping (String) -> Void) {
        session.downloadTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 120)) { location, response, error in
            if let refusal = refusal(response: response, error: error) { return fail(refusal) }
            guard let location else { return fail(fetchFailed) }
            // The task deletes its file when this handler returns: keep it until the panel closes.
            let held = FileManager.default.temporaryDirectory.appendingPathComponent("t3-media-\(UUID().uuidString)")
            do { try FileManager.default.moveItem(at: location, to: held) } catch { return fail(fetchFailed) }
            if let exportsRoot {
                do {
                    try FileManager.default.createDirectory(at: exportsRoot, withIntermediateDirectories: true)
                    let target = uniqueURL(exportsRoot.appendingPathComponent(name))
                    try FileManager.default.moveItem(at: held, to: target)
                    return answer(["ok": true, "started": true, "name": target.lastPathComponent])
                } catch { try? FileManager.default.removeItem(at: held); return fail("Could not save \(name).") }
            }
            DispatchQueue.main.async { present(held, name: name, answer: answer) }
        }.resume()
    }

    /// Electron's download save dialog: a sheet on the window when there is one.
    static func present(_ held: URL, name: String, answer: @escaping ([String: Any]) -> Void) {
        let panel = savePanel(name: name)
        let finish = { (response: NSApplication.ModalResponse) in
            defer { try? FileManager.default.removeItem(at: held) }
            guard response == .OK, let target = panel.url else { return }
            lastDirectory = target.deletingLastPathComponent()
            try? FileManager.default.removeItem(at: target)
            try? FileManager.default.copyItem(at: held, to: target)
        }
        if let window = NSApp.keyWindow ?? NSApp.mainWindow {
            panel.beginSheetModal(for: window, completionHandler: finish)
            answer(["ok": true, "started": true])
        } else {
            answer(["ok": true, "started": true])
            finish(panel.runModal())
        }
    }

    static func savePanel(name: String) -> NSSavePanel {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        panel.directoryURL = lastDirectory ?? FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        let ext = (name as NSString).pathExtension
        if !ext.isEmpty, let type = UTType(filenameExtension: ext) { panel.allowedContentTypes = [type]; panel.allowsOtherFileTypes = true }
        return panel
    }

    static func uniqueURL(_ url: URL) -> URL {
        guard FileManager.default.fileExists(atPath: url.path) else { return url }
        let stem = url.deletingPathExtension().lastPathComponent, ext = url.pathExtension, folder = url.deletingLastPathComponent()
        for index in 1...999 {
            let candidate = folder.appendingPathComponent(ext.isEmpty ? "\(stem) (\(index))" : "\(stem) (\(index)).\(ext)")
            if !FileManager.default.fileExists(atPath: candidate.path) { return candidate }
        }
        return folder.appendingPathComponent("\(UUID().uuidString)-\(url.lastPathComponent)")
    }

    /// readMediaPng: PNG as it is; anything else decoded and re-encoded, within the pixel limit.
    static func pngData(_ data: Data, mimeType: String) throws -> (data: Data, width: Int, height: Int) {
        let source = CGImageSourceCreateWithData(data as CFData, nil)
        let properties = source.flatMap { CGImageSourceCopyPropertiesAtIndex($0, 0, nil) as? [CFString: Any] }
        let pixelWidth = properties?[kCGImagePropertyPixelWidth] as? Int, pixelHeight = properties?[kCGImagePropertyPixelHeight] as? Int
        if mimeType == "image/png" || (source.flatMap { CGImageSourceGetType($0) as String? } == UTType.png.identifier && mimeType.isEmpty) {
            return (data, pixelWidth ?? 0, pixelHeight ?? 0)
        }
        if let source, let width = pixelWidth, let height = pixelHeight {
            guard width > 0, height > 0, width * height <= pixelLimit else { throw MediaError(message: tooLarge) }
            guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { throw MediaError(message: undecodable) }
            return try encode(image)
        }
        // Not a raster ImageIO reads (SVG): NSImage decodes it at its own size, as an <img> does.
        guard let image = NSImage(data: data), image.isValid else { throw MediaError(message: undecodable) }
        let width = Int(image.size.width.rounded()), height = Int(image.size.height.rounded())
        guard width > 0, height > 0, width * height <= pixelLimit else { throw MediaError(message: tooLarge) }
        guard let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                            isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { throw MediaError(message: notPng) }
        bitmap.size = NSSize(width: width, height: height)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
        image.draw(in: NSRect(x: 0, y: 0, width: width, height: height))
        NSGraphicsContext.restoreGraphicsState()
        guard let png = bitmap.representation(using: .png, properties: [:]) else { throw MediaError(message: notPng) }
        return (png, width, height)
    }

    static func encode(_ image: CGImage) throws -> (data: Data, width: Int, height: Int) {
        guard let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { throw MediaError(message: notPng) }
        return (png, image.width, image.height)
    }
}
