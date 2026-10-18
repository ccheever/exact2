// Explicit app edits restore both the source and its native selection.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class FieldUndoEntry {
    weak var node: NodeView?
    let value: String
    let selection: FieldSelection
    init(node: NodeView, value: String, selection: FieldSelection) {
        self.node = node; self.value = value; self.selection = selection
    }
    func register(in manager: UndoManager) {
        // UndoManager does not retain its target. The handler owns this
        // source snapshot until the corresponding history action retires.
        manager.registerUndo(withTarget: self) { [self, weak manager] _ in
            guard let manager, let node, let fields = node.presenter?.fieldSelections,
                  node.presenter?.views[node.id] === node else { return }
            FieldUndoEntry(node: node, value: node.fieldText, selection: fields.current(node)).register(in: manager)
            fields.quietly { node.replaceFieldValue(value, clearUndo: false) }
            fields.set(node, selection, notify: false)
            node.restyleMarkup()
        }
    }
}

extension NodeView {
    var fieldUndoManager: UndoManager? {
        #if os(macOS)
        textArea?.undoManager ?? field?.currentEditor()?.undoManager
        #else
        textArea?.undoManager ?? selectionField?.undoManager
        #endif
    }
    package var fieldComposing: Bool {
        #if os(macOS)
        textArea?.hasMarkedText() ?? (field?.currentEditor() as? NSTextView)?.hasMarkedText() ?? false
        #else
        (textArea?.markedTextRange ?? selectionField?.markedTextRange) != nil
        #endif
    }
}
