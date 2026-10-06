import AppKit
import XCTest

// Lane r5-panels (T3PanelsNative.swift): PullRequestUnavailableError reads as the reference's
// sentence; a sent attachment's text preview and Save file over a stubbed
// asset route (no network). Lane r13-panels: a press below the Files editor's last line, with the
// composer holding the focus, gives the mounted editor the focus with the caret at the end, and
// typed keys land in the file, not the draft.
// Compiled without T3Module.swift, so the test supplies the module the facade names.
final class PanelsStubModule: ExactModule {}
let exactModule: ExactModule.Type = PanelsStubModule.self

final class StubAssets: URLProtocol {
    static var body = Data()
    static var status = 200
    static var seenRange: String?
    override class func canInit(with request: URLRequest) -> Bool { request.url?.path.hasPrefix("/api/assets/") == true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        StubAssets.seenRange = request.value(forHTTPHeaderField: "Range")
        let response = HTTPURLResponse(url: request.url!, statusCode: StubAssets.status, httpVersion: "HTTP/1.1", headerFields: [:])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: StubAssets.body)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

private let panelsResolve: ExactHooks.ResolveFn = { _, _, _, _, _ in 0 }
private let panelsAct: ExactHooks.ActFn = { _, _, _ in 0 }
private let panelsLog: ExactHooks.LogFn = { _, _, _ in }
private let panelsDelegate: ExactHooks.DelegateFn = { _, _, _ in }
/// Exact's hook table with no host behind it: `element.focus()` is refused, as before a node exists.
private func makeHooks() -> ExactHooks {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(panelsResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(panelsAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(panelsLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(panelsDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHooks(host: nil, table: UnsafeRawPointer(table))!
}
private func tick(_ seconds: TimeInterval = 0.05) { let end = Date(timeIntervalSinceNow: seconds); while Date() < end { RunLoop.current.run(mode: .default, before: end) } }

/// The Files pane as the contract lays it out: the composer below, the file's numbered lines
/// (`file-lines`, a plain view that takes the press) and, once the press began editing, the clear
/// editor textarea over them (`file-editor`, left = the gutter).
final class FileEditorFixture {
    let hooks = makeHooks()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
    let composer = NSTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 80))
    let lines = NSView(frame: NSRect(x: 0, y: 100, width: 600, height: 300))
    let editor = NSTextView(frame: NSRect(x: 40, y: 0, width: 560, height: 300))
    init() {
        _ = NSApplication.shared
        window.isReleasedWhenClosed = false
        composer.isEditable = true; composer.string = "draft two"
        window.contentView!.addSubview(composer); window.contentView!.addSubview(lines)
        window.makeFirstResponder(composer); composer.setSelectedRange(NSRange(location: 9, length: 0))
        editor.isEditable = true; editor.isRichText = false
        editor.font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        editor.string = "# pr-demo\nStatus: main moved on\n"
    }
    /// The press began editing: the editor mounts (a new `t3-file-editor` node) and its hook runs.
    func mount() {
        lines.addSubview(editor)
        let element = ExactElement(hook: .t3FileEditor, id: "file-editor", node: 7, hooks: hooks)
        element.view = editor; element.platform = editor
        T3FileEditor.install(element)
        tick()
    }
    /// A key typed on the keyboard, sent the way the window routes it: to its first responder.
    func type(_ text: String) {
        for (index, character) in text.enumerated() {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: TimeInterval(index), windowNumber: window.windowNumber, context: nil,
                characters: String(character), charactersIgnoringModifiers: String(character), isARepeat: false, keyCode: 0)!
            window.sendEvent(event)
        }
        tick()
    }
    /// A window point `y` points below the top of the lines view (the editor's flipped coordinates).
    func point(x: CGFloat, below y: CGFloat) -> NSPoint { editor.convert(NSPoint(x: x, y: y), to: nil) }
}

final class PanelsTests: XCTestCase {
    /// The real-input failure (realinput2): a click below the last line, typing, and the text went
    /// to the composer. The press is real (an AppKit event), below the text: caret at the end.
    func testPressBelowTheLastLineFocusesTheEditorAtTheEnd() {
        let f = FileEditorFixture()
        XCTAssertTrue(f.window.firstResponder === f.composer)
        f.lines.addSubview(f.editor); f.editor.layoutManager?.ensureLayout(for: f.editor.textContainer!)
        let below = f.point(x: 20, below: 250)
        f.editor.removeFromSuperview()
        T3FileEditor.recordPress(f.window, at: below)
        f.mount()
        XCTAssertTrue(f.window.firstResponder === f.editor, "the editor takes the focus from the composer")
        XCTAssertEqual(f.editor.selectedRange(), NSRange(location: (f.editor.string as NSString).length, length: 0), "caret at the end of the last line")
        f.type("Z")
        XCTAssertEqual(f.editor.string, "# pr-demo\nStatus: main moved on\nZ", "typed keys land in the file")
        XCTAssertEqual(f.composer.string, "draft two", "and not in the draft")
    }

    /// A real press on a line puts the caret under it, as the reference's contenteditable does.
    func testPressOnALinePutsTheCaretUnderIt() {
        let f = FileEditorFixture()
        f.lines.addSubview(f.editor); f.editor.layoutManager?.ensureLayout(for: f.editor.textContainer!)
        let glyph = f.editor.layoutManager!.boundingRect(forGlyphRange: NSRange(location: 3, length: 1), in: f.editor.textContainer!)
        let onLine = f.editor.convert(NSPoint(x: glyph.minX + f.editor.textContainerOrigin.x + 1, y: glyph.midY + f.editor.textContainerOrigin.y), to: nil)
        f.editor.removeFromSuperview()
        T3FileEditor.recordPress(f.window, at: onLine)
        f.mount()
        XCTAssertTrue(f.window.firstResponder === f.editor)
        XCTAssertEqual(f.editor.selectedRange().location, 3, "the caret before the pressed character")
        f.type("!")
        XCTAssertEqual(f.editor.string, "# p!r-demo\nStatus: main moved on\n")
    }

    /// An agent's tap carries no AppKit event (and an old press does not count): caret at the end.
    func testATapWithoutAPressPointGoesToTheEnd() {
        let f = FileEditorFixture()
        T3FileEditor.recordPress(f.window, at: f.point(x: 20, below: 2))
        tick(1.1)
        f.window.makeFirstResponder(nil)
        f.mount()
        XCTAssertTrue(f.window.firstResponder === f.editor)
        XCTAssertEqual(f.editor.selectedRange().location, (f.editor.string as NSString).length)
        // The editor mounts once per press: a later hook call on the same node changes nothing.
        f.window.makeFirstResponder(f.composer)
        let element = ExactElement(hook: .t3FileEditor, id: "file-editor", node: 7, hooks: f.hooks)
        element.view = f.editor; element.platform = f.editor; element.isNew = false
        T3FileEditor.install(element); tick()
        XCTAssertTrue(f.window.firstResponder === f.composer)
    }

    func testUnavailableMessages() {
        XCTAssertEqual(T3PullRequestErrors.unavailable(reason: "cli-unauthenticated", provider: "github"), "GitHub CLI is not authenticated. Run `gh auth login` and retry.")
        XCTAssertEqual(T3PullRequestErrors.unavailable(reason: "cli-missing", provider: "gitlab"), "GitLab CLI (`glab`) is required to browse change requests on this host. Install it from https://gitlab.com/gitlab-org/cli and reload.")
        XCTAssertEqual(T3PullRequestErrors.unavailable(reason: "cli-unauthenticated", provider: ""), "This host has no working credentials.")
        XCTAssertEqual(T3PullRequestErrors.unavailable(reason: "provider-unsupported", provider: "github"), "Change requests cannot be browsed for this project's host yet.")
        XCTAssertEqual(T3PullRequestErrors.unavailable(reason: "other", provider: "github"), "")
    }

    func testAssetRouteOnly() {
        XCTAssertNotNil(T3AttachmentFiles.assetURL("http://127.0.0.1:1/api/assets/abc"))
        XCTAssertNil(T3AttachmentFiles.assetURL("file:///etc/hosts"))
        XCTAssertNil(T3AttachmentFiles.assetURL("http://127.0.0.1:1/api/orchestration/shell"))
        XCTAssertNil(T3AttachmentFiles.assetURL("https://user@host/api/assets/x"))
    }

    private func perform(_ request: [String: Any], exports: URL? = nil) -> [String: Any] {
        let done = expectation(description: "reply")
        var value: [String: Any] = [:]
        T3AttachmentFiles.perform(request, exportsRoot: exports) { reply in value = reply["value"] as? [String: Any] ?? [:]; done.fulfill() }
        wait(for: [done], timeout: 10)
        return value
    }

    func testTextPreviewSaveAndRefusals() throws {
        StubAssets.status = 200
        StubAssets.body = Data("# Notes\n\n- “one”\n".utf8)
        var value = perform(["op": "attachmentText", "url": "http://127.0.0.1:9/api/assets/a"])
        XCTAssertEqual(value["text"] as? String, "# Notes\n\n- “one”\n")
        XCTAssertEqual(value["truncated"] as? Bool, false)
        XCTAssertEqual(StubAssets.seenRange, "bytes=0-1048576")
        StubAssets.body = Data(repeating: 0x61, count: 1_048_577)
        value = perform(["op": "attachmentText", "url": "http://127.0.0.1:9/api/assets/a"])
        XCTAssertEqual(value["truncated"] as? Bool, true)
        XCTAssertEqual((value["text"] as? String)?.count, 1_048_576)
        StubAssets.body = Data([0x50, 0x00, 0x01])
        value = perform(["op": "attachmentText", "url": "http://127.0.0.1:9/api/assets/a"])
        XCTAssertEqual(value["message"] as? String, "This file contains binary data and cannot be shown as text.")
        StubAssets.body = Data([0xff, 0xfe, 0x41])
        value = perform(["op": "attachmentText", "url": "http://127.0.0.1:9/api/assets/a"])
        XCTAssertEqual(value["message"] as? String, "This file is not UTF-8 text. Open it in another app to view its contents.")
        value = perform(["op": "attachmentText", "url": "file:///etc/hosts"])
        XCTAssertEqual(value["message"] as? String, "The attachment is unavailable.")
        // Save file: the bytes land where the agent's exports go (an NSSavePanel for a person).
        let root = URL(fileURLWithPath: ProcessInfo.processInfo.environment["T3_PANELS_TEST_DIR"] ?? NSTemporaryDirectory()).appendingPathComponent("exports-\(UUID().uuidString)")
        StubAssets.body = Data("a,b\n1,2\n".utf8)
        value = perform(["op": "attachmentSave", "url": "http://127.0.0.1:9/api/assets/b", "name": "table.csv"], exports: root)
        XCTAssertEqual(value["saved"] as? Bool, true)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("table.csv")), Data("a,b\n1,2\n".utf8))
        StubAssets.status = 404
        value = perform(["op": "attachmentSave", "url": "http://127.0.0.1:9/api/assets/b", "name": "x"], exports: root)
        XCTAssertEqual(value["message"] as? String, "The file could not be loaded. Try again.")
        try? FileManager.default.removeItem(at: root)
    }
}

_ = NSApplication.shared
URLProtocol.registerClass(StubAssets.self)
let suite = XCTestSuite(forTestCaseClass: PanelsTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
