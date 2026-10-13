#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// x2apps codeedit #2 on UIKit: a field's `input` reports the selection its
/// edit left; `setSelectionRange` fires `select`, never focuses, and waits
/// for an input's focus when it has none; a person's non-collapsed selection
/// is a `select`, typing is not. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class FieldSelectionIOSTests: XCTestCase {
    private var window: UIWindow!
    private var selects: [String] = []

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.fieldSelections.onSelect = { [unowned self] id, value, s in selects.append("\(id) \(value) \(s.start)-\(s.end) \(s.direction)") }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 30.0],
        ]))
        return p
    }

    func testATextViewTakesTheScriptsRangeAndTypingReportsTheCaret() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        p.fieldSelections.setSelectionRange(["editor", 1, 3, "backward"])
        XCTAssertEqual(selects, ["2 hello 1-3 backward"])
        XCTAssertFalse(editor.isFirstResponder, "setSelectionRange does not focus")
        XCTAssertTrue(editor.becomeFirstResponder())
        XCTAssertEqual(editor.selectedRange, NSRange(location: 1, length: 2))
        editor.insertText("x")
        XCTAssertEqual(editor.text, "hxlo")
        XCTAssertEqual(p.fieldSelections.reported(2, editor.text), FieldSelection(start: 2, end: 2))
        XCTAssertEqual(selects.count, 1, "typing is no select")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.last, "2 hxlo 2-2 none")
        p.fieldSelections.setSelectionRange(["editor", 2, 2])
        XCTAssertEqual(selects.count, 2, "the same selection again: nothing")
        editor.selectedRange = NSRange(location: 2, length: 1)
        XCTAssertEqual(selects.last, "2 hxlo 2-3 forward", "a person's extension is a select")
    }

    func testAnInputWaitsForItsFocus() throws {
        let p = presenter()
        let field = try XCTUnwrap(p.views[3]?.field)
        p.fieldSelections.setSelectionRange(["line", 1, 4, "forward"])
        XCTAssertEqual(selects, ["3 hello 1-4 forward"])
        XCTAssertTrue(field.becomeFirstResponder())
        let range = try XCTUnwrap(field.selectedTextRange)
        XCTAssertEqual(field.offset(from: field.beginningOfDocument, to: range.start), 1)
        XCTAssertEqual(field.offset(from: field.beginningOfDocument, to: range.end), 4)
        field.insertText("y")
        XCTAssertEqual(field.text, "hyo")
        XCTAssertEqual(p.fieldSelections.reported(3, field.text ?? ""), FieldSelection(start: 2, end: 2))
        XCTAssertEqual(selects.count, 1)
    }

    func testPlainTextAreaKeepsUIKitUndoAndPublishesUndoRedo() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[2])
        let editor = try XCTUnwrap(node.textArea)
        XCTAssertTrue(editor.becomeFirstResponder())
        editor.selectedRange = NSRange(location: 5, length: 0)
        let manager = try XCTUnwrap(editor.undoManager)
        manager.removeAllActions()
        var inputs: [String] = []
        p.onInput = { _, value in inputs.append(value) }
        manager.beginUndoGrouping()
        editor.insertText(" world")
        manager.endUndoGrouping()
        XCTAssertTrue(manager.canUndo)
        manager.undo()
        XCTAssertEqual(editor.text, "hello")
        XCTAssertEqual(inputs.last, "hello")
        XCTAssertTrue(manager.canRedo)
        manager.redo()
        XCTAssertEqual(editor.text, "hello world")
        XCTAssertEqual(inputs.last, "hello world")
        node.replaceFieldValue("authoritative")
        XCTAssertFalse(manager.canUndo)
        XCTAssertFalse(manager.canRedo)
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
        var inputs: [String] = [], changes: [String] = []
        p.onInput = { _, value in inputs.append(value) }
        p.onChange = { _, value in changes.append(value) }
        p.fieldSelections.onReplacement = { _, value, _ in inputs.append(value) }
        for (name, id) in [("editor", UInt32(2)), ("line", UInt32(3))] {
            let node = try XCTUnwrap(p.views[id])
            node.handlers.insert("change")
            node.props["value"] = ""
            node.replaceFieldValue("")
            #if os(macOS)
            window.makeFirstResponder(node.textArea ?? node.field)
            let editor = try XCTUnwrap(node.textArea ?? node.field?.currentEditor() as? NSTextView)
            #else
            if let editor = node.textArea { XCTAssertTrue(editor.becomeFirstResponder(), name) }
            else { XCTAssertTrue(node.field?.becomeFirstResponder() ?? false, name) }
            #endif
            p.edited = nil
            inputs.removeAll()
            changes.removeAll()
            p.fieldSelections.setRangeText([name, "x", 0, 0, "end"])
            XCTAssertEqual(node.toldText, "x", "\(name): replacement updates the next user edit's comparison baseline")
            XCTAssertNil(p.edited, "\(name): a script is no pending user change")
            #if os(macOS)
            editor.deleteBackward(nil)
            #else
            if let editor = node.textArea { editor.deleteBackward() }
            else {
                let field = try XCTUnwrap(node.field)
                field.deleteBackward()
                XCTAssertEqual(field.actions(forTarget: node, forControlEvent: .editingChanged), ["fieldChanged"], name)
                // This XCTest host does not deliver UIApplication actions.
                // As in FieldCompositionIOSTests and KeyboardFocusIOSTests,
                // call the registered keyboard callback after the UITextInput edit.
                node.fieldChanged()
            }
            #endif
            XCTAssertEqual(node.fieldText, "", name)
            XCTAssertEqual(inputs, ["x", ""], name)
            XCTAssertEqual(p.edited, id, "\(name): the deletion must commit on blur")
            XCTAssertEqual(node.toldText, "", name)
            XCTAssertTrue(changes.isEmpty, "\(name): input does not commit the edit")
            if let editor = node.textArea { XCTAssertTrue(editor.resignFirstResponder(), name) }
            else { XCTAssertTrue(node.field?.resignFirstResponder() ?? false, name) }
            XCTAssertEqual(changes, [""], "\(name): blur commits the user deletion exactly once")
            XCTAssertNil(p.edited, name)
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
