import AppKit
import XCTest

// The OS attachment boundary on a real AppKit text view behind the
// forwarding delegate. Source edits and grammar belong to the shared field.

var pressedNodes: [UInt32] = [] // queuekey.swift reads it too
private let editorResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let editorAct: ExactHatches.ActFn = { _, node, action in
    if action == 0 { pressedNodes.append(node) }
    return 0
}
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
    var keys: [String: ExactElement] = [:]

    init() {
        _ = NSApplication.shared
        pressedNodes = []
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

    func key(_ name: String, node: UInt32) {
        let key = ExactElement(hatch: .t3ComposerKey, id: name, node: node, hatches: hooks)
        key.data = ExactData(["anchor": name])
        keys[name] = key
        composer.install(key)
    }

    func type(_ text: String) { editor.insertText(text, replacementRange: editor.selectedRange()); tick() }
    /// One event-loop turn: the undo manager closes its event group, as between keystrokes.
    func tick() { RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.005)) }

    func send(_ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = []) {
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber,
            context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
        window.sendEvent(event)
        tick()
    }
    func down() { send("\u{F701}", 125, [.numericPad, .function]) }
    func up() { send("\u{F700}", 126, [.numericPad, .function]) }
    func left() { send("\u{F702}", 123, [.numericPad, .function]) }
    func right() { send("\u{F703}", 124, [.numericPad, .function]) }
    func returnKey(_ flags: NSEvent.ModifierFlags = []) { send("\r", 36, flags) }
    func tab() { send("\t", 48) }
    func backtab() { send("\u{19}", 48, .shift) }
    func escape() { send("\u{1b}", 53) }
    func backspace() { send("\u{7f}", 51) }

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

    func testOpenMenuKeysPressItsKeyButtonsAndEveryOtherCommandReachesTheSharedField() {
        let fixture = EditorFixture()
        fixture.type("@a")
        fixture.down(); fixture.returnKey(); fixture.escape()
        XCTAssertEqual(pressedNodes, [], "no menu: nothing is pressed")
        XCTAssertEqual(fixture.inner.commands, [#selector(NSResponder.moveDown(_:)), #selector(NSResponder.insertNewline(_:)),
            #selector(NSResponder.cancelOperation(_:))])
        fixture.key("ArrowDown", node: 3); fixture.key("ArrowUp", node: 4); fixture.key("Enter", node: 5); fixture.key("Escape", node: 6)
        fixture.inner.commands = []
        fixture.down(); fixture.up(); fixture.returnKey(); fixture.tab(); fixture.escape()
        XCTAssertEqual(pressedNodes, [3, 4, 5, 5, 6])
        XCTAssertEqual(fixture.inner.commands, [], "the open menu takes its keys")
        XCTAssertEqual(fixture.editor.string, "@a")
        fixture.left()
        XCTAssertEqual(fixture.inner.commands, [#selector(NSResponder.moveLeft(_:))])
        // The menu closes: its buttons unmount and the keys are the field's again.
        for name in ["ArrowDown", "ArrowUp", "Enter", "Escape"] { fixture.composer.remove(fixture.keys[name]!) }
        fixture.inner.commands = []; pressedNodes = []
        fixture.down(); fixture.returnKey()
        XCTAssertEqual(pressedNodes, [])
        XCTAssertEqual(fixture.inner.commands, [#selector(NSResponder.moveDown(_:)), #selector(NSResponder.insertNewline(_:))])
        fixture.key("stash-Enter", node: 8); fixture.key("stash-Delete", node: 9)
        fixture.inner.commands = []
        fixture.returnKey(); fixture.send("\u{7f}", 51, .command)
        XCTAssertEqual(pressedNodes, [8, 9], "the stash menu's Return and ⌘⌫")
        fixture.editor.setMarkedText("あ", selectedRange: NSRange(location: 1, length: 0), replacementRange: fixture.editor.selectedRange())
        pressedNodes = []
        XCTAssertEqual(fixture.editor.delegate?.textView?(fixture.editor, doCommandBy: #selector(NSResponder.insertNewline(_:))), true)
        XCTAssertEqual(pressedNodes, [], "a composition keeps Return")
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
        fixture.sync(["foldLimit": 100_000, "localEnvironment": true])
        let large = String(repeating: "x", count: 40_000), range = NSRange(location: 0, length: 0)
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large))
        let oldID = adapter.folds.first?["id"] as? Int ?? 0
        fixture.replaceOwner()
        XCTAssertTrue(adapter.folds.isEmpty)
        fixture.sync(["foldLimit": 100_000, "localEnvironment": true]) // old pair
        XCTAssertEqual(adapter.foldLimit, 0); XCTAssertFalse(adapter.localEnvironment)
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
        adapter.pastingAsText = true
        XCTAssertTrue(adapter.shouldChange(fixture.editor, range: NSRange(location: 3, length: 0), text: huge))
        XCTAssertTrue(adapter.folds.isEmpty)
        XCTAssertEqual(fixture.editor.string, "abc")
    }

    func testFolderDropAndFoldShareBoundedMonotonicAcknowledgedFacts() throws {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("t3-drop-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let drag = NSPasteboard(name: NSPasteboard.Name("t3-drop-\(UUID().uuidString)"))
        defer { drag.releaseGlobally() }
        drag.writeObjects([folder as NSURL]); adapter.dragPasteboard = { drag }
        fixture.type("A😀Z"); fixture.sync(["foldLimit": 100_000, "localEnvironment": true])
        let range = NSRange(location: 1, length: 2)
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: folder.path))
        let drop = try XCTUnwrap(adapter.drops.first), firstID = try XCTUnwrap(drop["id"] as? Int)
        XCTAssertEqual(drop["value"] as? String, "A😀Z"); XCTAssertEqual(drop["expect"] as? String, "😀")
        XCTAssertEqual(drop["paths"] as? [String], [folder.path])
        XCTAssertEqual(drop["snapshotOwner"] as? String, "owner-a"); XCTAssertEqual(drop["fieldOwner"] as? String, "field-a")
        XCTAssertEqual(fixture.editor.string, "A😀Z", "folder source is never inserted by Swift")
        adapter.pasteDetector = { _ in true }
        let large = String(repeating: "x", count: 40_000)
        for _ in 0..<3 { XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: large)) }
        XCTAssertEqual(adapter.folds.count + adapter.drops.count, 4)
        let ids = [firstID] + adapter.folds.compactMap { $0["id"] as? Int }
        XCTAssertEqual(ids, Array(firstID...firstID + 3))
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: folder.path))
        XCTAssertEqual(adapter.folds.count + adapter.drops.count, 4)
        XCTAssertEqual((fixture.sync()["notices"] as? [[String: String]])?.first?["title"], "Too many pending pastes or drops")
        adapter.forgetFold(firstID)
        XCTAssertTrue(adapter.drops.isEmpty)
        XCTAssertEqual(adapter.folds.count, 3)
        fixture.sync(["localEnvironment": false])
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: range, text: folder.path))
        XCTAssertEqual((fixture.sync()["notices"] as? [[String: String]])?.first?["title"], "Folders can't be dropped into remote environments")
    }

    func testTooManyFoldersRefusesTheEntireDrop() throws {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-folders-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let folders = try (0..<101).map { index -> URL in
            let folder = root.appendingPathComponent(String(index))
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            return folder
        }
        let drag = NSPasteboard(name: NSPasteboard.Name("t3-drop-\(UUID().uuidString)"))
        defer { drag.releaseGlobally() }
        drag.writeObjects(folders as [NSURL]); adapter.dragPasteboard = { drag }
        fixture.sync(["localEnvironment": true])
        XCTAssertFalse(adapter.shouldChange(fixture.editor, range: NSRange(location: 0, length: 0), text: folders.map(\.path).joined(separator: "\n")))
        XCTAssertTrue(adapter.drops.isEmpty)
        XCTAssertEqual(fixture.editor.string, "")
        XCTAssertEqual((fixture.sync()["notices"] as? [[String: String]])?.first?["title"], "Too many folders")
    }

    func testCompositionPassesToTheSharedDelegateWithoutFolderOrFoldClaims() {
        let fixture = EditorFixture(), adapter = fixture.composer.editor
        adapter.pasteDetector = { _ in true }; fixture.sync(["foldLimit": 100_000])
        fixture.editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(adapter.shouldChange(fixture.editor, range: NSRange(location: 0, length: 1), text: String(repeating: "x", count: 40_000)))
        XCTAssertTrue(adapter.folds.isEmpty)
        XCTAssertTrue(fixture.editor.hasMarkedText())
    }
}
