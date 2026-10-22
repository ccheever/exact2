// @exact/clipboard on Apple (LLP 1133.006): `write` with a `ClipboardItem`
// of `image/png` and `text/plain`, through `NSPasteboard` on macOS and
// `UIPasteboard` on iOS. The app's composed module calls it, outside the
// agent and, as its own substitute, under it (D8 item 4).
//
// - **Sanitized** (D1): the PNG is decoded and a new PNG of its pixels
//   written, so nothing else in the file is carried; an animated PNG writes
//   its first frame (PNGFrame.swift), as Chromium does.
// - **Bounds** (D2): a file at most 64 MiB (the host refuses a larger path),
//   bytes at most 2 MiB (the binding refuses more), at most 64,000,000
//   pixels by the header before anything is decoded, text at most 1 MiB.
// - **Order** (D2): a write whose sequence number is older than one already
//   taken is superseded (`AbortError`), and so is one a later write overtook
//   while it decoded.
// - **What is written** (D5): on macOS, what a hand-built AppKit app writes
//   when it copies an image, `writeObjects([NSImage])` (its TIFF), with
//   `public.png` beside it and `public.utf8-plain-text` for text; on iOS one
//   item with `public.png` and `public.utf8-plain-text`, no options.
// - **Under the agent** nothing reaches the person's pasteboard: the write
//   goes to a private named one, `dev.exact.agent.<pid>` (also `copyText`'s
//   there, `Session.swift`), unless `EXACT_DEVICE=real`. Decoding and bounds
//   are production's either way. The answer's `$state` is what
//   `state.extensions.clipboard` shows of the last write: its types, the
//   text cut to 200 characters, the image's size, byte count and SHA-256.
import CryptoKit
import Foundation
import ImageIO
#if os(macOS)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

public final class ClipboardModule {
    static let maxPixels = 64_000_000
    static let maxText = 1 << 20
    static let maxFile = 64 << 20

    private let log: (String) -> Void
    /// Writing the private pasteboard rather than the person's.
    private let substituting: Bool
    private var newest = 0

    public init(changed: @escaping (String) -> Void, agent: Bool, log: @escaping (String) -> Void) {
        self.log = log
        substituting = agent && ProcessInfo.processInfo.environment["EXACT_DEVICE"] != "real"
        if substituting { Self.released }
    }

    /// The pasteboard a drive writes instead of the person's, released when
    /// the process exits (a named pasteboard outlives its writer otherwise).
    static var privateName: String { "dev.exact.agent.\(getpid())" }
    private static let released: Void = {
        #if os(macOS)
        atexit { NSPasteboard(name: NSPasteboard.Name("dev.exact.agent.\(getpid())")).releaseGlobally() }
        #endif
    }()

