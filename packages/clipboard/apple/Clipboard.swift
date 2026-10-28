// @exact/clipboard on Apple (LLP 1133.006): the Async Clipboard API's
// `write` with a `ClipboardItem` of `image/png` and `text/plain`.
//
// - The image is sanitized, as the specification asks and Chrome does: the
//   PNG is decoded and a new PNG of its pixels written, so nothing else in
//   the file is carried (D1). An animated PNG writes frame 0 of its
//   animation, which this file cuts out itself: ImageIO's frame 0 of an APNG
//   whose default image is not part of the animation is a later frame
//   (macOS 27), where Chromium's is the first `fcTL`'s.
// - macOS writes what a hand-built AppKit app writes when it copies an
//   image, `writeObjects([NSImage])`, and `public.png` beside it (D5); iOS
//   one item of `public.png` and `public.utf8-plain-text` (D5).
// - Under the agent it is its own substitute (D8 item 4, §7): it writes a
//   private pasteboard named for the session, by the rule and name the
//   host's `copyText` uses (AgentPasteboard.swift), unless `--bind
//   clipboard` or `EXACT_DEVICE=real` asks for the person's. Where the
//   private pasteboard cannot be made the write refuses.
// - A later write supersedes one still decoding, which fails `AbortError` (D2).
import CryptoKit
import ExactPackage
import Foundation
import ImageIO
import UniformTypeIdentifiers
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class ClipboardModule: ExactPackage {
    /// D2's bounds: an image file by path, as bytes, its decoded pixels, and the text.
    static let mostFile = 64 * 1024 * 1024, mostBytes = 2 * 1024 * 1024, mostPixels = 64_000_000, mostText = 1024 * 1024
    /// The types this module writes (D1).
    static let types: Set<String> = ["text/plain", "image/png"]

    let context: ExactPackageContext
    /// The number of the latest write: one still decoding whose number is older lands nothing.
    private var latest = 0
    /// The write in flight, by its number, and the runner's number for its call.
    private var inFlight: (number: Int, request: Int?, reply: ([String: Any]) -> Void)?
    private var writes = 0
    private let queue = DispatchQueue(label: "exact.clipboard", qos: .userInitiated)

    public init(context: ExactPackageContext) { self.context = context }

    func say(_ line: String) { context.log("clipboard: \(line)") }

    /// The pasteboard written: the person's, or under the agent the session's own (§7).
    var substituting: Bool { context.agent && ProcessInfo.processInfo.environment["EXACT_DEVICE"] != "real" }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "write":
            guard let items = args["data"] as? [Any] else { return reply(refused("TypeError", "write takes a list of ClipboardItem")) }
            write(items, reply: reply)
        case "writeText":
            guard let text = args["data"] as? String else { return reply(refused("TypeError", "writeText takes a string")) }
            write([["text/plain": text]], reply: reply)
        default:
            reply(["error": "NotSupportedError"])
        }
    }

    public func cancel(_ request: Int) {
        guard let w = inFlight, w.request == request else { return }
        latest += 1
        inFlight = nil
        w.reply(refused("AbortError", "the call was let go"))
    }

    public func destroy(_ done: @escaping () -> Void) {
        if let w = inFlight { inFlight = nil; latest += 1; w.reply(refused("AbortError", "the session ended")) }
        done()
    }

    /// A refusal as the binding rejects it, journaled.
    func refused(_ code: String, _ message: String) -> [String: Any] {
        say("write refused: \(code) (\(message))")
        return ["error": code, "message": message]
    }

    private func write(_ items: [Any], reply: @escaping ([String: Any]) -> Void) {
        // Chrome resolves an empty write and leaves the clipboard as it was.
        guard let item = items.first else { return reply([:]) }
        guard items.count == 1 else { return reply(refused("NotAllowedError", "Support for multiple ClipboardItems is not implemented.")) }
        guard let record = item as? [String: Any], !record.isEmpty else { return reply(refused("TypeError", "a ClipboardItem has at least one type")) }
        if let other = record.keys.sorted().first(where: { !Self.types.contains($0) }) {
            return reply(refused("NotAllowedError", "Type \(other) not supported on write."))
        }
        var text: String?
        if let t = record["text/plain"] {
            guard let s = t as? String else { return reply(refused("TypeError", "text/plain is a string")) }
            guard s.utf8.count <= Self.mostText else { return reply(refused("DataError", "text/plain is over 1 MiB")) }
            text = s
        }
        var source: Data?
        switch record["image/png"] {
        case nil: break
        case let path as String:
            guard let url = context.file(path) else { return reply(refused("NotFoundError", "no file at \(path)")) }
            switch Self.contents(url, path) {
            case .success(let data): source = data
            case .failure(let why): return reply(refused(why.code, why.message))
            }
        case let wire as [String: Any]:
            guard let b64 = wire["base64"] as? String, let data = Data(base64Encoded: b64) else { return reply(refused("TypeError", "image/png is an app:/ path or bytes")) }
            guard data.count <= Self.mostBytes else { return reply(refused("DataError", "image/png is over 2 MiB as bytes: pass an app:/ path")) }
            source = data
        default:
            return reply(refused("TypeError", "image/png is an app:/ path or bytes"))
        }
        // This write supersedes any still decoding (D2).
        latest += 1
        let number = latest
        if let w = inFlight { inFlight = nil; w.reply(refused("AbortError", "a later write superseded it")) }
        inFlight = (number, context.request, reply)
        let land = { [weak self] (image: Sanitized?) in
            guard let self, self.latest == number, let w = self.inFlight, w.number == number else { return }
            self.inFlight = nil
            w.reply(self.land(text: text, image: image))
        }
        guard let source else { return land(nil) }
        queue.async {
            let result = Self.sanitized(source)
            DispatchQueue.main.async { [weak self] in
                switch result {
                case .success(let image): land(image)
                case .failure(let why):
                    guard let self, self.latest == number, let w = self.inFlight, w.number == number else { return }
                    self.inFlight = nil
                    w.reply(self.refused("DataError", why.description))
                }
            }
        }
    }

    /// The pasteboard written now, and what state discloses of the write.
    private func land(text: String?, image: Sanitized?) -> [String: Any] {
        guard let board = paste(text: text, image: image) else {
            return refused("UnknownError", "the session's private pasteboard could not be made")
        }
        writes += 1
        var types: [String] = []
        if text != nil { types.append("text/plain") }
        if image != nil { types.append("image/png") }
        var shown: [String: Any] = ["types": types, "pasteboard": board.name, "platformTypes": board.types]
        if let text { shown["text"] = String(text.prefix(200)) }
        if let image {
            shown["image"] = ["width": image.width, "height": image.height, "bytes": image.png.count,
                              "sha256": SHA256.hash(data: image.png).map { String(format: "%02x", $0) }.joined()]
            let bytes = NumberFormatter.localizedString(from: image.png.count as NSNumber, number: .decimal)
            say("wrote image/png \(image.width)×\(image.height) (\(bytes) bytes)\(text == nil ? "" : " and text/plain")")
        } else {
            say("wrote text/plain (\(text?.utf8.count ?? 0) bytes)")
        }
        // The agent's view of the module (`state.extensions.clipboard.implementation`, §7 item 3).
        context.state(["writes": writes, "lastWrite": shown])
        return [:]
    }

    // MARK: the pasteboard (D5)

    /// The pasteboard's name under the agent: the session's, which the host's `copyText` writes too.
    var privateName: String { "exact.agent.clipboard.\(context.hostSession)" }

    #if os(macOS)
    private func paste(text: String?, image: Sanitized?) -> (name: String, types: [String])? {
        let board = substituting ? NSPasteboard(name: NSPasteboard.Name(privateName)) : NSPasteboard.general
        board.clearContents()
        if let image {
            // As a hand-built AppKit app copies an image, then the PNG the web names (D5).
            let picture = NSImage(cgImage: image.cgImage, size: NSSize(width: image.width, height: image.height))
            board.writeObjects([picture])
            board.addTypes([.png], owner: nil)
            board.setData(image.png, forType: .png)
        }
        if let text {
            if image == nil { board.setString(text, forType: .string) } else {
                board.addTypes([.string], owner: nil)
                board.setString(text, forType: .string)
            }
        }
        return (board.name.rawValue, (board.types ?? []).map(\.rawValue))
    }
    #else
    /// Nil when the session's private pasteboard could not be made: never the person's in its place.
    private func paste(text: String?, image: Sanitized?) -> (name: String, types: [String])? {
        guard let board = substituting ? UIPasteboard(name: UIPasteboard.Name(privateName), create: true) : UIPasteboard.general else { return nil }
        var item: [String: Any] = [:]
        if let image { item[UTType.png.identifier] = image.png }
        if let text { item[UTType.utf8PlainText.identifier] = text }
        board.setItems([item], options: [:])
        return (board.name.rawValue, board.types)
    }
    #endif

    // MARK: sanitizing (D1, D2)

    struct Sanitized {
        let png: Data
        let cgImage: CGImage
        let width: Int, height: Int
    }

    struct Refusal: Error, CustomStringConvertible { let description: String }
    struct FileRefusal: Error { let code: String, message: String }

    /// The file at `url`, opened once, so what is checked is what is read: the
    /// file a link names, a regular one (a device's size says nothing of what
    /// it gives), at most 64 MiB by its size and by what is read of it.
    static func contents(_ url: URL, _ path: String) -> Result<Data, FileRefusal> {
        let fd = open(url.path, O_RDONLY)
        guard fd >= 0 else {
            return .failure(errno == ENOENT ? FileRefusal(code: "NotFoundError", message: "no file at \(path)")
                : FileRefusal(code: "NotReadableError", message: "\(path) could not be read"))
        }
        let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        var info = stat()
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG else {
            return .failure(FileRefusal(code: "NotReadableError", message: "\(path) is not a file"))
        }
        let tooLarge = FileRefusal(code: "DataError", message: "too large: \(path) is over 64 MiB")
        guard Int(info.st_size) <= mostFile else { return .failure(tooLarge) }
        guard let data = try? handle.read(upToCount: mostFile + 1) ?? Data() else {
            return .failure(FileRefusal(code: "NotReadableError", message: "\(path) could not be read"))
        }
        return data.count <= mostFile ? .success(data) : .failure(tooLarge)
    }

    /// A new PNG of `data`'s pixels, frame 0 of an animation, or why not.
    static func sanitized(_ data: Data) -> Result<Sanitized, Refusal> {
        guard let still = firstFrame(data) else { return .failure(Refusal(description: "not a PNG")) }
        // The header's size is held to the bound before anything is decoded (D2).
        let side = { (at: Int) in still[still.startIndex + at..<still.startIndex + at + 4].reduce(0) { $0 << 8 | Int($1) } }
        guard still.count >= 24, String(decoding: still[still.startIndex + 12..<still.startIndex + 16], as: UTF8.self) == "IHDR" else {
            return .failure(Refusal(description: "not a PNG"))
        }
        // A side past the PNG specification's 2³¹ − 1 is no PNG, and the product is taken without overflow.
        let (hw, hh) = (side(16), side(20)), (pixels, over) = hw.multipliedReportingOverflow(by: hh)
        guard hw <= Int(Int32.max), hh <= Int(Int32.max) else { return .failure(Refusal(description: "not a PNG")) }
        guard !over, pixels <= mostPixels else { return .failure(Refusal(description: "too large: \(hw)×\(hh) is over 64,000,000 pixels")) }
        guard let source = CGImageSourceCreateWithData(still as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetType(source) as String? == UTType.png.identifier else {
            return .failure(Refusal(description: "not a PNG"))
        }
        let header = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        let w = header?[kCGImagePropertyPixelWidth] as? Int ?? 0, h = header?[kCGImagePropertyPixelHeight] as? Int ?? 0
        guard w > 0, h > 0 else { return .failure(Refusal(description: "not a PNG")) }
        let (decoded, wide) = w.multipliedReportingOverflow(by: h)
        guard !wide, decoded <= mostPixels else { return .failure(Refusal(description: "too large: \(w)×\(h) is over 64,000,000 pixels")) }
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary) else {
            return .failure(Refusal(description: "not a PNG: it does not decode"))
        }
        let out = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil) else {
            return .failure(Refusal(description: "the PNG encoder is unavailable"))
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { return .failure(Refusal(description: "the PNG did not encode")) }
        return .success(Sanitized(png: out as Data, cgImage: image, width: image.width, height: image.height))
    }

    /// `data` as a still PNG of frame 0 of its animation (the APNG specification's first `fcTL`), or `data`
    /// itself when it is not animated; nil for what is not a PNG. The default image is frame 0 when an `fcTL`
    /// precedes its `IDAT`; otherwise frame 0's `fdAT`s become the still's `IDAT`s.
    static func firstFrame(_ data: Data) -> Data? {
        let signature = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        guard data.count > 8, data.prefix(8) == signature else { return nil }
        var chunks: [(type: String, body: Data)] = []
        var at = data.startIndex + 8
        while at + 12 <= data.endIndex {
            let length = data[at..<at + 4].reduce(0) { $0 << 8 | Int($1) }
            guard length >= 0, at + 12 + length <= data.endIndex else { return nil }
            let type = String(decoding: data[at + 4..<at + 8], as: UTF8.self)
            chunks.append((type, Data(data[at + 8..<at + 8 + length])))
            at += 12 + length
            if type == "IEND" { break }
        }
        guard chunks.contains(where: { $0.type == "acTL" }) else { return data }
        guard let idat = chunks.firstIndex(where: { $0.type == "IDAT" }), let fctl = chunks.firstIndex(where: { $0.type == "fcTL" }) else { return nil }
        var kept: [(type: String, body: Data)] = []
        if fctl < idat {
            kept = chunks.filter { !["acTL", "fcTL", "fdAT"].contains($0.type) }
        } else {
            kept = Array(chunks[..<idat]).filter { !["acTL", "fcTL"].contains($0.type) }
            for c in chunks[(fctl + 1)...] {
                if c.type == "fcTL" || c.type == "IEND" { break }
                if c.type == "fdAT", c.body.count > 4 { kept.append(("IDAT", c.body.dropFirst(4))) }
            }
            kept.append(("IEND", Data()))
        }
        var out = signature
        for c in kept { out.append(chunk(c.type, c.body)) }
        return out
    }

    static func chunk(_ type: String, _ body: Data) -> Data {
        var out = Data()
        let n = UInt32(body.count)
        out.append(contentsOf: [UInt8(n >> 24), UInt8(n >> 16 & 255), UInt8(n >> 8 & 255), UInt8(n & 255)])
        let typed = Data(type.utf8) + body
        out.append(typed)
        let c = crc32(typed)
        out.append(contentsOf: [UInt8(c >> 24), UInt8(c >> 16 & 255), UInt8(c >> 8 & 255), UInt8(c & 255)])
        return out
    }

    static let crcTable: [UInt32] = (0..<256).map { n -> UInt32 in
        var c = UInt32(n)
        for _ in 0..<8 { c = c & 1 != 0 ? 0xEDB88320 ^ (c >> 1) : c >> 1 }
        return c
    }

    static func crc32(_ data: Data) -> UInt32 {
        var c: UInt32 = 0xFFFFFFFF
        for b in data { c = crcTable[Int((c ^ UInt32(b)) & 0xFF)] ^ (c >> 8) }
        return c ^ 0xFFFFFFFF
    }
}
