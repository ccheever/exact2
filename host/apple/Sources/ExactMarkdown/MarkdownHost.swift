// The Markdown capability (LLP 1047.001 D4; LLP 1045): an app's composition
// links this module only when its plan has `markup="markdown"`, and
// installs it before any session is made.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactKit

public enum ExactMarkdown {
    public static func install() { MarkdownLink.installed = MarkdownHost() }
}

final class MarkdownHost: MarkdownCapability {
    func expand(_ source: String, base: Run, color: [Double]?) -> [Run] { MarkupRuns.expand(source, base: base, color: color) }
    func editor() -> MarkdownEditing { MarkupEditor() }
    func plain(_ source: String) -> String? { MarkupCommands.plain(source) }
    func format(_ node: NodeView, command: String, argument: String, selection: NSRange?) -> Bool {
        #if os(macOS)
        node.markdownFormat(command, argument: argument)
        #else
        node.markdownFormat(command, argument: argument, selection: selection)
        #endif
    }
    func editLink(_ node: NodeView) { node.markdownEditLink() }
    func publishSelection(_ node: NodeView, force: Bool) { node.markdownPublishSelection(force: force) }
    func restyle(_ node: NodeView) { node.markdownRestyle() }
    func formatElement(_ presenter: Presenter, _ args: [Any]) { presenter.markdownFormatElement(args) }
    #if os(iOS) || os(tvOS)
    func menu(_ node: NodeView, _ textView: UITextView, range: NSRange, suggested: [UIMenuElement]) -> UIMenu? {
        node.markdownMenu(textView, editMenuForTextIn: range, suggestedActions: suggested)
    }
    #endif
}
