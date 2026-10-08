// @ref LLP 1047.001 D4 — Markdown (LLP 1045) is a linked capability: its
// implementation is the `ExactMarkdown` module, which an app's composition
// links only when its plan has `markup="markdown"`, and installs here. The
// core names no Markdown type; without the module, a plan that uses
// Markdown is refused at boot (`Unlinked("markdown")`), so nothing below is
// asked, and each forwarder answers as an app without Markdown would.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// What the core asks of the Markdown capability.
package protocol MarkdownCapability: AnyObject {
    /// `source` as display runs over `base` (the node's run); `color` its ink.
    func expand(_ source: String, base: Run, color: [Double]?) -> [Run]
    /// A Markdown text area's styler.
    func editor() -> MarkdownEditing
    /// `source` without its Markdown syntax (Copy Plain Text).
    func plain(_ source: String) -> String?
    /// Apply a formatting command to `node`'s editor.
    func format(_ node: NodeView, command: String, argument: String, selection: NSRange?) -> Bool
    /// Ask for a link and apply it to `node`'s selection.
    func editLink(_ node: NodeView)
    /// Tell the app what the selection's Markdown is.
    func publishSelection(_ node: NodeView, force: Bool)
    /// Restyle `node`'s editor for its text and selection.
    func restyle(_ node: NodeView)
    /// The `formatElement` command (LLP 1045 D6).
    func formatElement(_ presenter: Presenter, _ args: [Any])
    #if os(iOS) || os(tvOS)
    /// The edit menu's Markdown items for `textView`.
    func menu(_ node: NodeView, _ textView: UITextView, range: NSRange, suggested: [UIMenuElement]) -> UIMenu?
    #endif
}

/// A Markdown text area's styler, as the core's text areas see it.
package protocol MarkdownEditing: AnyObject {
    /// The editor is changing the text (an edit of its own).
    var applying: Bool { get set }
    /// The editor is changing attributes (a restyle).
    var styling: Bool { get }
    /// The selection to restore when focus returns.
    var bookmark: NSRange? { get set }
    /// The attributes plain typing takes.
    var plainAttributes: [NSAttributedString.Key: Any] { get }
    /// Remove every Markdown attribute.
    func detach(_ storage: NSTextStorage)
}

/// The Markdown module, installed by the composition.
package enum MarkdownLink {
    package static var installed: MarkdownCapability?
}

extension NodeView {
    @discardableResult func formatMarkup(_ command: String, argument: String = "", selection: NSRange? = nil) -> Bool {
        MarkdownLink.installed?.format(self, command: command, argument: argument, selection: selection) ?? false
    }
    func editMarkupLink() { MarkdownLink.installed?.editLink(self) }
    func publishMarkupSelection(force: Bool = false) { MarkdownLink.installed?.publishSelection(self, force: force) }
    func restyleMarkup() { MarkdownLink.installed?.restyle(self) }
}

#if os(iOS) || os(tvOS)
extension NodeView {
    /// The edit menu, with Markdown's items for a Markdown editor (UITextViewDelegate).
    package func textView(_ textView: UITextView, editMenuForTextIn range: NSRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
        MarkdownLink.installed?.menu(self, textView, range: range, suggested: suggestedActions)
    }
}
#endif

extension Presenter {
    func formatElement(_ args: [Any]) { MarkdownLink.installed?.formatElement(self, args) }
}
