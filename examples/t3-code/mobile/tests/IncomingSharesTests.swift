// Compile with modules/apple/T3MobileIncomingShares.swift and run the executable.
import Foundation

@main
struct IncomingSharesTests {
    static func main() throws {
        typealias Inbox = T3MobileIncomingShares
        typealias Payload = Inbox.Payload
        let fm = FileManager.default
        let base = fm.temporaryDirectory.appendingPathComponent("t3-share-test-" + UUID().uuidString)
        try fm.createDirectory(at: base, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: base) }
        let sourceRoot = base.appendingPathComponent("app-group"), sender = base.appendingPathComponent("sender")
        try fm.createDirectory(at: sourceRoot, withIntermediateDirectories: true)
        try fm.createDirectory(at: sender, withIntermediateDirectories: true)
        let root = base.appendingPathComponent("inbox")
        var checks = 0
        var failures: [String] = []
        func check(_ value: Bool, _ message: String) { if !value { failures.append(message) }; checks += 1 }
        func file(_ name: String, _ bytes: Data = Data("actual bytes\n".utf8), owned: Bool = true) throws -> URL {
            let url = (owned ? sourceRoot : sender).appendingPathComponent(name)
            try bytes.write(to: url); return url
        }
        func payload(_ url: URL, kind: String = "file", mime: String = "text/plain", name: String? = nil) -> Payload {
            Payload(shareType: kind, mimeType: mime, value: url.absoluteString, originalName: name)
        }
        let text = Payload(shareType: "text", mimeType: nil, value: "Fix this", originalName: nil)
        let expectedHash = "share-cc66a2fdf6bfd795a5c71df58bd987ebd345580e3625696d09bf8266b36f39ea"
        check(try Inbox.identity([text]) == expectedHash, "fingerprint agrees with pinned JavaScript JSON/SHA-256")
        let unicode = Payload(shareType: "text", mimeType: "text/plain", value: "héllo/😃\u{2028}\u{2029}\n\t\"\\", originalName: nil)
        check(try Inbox.identity([unicode]) == "share-0ccf0884d8888959a77a488d300a5fa8d02b73031e1f378b14399abfb06275b2", "fingerprint agrees for Unicode, separators, slashes and control escapes")
        let urlText = Payload(shareType: "url", mimeType: nil, value: " https://example.test/a ", originalName: nil)
        check(try Inbox.identity([text, urlText]) != Inbox.identity([urlText, text]), "payload order is identity")
        check(try Inbox.identity([text]) == Inbox.identity([Payload(shareType: "text", mimeType: nil, value: text.value, originalName: "different")]), "display name is not identity")
        let inbox = Inbox(directory: root, cleanupRoots: [sourceRoot])
        let image = try file("image.png", Data([137, 80, 78, 71])), generic = try file("file.png")
        let external = try file("sender.txt", owned: false)
        let raw = [text, urlText, text, payload(image, kind: "image", mime: "IMAGE/PNG"), payload(generic, mime: "image/png"), payload(external)]
        var acked = 0
        let entry = try inbox.ingest(raw) { id in
            check(try! inbox.entries().contains { $0.id == id }, "write precedes ACK")
            acked += 1
        }!
        check(entry.text == "Fix this\n\nhttps://example.test/a", "trim and dedupe text and URL")
        check(entry.attachments.map(\.kind) == ["image", "file", "file"], "generic images stay files")
        check(entry.attachments[0].mimeType == "image/png", "normalize actual MIME")
        check(!fm.fileExists(atPath: image.path) && !fm.fileExists(atPath: generic.path), "clean only owned source copies after durable write")
        check(fm.fileExists(atPath: external.path), "preserve sender open-in-place file")
        check(try Data(contentsOf: inbox.attachmentURL(shareID: entry.id, attachmentID: entry.attachments[0].id)) == Data([137, 80, 78, 71]), "original image bytes retained")
        let replay = try inbox.ingest(raw) { _ in acked += 1 }!
        check(replay.attachments.map(\.id) == entry.attachments.map(\.id) && acked == 2, "replay reuses committed byte owners even after source cleanup")
        check(try Inbox(directory: root).entries().count == 1, "cold inbox loads committed entry")
        let encoded = String(decoding: try JSONEncoder().encode(entry), as: UTF8.self)
        check(!encoded.contains("file:") && !encoded.contains(base.path), "no raw paths escape native")

