import AppKit
import XCTest

// The OS attachment boundary on a real AppKit text view behind the
// forwarding delegate. Source edits and grammar belong to the shared field.

private let editorResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let editorAct: ExactHatches.ActFn = { _, _, _ in 0 }
private let editorLog: ExactHatches.LogFn = { _, _, _ in }
private let editorDelegate: ExactHatches.DelegateFn = { _, _, _ in }

/// Stands in for Exact's node: the text view's real delegate.
final class InnerDelegate: NSObject, NSTextViewDelegate {
    var changes = 0, selections = 0
    var allowsChange = true
    var commands: [Selector] = []
    func textView(_ textView: NSTextView, shouldChangeTextIn range: NSRange, replacementString text: String?) -> Bool { allowsChange }
    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool { commands.append(selector); return true }
    func textDidChange(_ notification: Notification) { changes += 1 }
    func textViewDidChangeSelection(_ notification: Notification) { selections += 1 }
}

final class EditorFixture {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    let hooks: ExactHatches
    let window: NSWindow
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let inner = InnerDelegate()
    var topics: [String] = []
    lazy var composer = T3Composer(changed: { [weak self] topic in self?.topics.append(topic) })
    let element: ExactElement

    init() {
        _ = NSApplication.shared
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(editorResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHatches(host: nil, table: UnsafeRawPointer(table))!
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        editor.isEditable = true
        editor.isRichText = false
        editor.allowsUndo = true
        editor.font = NSFont.systemFont(ofSize: 14)
        editor.delegate = inner
        scroller.documentView = editor
        window.contentView?.addSubview(scroller)
        window.makeFirstResponder(editor)
        element = ExactElement(hatch: .t3Composer, id: "composer", node: 1, hatches: hooks)
        element.view = scroller
        element.platform = editor
        element.data = ExactData(["snapshot-owner": "owner-a", "owner": "field-a"])
        composer.install(element)
    }

    var identity: [String: Any] { ["snapshotOwner": "owner-a", "fieldOwner": "field-a"] }
    @discardableResult func sync(_ policy: [String: Any] = [:]) -> [String: Any] {
        composer.editor.sync(identity.merging(policy) { _, value in value })
    }
    func replaceOwner(_ snapshot: String = "owner-b", _ field: String = "field-b") {
        element.data = ExactData(["snapshot-owner": snapshot, "owner": field])
        composer.install(element)
    }

    func type(_ text: String) { editor.insertText(text, replacementRange: editor.selectedRange()); tick() }
    /// One event-loop turn: the undo manager closes its event group, as between keystrokes.
    func tick() { RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.005)) }

    deinit { composer.destroy(); window.close(); table.deallocate() }
}

final class ComposerEditorTests: XCTestCase {
    func testOSRecordIdsRefuseBeyondJavaScriptsExactIntegerRange() {
        XCTAssertEqual(T3ComposerEditor.nextRecordID(after: 0), 1)
        XCTAssertEqual(T3ComposerEditor.nextRecordID(after: T3ComposerEditor.maxRecordID - 1), T3ComposerEditor.maxRecordID)
        XCTAssertNil(T3ComposerEditor.nextRecordID(after: T3ComposerEditor.maxRecordID))
        XCTAssertNil(T3ComposerEditor.nextRecordID(after: Int.max))
    }

    func testDelegateForwardsTextSelectionAndCommandsWithoutNativeGrammar() {
        let fixture = EditorFixture()
        XCTAssertTrue(fixture.editor.delegate !== fixture.inner)
        fixture.type("**bold** [file](src/file) $skill")
        XCTAssertGreaterThan(fixture.inner.changes, 0)
        fixture.editor.setSelectedRange(NSRange(location: 3, length: 0))
        XCTAssertGreaterThan(fixture.inner.selections, 0)
        XCTAssertEqual(fixture.editor.selectedRange(), NSRange(location: 3, length: 0))
        let source = fixture.editor.string
        let command = #selector(NSResponder.insertTab(_:))
        XCTAssertEqual(fixture.editor.delegate?.textView?(fixture.editor, doCommandBy: command), true)
        XCTAssertEqual(fixture.inner.commands, [command], "shared command delegate remains authoritative")
        fixture.inner.allowsChange = false
        XCTAssertFalse(fixture.editor.shouldChangeText(in: NSRange(location: 3, length: 0), replacementString: "ordinary"))
        XCTAssertEqual(fixture.editor.string, source)
        for op in ["editorState", "editorEdit", "editorInsert", "editorChip", "editorChipFinish", "editorCiteOpen", "editorFocus"] {
            XCTAssertEqual(fixture.composer.perform(["op": op, "text": "changed"])["ok"] as? Bool, false)
        }
        XCTAssertEqual(fixture.editor.string, source)
        let state = fixture.sync()
        XCTAssertNil(state["trigger"]); XCTAssertNil(state["text"]); XCTAssertNil(state["caret"])
        XCTAssertEqual(state["snapshotOwner"] as? String, "owner-a")
        XCTAssertEqual(state["fieldOwner"] as? String, "field-a")
        XCTAssertEqual(state["composing"] as? Bool, false)
        fixture.composer.remove(fixture.element)
        XCTAssertTrue(fixture.editor.delegate === fixture.inner)
    }

