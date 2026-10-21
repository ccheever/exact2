// Native undo for every text area (LLP 1045 D6): edits register with the
// platform's undo manager, as typing and paste do.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// Native history, scoped to one text view. TextKit can restore its storage
/// without a text-change delegate callback; completion publishes the source
/// once, after the complete native undo/redo transaction and its selection.
final class NativeTextUndo {
    let manager: UndoManager
    private var observers: [NSObjectProtocol] = []

    init(manager: UndoManager = UndoManager(), before: @escaping () -> Void, after: @escaping () -> Void) {
        self.manager = manager
        let center = NotificationCenter.default
        for name in [Notification.Name.NSUndoManagerWillUndoChange, .NSUndoManagerWillRedoChange] {
            observers.append(center.addObserver(forName: name, object: manager, queue: nil) { _ in before() })
        }
        for name in [Notification.Name.NSUndoManagerDidUndoChange, .NSUndoManagerDidRedoChange] {
            observers.append(center.addObserver(forName: name, object: manager, queue: nil) { _ in after() })
        }
    }
    deinit { for observer in observers { NotificationCenter.default.removeObserver(observer) } }
}