        let failureSource = try file("write-failure.txt")
        let failedRoot = base.appendingPathComponent("failed")
        let failed = Inbox(directory: failedRoot, cleanupRoots: [sourceRoot], write: { _, _ in throw CocoaError(.fileWriteOutOfSpace) })
        do { _ = try failed.ingest([payload(failureSource)]) { _ in preconditionFailure("ACK before failed write") }; preconditionFailure("expected write failure") }
        catch { checks += 1 }
        check(fm.fileExists(atPath: failureSource.path), "failed write preserves native handoff")
        check(try failed.entries().isEmpty, "failed write has no committed inbox item")
        check(try fm.contentsOfDirectory(atPath: failedRoot.path).isEmpty, "failed write rolls back byte copies")

        let ackSource = try file("ack-failure.txt"), ackPayload = [payload(ackSource)]
        do { _ = try inbox.ingest(ackPayload) { _ in throw CocoaError(.fileWriteUnknown) }; preconditionFailure("expected ACK failure") }
        catch { checks += 1 }
        check(!fm.fileExists(atPath: ackSource.path), "ACK failure still has durable imported bytes")
        let recovered = try inbox.ingest(ackPayload) { _ in acked += 1 }!
        check(try Data(contentsOf: inbox.attachmentURL(shareID: recovered.id, attachmentID: recovered.attachments[0].id)) == Data("actual bytes\n".utf8), "recover crash after commit before ACK")

        let unsupported = try file("unsupported.heic"), empty = try file("empty.txt", Data()), huge = try file("huge.png", Data(repeating: 1, count: 10 * 1024 * 1024 + 1))
        let warned = try inbox.ingest([text, payload(unsupported, kind: "image", mime: "image/heic"), payload(empty), payload(huge, kind: "image", mime: "image/png")]) { _ in }!
        check(warned.attachments.isEmpty && warned.warnings.count == 3, "unsupported, empty, oversized produce warnings beside usable text")
        let nothing = try file("nothing.txt", Data())
        var unsupportedACK = false
        do { _ = try inbox.ingest([payload(nothing)]) { _ in unsupportedACK = true }; preconditionFailure("expected unsupported error") }
        catch { checks += 1 }
        check(unsupportedACK && !fm.fileExists(atPath: nothing.path), "unactionable share does not reopen forever")

        let many = try file("many.txt")
        let capped = try inbox.ingest(Array(repeating: payload(many), count: 102)) { _ in }!
        check(capped.attachments.count == 100 && capped.warnings.count == 1, "100 attachment cap and one warning")
        check(Set(capped.attachments.map(\.id)).count == 100, "repeated paths are distinct occurrences")
        check(!Inbox.ownedFile(sourceRoot, roots: [sourceRoot]), "never remove root itself")
        check(!Inbox.ownedFile(external, roots: [sourceRoot]), "refuse sibling root")
        let traversal = URL(string: sourceRoot.absoluteString + "/..%2Fsender/sender.txt")!
        check(!Inbox.ownedFile(traversal, roots: [sourceRoot]), "refuse decoded traversal")
        let symlink = sourceRoot.appendingPathComponent("escape")
        try fm.createSymbolicLink(at: symlink, withDestinationURL: sender)
        check(!Inbox.ownedFile(symlink.appendingPathComponent("sender.txt"), roots: [sourceRoot]), "symlink cannot authorize sender deletion")
        check(!Inbox.ownedFile(URL(string: "file://other" + sourceRoot.path + "/x")!, roots: [sourceRoot]), "refuse remote file host")
        check(Inbox.ownedFile(URL(fileURLWithPath: "/private" + failureSource.path), roots: [sourceRoot]), "private path alias contains same owned file")
        let lost = try inbox.attachmentURL(shareID: recovered.id, attachmentID: recovered.attachments[0].id)
        try fm.removeItem(at: lost)
        do { _ = try inbox.ingest(ackPayload) { _ in preconditionFailure("ACK for missing bytes") }; preconditionFailure("expected missing bytes refusal") }
        catch { checks += 1 }
        for failure in failures { print("FAIL: \(failure)") }
        print("Incoming shares: \(checks - failures.count)/\(checks) checks passed")
        if !failures.isEmpty { exit(1) }
    }
}
