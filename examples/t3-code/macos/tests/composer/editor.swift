import AppKit
import XCTest

// The prompt editor's behaviours (T3ComposerEditor.swift, T3ComposerText.swift,
// T3ComposerStyler.swift) on a real AppKit text view behind the real
// delegate proxy, driven by real key events through NSWindow.sendEvent.

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
        element.data = ExactData(["snapshot-owner": "owner-a"])
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
    func testContextInsertionRetainsCaretAndSelectionAfterComposerBlur() {
        for blurred in [false, true] {
            for replacing in [false, true] {
                let fixture = EditorFixture()
                fixture.type("LEFT RIGHT")
                let range = NSRange(location: 5, length: replacing ? 5 : 0)
                fixture.editor.setSelectedRange(range)
                if blurred {
                    let terminal = NSTextView(frame: NSRect(x: 0, y: 140, width: 100, height: 30))
                    fixture.window.contentView?.addSubview(terminal)
                    XCTAssertTrue(fixture.window.makeFirstResponder(terminal))
                    XCTAssertEqual(fixture.editor.selectedRange(), range)
                }
                let result = fixture.composer.editor.insert(["text": "[Terminal 1](t3-context://v1/terminal/probe)"])
                XCTAssertEqual(result["applied"] as? Bool, true)
                XCTAssertEqual(fixture.editor.string, "LEFT [Terminal 1](t3-context://v1/terminal/probe) " + (replacing ? "" : "RIGHT"),
                               "insertion must preserve the retained selection even after a terminal menu takes focus")
                XCTAssertTrue(fixture.window.firstResponder === fixture.editor)
                XCTAssertEqual(fixture.editor.selectedRange().length, 0)
            }
        }
    }

    // MARK: Pure rules

    func testTriggerDetectionMatchesComposerLogic() {
        func t(_ text: String, _ cursor: Int? = nil) -> T3ComposerTrigger? { T3ComposerText.trigger(text as NSString, cursor: cursor ?? (text as NSString).length) }
        XCTAssertEqual(t("/"), T3ComposerTrigger(kind: "slash-command", query: "", start: 0, end: 1))
        XCTAssertEqual(t("/mo"), T3ComposerTrigger(kind: "slash-command", query: "mo", start: 0, end: 3))
        XCTAssertEqual(t("a\n/plan"), T3ComposerTrigger(kind: "slash-command", query: "plan", start: 2, end: 7))
        XCTAssertNil(t("/model x"), "a space ends the command token")
        XCTAssertNil(t("hello /x"), "a slash command opens a line")
        XCTAssertEqual(t("see #12"), T3ComposerTrigger(kind: "pull-request", query: "12", start: 4, end: 7))
        XCTAssertEqual(t("#"), T3ComposerTrigger(kind: "pull-request", query: "", start: 0, end: 1))
        XCTAssertNil(t("#-x"))
        XCTAssertEqual(t("use $sk"), T3ComposerTrigger(kind: "skill", query: "sk", start: 4, end: 7))
        XCTAssertEqual(t("€x"), T3ComposerTrigger(kind: "skill", query: "x", start: 0, end: 2))
        XCTAssertEqual(t("@fix"), T3ComposerTrigger(kind: "path", query: "fix", start: 0, end: 4))
        XCTAssertEqual(t("x @"), T3ComposerTrigger(kind: "path", query: "", start: 2, end: 3))
        XCTAssertNil(t("@fix ", 5))
        XCTAssertEqual(t("@fix more", 4), T3ComposerTrigger(kind: "path", query: "fix", start: 0, end: 4))
    }

    func testListContinuationAndIndent() {
        func enter(_ text: String) -> T3ComposerListEdit? { T3ComposerText.listContinuation(text, cursor: (text as NSString).length) }
        XCTAssertEqual(enter("- one"), T3ComposerListEdit(start: 5, end: 5, replacement: "\n- "))
        XCTAssertEqual(enter("  * two"), T3ComposerListEdit(start: 7, end: 7, replacement: "\n  * "))
        XCTAssertEqual(enter("1. a"), T3ComposerListEdit(start: 4, end: 4, replacement: "\n2. "))
        XCTAssertEqual(enter("09) a"), T3ComposerListEdit(start: 5, end: 5, replacement: "\n10) "))
        XCTAssertEqual(enter("- [x] done"), T3ComposerListEdit(start: 10, end: 10, replacement: "\n- [ ] "))
        XCTAssertEqual(enter("- one\n- "), T3ComposerListEdit(start: 6, end: 8, replacement: ""), "an empty item exits the list")
        XCTAssertNil(enter("plain"))
        XCTAssertNil(T3ComposerText.listContinuation("- one", cursor: 1), "inside the marker")
        XCTAssertEqual(T3ComposerText.listIndent("- one", start: 3, end: 3), T3ComposerListEdit(start: 0, end: 0, replacement: "  "))
        XCTAssertNil(T3ComposerText.listIndent("- one", start: 1, end: 3))
        XCTAssertNil(T3ComposerText.listIndent("one", start: 3, end: 3))
    }

    func testHistorySteps() {
        let entries = [T3ComposerText.HistoryEntry(id: "1", prompt: "first"), T3ComposerText.HistoryEntry(id: "2", prompt: "second")]
        let back = T3ComposerText.historyStep(backward: true, entries: entries, position: nil, current: "")!
        XCTAssertEqual(back.prompt, "second")
        let older = T3ComposerText.historyStep(backward: true, entries: entries, position: back.position, current: "second")!
        XCTAssertEqual(older.prompt, "first")
        XCTAssertNil(T3ComposerText.historyStep(backward: true, entries: entries, position: older.position, current: "first"), "stops at the oldest")
        XCTAssertEqual(T3ComposerText.historyStep(backward: false, entries: entries, position: older.position, current: "first")?.prompt, "second")
        let past = T3ComposerText.historyStep(backward: false, entries: entries, position: back.position, current: "second")!
        XCTAssertNil(past.position); XCTAssertEqual(past.prompt, "")
        XCTAssertNil(T3ComposerText.historyStep(backward: true, entries: entries, position: nil, current: "draft"), "a typed draft never recalls")
        XCTAssertNil(T3ComposerText.historyStep(backward: false, entries: entries, position: back.position, current: "edited"), "editing ends recall")
    }

    func testChipTokens() {
        let thread = "[fixture complete](t3-context://v1/thread/thread_abc)"
        let text = "see \(thread) and [a.md](docs/a.md) @README.md @scope/pkg $deploy $20 [ext](https://x.y) end"
        let chips = T3ComposerText.chips(text)
        XCTAssertEqual(chips.map(\.kind), ["context", "mention", "mention", "skill"])
        XCTAssertEqual(chips.map(\.label), ["fixture complete", "a.md", "README.md", "deploy"])
        XCTAssertEqual(chips[0].detail, "thread")
        XCTAssertEqual(chips[1].detail, "docs/a.md")
        XCTAssertEqual((text as NSString).substring(with: chips[0].range), thread)
        XCTAssertTrue(T3ComposerText.chips("[a.md](docs/a.md)").isEmpty, "a file link becomes a chip once a delimiter follows")
        XCTAssertEqual(T3ComposerText.chips("[Assistant quote](t3-citation://v1/e/t/m?text=hello%20there&start=0&end=5&prefix=&suffix=) ").first?.label, "hello there")
    }

    func testInlineMarksHideTheirMarkers() {
        let (marks, hidden) = T3ComposerStyler.inlineMarks("**bold** and `code` and *it* ~~st~~ x", excluding: [])
        XCTAssertEqual(marks.map(\.kind).sorted(), ["bold", "code", "italic", "strike"])
        XCTAssertEqual(marks.first { $0.kind == "bold" }?.range, NSRange(location: 2, length: 4))
        XCTAssertEqual(marks.first { $0.kind == "code" }?.range, NSRange(location: 14, length: 4))
        XCTAssertEqual(hidden.count, 8)
        let (unclosed, none) = T3ComposerStyler.inlineMarks("**open and `x", excluding: [])
        XCTAssertTrue(unclosed.isEmpty); XCTAssertTrue(none.isEmpty, "unmatched markers stay literal")
        XCTAssertEqual(T3ComposerStyler.taskLines("- [ ] task\n- [x] done\n- [ ]", excluding: []).map(\.checked), [false, true])
    }

    // MARK: The editor on a real text view

    func testProxyForwardsTheNodesNotificationsAndReportsTheTrigger() {
        let fixture = EditorFixture()
        XCTAssertTrue(fixture.editor.delegate !== fixture.inner, "the proxy stands in")
        fixture.type("/")
        XCTAssertGreaterThan(fixture.inner.changes, 0, "Exact's node still hears every change")
        XCTAssertEqual(fixture.composer.editor.trigger?.kind, "slash-command")
        XCTAssertTrue(fixture.topics.contains("t3.editor"))
        let state = fixture.composer.perform(["op": "editorState"])
        let value = state["value"] as? [String: Any]
        XCTAssertEqual((value?["trigger"] as? [String: Any])?["kind"] as? String, "slash-command")
        XCTAssertEqual(value?["owner"] as? String, "owner-a")
        XCTAssertEqual(value?["atStart"] as? Bool, true)
        fixture.composer.remove(fixture.element)
        XCTAssertTrue(fixture.editor.delegate === fixture.inner, "removal restores Exact's delegate")
    }

    func testMenuKeysPressTheContractsButtonsAndEscapeDismisses() {
        let fixture = EditorFixture()
        fixture.key("ArrowDown", node: 11); fixture.key("ArrowUp", node: 12); fixture.key("Enter", node: 13)
        fixture.type("/m")
        fixture.down(); fixture.up(); fixture.returnKey(); fixture.tab()
        XCTAssertEqual(pressedNodes, [11, 12, 13, 13])
        XCTAssertEqual(fixture.editor.string, "/m", "the menu keys never edit the prompt")
        let send = ExactElement(hatch: .t3Send, id: "send-message", node: 2, hatches: fixture.hooks)
        let sendView = NSView(frame: NSRect(x: 0, y: 0, width: 10, height: 10))
        fixture.window.contentView?.addSubview(sendView)
        send.view = sendView
        fixture.composer.install(send)
        let ret = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
        XCTAssertTrue(fixture.composer.handle(ret) === ret, "Return passes the send monitor while the menu owns it")
        XCTAssertFalse(pressedNodes.contains(2))
        fixture.escape()
        XCTAssertNil(fixture.composer.editor.trigger)
        fixture.type("o")
        XCTAssertNil(fixture.composer.editor.trigger, "dismissed until the caret leaves the token")
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 0))
        fixture.editor.setSelectedRange(NSRange(location: 3, length: 0))
        XCTAssertEqual(fixture.composer.editor.trigger?.query, "mo", "leaving the token ends the dismissal")
        fixture.editor.selectAll(nil); fixture.type("x @a")
        XCTAssertEqual(fixture.composer.editor.trigger?.kind, "path")
    }

    func testPickedRowsReplaceOnlyTheTriggerTheyWereBuiltFor() {
        let fixture = EditorFixture()
        fixture.type("hi @fix more")
        fixture.editor.setSelectedRange(NSRange(location: 7, length: 0))
        let trigger = fixture.composer.editor.trigger!
        XCTAssertEqual(trigger, T3ComposerTrigger(kind: "path", query: "fix", start: 3, end: 7))
        let stale = fixture.composer.perform(["op": "editorEdit", "kind": "path", "start": 3, "end": 6, "text": "x "])
        XCTAssertEqual((stale["value"] as? [String: Any])?["applied"] as? Bool, false)
        let applied = fixture.composer.perform(["op": "editorEdit", "kind": "path", "start": 3, "end": 7, "text": "[fixture-result.md](fixture-result.md) ", "extendSpace": true])
        XCTAssertEqual((applied["value"] as? [String: Any])?["applied"] as? Bool, true)
        XCTAssertEqual(fixture.editor.string, "hi [fixture-result.md](fixture-result.md) more")
        XCTAssertEqual(fixture.editor.selectedRange().location, ("hi [fixture-result.md](fixture-result.md) " as NSString).length)
        fixture.editor.undoManager?.undo()
        XCTAssertEqual(fixture.editor.string, "hi @fix more", "a pick is one undo step")
    }

    func testListContinuationTabIndentAndPlanToggleOnRealKeys() {
        let fixture = EditorFixture()
        fixture.key("plan", node: 21)
        fixture.type("- one")
        fixture.returnKey(.shift)
        XCTAssertEqual(fixture.editor.string, "- one\n- ")
        fixture.type("two")
        fixture.returnKey(.shift)
        fixture.tab()
        XCTAssertEqual(fixture.editor.string, "- one\n- two\n  - ")
        fixture.returnKey(.shift)
        XCTAssertEqual(fixture.editor.string, "- one\n- two\n", "an empty item exits the list")
        fixture.backtab()
        XCTAssertEqual(pressedNodes, [21])
        fixture.editor.selectAll(nil); fixture.type("plain")
        fixture.returnKey(.shift)
        XCTAssertEqual(fixture.editor.string, "plain\n", "other lines take a plain newline")
    }

    func testArrowUpRecallsSentPromptsAndDownRestoresTheDraft() {
        let fixture = EditorFixture()
        _ = fixture.composer.perform(["op": "editorSync", "owner": "draft-a", "history": [["id": "m1", "prompt": "fixture complete"], ["id": "m2", "prompt": "two\nlines"]]])
        fixture.up()
        XCTAssertEqual(fixture.editor.string, "two\nlines")
        fixture.up()
        XCTAssertEqual(fixture.editor.string, "two\nlines", "↑ off the first visual line moves the caret")
        fixture.up()
        XCTAssertEqual(fixture.editor.string, "fixture complete")
        fixture.down()
        XCTAssertEqual(fixture.editor.string, "two\nlines")
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 0))
        fixture.down()
        XCTAssertEqual(fixture.editor.string, "two\nlines", "↓ on the first line only moves the caret")
        fixture.editor.setSelectedRange(NSRange(location: (fixture.editor.string as NSString).length, length: 0))
        fixture.down()
        XCTAssertEqual(fixture.editor.string, "", "past the newest entry the empty draft returns")
        fixture.type("draft"); fixture.up()
        XCTAssertEqual(fixture.editor.string, "draft", "a typed draft never recalls")
    }

    func testChipsAreAtomicForTheCaretAndDeletion() {
        let fixture = EditorFixture()
        let chip = "[a.md](docs/a.md)"
        fixture.type("x \(chip) y")
        let chipStart = 2, chipEnd = 2 + (chip as NSString).length
        XCTAssertEqual(fixture.composer.editor.styler.chips.count, 1)
        fixture.editor.setSelectedRange(NSRange(location: chipEnd, length: 0))
        fixture.left()
        XCTAssertEqual(fixture.editor.selectedRange().location, chipStart, "← steps over the chip")
        fixture.right()
        XCTAssertEqual(fixture.editor.selectedRange().location, chipEnd, "→ steps over the chip")
        fixture.editor.setSelectedRange(NSRange(location: chipStart + 3, length: 0))
        XCTAssertTrue([chipStart, chipEnd].contains(fixture.editor.selectedRange().location), "a caret inside a chip snaps to its edge")
        fixture.editor.setSelectedRange(NSRange(location: chipEnd, length: 0))
        fixture.backspace()
        XCTAssertEqual(fixture.editor.string, "x  y", "⌫ removes the whole chip")
        // The hidden source keeps zero width; the chip's space is its first character's kern.
        fixture.editor.selectAll(nil); fixture.type("\(chip) ")
        let kern = fixture.editor.textStorage?.attribute(.kern, at: 0, effectiveRange: nil) as? CGFloat ?? 0
        XCTAssertGreaterThan(kern, 40)
        XCTAssertEqual(fixture.editor.textStorage?.attribute(.foregroundColor, at: 3, effectiveRange: nil) as? NSColor, NSColor.clear)
    }

    func testRichTextStylesMarksAndTogglesTasks() {
        let fixture = EditorFixture()
        fixture.type("**bold** x")
        let font = fixture.editor.textStorage?.attribute(.font, at: 3, effectiveRange: nil) as? NSFont
        XCTAssertTrue(font?.fontDescriptor.symbolicTraits.contains(.bold) == true)
        XCTAssertEqual(fixture.editor.textStorage?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, NSColor.clear)
        _ = fixture.composer.perform(["op": "editorSync", "richText": false])
        let plain = fixture.editor.textStorage?.attribute(.font, at: 3, effectiveRange: nil) as? NSFont
        XCTAssertFalse(plain?.fontDescriptor.symbolicTraits.contains(.bold) == true, "rich text off is plain text")
        _ = fixture.composer.perform(["op": "editorSync", "richText": true])
        fixture.editor.selectAll(nil); fixture.type("- [ ] task")
        let styler = fixture.composer.editor.styler
        XCTAssertEqual(styler.tasks.count, 1)
        fixture.composer.editor.apply(styler.tasks[0].box, "[x]", in: fixture.editor)
        XCTAssertEqual(fixture.editor.string, "- [x] task")
    }

    func testInkAuthoredOnAnEmptyPromptReachesTypedText() {
        let fixture = EditorFixture()
        fixture.editor.textColor = NSColor(t3Hex: "#f5f5f5") // a dark-scheme ink, written first
        fixture.tick()
        let light = NSColor(t3Hex: "#27272a")
        fixture.editor.textColor = light // the scheme flips while the prompt is empty
        fixture.tick()
        fixture.type("/")
        fixture.tick()
        XCTAssertEqual(fixture.editor.textStorage?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, light)
        fixture.editor.font = NSFont.systemFont(ofSize: 20)
        fixture.tick()
        XCTAssertEqual((fixture.editor.textStorage?.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)?.pointSize, 20)
    }

    func testCommandBAndIToggleMarksOnTheSelection() {
        let fixture = EditorFixture()
        fixture.type("make this bold")
        fixture.editor.setSelectedRange(NSRange(location: 10, length: 4))
        let bold = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "b", charactersIgnoringModifiers: "b", isARepeat: false, keyCode: 11)!
        XCTAssertNil(fixture.composer.handle(bold))
        XCTAssertEqual(fixture.editor.string, "make this **bold**")
        XCTAssertEqual(fixture.editor.selectedRange(), NSRange(location: 12, length: 4))
        XCTAssertNil(fixture.composer.handle(bold))
        XCTAssertEqual(fixture.editor.string, "make this bold", "⌘B again unwraps")
        let italic = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "i", charactersIgnoringModifiers: "i", isARepeat: false, keyCode: 34)!
        fixture.editor.setSelectedRange(NSRange(location: 14, length: 0))
        XCTAssertNil(fixture.composer.handle(italic))
        fixture.type("x")
        XCTAssertEqual(fixture.editor.string, "make this bold*x*", "an empty selection opens a span for typing")
        _ = fixture.composer.perform(["op": "editorSync", "richText": false])
        XCTAssertTrue(fixture.composer.handle(bold) === bold, "plain text composer leaves ⌘B alone")
    }

    func testPasteBeyondThePromptLimitIsRefusedWithANotice() {
        let fixture = EditorFixture()
        fixture.composer.editor.pasteDetector = { _ in true }
        fixture.type("abc")
        let huge = String(repeating: "x", count: 120_000)
        fixture.editor.insertText(huge, replacementRange: fixture.editor.selectedRange())
        XCTAssertEqual(fixture.editor.string, "abc")
        let notices = fixture.composer.editor.takeNotices()
        XCTAssertEqual(notices.first?["title"], "Pasted text is too large for this message")
        fixture.composer.editor.pastingAsText = true
        fixture.editor.insertText(huge, replacementRange: fixture.editor.selectedRange())
        XCTAssertEqual((fixture.editor.string as NSString).length, 120_003, "⇧⌘V keeps a large paste inline")
    }

    func testLargePasteFoldsIntoAFileChipWhereThePasteWent() {
        let fixture = EditorFixture()
        fixture.composer.editor.pasteDetector = { _ in true }
        fixture.type("before")
        let large = String(repeating: "lorem ipsum ", count: 3_400) // 40,800 bytes
        // No file staging: a paste under the prompt limit stays inline.
        fixture.editor.insertText(large, replacementRange: fixture.editor.selectedRange())
        XCTAssertEqual((fixture.editor.string as NSString).length, 6 + 40_800)
        fixture.editor.string = "before"
        fixture.editor.setSelectedRange(NSRange(location: 6, length: 0))
        _ = fixture.composer.perform(["op": "editorSync", "foldLimit": 50 * 1024 * 1024])
        fixture.editor.insertText(large, replacementRange: fixture.editor.selectedRange())
        XCTAssertEqual(fixture.editor.string, "before", "the paste is held for TypeScript")
        let synced = fixture.composer.perform(["op": "editorSync"])
        let folds = (synced["value"] as? [String: Any])?["folds"] as? [[String: Any]] ?? []
        XCTAssertEqual(folds.count, 1)
        XCTAssertEqual(folds.first?["start"] as? Int, 6)
        XCTAssertEqual(folds.first?["expect"] as? String, "")
        XCTAssertEqual((folds.first?["text"] as? String)?.utf8.count, 40_800)
        let again = (fixture.composer.perform(["op": "editorSync"])["value"] as? [String: Any])?["folds"] as? [[String: Any]] ?? []
        XCTAssertEqual(again.count, 1, "a fold stays until an edit names it (a lost sync retries)")
        let chip = "[pasted-text.txt](t3-context://v1/file/file_abc) "
        let edited = fixture.composer.perform(["op": "editorEdit", "start": 6, "end": 6, "expect": "", "text": chip, "pad": true, "fold": folds.first?["id"] as? Int ?? -1])
        XCTAssertEqual((edited["value"] as? [String: Any])?["applied"] as? Bool, true)
        XCTAssertEqual(fixture.editor.string, "before " + chip)
        XCTAssertTrue(fixture.composer.editor.folds.isEmpty)
        // A file larger than the server takes is refused only past the prompt limit.
        _ = fixture.composer.perform(["op": "editorSync", "foldLimit": 1_000])
        fixture.editor.insertText(String(repeating: "y", count: 120_001), replacementRange: fixture.editor.selectedRange())
        XCTAssertEqual(fixture.composer.editor.takeNotices().first?["title"], "Pasted text is too large to attach")
        // ⇧⌘V never folds.
        fixture.composer.editor.pastingAsText = true
        _ = fixture.composer.perform(["op": "editorSync", "foldLimit": 50 * 1024 * 1024])
        fixture.editor.insertText(large, replacementRange: fixture.editor.selectedRange())
        XCTAssertTrue(fixture.editor.string.hasSuffix(large))
    }

    func testChipsSitHalfAnXHeightAboveTheBaseline() {
        let fixture = EditorFixture()
        fixture.type("see [a.md](a.md) x\nnext [b.md](b.md) y")
        let styler = fixture.composer.editor.styler
        let font = NSFont.systemFont(ofSize: 14)
        guard let first = styler.baseline(at: 0), let second = styler.baseline(at: 25) else { return XCTFail("no baseline") }
        let line = styler.segments(NSRange(location: 0, length: 1)).first!
        XCTAssertGreaterThan(first, line.minY + font.ascender * 0.5)
        XCTAssertLessThanOrEqual(first, line.maxY)
        XCTAssertGreaterThan(second - first, 10, "the second line's baseline is a line lower")
        let top = styler.middleAligned(height: 17, at: 4, fallback: .zero)
        XCTAssertEqual(top + 8.5, first - font.xHeight / 2, accuracy: 0.01)
    }

    func testRedrawingAfterTheTextIsReplacedKeepsTextKit2() {
        let fixture = EditorFixture()
        let textKit2 = fixture.editor.textLayoutManager != nil
        fixture.type("see [fixture complete](t3-context://v1/thread/thread_x) and [a.md](a.md) $x")
        fixture.window.display()
        // Exact's value write replaces the storage; the underlay may draw before the restyle.
        fixture.editor.textStorage?.replaceCharacters(in: NSRange(location: 0, length: (fixture.editor.string as NSString).length), with: "**bold** and `code`")
        fixture.window.display()
        fixture.tick()
        fixture.window.display()
        XCTAssertEqual(fixture.editor.textLayoutManager != nil, textKit2, "drawing never drops the view to TextKit 1")
        XCTAssertEqual(fixture.composer.editor.styler.chips.count, 0)
    }

    func testReplacingChipsWithShorterTextNeverSnapsOnTheOldChips() {
        let fixture = EditorFixture()
        fixture.type("see [fixture complete](t3-context://v1/thread/thread_x) and [a.md](a.md) $x")
        // The agent's (and ⌘A then typing's) path: select all, insert. NSTextView moves the
        // selection before announcing the change, while the chips still describe the old text.
        fixture.editor.selectAll(nil)
        fixture.editor.insertText("**bold** and `code`", replacementRange: fixture.editor.selectedRange())
        fixture.tick()
        XCTAssertEqual(fixture.editor.string, "**bold** and `code`")
        XCTAssertEqual(fixture.editor.selectedRange(), NSRange(location: 19, length: 0))
    }

    func testAChipStaysAChipOnceTheSpaceAfterItIsGone() {
        let fixture = EditorFixture()
        fixture.type("see [a.md](a.md) ")
        let styler = fixture.composer.editor.styler
        XCTAssertEqual(styler.chips.count, 1)
        fixture.backspace() // the trailing space
        XCTAssertEqual(fixture.editor.string, "see [a.md](a.md)")
        XCTAssertEqual(styler.chips.map(\.label), ["a.md"], "the source alone would not parse at the end, the node stays")
        fixture.type("x") // typing right after the chip
        XCTAssertEqual(styler.chips.count, 1)
        fixture.backspace()
        fixture.backspace() // now the whole chip
        XCTAssertEqual(fixture.editor.string, "see ")
        XCTAssertEqual(styler.chips.count, 0)
        // A value written from outside is parsed afresh.
        fixture.editor.string = "[b.md](b.md)"
        fixture.tick()
        styler.restyle()
        XCTAssertEqual(styler.chips.count, 0)
    }

    func testEditorStateCarriesTheLiveTextForTheStash() {
        let fixture = EditorFixture()
        fixture.type("stash two")
        let state = fixture.composer.perform(["op": "editorState"])["value"] as? [String: Any]
        XCTAssertEqual(state?["text"] as? String, "stash two")
    }

    func testFileChipsDrawTheirSizeAndTheTextIcon() {
        let fixture = EditorFixture()
        let styler = fixture.composer.editor.styler
        styler.contexts = ["file/file_abc": "file\t40 KB"]
        fixture.type("see [pasted-text.txt](t3-context://v1/file/file_abc) x")
        let chip = styler.chips.first!
        XCTAssertEqual(styler.chipKind(chip), "file")
        XCTAssertEqual(styler.chipSuffix(chip), "40 KB")
        XCTAssertTrue(styler.chipResolved(chip))
        XCTAssertEqual(T3ComposerIcons.name(for: chip, kind: "file"), "pierre:text")
        let font = NSFont.systemFont(ofSize: 14)
        styler.contexts = ["file/file_abc": "file"]
        XCTAssertLessThan(styler.chipWidth(chip, font: font) + 20, { styler.contexts = ["file/file_abc": "file\t40 KB"]; return styler.chipWidth(chip, font: font) }())
    }

    func testFinderFolderDropInsertsAFileLinkAtTheEnd() {
        let fixture = EditorFixture()
        let folder = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3 drop (1)")
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let drag = NSPasteboard(name: NSPasteboard.Name("t3-composer-tests-drag"))
        drag.clearContents(); drag.writeObjects([folder as NSURL])
        fixture.composer.editor.dragPasteboard = { drag }
        fixture.type("see")
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 0))
        fixture.editor.insertText(folder.path, replacementRange: fixture.editor.selectedRange())
        // The drop applies on the next main-queue turn; other run-loop sources may take a turn first.
        for _ in 0..<5 where fixture.editor.string == "see" { fixture.tick() }
        XCTAssertEqual(fixture.editor.string, "see [t3 drop (1)](\(folder.path.replacingOccurrences(of: " ", with: "%20").replacingOccurrences(of: "(", with: "%28").replacingOccurrences(of: ")", with: "%29"))) ")
        fixture.composer.editor.localEnvironment = false
        fixture.editor.insertText(folder.path, replacementRange: fixture.editor.selectedRange())
        fixture.tick()
        XCTAssertEqual(fixture.composer.editor.takeNotices().first?["title"], "Folders can't be dropped into remote environments")
        drag.releaseGlobally()
        try? FileManager.default.removeItem(at: folder)
    }

    func testUndoGroupsSplitBetweenInsertingAndDeleting() {
        let fixture = EditorFixture()
        var clock: TimeInterval = 100
        fixture.composer.editor.now = { clock }
        for character in "abc" { fixture.type(String(character)); clock += 0.1 }
        fixture.backspace(); clock += 0.1
        fixture.type("d"); clock += 2
        fixture.type("e")
        XCTAssertEqual(fixture.editor.string, "abde")
        fixture.editor.undoManager?.undo()
        XCTAssertEqual(fixture.editor.string, "abd", "a pause over 1 s starts a step")
        fixture.editor.undoManager?.undo()
        XCTAssertEqual(fixture.editor.string, "ab", "switching to inserting starts a step")
        fixture.editor.undoManager?.undo()
        XCTAssertEqual(fixture.editor.string, "abc", "the deletion was its own step")
    }

    func testTypeToFocusAndPasteToFocusReachTheComposer() {
        let fixture = EditorFixture()
        fixture.type("hi")
        fixture.window.makeFirstResponder(nil)
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "x", charactersIgnoringModifiers: "x", isARepeat: false, keyCode: 7)!
        XCTAssertNil(fixture.composer.handle(event))
        XCTAssertEqual(fixture.editor.string, "hix")
        XCTAssertTrue(fixture.window.firstResponder === fixture.editor)
        let field = NSTextField(frame: NSRect(x: 0, y: 150, width: 100, height: 20))
        fixture.window.contentView?.addSubview(field)
        fixture.window.makeFirstResponder(field)
        let typed = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "y", charactersIgnoringModifiers: "y", isARepeat: false, keyCode: 16)!
        XCTAssertTrue(fixture.composer.handle(typed) === typed, "an editable field keeps its keys")
        fixture.window.makeFirstResponder(nil)
        let command = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        XCTAssertTrue(fixture.composer.handle(command) === command, "chords are not typing")
    }
}
