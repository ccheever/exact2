// The shape of a confirmation (LLP 1115 D6, by Apple's HIG), shared by the
// hosts that present one natively: UIKit's alert (`IOS/MenusIOS.swift`) and
// AppKit's (`Mac/AlertsMac.swift`, LLP 1115.000 D7.6). An `alertdialog`'s
// rows are its text, its actions (buttons with a `press` that also close it)
// and its cancel (a button that only closes it).
//
// What counts as an alert is one rule, the kernel's
// (`exact_kernel::confirmation`): lowering reads it to imply an alert's
// `closedby` (LLP 1115.000 D13.4: an alert has no outside), the Linux host to
// draw Android's, and these hosts ask it here, so the three cannot drift. The
// counting of rows stays each host's, since each reads its own tree.
import CExact
import Foundation

enum ConfirmationShape {
    /// The alert shape: explanatory text, one cancel, and at most three
    /// buttons with it. Anything else is a chooser (a menu or action sheet)
    /// or stays as authored.
    static func isAlert(texts: Int, actions: Int, cancels: Int) -> Bool {
        exact_confirmation_is_alert(UInt32(max(0, texts)), UInt32(max(0, actions)), UInt32(max(0, cancels))) != 0
    }

    /// An alert's words: its title is the `aria-label` ("Remove Book?"),
    /// else its first line; its message is the rest, a line each.
    static func alertText(label: String?, lines: [String]) -> (title: String?, message: String?) {
        var title = label, rest = lines
        if title == nil, !rest.isEmpty { title = rest.removeFirst() }
        return (title, rest.isEmpty ? nil : rest.joined(separator: "\n"))
    }
}

extension NodeView {
    /// A prompt's text field (a rename's): an `input` that edits one line of
    /// text, which the alert shows as its own field
    /// (`UIAlertController.addTextField`, `NSAlert`'s accessory view) and
    /// which counts with its text in the alert shape, as lowering counts it.
    var isPromptField: Bool {
        kind == "input" && ["text", "password", "email", "url", "search", "tel", "number"].contains(props["type"] ?? "text")
    }

    /// HTML queues `close` after restoring focus; app code may open it again.
    func queueDialogClose() {
        guard props["semanticTag"] == "dialog", handlers.contains("close") else { return }
        DispatchQueue.main.async { [weak self, weak presenter] in
            guard let self, let presenter, presenter.views[self.id] === self, self.superview != nil else { return }
            presenter.onClose?(self.id)
        }
    }
}
