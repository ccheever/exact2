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
    /// `source` as display runs over `base` (the node's run); `color` its ink,
    /// `linkColor` a link's and `atomColor` an atom's where it is not `color`.
    /// `boldText`: Bold Text weights `base` (LLP 1115.000 D12.3), so a
    /// piece's own weight (bold) takes the same step.
    func expand(_ source: String, base: Run, color: [Double]?, linkColor: [Double]?, atomColor: [Double]?, boldText: Bool, atoms: MarkdownAtoms) -> [Run]
    /// A Markdown text area's styler.
    func editor() -> MarkdownEditing
    /// `range` (UTF-16, the whole source when nil) of `source` without its
    /// Markdown syntax, each atom the whole source recognizes inside it its
    /// label (Copy Plain Text), or `spoken` its accessible name (what
    /// VoiceOver reads); `atoms` and `faces` are the node's (LLP 1045.000).
    func plain(_ source: String, range: NSRange?, atoms: String, faces: String, spoken: Bool) -> String?
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
    /// The `paragraphDirs` its last restyle drew (LLP 1045 D11).
    var paragraphDirs: String { get }
    /// The node's `atoms`, `atomFaces` and `atomStyles` as its last restyle
    /// drew them (LLP 1045.000 D2).
    var atoms: MarkdownAtoms { get }
    /// The base direction of the line holding `location`, as drawn: an empty
    /// editor's placeholder takes its first line's (LLP 1045 D11).
    func direction(at location: Int) -> NSWritingDirection
    /// Remove every Markdown attribute.
    func detach(_ storage: NSTextStorage)
    /// Where a person's selection change from `old` to `new` lands beside a
    /// hidden link destination (LLP 1045 D5).
    func place(_ new: NSRange, from old: NSRange?, in storage: NSAttributedString) -> NSRange
    /// The caret a native move or deletion should start from: the other
    /// edge of a hidden run the caret sits at, on the side it moves to; nil
    /// when it starts where it is.
    func crossing(_ caret: Int, backward: Bool, in storage: NSAttributedString) -> Int?
    /// A native replacement of `range` by `text` as the editor's rules make
    /// it: hidden source stays, and only the visible parts change. `caret`
    /// is the collapsed selection a deletion starts from, else nil. Nil when
    /// the native edit is already that.
    func rewrite(_ range: NSRange, with text: String, caret: Int?, in storage: NSAttributedString) -> MarkupRewrite?
    /// The source a copy of `range` takes.
    func copyRange(_ range: NSRange, in storage: NSAttributedString) -> NSRange
}

/// An edit the Markdown editor makes in place of a native one: replacements
/// in ascending order, and the caret after them (nil: unchanged).
package struct MarkupRewrite {
    package var edits: [(range: NSRange, text: String)]
    package var caret: Int?
    package init(edits: [(range: NSRange, text: String)], caret: Int?) {
        self.edits = edits
        self.caret = caret
    }
}

/// A Markdown node's inline atoms (LLP 1045.000 D2), as its props hold them:
/// the compiled kinds, the app's faces, and its kinds' paint rows. Empty
/// kinds are none.
package struct MarkdownAtoms: Hashable {
    package var kinds = ""
    package var faces = ""
    package var styles = ""
    /// The appearance a pill is drawn for: dark, and Increase Contrast;
    /// and whether a selected one is emphasized (its window is key).
    package var dark = false
    package var contrast = false
    package var emphasized = true
    /// A reader's lines run right to left, its node's `direction`: a pill
    /// whose face pins none is isolated in it (LLP 1045.000 D4).
    package var rtl = false
    package init(kinds: String = "", faces: String = "", styles: String = "", dark: Bool = false, contrast: Bool = false, rtl: Bool = false) {
        (self.kinds, self.faces, self.styles, self.dark, self.contrast, self.rtl) = (kinds, faces, styles, dark, contrast, rtl)
    }
    package var isEmpty: Bool { kinds.isEmpty || kinds == "[]" }
}

/// One inline atom in a Markdown editor's storage, the value of its
/// `.markupAtom` run (LLP 1045.000): what its pill draws and what a press on
/// it reports. A new object each restyle, so equal neighbours stay two runs.
package final class MarkupAtom: NSObject {
    package let kind: String, label: String, icon: String, title: String, role: String, href: String
    /// Its paint, as `TextEngine.color` reads it (RGBA, and a face's colour
    /// in its own space after them): its face's `color` and
    /// `backgroundColor`, else its kind's `color` and `background-color`;
    /// nil where neither says.
    package let ink: [Double]?, background: [Double]?
    /// The selection covers it: drawn in the selection's colour (D4),
    /// emphasized or not.
    package let selected: Bool, emphasized: Bool
    package let dark: Bool, contrast: Bool
    /// Its face's direction, `ltr` or `rtl`, which lays out its icon and
    /// label (D4); empty to follow its line's.
    package let direction: String
    /// Its face's `ariaLabel`; empty for its label.
    package let ariaLabel: String
    package init(kind: String, label: String, icon: String, title: String, role: String, href: String, ink: [Double]?, background: [Double]?,
                 selected: Bool, emphasized: Bool = true, dark: Bool, contrast: Bool, direction: String = "", ariaLabel: String = "") {
        (self.kind, self.label, self.icon, self.title, self.role, self.href) = (kind, label, icon, title, role, href)
        (self.ink, self.background, self.selected, self.emphasized, self.dark, self.contrast) = (ink, background, selected, emphasized, dark, contrast)
        (self.direction, self.ariaLabel) = (direction, ariaLabel)
    }
    /// What assistive technology reads it as (D8): its `ariaLabel`, else its label.
    package var spokenName: String { ariaLabel.isEmpty ? label : ariaLabel }
}

