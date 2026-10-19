#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

/// x2apps codeedit #2 on UIKit: a field's `input` reports the selection its
/// edit left; `setSelectionRange` fires `select`, never focuses, and waits
/// for an input's focus when it has none; a person's non-collapsed selection
/// is a `select`, typing is not. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class FieldSelectionIOSTests: XCTestCase {
    private var window: UIWindow!
    private var selects: [String] = []

    private func presenter(session: ExactSession? = nil) -> Presenter {
        let p = session?.presenter ?? Presenter()
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

    func testCancelableBeforeInputAndCaretChange() throws {
        let p = presenter()
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            node.handlers.formUnion(["selectionchange", "beforeinput"])
            var edits: [FieldInput] = [], changes: [FieldSelection] = []
            p.fieldSelections.onBeforeInput = { view, _, _, edit in
                if view == id { edits.append(edit); p.defaultPrevented = true }
            }
            p.fieldSelections.onChange = { view, _, selection in if view == id { changes.append(selection) } }
            let editor: UIView & UITextInput = try XCTUnwrap((node.textArea as (UIView & UITextInput)?) ?? node.field)
            editor.becomeFirstResponder()
            p.fieldSelections.setSelectionRange([id == 2 ? "editor" : "line", 2, 2])
            let deadline = Date().addingTimeInterval(2)
            while changes.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
            XCTAssertEqual(changes.last?.start, 2)
            editor.insertText("X")
            XCTAssertEqual(node.fieldText, "hello")
            XCTAssertEqual(edits.last?.inputType, "insertText")
            XCTAssertEqual(edits.last?.data, "X")
            XCTAssertEqual(edits.last?.cancelable, true)
            // The XCTest host has no pasteboard permission. Deliver UIKit's
            // resolved string to its registered paste delegate.
            let supporting = try XCTUnwrap(editor as? UITextPasteConfigurationSupporting)
            XCTAssertTrue(supporting.pasteDelegate === node)
            _ = node.textPasteConfigurationSupporting(supporting, performPasteOf: NSAttributedString(string: "PASTE"), to: try XCTUnwrap(editor.selectedTextRange))
            XCTAssertEqual(node.fieldText, "hello", "paste: field \(id)")
            XCTAssertEqual(edits.last?.inputType, "insertFromPaste", "field \(id)")
            XCTAssertEqual(edits.last?.data, "PASTE", "field \(id)")
            editor.setMarkedText("あ", selectedRange: NSRange(location: 1, length: 0))
            XCTAssertTrue(node.fieldText.contains("あ"))
            XCTAssertEqual(edits.last?.inputType, "insertCompositionText")
            XCTAssertEqual(edits.last?.cancelable, false)
            editor.unmarkText()
            editor.resignFirstResponder()
            p.fieldSelections.onBeforeInput = nil
        }
    }

    func testSearchClearButtonIsCancelable() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        node.applyProps(set: ["type": "search"], clear: [])
        node.handlers.insert("beforeinput")
        let editor = try XCTUnwrap(node.field)
        XCTAssertTrue(editor.becomeFirstResponder())
        var edits: [FieldInput] = []
        var prevent = true
        p.fieldSelections.onBeforeInput = { _, value, _, edit in
            XCTAssertEqual(value, "hello")
            edits.append(edit)
            p.defaultPrevented = prevent
        }
        // UIKit's clear button asks this optional delegate, not shouldChangeCharacters.
        let allowed = editor.delegate?.textFieldShouldClear?(editor) ?? true
        if allowed { editor.text = ""; node.fieldChanged() }
        XCTAssertFalse(allowed)
        XCTAssertEqual(editor.text, "hello")
        XCTAssertEqual(edits.last?.inputType, "deleteContentBackward")
        XCTAssertNil(edits.last?.data)
        XCTAssertEqual(edits.last?.cancelable, true)
        editor.text = "hello"
        prevent = false
        XCTAssertTrue(editor.delegate?.textFieldShouldClear?(editor) ?? true)
        XCTAssertEqual(edits.count, 2)
        var values: [String] = []
        p.onInput = { _, value in values.append(value) }
        editor.text = ""
        node.fieldChanged()
        XCTAssertEqual(values, [""])
        XCTAssertEqual(p.fieldSelections.input(node.id)?.inputType, "deleteContentBackward")
        editor.resignFirstResponder()
        p.fieldSelections.onBeforeInput = nil
    }

    func testResolvedPasteIsCancelableForEveryPlainInputType() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        node.handlers.insert("beforeinput")
        var edits: [FieldInput] = []
        p.fieldSelections.onBeforeInput = { _, _, _, edit in
            edits.append(edit)
            p.defaultPrevented = edit.inputType == "insertFromPaste"
        }
        let editor = try XCTUnwrap(node.field)
        XCTAssertTrue(editor.becomeFirstResponder())
        for type in ["text", "search", "url", "tel", "password", "email", "number", "custom"] {
            node.applyProps(set: ["type": type], clear: [])
            let pasted = type == "number" ? "123" : "a@b.com"
            edits.removeAll()
            XCTAssertTrue(editor.pasteDelegate === node, type)
            let range = try XCTUnwrap(editor.selectedTextRange)
            _ = editor.pasteDelegate?.textPasteConfigurationSupporting?(editor, performPasteOf: NSAttributedString(string: pasted), to: range)
            XCTAssertEqual(edits.last?.inputType, "insertFromPaste", type)
            XCTAssertEqual(edits.last?.data, pasted, type)
            XCTAssertEqual(editor.text, "hello", type)
        }
        editor.resignFirstResponder()
        p.fieldSelections.onBeforeInput = nil
    }

    func testHeaderSearchCancelsEditsAndReportsItsLiveCaret() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        node.applyProps(set: ["type": "search"], clear: [])
        node.handlers.formUnion(["beforeinput", "selectionchange"])
        let search = HeaderSearch(host: p.navigation)
        search.field = node
        let bar = search.controller.searchBar
        bar.frame = CGRect(x: 0, y: 180, width: 350, height: 60)
        window.addSubview(bar)
        bar.text = "hello"
        let editor = bar.searchTextField
        XCTAssertTrue(editor.becomeFirstResponder())
        var edits: [FieldInput] = [], changes: [FieldSelection] = []
        p.fieldSelections.onBeforeInput = { _, value, selection, edit in
            XCTAssertEqual(value, "hello")
            XCTAssertEqual(selection.start, 2)
            edits.append(edit)
            p.defaultPrevented = true
        }
        p.fieldSelections.onChange = { _, value, selection in
            XCTAssertEqual(value, "hello")
            changes.append(selection)
        }
        let caret = try XCTUnwrap(editor.position(from: editor.beginningOfDocument, offset: 2))
        editor.selectedTextRange = editor.textRange(from: caret, to: caret)
        drainSelectTasks()
        XCTAssertEqual(changes.last?.start, 2)
        // Ask the real field delegate, as UIKit does before keyboard replacement.
        let range = NSRange(location: 2, length: 0)
        let allowed = editor.delegate?.textField?(editor, shouldChangeCharactersIn: range, replacementString: "X") ?? true
        if allowed { editor.insertText("X") }
        XCTAssertFalse(allowed)
        XCTAssertEqual(editor.text, "hello")
        XCTAssertEqual(edits.last?.inputType, "insertText")
        XCTAssertEqual(edits.last?.data, "X")
        XCTAssertFalse(editor.delegate?.textFieldShouldClear?(editor) ?? true)
        XCTAssertEqual(edits.last?.inputType, "deleteContentBackward")
        XCTAssertTrue(editor.pasteDelegate === node)
        _ = editor.pasteDelegate?.textPasteConfigurationSupporting?(editor, performPasteOf: NSAttributedString(string: "PASTE"), to: try XCTUnwrap(editor.selectedTextRange))
        XCTAssertEqual(editor.text, "hello")
        XCTAssertEqual(edits.last?.inputType, "insertFromPaste")
        p.fieldSelections.onBeforeInput = nil
        p.fieldSelections.onChange = nil
        var values: [String] = []
        p.onInput = { _, value in values.append(value) }
        // UISearchTextField's programmatic insertText does not ask its delegate.
        // Deliver the keyboard's permission callback before applying the allowed edit.
        XCTAssertTrue(editor.delegate?.textField?(editor, shouldChangeCharactersIn: range, replacementString: "Z") ?? true)
        editor.insertText("Z")
        search.updateSearchResults(for: search.controller)
        XCTAssertEqual(editor.text, "heZllo")
        XCTAssertEqual(values, ["heZllo"])
        XCTAssertEqual(p.fieldSelections.input(node.id)?.inputType, "insertText")
        search.field = nil
        XCTAssertNil(node.headerField)
        editor.resignFirstResponder()
        p.fieldSelections.onBeforeInput = nil
        p.fieldSelections.onChange = nil
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
    func testDecorationEchoRefreshesMarkerAndKeepsUndoableSourceOnUIKit() throws {
        ExactMarkdown.install()
        let session = ExactApp.shared.makeSession(label:"decoration-echo-ios")
        defer { window.isHidden = true; session.destroy() }
        let p = presenter(session:session), original = "- [ ] task", checked = "- [x] task"
        let node = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(node.textArea)
        func metadata(_ source: String, _ on: Bool) throws -> String {
            let value: [String:Any] = ["value":source,"spans":[],
                "blocks":[["name":"task","start":0,"end":10,"marker":on ? "checkbox:on" : "checkbox:off"]],
                "hidden":[["start":0,"end":6]],"atoms":[]]
            return String(decoding:try JSONSerialization.data(withJSONObject:value),as:UTF8.self)
        }
        p.apply(wireBatch([["op":"props","id":2,"set":["value":original,"decorations":try metadata(original,false),"decorationStyles":"{\"task\":[]}"]]]))
        XCTAssertTrue(field.becomeFirstResponder())
        let manager = try XCTUnwrap(field.undoManager)
        manager.groupsByEvent = false
        manager.removeAllActions()
        var inputs: [String] = []
        p.onInput = { _,value in inputs.append(value) }
        p.fieldSelections.setRangeText(["editor","x",3,4,"preserve","undo"])
        XCTAssertEqual(field.text,checked)
        let selection = field.selectedRange
        func echo(_ source: String, _ on: Bool) throws {
            p.apply(wireBatch([["op":"props","id":2,"set":["value":source,"decorations":try metadata(source,on)]]]))
            let state = try XCTUnwrap(node.fieldDecorations?.state)
            XCTAssertEqual(state["provisional"] as? Bool,false)
            let blocks = (state["record"] as? [String:Any])?["blocks"] as? [[String:Any]]
            XCTAssertEqual(blocks?.last?["marker"] as? String,on ? "checkbox:on" : "checkbox:off")
            XCTAssertEqual((state["hidden"] as? [[String:Int]])?.count,0,"phone syntax remains visible")
        }
        try echo(checked,true)
        XCTAssertEqual(field.selectedRange,selection)
        manager.undo()
        XCTAssertEqual(field.text,original)
        XCTAssertEqual(inputs,[original])
        try echo(original,false)
        XCTAssertTrue(manager.canRedo)
        manager.redo()
        XCTAssertEqual(field.text,checked)
        XCTAssertEqual(inputs,[original,checked])
        try echo(checked,true)
        p.apply(wireBatch([["op":"props","id":2,"set":[:],"clear":["decorations"]]]))
        XCTAssertNil(node.fieldDecorations)
        XCTAssertEqual(field.text,checked)
        XCTAssertTrue(manager.canUndo)
    }

    private final class DecorationTouch: UITouch {
        let field: UITextView
        let point: CGPoint
        init(_ field: UITextView, _ point: CGPoint) { self.field = field; self.point = point; super.init() }
        override var view: UIView? { field }
        override var window: UIWindow? { field.window }
        override func location(in view: UIView?) -> CGPoint { field.convert(point, to: view) }
    }

    func testDecorationTapAdmissionIsSideEffectFreeAndDetachesWithMetadata() throws {
        ExactMarkdown.install()
        let session = ExactApp.shared.makeSession(label:"decoration-tap-ios")
        defer { window.isHidden = true; session.destroy() }
        let p = presenter(session:session), source = "hello @alice"
        let node = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(node.textArea)
        let metadata = "{\"value\":\"hello @alice\",\"spans\":[],\"blocks\":[],\"hidden\":[],\"atoms\":[{\"name\":\"pill\",\"start\":6,\"end\":12,\"label\":\"Alice\"}]}"
        let ordinary = field.gestureRecognizers?.count ?? 0
        func attach() {
            p.apply(wireBatch([["op":"props","id":2,"set":["value":source,"decorations":metadata,"decorationStyles":"{\"pill\":[]}"]]]))
        }
        attach(); attach()
        let tap = try XCTUnwrap(field.gestureRecognizers?.first { $0.delegate is DecorationTap })
        XCTAssertEqual(field.gestureRecognizers?.count,ordinary + 1,"a metadata echo does not add another carrier")
        let delegate = try XCTUnwrap(tap.delegate)
        let frame = field.convert(try XCTUnwrap(p.rangeFrame(2,start:6,end:12)),from:p.viewport)
        let point = CGPoint(x:frame.midX,y:frame.midY)
        field.selectedRange = NSRange(location:0,length:0)
        let selection = field.selectedRange
        func admitted(_ point: CGPoint) -> Bool {
            delegate.gestureRecognizer?(tap,shouldReceive:DecorationTouch(field,point)) ?? false
        }
        XCTAssertTrue(admitted(point))
        XCTAssertEqual(field.selectedRange,selection,"admission must not select the atom")
        XCTAssertEqual(field.text,source)
        XCTAssertFalse(admitted(CGPoint(x:field.bounds.maxX - 2,y:field.bounds.maxY - 2)))
        XCTAssertTrue(delegate.gestureRecognizer?(tap,shouldRecognizeSimultaneouslyWith:field.panGestureRecognizer) ?? false)
        for key in ["disabled","inert","editable"] {
            node.props[key] = key == "editable" ? "false" : "true"
            XCTAssertFalse(admitted(point),key)
            node.props.removeValue(forKey:key)
        }
        field.text = source + "!"
        XCTAssertFalse(admitted(point),"a changed source cannot use stale hit geometry")
        field.text = source
        node.fieldDecorations?.restyle(node)
        field.setMarkedText("に",selectedRange:NSRange(location:1,length:0))
        XCTAssertFalse(admitted(point),"composition belongs to the native editor")
        field.unmarkText()
        p.apply(wireBatch([["op":"props","id":2,"set":[:],"clear":["decorations"]]]))
        XCTAssertFalse(field.gestureRecognizers?.contains { $0 === tap } ?? false)
        XCTAssertNil(node.fieldDecorations)
    }

    func testAtomicDeletionKeepsAdjacentSourceAndNativeUndo() throws {
        ExactMarkdown.install()
        let session = ExactApp.shared.makeSession(label:"decoration-delete-ios")
        defer { window.isHidden = true; session.destroy() }
        let p = presenter(session:session)
        let node = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(node.textArea)
        XCTAssertTrue(field.becomeFirstResponder())
        let manager = try XCTUnwrap(field.undoManager)
        manager.groupsByEvent = false
        var inputs: [String] = [], beforeInput: [String] = []
        p.onInput = { _,value in inputs.append(value) }
        node.handlers.insert("beforeinput")
        p.fieldSelections.onBeforeInput = { _,_,selection,edit in beforeInput.append("\(selection.start):\(selection.end) \(edit.inputType)") }
        for source in ["hello @alice there", "hello **world** @alice", "## Heading\nhello **world** @alice\n- [ ] task"] {
            let range = (source as NSString).range(of:"@alice")
            let removed = (source as NSString).replacingCharacters(in:range,with:"")
            let control = UITextView(frame:CGRect(x:0,y:150,width:300,height:80))
            window.addSubview(control)
            var native: [String:String] = [:]
            for (name,smart) in [("default",UITextSmartInsertDeleteType.default),("no",.no)] {
                control.text = source
                control.smartInsertDeleteType = smart
                XCTAssertTrue(control.becomeFirstResponder())
                control.selectedRange = range
                control.deleteBackward()
                native[name] = control.text
            }
            print("UITextView atomic-range delete source=\(source) controls=\(native)")
            XCTAssertEqual(native["no"],removed)
            control.removeFromSuperview()
            p.apply(wireBatch([["op":"props","id":2,"set":["value":source],"clear":["decorations","decorationStyles"]]]))
            node.replaceFieldValue(source)
            XCTAssertTrue(field.becomeFirstResponder())
            field.smartInsertDeleteType = .default
            field.selectedRange = range
            manager.beginUndoGrouping()
            field.deleteBackward()
            manager.endUndoGrouping()
            XCTAssertEqual(field.text,native["default"],"ordinary fields retain the platform's smart deletion")
            let metadata: [String:Any] = ["value":source,"spans":[],"blocks":[],"hidden":[],
                "atoms":[["name":"pill","start":range.location,"end":NSMaxRange(range),"label":"Alice"]]]
            let record = String(decoding:try JSONSerialization.data(withJSONObject:metadata),as:UTF8.self)
            for caret in [false,true] {
                node.replaceFieldValue(source)
                p.apply(wireBatch([["op":"props","id":2,"set":["value":source,"decorations":record,"decorationStyles":"{\"pill\":[]}"]]]))
                manager.removeAllActions();inputs.removeAll();beforeInput.removeAll()
                let smart: UITextSmartInsertDeleteType = caret ? .yes : .default
                field.smartInsertDeleteType = smart
                field.selectedRange = caret ? NSRange(location:NSMaxRange(range),length:0) : range
                let before = field.selectedRange
                manager.beginUndoGrouping()
                field.deleteBackward()
                manager.endUndoGrouping()
                print("atomic caret=\(caret) selected=\(before) beforeinput=\(beforeInput) source=\(field.text ?? "")")
                XCTAssertEqual(beforeInput,["\(range.location):\(NSMaxRange(range)) deleteContentBackward"])
                XCTAssertEqual(field.text,removed,"atomic deletion must preserve every source unit outside its range")
                XCTAssertEqual(inputs,[removed])
                XCTAssertEqual(field.smartInsertDeleteType,smart)
                XCTAssertTrue(manager.canUndo)
                manager.undo()
                XCTAssertEqual(field.text,source)
                XCTAssertEqual(inputs,[removed,source])
                manager.redo()
                XCTAssertEqual(field.text,removed)
                XCTAssertEqual(inputs,[removed,source,removed])
            }
        }
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

    /// An `input` in a closed dialog has no UIKit field until its box is laid
    /// out (`MenuHost.waitingFields`); a selection or a range set meanwhile
    /// makes it, and stands, as it did when the field was made at once.
    func testASelectionInAClosedDialogsFieldStands() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "form", "semanticTag": "dialog"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "name", "value": "hello"], "handlers": ["input", "select"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
        ]))
        let node = try XCTUnwrap(p.views[3])
        XCTAssertNil(node.field, "it waits for its box")
        p.fieldSelections.setSelectionRange(["name", 0, 4])
        XCTAssertNotNil(node.field)
        XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 0, end: 4))
        p.fieldSelections.setRangeText(["name", "J", 0, 1])
        XCTAssertEqual(node.fieldText, "Jello")
        // Its box laid out: the field it has is kept, with its selection.
        let field = node.field
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 200.0, "h": 30.0]]))
        XCTAssertTrue(node.field === field)
        XCTAssertEqual(p.fieldSelections.current(node), FieldSelection(start: 0, end: 4))
    }
}
#endif
