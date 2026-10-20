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
    /// `column` says so, holding a textarea (2), a box (3) and an input (4).
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
            ["op": "create", "id": 4, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 150.0, "w": 300.0, "h": 24.0],
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
        // This presenter has no session to mint the file, so no drop lands
        // and the drag ends at the column with a leave
        // (`testADropThatLandsIsTheColumnsActionAndNoLeave` mints it).
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
        // Back in and dropped where no file could be opened (no session): a leave.
        heard = []
        _ = column.draggingEntered(drag([file], at: middle(box)))
        _ = column.performDragOperation(drag([file], at: middle(box)))
        XCTAssertEqual(heard, ["1:55", "1:56"])
    }

    /// An input that is not being edited takes the drag itself (`Field`).
    func testAFileDragOverAnInputIsTheColumnsDrop() throws {
        let p = presenter(column: ["drop", "dragenter", "dragleave"])
        let field = try XCTUnwrap(p.views[4]?.field as? Field)
        let info = drag([file], at: middle(field))
        XCTAssertEqual(field.draggingEntered(info), .copy)
        XCTAssertTrue(field.prepareForDragOperation(info))
        _ = field.performDragOperation(info)
        field.concludeDragOperation(info)
        XCTAssertEqual(field.stringValue, "hello", "no path is inserted")
        XCTAssertEqual(heard, ["1:55", "1:56"])
    }

    /// A dropped file of a type the manifest declares is kept (bookmarked,
    /// on Open Recent); any other, and a folder, is only minted.
    func testOnlyADeclaredTypeIsKept() {
        let notes = URL(fileURLWithPath: "/tmp/a.notes"), other = URL(fileURLWithPath: "/tmp/b.zip"), folder = URL(fileURLWithPath: "/tmp/photos", isDirectory: true)
        var kept: [String] = [], minted: [String] = []
        let docs = NodeView.droppedDocuments([notes, other, folder, URL(string: "https://example.com/x")!],
            accepts: { $0.pathExtension == "notes" },
            keep: { kept.append($0.lastPathComponent); return "doc:/k/\($0.lastPathComponent)" },
            mint: { minted.append($0.lastPathComponent); return "doc:/m/\($0.lastPathComponent)" })
        XCTAssertEqual(docs, ["doc:/k/a.notes", "doc:/m/b.zip", "doc:/m/photos"])
        XCTAssertEqual(kept, ["a.notes"])
        XCTAssertEqual(minted, ["b.zip", "photos"], "a web URL is no file")
    }

    /// With a session: the file over the textarea is minted, the column's
    /// `drop` action runs with its handle, and no `dragleave` follows it,
    /// as a browser's drop is the last event of a drag that lands.
    func testADropThatLandsIsTheColumnsActionAndNoLeave() throws {
        _ = NSApplication.shared
        let source = """
        component App
          state log = ""
          action entered
            log = `${log}enter;`
          action left
            log = `${log}leave;`
          action dropped(e: DragEvent)
            log = `${log}drop:${e.files.join(",")};`
          view
            column testId="column" width="100%" height="100%" dragenter=entered dragleave=left drop=dropped
              textarea testId="field" value="hello" height=80

        """
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let mount = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        window.contentView = mount
        view.frame = mount.bounds; mount.addSubview(view)
        XCTAssertNil(session.boot(plan: try compile(source), size: CGSize(width: 400, height: 300)).error)
        window.layoutIfNeeded()
        let editor = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "field" }?.textArea)
        let info = drag([file], at: middle(editor))
        XCTAssertEqual(editor.draggingEntered(info), .copy)
        XCTAssertTrue(editor.prepareForDragOperation(info))
        XCTAssertTrue(editor.performDragOperation(info), "the column took the drop")
        editor.concludeDragOperation(info)
        editor.draggingEnded(info)
        XCTAssertEqual(editor.string, "hello", "no path is inserted")
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any])
        let log = try XCTUnwrap((state["slots"] as? [String: Any])?["log"] as? String)
        XCTAssertTrue(log.hasPrefix("enter;drop:doc:/") && log.hasSuffix("/\(file.lastPathComponent);"), log)
        XCTAssertFalse(log.contains("leave"), "a drop that lands is the drag's last event: \(log)")
    }

    private func compile(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
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
