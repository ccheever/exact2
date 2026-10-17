// The right panel's native needs.
//
// 1. The Files editor (hook `t3-file-editor`) takes the focus when a press in the
//    file's body mounts it. It writes exactly what was typed through its
//    `autocorrect="off"`, which exact2 #111 maps to no smart quotes, dashes or
//    text replacement (the reference's editor is a browser textarea, which never
//    substitutes).
// 2. A sent attachment's preview (MIT reference, see LICENSE-T3:
//    components/files/AttachmentFilePreview.tsx): its text is read from the
//    signed asset URL with a 1 MB range (FILE_TEXT_PREVIEW_MAX_BYTES), and Save
//    file writes its bytes where the person chooses (an NSSavePanel; under the
//    agent, the isolated data root's exports/ directory).
import AppKit
import Foundation

enum T3FileEditor {
    static func install(_ element: ExactElement) {
        guard element.hatch == .t3FileEditor, let view = element.textView else { return }
        watchPresses()
        if element.isNew, element.id == fileEditorId { focusFileEditor(element, view) }
    }

    /// The Files editor's textarea (`files-surface.contract`).
    static let fileEditorId = "file-editor"

    /// A press anywhere in a Files editor's body (`file-lines`: below the last line, the gutter)
    /// begins editing and mounts the editor, but the press left the focus where it was (the
    /// composer, which opening a thread focuses) and Exact's focus on the pressed view, which blocks
    /// the textarea's autofocus: typing then went to the composer. The editor mounts only from that
    /// press, so it takes the focus as the reference's editor does (measured on the
    /// f870c41 oracle with @pierre/diffs 1.3.0-beta.10: a click below the last line focuses the
    /// contenteditable with the caret at the end of the last line, whatever the click's x; a click on
    /// a line puts the caret under it). A real press's point is kept (`watchPresses`): below the
    /// text the caret goes to the end, on a line to the character under it; an agent's plain tap
    /// carries no AppKit event, so its caret goes to the end (`tap … mouse at x y` is a real press
    /// the monitor sees since exact2 #186).
    static func focusFileEditor(_ element: ExactElement, _ view: NSTextView, attempts: Int = 20) {
        DispatchQueue.main.async { [weak element, weak view] in
            guard let element, element.isLive, let view else { return }
            guard let window = view.window else {
                if attempts > 0 { DispatchQueue.main.asyncAfter(deadline: .now() + 0.025) { focusFileEditor(element, view, attempts: attempts - 1) } }
                return
            }
            if window.firstResponder !== view {
                element.focus()
                if window.firstResponder !== view { window.makeFirstResponder(view) }
            }
            guard window.firstResponder === view else { return }
            view.setSelectedRange(NSRange(location: caret(in: view, press: recentPress(in: window)), length: 0))
        }
    }

    /// Where a press at `point` (window coordinates) puts the caret: the end below the text (or
    /// with no press), else the insertion point nearest it.
    static func caret(in view: NSTextView, press point: NSPoint?) -> Int {
        let end = (view.string as NSString).length
        guard let point, let manager = view.layoutManager, let container = view.textContainer else { return end }
        let local = view.convert(point, from: nil)
        guard view.bounds.contains(local) else { return end }
        manager.ensureLayout(for: container)
        let used = manager.usedRect(for: container).offsetBy(dx: view.textContainerOrigin.x, dy: view.textContainerOrigin.y)
        if view.isFlipped ? local.y >= used.maxY : local.y <= used.minY { return end }
        return min(end, view.characterIndexForInsertion(at: local))
    }

    /// The last real left press (an AppKit event; an agent's plain tap has none) in a window.
    private static var press: (window: NSWindow, point: NSPoint, at: TimeInterval)?
    private static var pressMonitor: Any?
    static func watchPresses() {
        guard pressMonitor == nil else { return }
        pressMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown]) { event in
            if let window = event.window { press = (window, event.locationInWindow, ProcessInfo.processInfo.systemUptime) }
            return event
        }
    }
    /// A press in `window` within the last second (the one that began editing), consumed once.
    static func recentPress(in window: NSWindow) -> NSPoint? {
        guard let last = press, last.window === window, ProcessInfo.processInfo.systemUptime - last.at < 1 else { return nil }
        press = nil
        return last.point
    }
    /// Tests stand in for a press.
    static func recordPress(_ window: NSWindow, at point: NSPoint) { press = (window, point, ProcessInfo.processInfo.systemUptime) }
}

enum T3AttachmentFiles {
    static let previewBytes = 1024 * 1024
    static let saveBytes = 64 * 1024 * 1024

    /// Only a signed asset route of an http(s) server: never a file URL or another path.
    static func assetURL(_ raw: Any?) -> URL? {
        guard let raw = raw as? String, let url = URL(string: raw), let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https", url.host != nil, url.user == nil, url.path.hasPrefix("/api/assets/") else { return nil }
        return url
    }

