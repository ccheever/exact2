// The shape of a confirmation (LLP 1115 D6, by Apple's HIG), shared by the
// hosts that present one natively: UIKit's alert (`IOS/MenusIOS.swift`) and
// AppKit's (`Mac/AlertsMac.swift`, LLP 1115.000 D7.6). An `alertdialog`'s
// rows are its text, its actions (buttons with a `press` that also close it)
// and its cancel (a button that only closes it).
import Foundation

enum ConfirmationShape {
    /// The alert shape: explanatory text, one cancel, and at most three
    /// buttons with it. Anything else is a chooser (a menu or action sheet)
    /// or stays as authored.
    static func isAlert(texts: Int, actions: Int, cancels: Int) -> Bool {
        texts > 0 && cancels == 1 && actions + cancels <= 3
    }

    /// An alert's words: its title is the `aria-label` ("Remove Book?"),
    /// else its first line; its message is the rest, a line each.
    static func alertText(label: String?, lines: [String]) -> (title: String?, message: String?) {
        var title = label, rest = lines
        if title == nil, !rest.isEmpty { title = rest.removeFirst() }
        return (title, rest.isEmpty ? nil : rest.joined(separator: "\n"))
    }
}
