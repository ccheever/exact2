#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// x2apps codeedit #2 on AppKit: a text field's `input` reports the
/// selection its edit left; its `select` fires for the person's
/// non-collapsed selection and for a script's `setSelectionRange`, which
/// never focuses the field, waits for its focus when it has none, and is
/// clamped as HTML clamps it.
final class FieldSelectionMacTests: XCTestCase {
    private var window: NSWindow!
    private var selects: [String] = []

    override func tearDown() { window?.close(); window = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.fieldSelections.onSelect = { [unowned self] id, value, s in selects.append("\(id) \(value) \(s.start)-\(s.end) \(s.direction)") }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "box"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 140.0, "w": 30.0, "h": 24.0],
        ]))
        return p
    }

    func testTypingReportsTheCaretAndAScriptsRangeWaitsForTheFocus() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects, ["2 hello 1-3 backward"])
        XCTAssertFalse(window.firstResponder === editor, "setSelectionRange does not focus")
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects.count, 1, "the same selection set again fires nothing")
        window.makeFirstResponder(editor)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 1, length: 2))
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string), FieldSelection(start: 1, end: 3, direction: "backward"))
        editor.insertText("x", replacementRange: editor.selectedRange())
        XCTAssertEqual(editor.string, "hxlo")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string), FieldSelection(start: 2, end: 2), "typing leaves a caret, direction none")
        XCTAssertEqual(selects.count, 1, "typing is no select")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.last, "2 hxlo 2-2 none", "a script's caret is a select, even where the caret was")
        // A person's Shift-extension: right is forward, then back past the anchor is backward.
        editor.moveRightAndModifySelection(nil)
        XCTAssertEqual(selects.last, "2 hxlo 2-3 forward")
        editor.moveLeftAndModifySelection(nil)
        XCTAssertEqual(selects.count, 3, "back to a caret: no select")
        editor.moveLeftAndModifySelection(nil)
        XCTAssertEqual(selects.last, "2 hxlo 1-2 backward")
        editor.moveRight(nil)
        XCTAssertEqual(selects.count, 4, "a caret move is no select")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.string)?.direction, "none")
    }

    func testAnInputsFieldEditorTakesTheScriptsRangeNotItsSelectAll() throws {
        let p = presenter()
        let field = try XCTUnwrap(p.views[3]?.field)
        // ToUint32: -1 is past the end, clamped to the length.
        p.fieldSelections.setSelectionRange(["line", 1, -1])
        XCTAssertEqual(selects, ["3 hello 1-5 none"])
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 1, length: 4))
        XCTAssertEqual(selects.count, 1, "the focus's own selection is no select")
        p.fieldSelections.setSelectionRange(["line", 9, 2, "forward"])
        XCTAssertEqual(selects.last, "3 hello 2-2 forward", "an end before the start moves the start to it")
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 2, length: 0))
        editor.insertText("y", replacementRange: editor.selectedRange())
        XCTAssertEqual(p.fieldSelections.reported(3, field.stringValue), FieldSelection(start: 3, end: 3))
        p.fieldSelections.selectAll(try XCTUnwrap(p.views[3]))
        XCTAssertEqual(selects.last, "3 heyllo 0-6 none", "selectText is the DOM's select()")
        // Refusals change nothing.
        p.fieldSelections.setSelectionRange(["box", 0, 1])
        p.fieldSelections.setSelectionRange(["missing", 0, 1])
        p.fieldSelections.setSelectionRange(["line"])
        XCTAssertEqual(selects.count, 3)
        XCTAssertNil(p.fieldSelections.reported(4, ""), "a box is no text field")
    }

    func testClampingIsHTMLs() {
        XCTAssertEqual(FieldSelection.clamped(-1, -1, nil, length: 4), FieldSelection(start: 4, end: 4))
        XCTAssertEqual(FieldSelection.clamped(.nan, .infinity, "sideways", length: 4), FieldSelection(start: 0, end: 0))
        XCTAssertEqual(FieldSelection.clamped(1.9, 3.2, "backward", length: 4), FieldSelection(start: 1, end: 3, direction: "backward"))
        XCTAssertEqual(FieldSelection(start: 1, end: 3, direction: "forward").payload("a,b\nc"), "1,3,forward,a,b\nc")
    }
    func testRangeTextPreservesSelectionWithoutFocusOrUndo() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[2])
        p.fieldSelections.setSelectionRange(["editor", 1, 4, "backward"])
        p.fieldSelections.setRangeText(["editor", "XY", 2, 3])
        XCTAssertEqual(node.fieldText, "heXYlo")
        XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 1, end: 5))
        XCTAssertFalse(node.editingField)
        XCTAssertFalse(node.textArea?.undoManager?.canUndo ?? false)
        p.fieldSelections.setRangeText(["line", "!", 1, 4, "select"])
        XCTAssertEqual(p.views[3]?.fieldText, "h!o")
        XCTAssertEqual(p.fieldSelections.current(try XCTUnwrap(p.views[3])), FieldSelection(start: 1, end: 2))
    }

    func testRangeTextModesUTF16RefusalsAndOneInput() throws {
        let p = presenter()
        var inputs: [String] = []
        p.onInput = { _, _ in inputs.append("unexpected user input") }
        p.fieldSelections.onReplacement = { id, value, s in inputs.append("\(id):\(value):\(s.start)-\(s.end)") }
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            for (mode, a, b) in [("preserve", 1, 4), ("select", 1, 2), ("start", 1, 1), ("end", 2, 2)] {
                node.replaceFieldValue("a😀bc")
                p.fieldSelections.setSelectionRange([name, 3, 5, "backward"])
                let before = inputs.count
                p.fieldSelections.setRangeText([name, "X", 1, 3, mode])
                XCTAssertEqual(node.fieldText, "aXbc")
                XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: a, end: b))
                XCTAssertEqual(inputs.count, before + 1)
                XCTAssertEqual(inputs.last, "\(id):aXbc:\(a)-\(b)")
            }
            node.replaceFieldValue("see @a.ts now")
            p.fieldSelections.setRangeText([name, "a\r\nb", 4, 9, "end"])
            XCTAssertEqual(node.fieldText, name == "editor" ? "see a\nb now" : "see ab now")
            XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 8, end: 8), "Chrome counts the raw replacement before newline sanitization")
            p.fieldSelections.setSelectionRange([name, 0, 3])
            p.fieldSelections.setRangeText([name, "See"])
            XCTAssertTrue(node.fieldText.hasPrefix("See"))
            let before = inputs.count, value = node.fieldText
            let refusals: [[Any]] = [[name, "bad", 9, 4], [name, "bad", 0, 1, "invalid"], ["missing", "bad"]]
            for args in refusals {
                p.fieldSelections.setRangeText(args)
            }
            XCTAssertEqual(node.fieldText, value)
            XCTAssertEqual(inputs.count, before)
            p.fieldSelections.setRangeText([name, "", -1, -1, "end"])
            XCTAssertEqual(inputs.count, before + 1, "a successful no-op still emits input")
        }
    }

    func testRangeTextWhileEditingAddsNoUndoStep() throws {
        let p = presenter()
        var replacements = 0
        p.onInput = { _, _ in XCTFail("a scripted edit must not also emit a user input") }
        p.fieldSelections.onReplacement = { _, _, _ in replacements += 1 }
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            #if os(macOS)
            if let editor = node.textArea { window.makeFirstResponder(editor) }
            else { window.makeFirstResponder(node.field) }
            let editor = try XCTUnwrap(node.textArea ?? node.field?.currentEditor() as? NSTextView)
            editor.undoManager?.removeAllActions()
            #else
            if let editor = node.textArea { XCTAssertTrue(editor.becomeFirstResponder()) }
            else { XCTAssertTrue(node.field?.becomeFirstResponder() ?? false) }
            let undo = node.textArea?.undoManager ?? node.field?.undoManager
            undo?.removeAllActions()
            #endif
            p.fieldSelections.setSelectionRange([name, 1, 4])
            let before = replacements
            p.fieldSelections.setRangeText([name, "X"])
            XCTAssertEqual(replacements, before + 1)
            XCTAssertEqual(node.fieldText, "hXo")
            XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 1, end: 2))
            #if os(macOS)
            XCTAssertEqual(editor.string, "hXo", "the NSTextField's active editor agrees")
            XCTAssertFalse(editor.undoManager?.canUndo ?? false)
            #else
            XCTAssertFalse(undo?.canUndo ?? false)
            #endif
            XCTAssertTrue(node.editingField)
        }
    }

    func testRangeReplacementDoesNotSwallowTheNextUserDeletion() throws {
        let p = presenter()
        var inputs: [String] = []
        p.onInput = { _, value in inputs.append(value) }
        p.fieldSelections.onReplacement = { _, value, _ in inputs.append(value) }
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            node.props["value"] = ""
            node.replaceFieldValue("")
            #if os(macOS)
            window.makeFirstResponder(node.textArea ?? node.field)
            let editor = try XCTUnwrap(node.textArea ?? node.field?.currentEditor() as? NSTextView)
            #else
            if let editor = node.textArea { XCTAssertTrue(editor.becomeFirstResponder()) }
            else { XCTAssertTrue(node.field?.becomeFirstResponder() ?? false) }
            #endif
            p.edited = nil
            inputs.removeAll()
            p.fieldSelections.setRangeText([name, "x", 0, 0, "end"])
            XCTAssertNil(p.edited, "a script is no pending user change")
            #if os(macOS)
            editor.deleteBackward(nil)
            #else
            if let editor = node.textArea { editor.deleteBackward() } else { node.field?.deleteBackward() }
            #endif
            XCTAssertEqual(node.fieldText, "")
            XCTAssertEqual(inputs, ["x", ""])
            XCTAssertEqual(p.edited, id, "the deletion must commit on blur")
        }
    }

    func testRangeReplacementSanitizesURLWhitespace() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        node.props["type"] = "url"
        var inputs: [String] = []
        p.fieldSelections.onReplacement = { _, value, _ in inputs.append(value) }
        for (raw, expected) in [(" \t\nhttps://example.com\r\u{0c} ", "https://example.com"), ("\u{00a0}x\u{00a0}", "\u{00a0}x\u{00a0}")] {
            node.replaceFieldValue("")
            p.fieldSelections.setRangeText(["line", raw, 0, 0, "end"])
            XCTAssertEqual(node.fieldText, expected)
            XCTAssertEqual(inputs.last, expected)
            XCTAssertEqual(p.fieldSelections.current(node).start, expected.utf16.count)
        }
    }

    func testRangeReplacementSelectOnlyWhenSelectionChangesAfterInput() throws {
        let p = presenter()
        var events: [String] = []
        p.fieldSelections.onSelect = { _, _, _ in events.append("select") }
        p.fieldSelections.onReplacement = { _, _, _ in events.append("input") }
        for name in ["editor", "line"] {
            p.fieldSelections.setSelectionRange([name, 0, 0])
            events.removeAll()
            p.fieldSelections.setRangeText([name, "!", 2, 2])
            XCTAssertEqual(events, ["input"], "Chrome emits no select for the unchanged caret")
            events.removeAll()
            p.fieldSelections.setRangeText([name, "!", 2, 2, "end"])
            XCTAssertEqual(events, ["input"], "select is queued after the command")
            drainSelectTasks()
            XCTAssertEqual(events, ["input", "select"], "Chrome delivers select after the added input")
        }
    }

    func testHatchInputReportsReplacementAndKeepsPendingChange() throws {
        let p = presenter()
        var events: [String] = []
        p.onInput = { _, _ in events.append("ordinary") }
        p.fieldSelections.onReplacement = { _, value, s in events.append("\(value):\(s.start)-\(s.end)") }
        for id in [UInt32(2), UInt32(3)] {
            events.removeAll()
            XCTAssertTrue(p.elements.input(id, "Palo"))
            p.elements.drain()
            XCTAssertEqual(events, ["Palo:4-4"])
            XCTAssertEqual(p.edited, id)
        }
    }

    private func drainSelectTasks() {
        let done = expectation(description: "queued select tasks finished")
        DispatchQueue.main.async { done.fulfill() }
        wait(for: [done], timeout: 5)
    }

    func testRangeReplacementQueuesSelectAfterBothCommands() throws {
        let p = presenter()
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            node.replaceFieldValue("")
            var events: [String] = []
            p.fieldSelections.onReplacement = { _, value, _ in events.append("input:\(value)") }
            p.fieldSelections.onSelect = { _, value, _ in events.append("select:\(value)") }
            p.fieldSelections.setRangeText([name, "A", 0, 0, "end"])
            p.fieldSelections.setRangeText([name, "B", 1, 1, "end"])
            XCTAssertEqual(events, ["input:A", "input:AB"])
            drainSelectTasks()
            XCTAssertEqual(events, ["input:A", "input:AB", "select:AB", "select:AB"])
        }
    }

    func testRangeReplacementDirectionMatchesMacChrome() throws {
        let p = presenter()
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            p.fieldSelections.setSelectionRange([name, 0, 2, "forward"])
            drainSelectTasks()
            var events: [String] = []
            p.fieldSelections.onReplacement = { _, _, s in events.append("input:\(s.direction)") }
            p.fieldSelections.onSelect = { _, _, s in events.append("select:\(s.direction)") }
            p.fieldSelections.setRangeText([name, "!", 4, 4])
            drainSelectTasks()
            XCTAssertEqual(node.fieldText, "hell!o")
            XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 0, end: 2))
            XCTAssertEqual(events, ["input:none", "select:none"])
            node.replaceFieldValue("hello")
            p.fieldSelections.setSelectionRange([name, 1, 4, "backward"])
            drainSelectTasks()
            events.removeAll()
            p.fieldSelections.setRangeText([name, "XY", 2, 3])
            drainSelectTasks()
            XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 1, end: 5))
            XCTAssertEqual(events, ["input:none", "select:none"])
        }
    }

}
#endif