    /// `attachmentText` (the first megabyte as UTF-8 text) and `attachmentSave` of a signed asset `url`, or of bytes this
    /// device holds for a draft (AttachmentFilePreview's `file` Blob): a staged copy named by `staged` (`stagedFile`
    /// resolves it) or a folded paste's text as `base64`.
    static func perform(_ request: [String: Any], exportsRoot: URL?, stagedFile: (String) -> URL? = { _ in nil }, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let failure = { (message: String) in reply(["ok": true, "generation": generation, "value": ["ok": false, "message": message]]) }
        let text = request["op"] as? String == "attachmentText"
        if let id = request["staged"] as? String {
            guard let file = stagedFile(id) else { return failure(text ? "Could not load this file." : "The file could not be loaded. Try again.") }
            DispatchQueue.global(qos: .userInitiated).async {
                guard let handle = try? FileHandle(forReadingFrom: file) else { return failure("Could not load this file.") }
                defer { try? handle.close() }
                let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? Int) ?? 0
                if text {
                    let data = (try? handle.read(upToCount: previewBytes + 1)) ?? Data()
                    return decode(data, truncated: size > previewBytes, generation: generation, reply: reply, failure: failure)
                }
                guard size <= saveBytes else { return failure("The file is too large to save here.") }
                guard let data = try? handle.readToEnd() else { return failure("The file could not be loaded. Try again.") }
                save(data, request: request, exportsRoot: exportsRoot, generation: generation, reply: reply, failure: failure)
            }
            return
        }
        if let base64 = request["base64"] as? String {
            guard let data = Data(base64Encoded: base64) else { return failure("The file could not be loaded. Try again.") }
            if text { return decode(data, truncated: data.count > previewBytes, generation: generation, reply: reply, failure: failure) }
            guard data.count <= saveBytes else { return failure("The file is too large to save here.") }
            return save(data, request: request, exportsRoot: exportsRoot, generation: generation, reply: reply, failure: failure)
        }
        guard let url = assetURL(request["url"]) else { return failure("The attachment is unavailable.") }
        if text {
            var urlRequest = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
            urlRequest.setValue("bytes=0-\(previewBytes)", forHTTPHeaderField: "Range")
            URLSession.shared.dataTask(with: urlRequest) { data, response, error in
                if error != nil { return failure("Could not load this file.") }
                guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode), let data else { return failure("Could not load this file.") }
                let truncated = data.count > previewBytes || http.statusCode == 206 && total(http).map { $0 > previewBytes } == true
                decode(data, truncated: truncated, generation: generation, reply: reply, failure: failure)
            }.resume()
            return
        }
        URLSession.shared.dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 60)) { data, response, error in
            guard error == nil, let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode), let data else {
                return failure("The file could not be loaded. Try again.")
            }
            guard data.count <= saveBytes else { return failure("The file is too large to save here.") }
            save(data, request: request, exportsRoot: exportsRoot, generation: generation, reply: reply, failure: failure)
        }.resume()
    }

    /// decodeFilePreviewText: binary data and non-UTF-8 text are refused, not shown as replacement characters.
    private static func decode(_ data: Data, truncated: Bool, generation: Int, reply: @escaping ([String: Any]) -> Void, failure: (String) -> Void) {
        let bounded = data.prefix(previewBytes)
        if bounded.contains(0) { return failure("This file contains binary data and cannot be shown as text.") }
        var text = String(data: bounded, encoding: .utf8)
        if text == nil && truncated {
            // A cut can split one UTF-8 sequence; drop at most its three leading bytes.
            for drop in 1...3 where text == nil { text = String(data: bounded.dropLast(drop), encoding: .utf8) }
        }
        guard let text else { return failure("This file is not UTF-8 text. Open it in another app to view its contents.") }
        reply(["ok": true, "generation": generation, "value": ["ok": true, "text": text, "truncated": truncated]])
    }

    /// Save file: into the agent's exports, else where the person picks in a save panel.
    private static func save(_ data: Data, request: [String: Any], exportsRoot: URL?, generation: Int, reply: @escaping ([String: Any]) -> Void, failure: @escaping (String) -> Void) {
        let name = (request["name"] as? String ?? "attachment").replacingOccurrences(of: "/", with: "-").replacingOccurrences(of: ":", with: "-")
        let write = { (target: URL) in
            do {
                try FileManager.default.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
                try data.write(to: target, options: .atomic)
                reply(["ok": true, "generation": generation, "value": ["ok": true, "saved": true, "name": target.lastPathComponent]])
            } catch { failure("Could not save \(name).") }
        }
        if let exportsRoot { return write(exportsRoot.appendingPathComponent(name.isEmpty ? "attachment" : name)) }
        DispatchQueue.main.async {
            let panel = NSSavePanel()
            panel.nameFieldStringValue = name
            panel.canCreateDirectories = true
            guard panel.runModal() == .OK, let target = panel.url else {
                return reply(["ok": true, "generation": generation, "value": ["ok": true, "saved": false]])
            }
            DispatchQueue.global(qos: .userInitiated).async { write(target) }
        }
    }

    /// Content-Range: bytes 0-1048576/4194304 → 4194304.
    private static func total(_ response: HTTPURLResponse) -> Int? {
        guard let range = response.value(forHTTPHeaderField: "Content-Range"), let slash = range.lastIndex(of: "/") else { return nil }
        return Int(range[range.index(after: slash)...])
    }
}
