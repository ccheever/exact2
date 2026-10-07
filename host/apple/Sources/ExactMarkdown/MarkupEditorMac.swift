// @ref LLP 1045 D5, D6 — AppKit editing stays on TextKit 2 and native undo.
#if os(macOS)
import AppKit
import ExactKit

extension NodeView {
    func markdownRestyle() {
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor, let t = text,
              !f.hasMarkedText(), let storage = f.textStorage else { return }
        let look = MarkupEditor.Look(
            font: { size, weight, family, italic in t.font(size: size, weight: weight, family: family, italic: italic) },
            size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")),
            italic: (style["font_style"]?.string) == "italic", lineHeight: usedLineHeight, ink: color("text_color", .textColor))
        editor.restyle(storage, selection: f.selectedRange(), look: look)
        f.typingAttributes = editor.baseAttributes(look)
    }

    @discardableResult func markdownFormat(_ command: String, argument: String = "") -> Bool {
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor, f.isEditable, !disabled,
              !f.hasMarkedText(), !editor.applying else { return false }
        let selection = f.window?.firstResponder === f ? f.selectedRange() : (editor.bookmark ?? f.selectedRange())
        // A link field can own the responder when its format arrives.
        // Capture its bookmark first, then return native editing ownership
        // before insertText so the command gets the editor's undo transaction.
        editor.applying = true
        if f.window?.firstResponder !== f, f.window?.makeFirstResponder(f) != true { editor.applying = false; return false }
        guard let result = MarkupCommands.edit(f.string, selection: selection, command: command, argument: argument) else { editor.applying = false; return false }
        let edit = minimalTextEdit(from: f.string, to: result.source)
        f.breakUndoCoalescing()
        if let edit {
            f.undoManager?.beginUndoGrouping()
            f.insertText(edit.text, replacementRange: edit.range)
            f.setSelectedRange(result.selection)
            f.undoManager?.endUndoGrouping()
            f.undoManager?.setActionName(command == "newline" ? "Typing" : "Markdown \(command)")
        } else { f.setSelectedRange(result.selection) }
        editor.bookmark = f.selectedRange()
        editor.applying = false
        f.breakUndoCoalescing()
        if edit != nil { textDidChange(Notification(name: NSText.didChangeNotification, object: f)) }
        else { markdownRestyle(); markdownPublishSelection() }
        f.scrollRangeToVisible(f.selectedRange())
        return true
    }

    func markdownPublishSelection(force: Bool = false) {
        guard handlers.contains("select"), let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor,
              !editor.applying, !editor.styling, !f.hasMarkedText(),
              let state = MarkupCommands.selection(f.string, range: f.selectedRange()), force || editor.lastSelectionState != state else { return }
        editor.lastSelectionState = state
        presenter?.session?.selection(node: id, json: state)
    }

    func markdownEditLink() {
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor, f.isEditable, !f.hasMarkedText(), let window = f.window else { return }
        editor.bookmark = f.selectedRange()
        let alert = NSAlert()
        alert.messageText = "Link URL"
        alert.addButton(withTitle: "Apply Link")
        alert.addButton(withTitle: "Cancel")
        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 340, height: 24))
        input.stringValue = "https://"
        if let state = MarkupCommands.selection(f.string, range: f.selectedRange()), let data = state.data(using: .utf8),
           let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let link = object["link"] as? String, !link.isEmpty { input.stringValue = link }
        alert.accessoryView = input
        alert.window.initialFirstResponder = input
        alert.beginSheetModal(for: window) { [weak self, weak f] response in
            guard let self, let f else { return }
            if response == .alertFirstButtonReturn, !input.stringValue.isEmpty { self.markdownFormat("link", argument: input.stringValue) }
            window.makeFirstResponder(f)
        }
    }
}
#endif
