#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A drag of files from Finder over a window (FileDragMac.swift): a file
/// drag over a textarea goes to the `drop` above it instead of inserting its
/// path, `dragenter` and `dragleave` bracket it at the box that hears them,
/// and a paste's files are claimed from the pasteboard (PastedFiles.swift).
final class FileDragMacTests: XCTestCase {
    private var window: NSWindow!
    private var heard: [String] = []
    private var file: URL!

    override func setUpWithError() throws {
        file = FileManager.default.temporaryDirectory.appendingPathComponent("drag-\(UUID().uuidString).txt")
        try Data("notes".utf8).write(to: file)
    }
    override func tearDown() {
        FileDrag.ended()
        window?.close(); window = nil
        try? FileManager.default.removeItem(at: file)
    }

    /// A column (1) hearing `drop`, `dragenter` and `dragleave` when
    /// `column` says so, holding a textarea (2) and a box (3).
    private func presenter(column: [String]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.onClipboard = { [unowned self] id, kind, line in heard.append("\(id):\(kind)") }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": column],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": [String]()],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 40.0],
        ]))
        window.layoutIfNeeded()
        return p
    }

    private func middle(_ view: NSView) -> NSPoint { view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil) }

    private func drag(_ urls: [URL], at point: NSPoint, text: String? = nil) -> DraggingInfo {
        let board = NSPasteboard(name: .init("exact-test-\(UUID().uuidString)"))
        board.clearContents()
        var items: [NSPasteboardWriting] = urls.map { $0 as NSURL }
        if let text { items.append(text as NSString) }
        board.writeObjects(items)
        return DraggingInfo(board: board, location: point, window: window)
    }

    func testAFileDragOverATextareaIsTheColumnsDrop() throws {
        let p = presenter(column: ["drop", "dragenter", "dragleave"])
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        let info = drag([file], at: middle(editor))
        XCTAssertEqual(editor.draggingEntered(info), .copy, "the column above takes it")
        XCTAssertEqual(editor.draggingUpdated(info), .copy)
        XCTAssertTrue(editor.prepareForDragOperation(info))
        _ = editor.performDragOperation(info)
        editor.concludeDragOperation(info)
        XCTAssertEqual(editor.string, "hello", "no path is inserted")
        // This presenter has no session to mint the file, so the drop itself
        // is refused; the drag still ended at the column.
        XCTAssertEqual(heard, ["1:55", "1:56"])
    }

    func testAFileDragWithNoDropAboveAndATextDragAreTheFieldsOwn() throws {
        let p = presenter(column: ["dragenter", "dragleave"])
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        let files = drag([file], at: middle(editor))
        XCTAssertFalse(FileDrag.takes(editor, files), "no node declares drop")
        XCTAssertNil(FileDrag.moved(editor, files), "the field's own")
        XCTAssertEqual(heard, ["1:55"], "a drag no drop takes is still heard")
        let text = drag([], at: middle(editor), text: "words")
        XCTAssertNil(FileDrag.moved(editor, text), "a text drag is the field's")
        XCTAssertFalse(FileDrag.takes(editor, text))
    }

    func testEnterAndLeaveAreTheColumnsBoxNotItsChildren() throws {
        let p = presenter(column: ["drop", "dragenter", "dragleave"])
        let column = try XCTUnwrap(p.views[1]), editor = try XCTUnwrap(p.views[2]?.textArea), box = try XCTUnwrap(p.views[3])
        XCTAssertEqual(column.draggingEntered(drag([file], at: middle(box))), .copy)
        // Into a registered child: the column's exit, the child's enter.
        column.draggingExited(drag([file], at: middle(editor)))
        _ = editor.draggingEntered(drag([file], at: middle(editor)))
        XCTAssertEqual(heard, ["1:55"], "moving inside the column is no leave")
        // Out of the window: what was under the last point lets go.
        editor.draggingExited(drag([file], at: NSPoint(x: -20, y: -20)))
        XCTAssertEqual(heard, ["1:55", "1:56"])
        // Back in and dropped: the drop first, then the leave.
        heard = []
        _ = column.draggingEntered(drag([file], at: middle(box)))
        _ = column.performDragOperation(drag([file], at: middle(box)))
        XCTAssertEqual(heard, ["1:55", "1:56"])
    }

    func testAPastesFilesAreItsFileURLsElseItsImageAsPNG() throws {
        let board = NSPasteboard(name: .init("exact-test-\(UUID().uuidString)"))
        defer { board.releaseGlobally() }
        let icon = try XCTUnwrap(NSImage(size: NSSize(width: 4, height: 4), flipped: false) { r in NSColor.red.setFill(); r.fill(); return true }.tiffRepresentation)
        // A Finder copy: the file, its icon and its name.
        board.clearContents()
        let item = NSPasteboardItem()
        item.setString(file.absoluteString, forType: .fileURL)
        item.setData(icon, forType: .tiff)
        item.setString(file.lastPathComponent, forType: .string)
        board.writeObjects([item])
        let finder = NodeView.pasteClaim(board)
        XCTAssertEqual(finder.urls.map(\.standardizedFileURL), [file.standardizedFileURL])
        XCTAssertNil(finder.png, "the icon is not pasted beside its file")
        // A screenshot: PNG data alone.
        let png = try XCTUnwrap(NSBitmapImageRep(data: icon)?.representation(using: .png, properties: [:]))
        board.clearContents()
        board.setData(png, forType: .png)
        XCTAssertEqual(NodeView.pasteClaim(board).png, png)
        XCTAssertTrue(NodeView.pasteClaim(board).urls.isEmpty)
        // An image copied from an app that writes TIFF: converted.
        board.clearContents()
        board.setData(icon, forType: .tiff)
        let converted = try XCTUnwrap(NodeView.pasteClaim(board).png)
        XCTAssertEqual(Array(converted.prefix(4)), [0x89, 0x50, 0x4E, 0x47])
        // Text alone has none.
        board.clearContents()
        board.setString("words", forType: .string)
        let words = NodeView.pasteClaim(board)
        XCTAssertTrue(words.urls.isEmpty && words.png == nil)
        let written = try XCTUnwrap(NodeView.writePastedImage(converted))
        defer { try? FileManager.default.removeItem(at: written.deletingLastPathComponent()) }
        XCTAssertEqual(written.lastPathComponent, "image.png")
        XCTAssertEqual(try Data(contentsOf: written), converted)
    }
}

