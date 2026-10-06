// A text field's selection as the DOM reports it (x2apps codeedit #2): what
// an input's or a textarea's `input` and `change` carry (ABI kinds 40, 41),
// its `select` (42), and `setSelectionRange`. Offsets are UTF-16, as
// NSString's and UITextInput's are. `select` fires as Chrome fires it: when
// the person makes the selection a non-collapsed one (each extension, never
// a caret move or typing), and when a script sets a selection (even a caret)
// unless it set that same one last with nothing between. A selection set on
// a field that is not being edited waits for its focus, where AppKit's field
// editor would otherwise select all. The Markdown editor keeps its own
// (`MarkupEditor`, kind 21).
import Foundation
#if canImport(AppKit)
import AppKit
#else
import UIKit
#endif

/// `selectionStart`, `selectionEnd` and `selectionDirection`
/// (`exact_runner::FieldSelection`).
struct FieldSelection: Equatable {
    var start: Int
    var end: Int
    var direction = "none"

    init(start: Int, end: Int, direction: String = "none") {
        self.start = start
        self.end = end
        self.direction = direction
    }
    init(_ range: NSRange) { self.init(start: range.location, end: NSMaxRange(range)) }

    var collapsed: Bool { start == end }
    var range: NSRange { NSRange(location: start, length: end - start) }
    func same(_ other: FieldSelection?) -> Bool { other.map { $0.start == start && $0.end == end } ?? false }

    /// ABI kinds 40 to 42's payload: `start,end,direction,` then the value verbatim.
    func payload(_ value: String) -> String { "\(start),\(end),\(direction),\(value)" }

    /// `setSelectionRange(start, end, direction)` on a value `length` UTF-16
    /// units long, as HTML converts and clamps it (`FieldSelection::clamped`):
    /// WebIDL's ToUint32 (so -1 is past the end), each end to the length, a
    /// start past the end moved to it; a word other than `forward` or
    /// `backward` is `none`.
    static func clamped(_ start: Double, _ end: Double, _ word: String?, length: Int) -> FieldSelection {
        func at(_ n: Double) -> Int {
            let wrap = 4_294_967_296.0
            let u = n.isFinite ? (n.rounded(.towardZero).truncatingRemainder(dividingBy: wrap) + wrap).truncatingRemainder(dividingBy: wrap) : 0
            return Int(min(u, Double(length)))
        }
        let e = at(end)
        return FieldSelection(start: min(at(start), e), end: e, direction: word == "forward" || word == "backward" ? word! : "none")
    }

    /// The direction a person's change from `last` to `next` has, as Chrome
    /// reports a Shift-extension: `forward` when the end that moved is the
    /// later one, `backward` when the earlier; the anchor is the end the
    /// last selection kept (a caret's own place). `none` for a caret, or a
    /// selection that keeps neither end (a double-click's word).
    static func direction(from last: FieldSelection?, to next: FieldSelection) -> String {
        guard !next.collapsed, let last else { return "none" }
        let anchor: Int? = last.collapsed ? last.start
            : last.direction == "forward" ? last.start : last.direction == "backward" ? last.end : nil
        if let anchor { return anchor == next.start ? "forward" : anchor == next.end ? "backward" : "none" }
        return next.start == last.start ? "forward" : next.end == last.end ? "backward" : "none"
    }
}

final class FieldSelections {
    unowned let presenter: Presenter
    /// A text field's `select` (`Bridge.fieldSelect`), where it has a handler.
    var onSelect: ((UInt32, String, FieldSelection) -> Void)?
    /// Each field's selection as last seen, with its direction.
    private var last: [UInt32: FieldSelection] = [:]
    /// The selection a script last set, until anything else moves it.
    private var scripted: [UInt32: FieldSelection] = [:]
    /// A selection set while the field was not being edited, for its focus.
    private var pending: [UInt32: FieldSelection] = [:]
    /// Selection changes the host makes, which are not the person's.
    private var quiet = 0
    #if os(macOS)
    private var observer: NSObjectProtocol?
    #endif

    init(_ presenter: Presenter) {
        self.presenter = presenter
        #if os(macOS)
        // AppKit tells a field editor's selection to its field, not to the
        // field's delegate; every text view posts the notification.
        observer = NotificationCenter.default.addObserver(forName: NSTextView.didChangeSelectionNotification, object: nil, queue: nil) { [weak self] note in
            guard let self, let editor = note.object as? NSTextView else { return }
            let owner = (editor as? TextArea)?.owner ?? ((editor.delegate as? NSTextField)?.delegate as? NodeView)
            if let owner { changed(owner) }
        }
        #endif
    }
    deinit {
        #if os(macOS)
        if let observer { NotificationCenter.default.removeObserver(observer) }
        #endif
    }

    private var applying: Bool {
        #if os(macOS)
        presenter.isApplying
        #else
        presenter.applying
        #endif
    }

    func quietly<T>(_ work: () -> T) -> T {
        quiet += 1
        defer { quiet -= 1 }
        return work()
    }

    /// A text field this follows: an input's field or a textarea, never the
    /// Markdown editor (its own `select`) or the emoji picker (cleared as
    /// each emoji is taken).
    static func tracks(_ node: NodeView) -> Bool {
        (node.field != nil || node.textArea != nil) && node.props["markup"] != "markdown" && node.props["emojiPicker"] != "true"
    }

    /// The field's selection now: the editor's, its direction the one last
    /// seen for that range; else the one waiting for its focus or last seen;
    /// else the caret after its value.
    func current(_ node: NodeView) -> FieldSelection {
        if let live = node.liveSelection.map(FieldSelection.init) {
            if let known = last[node.id], known.same(live) { return known }
            return live
        }
        return pending[node.id] ?? last[node.id] ?? FieldSelection(start: node.fieldText.utf16.count, end: node.fieldText.utf16.count)
    }

