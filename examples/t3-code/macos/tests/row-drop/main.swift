import AppKit
import ObjectiveC
import XCTest

// legacy-sidebar-full LS-3: a legacy sidebar row's file drop target (T3RowDrop.swift) with real AppKit. The
// target is AppKit's drag destination over the row although its `hitTest` gives every click to the row; a file
// drag rings it and a drop reports the row's key with the dropped files (folders let go, as the row's
// addFolders does nothing); the queue keeps drops in order per key, and taking a key's drops stages each file
// as the composer's Attach does (an image to a PNG draft, another file into staged/).

/// A drag carrying file URLs, as AppKit hands one to a destination.
final class FileDrag: NSObject, NSDraggingInfo {
    let draggingPasteboard: NSPasteboard
    init(_ urls: [URL]) {
        draggingPasteboard = NSPasteboard(name: NSPasteboard.Name("t3-row-drop-\(UUID().uuidString)"))
        draggingPasteboard.clearContents()
        draggingPasteboard.writeObjects(urls as [NSURL])
    }
    var draggingDestinationWindow: NSWindow? { nil }
    var draggingSourceOperationMask: NSDragOperation { .copy }
    var draggingLocation: NSPoint { .zero }
    var draggedImageLocation: NSPoint { .zero }
    var draggedImage: NSImage? { nil }
    var draggingSource: Any? { nil }
    var draggingSequenceNumber: Int { 1 }
    func slideDraggedImage(to screenPoint: NSPoint) {}
    override func namesOfPromisedFilesDropped(atDestination dropDestination: URL) -> [String]? { nil }
    var draggingFormation: NSDraggingFormation = .default
    var animatesToDestination = false
    var numberOfValidItemsForDrop = 1
    func enumerateDraggingItems(options enumOpts: NSDraggingItemEnumerationOptions = [], for view: NSView?, classes classArray: [AnyClass],
                                searchOptions: [NSPasteboard.ReadingOptionKey: Any] = [:], using block: (NSDraggingItem, Int, UnsafeMutablePointer<ObjCBool>) -> Void) {}
    var springLoadingHighlight: NSSpringLoadingHighlight { .none }
    func resetSpringLoading() {}
}

final class RowDropTests: XCTestCase {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-row-drop-\(UUID().uuidString)", isDirectory: true)