extension NSAttributedString.Key {
    /// An inline atom's source (LLP 1045.000 §4): TextKit draws its first
    /// code unit as the pill and the rest as nothing; the storage keeps it.
    /// Its value is the atom's `MarkupAtom`.
    package static let markupAtom = NSAttributedString.Key("ExactMarkupAtom")
    /// Source the Markdown editor keeps out of layout: a link's `](target)`
    /// (LLP 1045 D5). TextKit draws it as nothing; the storage keeps it.
    package static let markupHidden = NSAttributedString.Key("ExactMarkupHidden")
    /// The `[` of a link whose destination is hidden: an edit around the
    /// destination keeps it. Its value is the destination's.
    package static let markupOpener = NSAttributedString.Key("ExactMarkupOpener")
}

extension NSAttributedString {
    /// The text in `range` assistive technology reads: without what the
    /// editor hides, each atom its accessible name (LLP 1045.000 D8).
    package func shownText(in range: NSRange) -> String {
        let text = string as NSString
        var out = ""
        var at = range.location
        enumerateAttributes(in: range) { attributes, run, _ in
            let atom = attributes[.markupAtom] as? MarkupAtom
            guard attributes[.markupHidden] != nil || atom != nil else { return }
            out += text.substring(with: NSRange(location: at, length: run.location - at))
            // An atom's name, once, at its first code unit.
            if let atom, atomRange(at: run.location)?.location == run.location { out += atom.spokenName }
            at = NSMaxRange(run)
        }
        return out + text.substring(with: NSRange(location: at, length: NSMaxRange(range) - at))
    }
    /// Whether any source in `range` is hidden or drawn as an atom.
    package func hidesSource(in range: NSRange) -> Bool {
        var found = false
        enumerateAttributes(in: range) { attributes, _, stop in
            if attributes[.markupHidden] != nil || attributes[.markupAtom] != nil { found = true; stop.pointee = true }
        }
        return found
    }
    /// The atom whose source holds `p` (`start <= p < end`).
    package func atomRange(at p: Int) -> NSRange? {
        guard p >= 0, p < length else { return nil }
        var run = NSRange()
        return attribute(.markupAtom, at: p, longestEffectiveRange: &run, in: NSRange(location: 0, length: length)) != nil ? run : nil
    }
    /// Each atom's source range and mark, in order.
    package var atomRuns: [(range: NSRange, atom: MarkupAtom)] {
        var out: [(NSRange, MarkupAtom)] = []
        enumerateAttribute(.markupAtom, in: NSRange(location: 0, length: length)) { value, run, _ in
            if let atom = value as? MarkupAtom { out.append((run, atom)) }
        }
        return out
    }
}

/// The Markdown module, installed by the composition.
package enum MarkdownLink {
    package static var installed: MarkdownCapability?
}

extension NodeView {
    /// Its inline atoms (LLP 1045.000), as its props hold them, for the
    /// appearance it draws in.
    package var markdownAtoms: MarkdownAtoms {
        MarkdownAtoms(kinds: props["atoms"] ?? "", faces: props["atomFaces"] ?? "", styles: props["atomStyles"] ?? "",
                      dark: drawsDark, contrast: drawsHighContrast == true)
    }
    @discardableResult func formatMarkup(_ command: String, argument: String = "", selection: NSRange? = nil) -> Bool {
        MarkdownLink.installed?.format(self, command: command, argument: argument, selection: selection) ?? false
    }
    func editMarkupLink() { MarkdownLink.installed?.editLink(self) }
    func publishMarkupSelection(force: Bool = false) { MarkdownLink.installed?.publishSelection(self, force: force) }
    func restyleMarkup() {
        MarkdownLink.installed?.restyle(self)
        restyleDecorations()
    }
    /// The field's own direction is right to left: its `direction`, else the
    /// document's. A Markdown editor's line without a word takes it (LLP 1045 D11).
    package var markdownRTL: Bool { (style["direction"]?.string ?? presenter?.documentDirection ?? "ltr") == "rtl" }
    /// Restyle the editor when its props change what it draws but not its
    /// text, changes that come with no edit and so with no edit's restyle:
    /// new `paragraphDirs` (LLP 1045 D11), or new `atoms`, `atomFaces` or
    /// `atomStyles`, such as a face that follows an upload while its source
    /// stays (LLP 1045.000 D2). An unchanged prop is the very string the last
    /// restyle kept, so comparing it returns at once.
    func restyleMarkupProps() {
        guard let editor = (textArea as? TextArea)?.markup else { return }
        let drawn = editor.atoms
        guard editor.paragraphDirs != props["paragraphDirs"] ?? "" || drawn.kinds != props["atoms"] ?? ""
            || drawn.faces != props["atomFaces"] ?? "" || drawn.styles != props["atomStyles"] ?? "" else { return }
        restyleMarkup()
    }
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