    func testFoldIsOwnerScopedObservedSourceUntilExplicitAcknowledgement() throws {
        let fixture = EditorFixture()
        fixture.composer.editor.pasteDetector = { _ in true }
        fixture.type("A😀Z")
        fixture.sync(["foldLimit": 100_000])
        let range = NSRange(location: 1, length: 2), large = String(repeating: "x", count: 40_000)
        XCTAssertFalse(fixture.composer.editor.shouldChange(fixture.editor, range: range, text: large))
        XCTAssertEqual(fixture.editor.string, "A😀Z")
        let fact = try XCTUnwrap((fixture.sync()["folds"] as? [[String: Any]])?.first)
        XCTAssertEqual(fact["snapshotOwner"] as? String, "owner-a")
        XCTAssertEqual(fact["fieldOwner"] as? String, "field-a")
        XCTAssertEqual(fact["value"] as? String, "A😀Z")
        XCTAssertEqual(fact["start"] as? Int, 1); XCTAssertEqual(fact["end"] as? Int, 3)
        XCTAssertEqual(fact["expect"] as? String, "😀"); XCTAssertEqual(fact["text"] as? String, large)
        let id = try XCTUnwrap(fact["id"] as? Int)
        XCTAssertEqual((fixture.sync()["folds"] as? [[String: Any]])?.count, 1)
        fixture.composer.editor.forgetFold(id - 1)
        XCTAssertEqual(fixture.composer.editor.folds.count, 1)
        fixture.composer.editor.forgetFold(id)
        XCTAssertTrue(fixture.composer.editor.folds.isEmpty)
        XCTAssertEqual(fixture.editor.string, "A😀Z", "ACK never applies an edit")
    }

    func testStalePolicyAndOwnerReplacementCannotReuseFoldFacts() {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        adapter.pasteDetector = { _ in true }
        fixture.sync(["foldLimit": 100_000])
        let large = String(repeating: "x", count: 40_000), range = NSRange(location: 0, length: 0)
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large))
        let oldID = adapter.folds.first?["id"] as? Int ?? 0
        fixture.replaceOwner()
        XCTAssertTrue(adapter.folds.isEmpty)
        fixture.sync(["foldLimit": 100_000]) // old pair
        XCTAssertEqual(adapter.foldLimit, 0)
        _ = adapter.sync(["snapshotOwner": "owner-b", "fieldOwner": "field-b", "foldLimit": 100_000])
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large))
        let nextID = adapter.folds.first?["id"] as? Int ?? 0
        XCTAssertGreaterThan(nextID, oldID)
        adapter.forgetFold(oldID)
        XCTAssertEqual(adapter.folds.first?["id"] as? Int, nextID)
    }

    func testPromptLimitAndPasteAsTextKeepTheirOSGuard() {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        adapter.pasteDetector = { _ in true }
        fixture.type("abc")
        let huge = String(repeating: "x", count: 120_000)
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: NSRange(location: 3, length: 0), text: huge))
        XCTAssertEqual((fixture.sync()["notices"] as? [[String: String]])?.first?["title"], "Pasted text is too large for this message")
        adapter.pastingAsText = { true }
        XCTAssertTrue(adapter.shouldChange(fixture.editor, range: NSRange(location: 3, length: 0), text: huge))
        XCTAssertTrue(adapter.folds.isEmpty)
        XCTAssertEqual(fixture.editor.string, "abc")
    }

    func testFoldsAreBoundedMonotonicAcknowledgedFacts() throws {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        adapter.pasteDetector = { _ in true }
        fixture.type("A😀Z"); fixture.sync(["foldLimit": 100_000])
        let range = NSRange(location: 1, length: 2), large = String(repeating: "x", count: 40_000)
        for _ in 0..<4 { XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large)) }
        let fold = try XCTUnwrap(adapter.folds.first), firstID = try XCTUnwrap(fold["id"] as? Int)
        XCTAssertEqual(fold["value"] as? String, "A😀Z"); XCTAssertEqual(fold["expect"] as? String, "😀")
        XCTAssertEqual(fold["snapshotOwner"] as? String, "owner-a"); XCTAssertEqual(fold["fieldOwner"] as? String, "field-a")
        XCTAssertEqual(adapter.folds.compactMap { $0["id"] as? Int }, Array(firstID...firstID + 3))
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large))
        XCTAssertEqual(adapter.folds.count, 4)
        XCTAssertEqual((fixture.sync()["notices"] as? [[String: String]])?.first?["title"], "Too many pending pastes")
        adapter.forgetFold(firstID)
        XCTAssertEqual(adapter.folds.count, 3)
        XCTAssertEqual(fixture.editor.string, "A😀Z", "a fold's source is never inserted by Swift")
    }

    func testCompositionPassesToTheSharedDelegateWithoutFoldClaims() {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        adapter.pasteDetector = { _ in true }; fixture.sync(["foldLimit": 100_000])
        fixture.editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(adapter.shouldChange(fixture.editor, range: NSRange(location: 0, length: 1), text: String(repeating: "x", count: 40_000)))
        XCTAssertTrue(adapter.folds.isEmpty)
        XCTAssertTrue(fixture.editor.hasMarkedText())
    }
}
