// LLP 1133.024 D4, D9: the share extension's inbox, written through a stub
// extension context, as the app's claim reads it.
import Foundation
import UniformTypeIdentifiers
import XCTest
@testable import ExactShare
#if canImport(AppKit) && !targetEnvironment(macCatalyst)
import AppKit
#endif

final class ShareExtensionTests: XCTestCase {
    private var scratch: URL!

    override func setUpWithError() throws {
        scratch = FileManager.default.temporaryDirectory.appendingPathComponent("share-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws { try? FileManager.default.removeItem(at: scratch) }

    private let info = ShareTargetInfo(group: "group.com.exact.test", appId: "com.exact.test", mapped: ["title", "text"],
                                       members: [(name: "files", types: ["public.plain-text", "public.image"])])

    /// A note, a link, a file no member accepts and the sheet's own text: the note is stored under a counter's
    /// name, the link is `url`, the odd file is counted, and the text is not an attachment.
    private func items() throws -> [NSExtensionItem] {
        let note = scratch.appendingPathComponent("packing.txt"), odd = scratch.appendingPathComponent("data.bin")
        try "water, map".write(to: note, atomically: true, encoding: .utf8)
        try Data([0, 1, 2, 3]).write(to: odd)
        let item = NSExtensionItem()
        item.attributedTitle = NSAttributedString(string: "Trip")
        item.attributedContentText = NSAttributedString(string: "Bring water")
        item.attachments = [NSItemProvider(contentsOf: note)!, NSItemProvider(object: URL(string: "https://x.test/a")! as NSURL),
                            NSItemProvider(contentsOf: odd)!, NSItemProvider(object: "Bring water" as NSString)]
        return [item]
    }

    private func record(in container: URL) throws -> (URL, [String: Any]) {
        let shares = try FileManager.default.contentsOfDirectory(at: container.appendingPathComponent("shares"), includingPropertiesForKeys: nil)
        XCTAssertEqual(shares.count, 1)
        let data = try Data(contentsOf: shares[0].appendingPathComponent("share.json"))
        return (shares[0], try JSONSerialization.jsonObject(with: data) as! [String: Any])
    }

    func testWriterStoresAcceptedFilesUnderItsOwnNames() throws {
        let writer = try XCTUnwrap(ShareInboxWriter(info: info, container: scratch))
        let done = expectation(description: "taken")
        writer.take(try items()) { done.fulfill() }
        wait(for: [done], timeout: 10)
        try writer.finish(title: "Trip", text: "Bring water")
        let (dir, record) = try record(in: scratch)
        XCTAssertEqual(record["v"] as? Int, 1)
        XCTAssertEqual(record["title"] as? String, "Trip")
        XCTAssertEqual(record["text"] as? String, "Bring water")
        XCTAssertEqual(record["url"] as? String, "https://x.test/a")
        let files = try XCTUnwrap(record["files"] as? [[String: String]])
        XCTAssertEqual(files.count, 1)
        XCTAssertEqual(files.first?["file"], "1.txt")
        XCTAssertEqual(files.first?["name"], "packing.txt")
        XCTAssertEqual(files.first?["type"], "text/plain")
        XCTAssertEqual(files.first?["member"], "files")
        XCTAssertEqual(record["dropped"] as? [String: Int], ["files": 1])
        XCTAssertEqual(try String(contentsOf: dir.appendingPathComponent("1.txt"), encoding: .utf8), "water, map")
        XCTAssertFalse(FileManager.default.fileExists(atPath: dir.appendingPathComponent("share.json.partial").path))
    }

    func testDiscardLeavesNothing() throws {
        let writer = try XCTUnwrap(ShareInboxWriter(info: info, container: scratch))
        writer.discard()
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: scratch.appendingPathComponent("shares").path), [])
    }

    #if canImport(AppKit) && !targetEnvironment(macCatalyst)
    /// The Mac's sheet, through a stub context: Post writes the inbox with the sheet's edited text and completes.
    func testMacSheetPostsThroughTheContext() throws {
        final class StubContext: NSExtensionContext {
            var items: [Any] = []
            var completed: XCTestExpectation?
            override var inputItems: [Any] { items }
            override func completeRequest(returningItems: [Any]?, completionHandler: ((Bool) -> Void)? = nil) { completed?.fulfill() }
        }
        let context = StubContext()
        context.items = try items()
        context.completed = expectation(description: "completed")
        let sheet = ExactShareViewController()
        sheet.info = info
        sheet.container = scratch
        sheet.stubContext = context
        _ = sheet.view
        sheet.viewDidLoad()
        XCTAssertEqual(sheet.text, "Bring water")
        sheet.text = "Bring water and a hat"
        sheet.post(nil)
        wait(for: [context.completed!], timeout: 10)
        let (_, record) = try record(in: scratch)
        XCTAssertEqual(record["text"] as? String, "Bring water and a hat")
        XCTAssertEqual((record["files"] as? [[String: String]])?.count, 1)
    }
    #endif
}