/// An `NSDraggingInfo` for a drag at a window point, as AppKit hands one.
private final class DraggingInfo: NSObject, NSDraggingInfo {
    let board: NSPasteboard, location: NSPoint
    weak var window: NSWindow?
    init(board: NSPasteboard, location: NSPoint, window: NSWindow) { self.board = board; self.location = location; self.window = window }
    var draggingDestinationWindow: NSWindow? { window }
    var draggingSourceOperationMask: NSDragOperation { .copy }
    var draggingLocation: NSPoint { location }
    var draggedImageLocation: NSPoint { location }
    var draggedImage: NSImage? { nil }
    var draggingPasteboard: NSPasteboard { board }
    var draggingSource: Any? { nil }
    var draggingSequenceNumber: Int { 1 }
    func slideDraggedImage(to screenPoint: NSPoint) {}
    var draggingFormation: NSDraggingFormation = .default
    var animatesToDestination = false
    var numberOfValidItemsForDrop = 1
    func enumerateDraggingItems(options enumOpts: NSDraggingItemEnumerationOptions = [], for view: NSView?, classes classArray: [AnyClass], searchOptions: [NSPasteboard.ReadingOptionKey: Any] = [:], using block: @escaping (NSDraggingItem, Int, UnsafeMutablePointer<ObjCBool>) -> Void) {}
    var springLoadingHighlight: NSSpringLoadingHighlight { .none }
    func resetSpringLoading() {}
}
#endif