    /// What an `input` or `change` of `value` at `id` reports: the field's
    /// selection, a caret's direction `none`; nil for a node that is no text
    /// field, or a selection `value` does not hold (the runner then assumes
    /// the caret after it).
    func reported(_ id: UInt32, _ value: String) -> FieldSelection? {
        guard let node = presenter.views[id], Self.tracks(node) else { return nil }
        var s = current(node)
        if s.collapsed { s.direction = "none" }
        return s.end <= value.utf16.count ? s : nil
    }

    /// The editor's selection moved (the platform's notification).
    func changed(_ node: NodeView) {
        guard presenter.views[node.id] === node, Self.tracks(node), let range = node.liveSelection else { return }
        var next = FieldSelection(range)
        let before = last[node.id]
        guard !next.same(before) else { return }
        // A batch's value written in, a focus's own select-all, a script's
        // range: not the person's.
        if quiet > 0 || applying {
            last[node.id] = next
            return
        }
        next.direction = FieldSelection.direction(from: before, to: next)
        last[node.id] = next
        scripted[node.id] = nil
        pending[node.id] = nil
        if !next.collapsed { select(node, next) }
    }

    /// The field took the focus: a selection set while it had none is
    /// shown now, else the editor's is what was last seen.
    func focused(_ node: NodeView) {
        guard Self.tracks(node) else { return }
        quietly {
            if let s = pending.removeValue(forKey: node.id) {
                _ = node.showSelection(s.range)
                last[node.id] = s
            } else if let live = node.liveSelection.map(FieldSelection.init), !live.same(last[node.id]) {
                last[node.id] = live
            }
        }
    }

    /// The command `setSelectionRange(id, start, end[, direction])`, after
    /// the batch is applied: it never moves the focus.
    func setSelectionRange(_ args: [Any]) {
        let number = { (a: Any) in (a as? NSNumber)?.doubleValue }
        guard args.count == 3 || args.count == 4, let name = args[0] as? String,
              let start = number(args[1]), let end = number(args[2]) else {
            presenter.session?.log("setSelectionRange refused: (id, start, end[, direction]) expected")
            return
        }
        guard let node = presenter.views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else {
            presenter.session?.log("setSelectionRange \"\(name)\" refused: no live node with that id")
            return
        }
        guard Self.tracks(node) else {
            presenter.session?.log("setSelectionRange \"\(name)\" refused: not a text field")
            return
        }
        let word = args.count == 4 ? args[3] as? String : nil
        set(node, .clamped(start, end, word, length: node.fieldText.utf16.count))
    }

    /// `selectText` (the DOM's `select()`) on a focused editor: the whole
    /// value, direction none; the Markdown editor and the emoji picker
    /// select natively.
    func selectAll(_ node: NodeView) {
        guard Self.tracks(node) else {
            #if os(macOS)
            (node.textArea ?? node.field?.currentEditor())?.selectAll(nil)
            #else
            if let editor = node.textArea { editor.selectAll(editor) } else if let editor = node.field { editor.selectAll(editor) }
            #endif
            return
        }
        let length = node.fieldText.utf16.count
        set(node, FieldSelection(start: 0, end: length))
    }

    private func set(_ node: NodeView, _ s: FieldSelection) {
        let again = scripted[node.id] == s
        scripted[node.id] = s
        last[node.id] = s
        quietly {
            if node.showSelection(s.range), node.editingField { pending[node.id] = nil } else { pending[node.id] = s }
        }
        if !again { select(node, s) }
    }

    private func select(_ node: NodeView, _ s: FieldSelection) {
        guard node.handlers.contains("select"), !node.disabled else { return }
        onSelect?(node.id, node.fieldText, s)
    }

    func reset() {
        last.removeAll()
        scripted.removeAll()
        pending.removeAll()
    }
}

extension NodeView {
    /// The field's value as its editor holds it.
    var fieldText: String {
        #if os(macOS)
        textArea?.string ?? field?.stringValue ?? ""
        #else
        textArea?.text ?? field?.text ?? ""
        #endif
    }

    #if os(macOS)
    /// The text view whose selection is the field's: a textarea's own, an
    /// input's field editor while it edits that input.
    private var selectionEditor: NSTextView? {
        if let textArea { return textArea }
        guard let field, let editor = field.currentEditor() as? NSTextView, field.window?.firstResponder === editor else { return nil }
        return editor
    }
    var liveSelection: NSRange? { selectionEditor?.selectedRange() }
    var editingField: Bool {
        if let textArea { return textArea.window?.firstResponder === textArea }
        return selectionEditor != nil
    }
    func showSelection(_ range: NSRange) -> Bool {
        guard let editor = selectionEditor, NSMaxRange(range) <= (editor.string as NSString).length else { return false }
        editor.setSelectedRange(range)
        return true
    }
    #else
    var liveSelection: NSRange? {
        if let textArea { return textArea.selectedRange }
        guard let field, field.isFirstResponder, let r = field.selectedTextRange else { return nil }
        return NSRange(location: field.offset(from: field.beginningOfDocument, to: r.start), length: field.offset(from: r.start, to: r.end))
    }
    var editingField: Bool { textArea?.isFirstResponder ?? field?.isFirstResponder ?? false }
    func showSelection(_ range: NSRange) -> Bool {
        if let textArea {
            guard NSMaxRange(range) <= (textArea.text as NSString).length else { return false }
            textArea.selectedRange = range
            return true
        }
        guard let field, field.isFirstResponder, let start = field.position(from: field.beginningOfDocument, offset: range.location),
              let end = field.position(from: start, offset: range.length) else { return false }
        field.selectedTextRange = field.textRange(from: start, to: end)
        return true
    }
    #endif
}
