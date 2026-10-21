// A paste's files (DOM's `clipboardData.files`): what the pasteboard holds
// beside its text, each minted a `doc:` handle for this session (LLP
// 1069.010 D1) and heard in the `ClipboardEvent`'s `files` (ABI kind 57).
// File URLs come first, folders included (a Finder copy, which also carries
// the file's icon as an image and its name as text: the file is what is
// pasted, not its icon). With none, image data (a screenshot copied to the
// clipboard, an image copied from a page) becomes one PNG, `image.png`, as
// Chrome names a pasted image's file; a TIFF is converted.
import Foundation
#if os(macOS)
import AppKit
typealias Pasteboard = NSPasteboard
#elseif os(iOS)
import UIKit
typealias Pasteboard = UIPasteboard
#else
import UIKit
/// tvOS has no pasteboard.
final class Pasteboard {}
#endif

extension NodeView {
    /// A paste heard here: `text`, else the pasteboard's, and `board`'s files.
    func sendPaste(text: String?, board: Pasteboard?) {
        guard let presenter else { return }
        let files = board.map { Self.pastedFiles($0, in: presenter.session) } ?? []
        let text = text ?? board.flatMap(Self.plainText) ?? ""
        if files.isEmpty { presenter.clipboard(id, 34, text); return }
        presenter.clipboard(id, 57, "\(files.count)\n" + files.joined(separator: "\n") + "\n" + text)
    }

    static func plainText(_ board: Pasteboard) -> String? {
        #if os(macOS)
        board.string(forType: .string)
        #elseif os(iOS)
        board.string
        #else
        nil
        #endif
    }

    /// The files on `board`, minted: its file URLs, else its image as a PNG.
    static func pastedFiles(_ board: Pasteboard, in session: ExactSession?) -> [String] {
        guard let session else { return [] }
        let claim = pasteClaim(board)
        if !claim.urls.isEmpty { return claim.urls.compactMap { session.mintDocument($0) } }
        guard let png = claim.png, let url = writePastedImage(png) else { return [] }
        let folder = url.deletingLastPathComponent()
        guard let doc = session.mintDocument(url) else { try? FileManager.default.removeItem(at: folder); return [] }
        PastedImages.held(folder, by: session)
        return [doc]
    }

    /// What a paste from `board` carries as files: its file URLs, or with
    /// none its image's PNG data.
    static func pasteClaim(_ board: Pasteboard) -> (urls: [URL], png: Data?) {
        #if os(macOS)
        let urls = board.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        if !urls.isEmpty { return (urls, nil) }
        if let png = board.data(forType: .png) { return ([], png) }
        let tiff = board.data(forType: .tiff).flatMap { NSBitmapImageRep(data: $0)?.representation(using: .png, properties: [:]) }
        return ([], tiff)
        #elseif os(iOS)
        let urls = (board.urls ?? []).filter(\.isFileURL)
        if !urls.isEmpty { return (urls, nil) }
        // `hasImages` asks without reading, so a text paste decodes nothing.
        return ([], board.hasImages ? board.image?.pngData() : nil)
        #else
        return ([], nil)
        #endif
    }

    #if os(macOS)
    /// Whether `board` holds what a paste carries as files (`pasteClaim`),
    /// asked without reading it, as a menu validates.
    static func holdsFiles(_ board: NSPasteboard) -> Bool {
        board.canReadObject(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) || board.availableType(from: [.png, .tiff]) != nil
    }
    #endif

    /// The pasted image's bytes in a folder of its own under the temporary
    /// directory, as `image.png`.
    static func writePastedImage(_ png: Data) -> URL? {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("exact-pasted", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let file = folder.appendingPathComponent("image.png")
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try png.write(to: file, options: .atomic)
            return file
        } catch {
            try? FileManager.default.removeItem(at: folder)
            return nil
        }
    }
}

/// The folders a session's pasted images were written to: removed when its
/// handles go (`forgetDocuments`), which is when nothing can read them.
enum PastedImages {
    nonisolated(unsafe) private static var folders: [ObjectIdentifier: [URL]] = [:]
    static func held(_ folder: URL, by session: ExactSession) { folders[ObjectIdentifier(session), default: []].append(folder) }
    static func forget(_ session: ExactSession) {
        for folder in folders.removeValue(forKey: ObjectIdentifier(session)) ?? [] { try? FileManager.default.removeItem(at: folder) }
    }
}

extension Agent {
    /// The driver's paste of files (`type <id> paste --file <path…>`, or
    /// `--image <path>`: an image's data with no file) on a pasteboard of its
    /// own, released after `body`; the general pasteboard is never touched.
    func withAgentPasteboard<T>(_ req: [String: Any], _ body: (Pasteboard?) -> T) -> T {
        let files = (req["files"] as? [String] ?? []).map { URL(fileURLWithPath: $0) }
        let image = (req["image"] as? String).flatMap { FileManager.default.contents(atPath: $0) }
        guard !files.isEmpty || image != nil else { return body(nil) }
        #if os(macOS)
        let board = NSPasteboard(name: .init("exact-agent-\(UUID().uuidString)"))
        defer { board.releaseGlobally() }
        board.clearContents()
        var items: [NSPasteboardWriting] = files.map { $0 as NSURL }
        if let image, let png = NSBitmapImageRep(data: image)?.representation(using: .png, properties: [:]) {
            let item = NSPasteboardItem()
            item.setData(png, forType: .png)
            items.append(item)
        }
        board.writeObjects(items)
        return body(board)
        #elseif os(iOS)
        let board = UIPasteboard.withUniqueName()
        defer { UIPasteboard.remove(withName: board.name) }
        if !files.isEmpty { board.urls = files } else if let image { board.image = UIImage(data: image) }
        return body(board)
        #else
        return body(nil)
        #endif
    }
}