    override func setUp() { try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true) }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    func write(_ name: String, _ data: Data) throws -> URL {
        let url = root.appendingPathComponent("in", isDirectory: true).appendingPathComponent(name)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url)
        return url
    }
    func png() -> Data {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 4, pixelsHigh: 4, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        return rep.representation(using: .png, properties: [:])!
    }

    /// AppKit's own drag-destination search (`_hitTest:dragTypes:`, what `NSWindow` asks under a drag).
    func dragDestination(_ content: NSView, at point: NSPoint) -> NSView? {
        let selector = NSSelectorFromString("_hitTest:dragTypes:")
        guard let method = class_getInstanceMethod(NSView.self, selector) else { return nil }
        typealias Search = @convention(c) (AnyObject, Selector, UnsafeMutablePointer<NSPoint>, NSSet) -> AnyObject?
        var at = point
        return unsafeBitCast(method_getImplementation(method), to: Search.self)(content, selector, &at, NSSet(array: [NSPasteboard.PasteboardType.fileURL.rawValue])) as? NSView
    }

    func testTheTargetTakesDragsWhileTheRowTakesClicks() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 120), styleMask: [.titled], backing: .buffered, defer: false)
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 120))
        window.contentView = content
        let row = NSButton(frame: NSRect(x: 10, y: 40, width: 260, height: 32)) // the row's button, as Exact draws it
        content.addSubview(row)
        let target = T3RowDropTarget(frame: row.bounds)
        target.autoresizingMask = [.width, .height]
        row.addSubview(target)
        XCTAssertEqual(target.registeredDraggedTypes, [.fileURL])
        XCTAssertTrue(content.hitTest(NSPoint(x: 100, y: 55)) === row, "a click, a hover and a right click reach the row")
        XCTAssertTrue(dragDestination(content, at: NSPoint(x: 100, y: 55)) === target, "a file drag reaches the drop target")
        XCTAssertNil(dragDestination(content, at: NSPoint(x: 100, y: 100)), "outside the row nothing takes the drag")
        row.frame.size.width = 200
        XCTAssertEqual(target.frame.size.width, 200, "the target follows the row's size")
    }

    func testADragRingsTheRowAndADropReportsItsFiles() throws {
        let target = T3RowDropTarget(frame: NSRect(x: 0, y: 0, width: 260, height: 32))
        target.key = "thread-1"
        var reported: [(String, [URL])] = []
        target.dropped = { reported.append(($0, $1)) }
        let file = try write("notes.txt", Data("hello".utf8))
        let folder = root.appendingPathComponent("in/folder", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        XCTAssertEqual(target.draggingEntered(FileDrag([file])), .copy)
        XCTAssertTrue(target.ringed)
        XCTAssertEqual(target.layer?.borderWidth, 1, "ring-1")
        target.draggingExited(FileDrag([file]))
        XCTAssertFalse(target.ringed)
        XCTAssertEqual(target.draggingEntered(FileDrag([])), [], "a drag without files passes by")
        XCTAssertFalse(target.ringed)
        XCTAssertTrue(target.performDragOperation(FileDrag([file, folder])))
        XCTAssertFalse(target.ringed)
        XCTAssertEqual(reported.map(\.0), ["thread-1"])
        XCTAssertEqual(reported.first?.1.map(\.lastPathComponent), ["notes.txt"], "the folder is let go")
        XCTAssertTrue(target.performDragOperation(FileDrag([folder])), "a drop of folders alone is taken")
        XCTAssertEqual(reported.count, 1, "and reports nothing, so the row does not open")
    }

    func testTakingAThreadsDropsStagesThemInOrder() throws {
        let attach = T3ComposerAttach(dataRoot: root, agent: false)
        var topics: [String] = []
        let drops = T3RowDrop(attach: attach) { topics.append($0) }
        let text = try write("notes.txt", Data("hello".utf8)), image = try write("shot.png", png()), other = try write("other.txt", Data("x".utf8))
        drops.queue("thread-1", [text])
        drops.queue("fleet:env-2:thread-9", [other])
        drops.queue("thread-1", [image])
        XCTAssertEqual(drops.status["rowDrops"] as? [String], ["thread-1", "fleet:env-2:thread-9", "thread-1"])
        let reply = drops.take(["keys": ["thread-1", "fleet:env-1:thread-1"], "fileLimit": 1024 * 1024, "generation": 4])
        XCTAssertEqual(reply["generation"] as? Int, 4)
        let files = try XCTUnwrap((reply["value"] as? [String: Any])?["files"] as? [[String: Any]])
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["file", "image"], "oldest first")
        XCTAssertEqual(files.map { $0["name"] as? String }, ["notes.txt", "shot.png"])
        let fileId = try XCTUnwrap(files[0]["id"] as? String), imageId = try XCTUnwrap(files[1]["id"] as? String)
        XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("composer-files/\(fileId)").path), "a file is staged as Attach stages it")
        XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("snapshots/drafts/\(imageId).png").path), "an image becomes a PNG draft")
        XCTAssertEqual(drops.status["rowDrops"] as? [String], ["fleet:env-2:thread-9"], "another thread's drop stays queued")
        let none = drops.take(["keys": ["thread-1"], "fileLimit": 1024, "generation": 5])
        XCTAssertEqual(((none["value"] as? [String: Any])?["files"] as? [Any])?.count, 0)
        XCTAssertEqual(topics, ["t3.status", "t3.status", "t3.status", "t3.status"])
        let refused = drops.take(["keys": ["fleet:env-2:thread-9"], "fileLimit": 0, "generation": 6])
        let plain = try XCTUnwrap(((refused["value"] as? [String: Any])?["files"] as? [[String: Any]])?.first)
        XCTAssertNil(plain["id"], "a server that takes no files stages nothing; the client names the file in its error")
    }
}

extension RowDropTests {
    /// handleThreadFileDrop: a row that is not the open thread's is pressed; the open thread's own row only queues.
    func testADropPressesOnlyARowThatIsNotOpen() throws {
        let drops = T3RowDrop(attach: T3ComposerAttach(dataRoot: root, agent: false)) { _ in }
        let file = try write("notes.txt", Data("hello".utf8))
        var presses = 0
        drops.drop("thread-2", [file], open: false) { presses += 1 }
        XCTAssertEqual(presses, 1, "another thread's row opens its thread")
        drops.drop("thread-1", [file], open: true) { presses += 1 }
        XCTAssertEqual(presses, 1, "the open thread's row does not press itself again (openSelected would reset its panel)")
        drops.drop("thread-3", [], open: false) { presses += 1 }
        XCTAssertEqual(presses, 1, "an empty drop does nothing")
        XCTAssertEqual(drops.status["rowDrops"] as? [String], ["thread-2", "thread-1"])
    }

    /// clearPendingFileDrop: a drop whose thread did not open goes; the others stay.
    func testClearingLetsTheNamedDropsGo() throws {
        var topics: [String] = []
        let drops = T3RowDrop(attach: T3ComposerAttach(dataRoot: root, agent: false)) { topics.append($0) }
        let file = try write("notes.txt", Data("hello".utf8))
        drops.queue("thread-2", [file]); drops.queue("thread-3", [file]); drops.queue("thread-2", [file])
        let reply = drops.clear(["keys": ["thread-2", "thread-9"], "generation": 7])
        XCTAssertEqual(reply["generation"] as? Int, 7)
        XCTAssertEqual((reply["value"] as? [String: Any])?["cleared"] as? Int, 2)
        XCTAssertEqual(drops.status["rowDrops"] as? [String], ["thread-3"])
        XCTAssertEqual(topics.count, 4, "three queued and one clear")
        _ = drops.clear(["keys": ["thread-9"]])
        XCTAssertEqual(topics.count, 4, "nothing cleared, nothing changed")
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: RowDropTests.self)
suite.run()
let run = suite.testRun!
print("Row drop tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
