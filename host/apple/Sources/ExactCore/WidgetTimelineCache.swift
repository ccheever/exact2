// @ref LLP 1126.000 WP5, LLP 1126 D8 — account data expires on a store change.
import Foundation

/// A refresh captures the app's store counter before reading any account data.
/// The same identity scopes in-flight sharing and the extension's persisted answer.
public struct WidgetTimelineCache: Hashable {
    private let file: URL
    private let counterFile: URL
    /// Nil when the counter file is unreadable: the refresh still answers from
    /// the live store, but nothing is kept or reused under an unknown revision.
    private let counter: UInt64?
    private enum Failure: Error { case storeChanged }
    private struct Saved: Codable {
        let counter: UInt64
        let bytes: Data
    }

    public init(root: URL, kind: String, generation: String) throws {
        file = root.appendingPathComponent("Library/Caches/widget-timelines/\(generation)/\(kind).json")
        counterFile = root.appendingPathComponent("widget-store-generation")
        counter = try? Self.readCounter(counterFile)
    }

    private static func readCounter(_ file: URL) throws -> UInt64 {
        let text: String
        do { text = try String(contentsOf: file, encoding: .utf8) }
        catch CocoaError.fileReadNoSuchFile { return 0 }
        guard let counter = UInt64(text) else { throw CocoaError(.fileReadCorruptFile) }
        return counter
    }

    /// Recheck after refresh and before delivery: a write can race either path.
    /// An unreadable counter cannot authorize account content: it stays current
    /// only while it stays unreadable, and the app's next write makes it a number.
    public var isCurrent: Bool { (try? Self.readCounter(counterFile)) == counter }

    /// Only validated timeline bytes belong here. A failure may reuse an answer
    /// from this store revision, never one written before an account switch.
    public func resolve(_ refresh: () throws -> Data) throws -> Data {
        guard let counter else {
            guard isCurrent else { throw Failure.storeChanged }
            let bytes = try refresh()
            guard isCurrent else { throw Failure.storeChanged }
            return bytes
        }
        let bytes: Data
        do {
            guard isCurrent else { throw Failure.storeChanged }
            bytes = try refresh()
            guard isCurrent else { throw Failure.storeChanged }
        } catch {
            // This cache is separate from the app's store and update pointer.
            // A fresh process must prove the saved counter matches as well.
            let saved = try? JSONDecoder().decode(Saved.self, from: Data(contentsOf: file))
            if let saved, saved.counter < counter { try? FileManager.default.removeItem(at: file) }
            guard isCurrent, let saved, saved.counter == counter, isCurrent else { throw error }
            return saved.bytes
        }
        // Keeping the answer is best effort: a full or purged cache must not
        // turn a good refresh into a failure.
        if let saved = try? JSONEncoder().encode(Saved(counter: counter, bytes: bytes)) {
            try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? saved.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
        }
        guard isCurrent else { throw Failure.storeChanged }
        return bytes
    }
}