    public func later(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let seq = (args["$sequence"] as? [String: Any])?["seq"] as? Int ?? 0
        let items: [[String: Any]]
        switch name {
        case "write": items = args["items"] as? [[String: Any]] ?? []
        case "writeText": items = [["text/plain": args["text"] ?? ""]]
        default: return reply(["error": "NotSupportedError"])
        }
        // A binding's sequence starts at 1 again when its runtime reloads.
        if seq == 1 { newest = 0 }
        guard seq > newest else { return reply(refuse("AbortError", "a later write superseded it")) }
        newest = seq
        // Chrome's refusals, in its words (measured: HeadlessChrome 156).
        if items.isEmpty { return reply(["$state": state(nil)]) }
        guard items.count == 1, let item = items.first else {
            return reply(refuse("NotAllowedError", "Support for multiple ClipboardItems is not implemented."))
        }
        if let other = item.keys.sorted().first(where: { $0 != "text/plain" && $0 != "image/png" }) {
            return reply(refuse("NotAllowedError", "Type \(other) not supported on write."))
        }
        let text = item["text/plain"] as? String
        if let text, text.utf8.count > Self.maxText {
            return reply(refuse("DataError", "text/plain: \(text.utf8.count) bytes is more than 1 MiB"))
        }
        guard let png = item["image/png"] else { return finish(seq, text: text, image: nil, reply) }
        // Decoding is off the main thread; the write comes back to it.
        DispatchQueue.global(qos: .userInitiated).async { [self] in
            let decoded = Result { try sanitize(png) }
            DispatchQueue.main.async { [self] in
                switch decoded {
                case .success(let image): finish(seq, text: text, image: image, reply)
                case .failure(let e as Refusal): reply(refuse("DataError", e.message))
                case .failure(let e): reply(refuse("DataError", "\(e)"))
                }
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }
    public func release(_ name: String) {}

    struct Refusal: Error { let message: String }
    struct Image { let png: Data; let width: Int; let height: Int; let cgImage: CGImage }

    private func refuse(_ name: String, _ message: String) -> [String: Any] {
        log("clipboard: write refused: \(name) (\(message))")
        return ["error": name, "message": "clipboard.write: \(message)"]
    }

    /// The item's PNG, decoded and written again.
    private func sanitize(_ value: Any) throws -> Image {
        let bytes: Data
        if let object = value as? [String: Any], let b64 = object["$bytes"] as? String, let data = Data(base64Encoded: b64) {
            bytes = data
        } else if let object = value as? [String: Any], let file = object["$file"] as? String {
            let size = (try? FileManager.default.attributesOfItem(atPath: file)[.size] as? Int) ?? 0
            guard size <= Self.maxFile else { throw Refusal(message: "image/png: \(size) bytes is more than 64 MiB") }
            guard let data = FileManager.default.contents(atPath: file) else { throw Refusal(message: "image/png: the file did not read") }
            bytes = data
        } else {
            throw Refusal(message: "Failed to read or decode ClipboardItemData for type image/png.")
        }
        guard let (width, height) = PNGFrame.size(bytes) else {
            throw Refusal(message: "Failed to read or decode ClipboardItemData for type image/png.")
        }
        guard width > 0, height > 0, width * height <= Self.maxPixels else {
            throw Refusal(message: "image/png: too large, \(width) × \(height) is more than 64,000,000 pixels")
        }
        guard let still = PNGFrame.first(bytes),
              let source = CGImageSourceCreateWithData(still as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetType(source) as String? == "public.png",
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw Refusal(message: "Failed to read or decode ClipboardItemData for type image/png.")
        }
        let out = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil) else {
            throw Refusal(message: "image/png: no PNG encoder")
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw Refusal(message: "image/png: the PNG did not encode") }
        return Image(png: out as Data, width: image.width, height: image.height, cgImage: image)
    }

    /// Write, unless a later write overtook this one while it decoded.
    private func finish(_ seq: Int, text: String?, image: Image?, _ reply: ([String: Any]) -> Void) {
        guard seq == newest || seq == 0 else { return reply(refuse("AbortError", "a later write superseded it")) }
        let written = write(text: text, image: image)
        if let image {
            log("clipboard: wrote image/png \(image.width)×\(image.height) (\(image.png.count.formatted()) bytes)\(substituting ? " to the drive's pasteboard" : "")")
        } else if let text {
            log("clipboard: wrote text/plain (\(text.count) characters)\(substituting ? " to the drive's pasteboard" : "")")
        }
        var last = state((text, image))
        last["written"] = written
        reply(["$state": last])
    }

    /// The pasteboard's types after the write.
    private func write(text: String?, image: Image?) -> [String] {
        #if os(macOS)
        let pasteboard = substituting ? NSPasteboard(name: NSPasteboard.Name(Self.privateName)) : NSPasteboard.general
        pasteboard.clearContents()
        if let image {
            pasteboard.writeObjects([NSImage(cgImage: image.cgImage, size: NSSize(width: image.width, height: image.height))])
            pasteboard.setData(image.png, forType: .png)
        }
        if let text { pasteboard.setString(text, forType: .string) }
        return pasteboard.types?.map(\.rawValue) ?? []
        #elseif os(iOS)
        let pasteboard = substituting ? (UIPasteboard(name: UIPasteboard.Name(Self.privateName), create: true) ?? .general) : UIPasteboard.general
        var item: [String: Any] = [:]
        if let image { item["public.png"] = image.png }
        if let text { item["public.utf8-plain-text"] = text }
        pasteboard.setItems([item])
        return pasteboard.types
        #else
        return []
        #endif
    }

    private func state(_ last: (String?, Image?)?) -> [String: Any] {
        var out: [String: Any] = ["substituting": substituting]
        guard let (text, image) = last else { out["last"] = ["types": [String]()]; return out }
        var write: [String: Any] = ["types": [image != nil ? "image/png" : nil, text != nil ? "text/plain" : nil].compactMap { $0 }]
        if let text { write["text"] = String(text.prefix(200)) }
        if let image {
            let digest = SHA256.hash(data: image.png).map { String(format: "%02x", $0) }.joined()
            write["image"] = ["width": image.width, "height": image.height, "bytes": image.png.count, "sha256": digest]
        }
        out["last"] = write
        return out
    }
}
