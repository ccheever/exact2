// @ref LLP 1045 D5, D6 — source replacements through UIKit's own input path.
#if os(iOS) || os(tvOS)
import UIKit
import ExactKit

extension NodeView {
    /// Restyle a Markdown editor's storage for its text and selection.
    func markdownRestyle() {
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let t = text, let editor = f.markup as? MarkupEditor, f.markedTextRange == nil else { return }
        let look = MarkupEditor.Look(
            font: { size, weight, family, italic in t.font(size: size, weight: weight, family: family, italic: italic) },
            size: number("font_size", PageFacts.defaultRootFontSize), weight: Int(number("font_weight", 400)), family: Int(number("font_family")),
            italic: (style["font_style"]?.string) == "italic", lineHeight: usedLineHeight, ink: color("text_color", SystemColor.canvasText))
        editor.restyle(f.textStorage, selection: f.selectedRange, look: look)
        f.typingAttributes = editor.baseAttributes(look)
    }

    @discardableResult func markdownFormat(_ command: String, argument: String = "", selection override: NSRange? = nil) -> Bool {
        #if os(tvOS)
        // tvOS text views do not edit.
        return false
        #else
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor, f.isEditable, !disabled,
              f.markedTextRange == nil, !editor.applying else { return false }
        let selection = override ?? (f.isFirstResponder ? f.selectedRange : (editor.bookmark ?? f.selectedRange))
        // Restore native editing ownership after capturing the bookmark:
        // formatting from a link field must register in this editor's undo.
        editor.applying = true
        if !f.isFirstResponder, !f.becomeFirstResponder() { editor.applying = false; return false }
        guard let result = MarkupCommands.edit(f.text ?? "", selection: selection, command: command, argument: argument) else { editor.applying = false; return false }
        let edit = minimalTextEdit(from: f.text ?? "", to: result.source)
        if let edit {
            f.undoManager?.beginUndoGrouping()
            f.selectedRange = edit.range
            // insertText uses UIKit's native undo registration, as typing and
            // paste do. Direct mutations of textStorage would not register it.
            f.insertText(edit.text)
            f.selectedRange = result.selection
            f.undoManager?.endUndoGrouping()
            f.undoManager?.setActionName(command == "newline" ? "Typing" : "Markdown \(command)")
        } else { f.selectedRange = result.selection }
        editor.bookmark = f.selectedRange
        editor.applying = false
        if edit != nil { textViewDidChange(f) }
        else { markdownRestyle(); markdownPublishSelection() }
        f.scrollRangeToVisible(f.selectedRange)
        return true
        #endif
    }

    func markdownPublishSelection(force: Bool = false) {
        guard handlers.contains("select"), let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor,
              !editor.applying, !editor.styling, f.markedTextRange == nil,
              let state = MarkupCommands.selection(f.text ?? "", range: f.selectedRange), force || editor.lastSelectionState != state else { return }
        editor.lastSelectionState = state
        presenter?.session?.selection(node: id, json: state)
    }

    func markdownEditLink() {
        #if !os(tvOS)
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let editor = f.markup as? MarkupEditor, f.isEditable, f.markedTextRange == nil,
              var controller = f.window?.rootViewController else { return }
        while let presented = controller.presentedViewController { controller = presented }
        editor.bookmark = f.selectedRange
        let alert = UIAlertController(title: "Link URL", message: nil, preferredStyle: .alert)
        var existing = "https://"
        if let state = MarkupCommands.selection(f.text ?? "", range: f.selectedRange), let data = state.data(using: .utf8),
           let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let link = object["link"] as? String, !link.isEmpty { existing = link }
        alert.addTextField { field in
            field.text = existing; field.keyboardType = .URL; field.autocapitalizationType = .none; field.autocorrectionType = .no
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { [weak f] _ in f?.becomeFirstResponder() })
        alert.addAction(UIAlertAction(title: "Apply Link", style: .default) { [weak self, weak f, weak alert] _ in
            guard let self, let f, let url = alert?.textFields?.first?.text, !url.isEmpty else { return }
            self.markdownFormat("link", argument: url)
            f.becomeFirstResponder()
        })
        controller.present(alert, animated: true)
        #endif
    }

    func markdownMenu(_ textView: UITextView, editMenuForTextIn range: NSRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard let f = textView as? TextArea, (f.markup is MarkupEditor) else { return nil }
        var actions: [UIMenuElement] = []
        #if !os(tvOS)
        if f.isEditable, f.markedTextRange == nil {
            for (title, command) in [("Bold", "bold"), ("Italic", "italic"), ("Code", "code"), ("Strikethrough", "strike")] {
                actions.append(UIAction(title: title) { [weak self] _ in self?.markdownFormat(command) })
            }
            actions.append(UIAction(title: "Link…") { [weak self] _ in self?.markdownEditLink() })
        }
        #endif
        if range.length > 0 { actions.append(UIAction(title: "Copy Plain Text") { [weak f] _ in f?.copyPlainText() }) }
        return UIMenu(children: suggestedActions + [UIMenu(title: "Markdown", children: actions)])
    }
}
#endif
