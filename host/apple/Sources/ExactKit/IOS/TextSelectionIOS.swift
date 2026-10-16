// @ref LLP 1115.000 D11.6 — `user-select="text"` on UIKit. Text its author
// made selectable is selected as a hand-built screen's is: a long press or a
// double tap takes the word under it with UIKit's handles, loupe and
// highlight, the handles drag, and the edit menu is UIKit's (Copy, Look Up,
// Translate, Search Web, Share…). All of that is a `UITextInteraction` in
// its non-editable mode; what Exact supplies is the text and its geometry,
// from the paragraph's own lines (`paragraphLayout`), so a selection's rects
// are the rects of the lines the kernel broke.
//
// Measured against a reference kept outside the repository (iOS 27.0, a
// `UITextView` and a custom `UITextInput` view side by side):
// - the interaction's recognizers take a touch only when its view is the
//   touch's hit view, so the interaction is on the paragraph's own view and
//   one selection is inside one paragraph (one `text`, its inline runs
//   included). A selection across paragraphs needs one view that takes every
//   touch on their text; that is not built.
// - nothing is paid for until a finger lands: the interaction (15
//   recognizers and 18 interactions of UIKit's) is added in the hit test
//   that finds a selectable paragraph, before UIKit gathers the touch's
//   recognizers, as a swipe row's cell is (`NodeView.hitTest`). At most
//   `TextSelectionHost.kept` paragraphs hold one. Text nobody made
//   selectable is never touched by any of this.
// - the text input is a responder that is not a view: no view per
//   paragraph, and it reports `isEditable` false, so no keyboard comes.
// - the menu asks the interaction's view for Copy, so the node answers it
//   (`Clipboard.swift`).
// A tap still presses: UIKit's single taps swallow a touch, so they are off
// until something is selected, when a tap in the text clears the selection
// and presses nothing, as in a text view.
#if os(iOS)
import CoreText
import UIKit

/// The paragraphs that hold a text interaction, and the one selection.
final class TextSelectionHost {
    private weak var presenter: Presenter?
    /// Oldest first.
    private(set) var texts: [SelectableText] = []
    /// How many paragraphs keep their interaction once a finger has left.
    static let kept = 3
    /// Where Copy writes: the system's pasteboard (a test names its own,
    /// since a unit test's process may not read the system's).
    var pasteboard = UIPasteboard.general
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Whether `node` is text a reader can select now.
    func admits(_ node: NodeView) -> Bool {
        node.isParagraph && presenter?.views[node.id] === node && node.textSelectable && !node.inert && !node.isHidden
    }
    func text(for node: NodeView) -> SelectableText? { texts.first { $0.node === node } }
    /// The selection, if there is one: its paragraph, and its range in that
    /// paragraph's own text (its source, as `selectionchange` reports it).
    var selection: (node: NodeView, text: String, range: NSRange)? {
        for held in texts { if let node = held.node, let found = held.reportable { return (node, found.text, found.range) } }
        return nil
    }

    /// A touch hit-tested to `node` (`NodeView.hitTest`): selectable text
    /// takes its interaction now, in time for this touch.
    func touched(_ node: NodeView) {
        guard text(for: node) == nil, admits(node) else { return }
        install(node)
    }
    @discardableResult
    func install(_ node: NodeView) -> SelectableText {
        if let held = text(for: node) { return held }
        while texts.count >= Self.kept, let old = texts.first(where: { !$0.busy }) { drop(old) }
        let made = SelectableText(node: node, host: self)
        texts.append(made)
        return made
    }
    /// The node is leaving (destroyed, or parked for another row).
    func remove(_ node: NodeView) { if let held = text(for: node) { drop(held) } }
    private func drop(_ held: SelectableText) {
        texts.removeAll { $0 === held }
        held.detach()
    }
    /// A batch was applied: text that changed drops its selection, text
    /// that is no longer selectable gives its interaction back, and a
    /// selection whose paragraph was laid out again is shown where it is.
    func batchEnded() {
        for held in texts where !held.settle() { drop(held) }
    }
    func clear() { texts.forEach { $0.clear() } }
    /// One selection on the page: `keep`'s.
    func selected(in keep: SelectableText) { for held in texts where held !== keep { held.clear() } }
    func report(_ node: NodeView, _ text: String, _ range: NSRange) {
        presenter?.selectionChange(node.id, text, range.location, NSMaxRange(range))
    }

    /// The word at `point` of `node` becomes the selection, as a double tap
    /// or a long press there makes it: the agent's `tap <id> dblclick` and
    /// `contextmenu`, which synthesize no touch. Nil when `node` is not
    /// selectable text or has no word there.
    @discardableResult
    func selectWord(in node: NodeView, at point: CGPoint) -> (text: String, range: NSRange)? {
        guard admits(node) else { return nil }
        let held = install(node)
        guard held.selectWord(at: point) else { return nil }
        return held.reportable
    }
    /// Copy: the selected text to the pasteboard. False with nothing selected.
    @discardableResult
    func copy(_ node: NodeView) -> Bool {
        guard let chosen = text(for: node)?.selectedText, !chosen.isEmpty else { return false }
        pasteboard.string = chosen
        return true
    }
}

private final class SelectablePosition: UITextPosition {
    let offset: Int
    init(_ offset: Int) { self.offset = offset }
}
private final class SelectableRange: UITextRange {
    let range: NSRange
    init(_ range: NSRange) { self.range = range }
    override var start: UITextPosition { SelectablePosition(range.location) }
    override var end: UITextPosition { SelectablePosition(NSMaxRange(range)) }
    override var isEmpty: Bool { range.length == 0 }
}
private final class SelectableRect: UITextSelectionRect {
    private let box: CGRect, first: Bool, last: Bool, rtl: Bool
    init(_ box: CGRect, first: Bool, last: Bool, rtl: Bool) { self.box = box; self.first = first; self.last = last; self.rtl = rtl }
    override var rect: CGRect { box }
    override var writingDirection: NSWritingDirection { rtl ? .rightToLeft : .leftToRight }
    override var containsStart: Bool { first }
    override var containsEnd: Bool { last }
    override var isVertical: Bool { false }
}

/// One selectable paragraph's text input: what UIKit's interaction asks for
/// the text, the selection and where they are. Offsets are UTF-16 offsets of
/// the shaped text, the layout's own (CoreText's and NSString's).
final class SelectableText: UIResponder, UITextInput, UITextInteractionDelegate {
    private(set) weak var node: NodeView?
    private unowned let host: TextSelectionHost
    let interaction = UITextInteraction(for: .nonEditable)
    /// The recognizers and the interactions UIKit added with the
    /// interaction, which it does not all take back with it.
    private var recognizers: [UIGestureRecognizer] = []
    private var companions: [UIInteraction] = []
    private(set) var selected = NSRange(location: 0, length: 0)
    /// The text the selection is in; a different one drops it (`settle`).
    private var text: NSString
    private var laid = CGSize.zero
    private var reported = NSRange(location: 0, length: 0)

    static func shaped(_ node: NodeView) -> NSString { node.paragraphSpec().runs.map(\.text).joined() as NSString }

    init(node: NodeView, host: TextSelectionHost) {
        self.node = node; self.host = host
        text = Self.shaped(node); laid = node.bounds.size
        super.init()
        interaction.textInput = self
        interaction.delegate = self
        let before = node.gestureRecognizers ?? [], held = node.interactions
        node.addInteraction(interaction)
        recognizers = (node.gestureRecognizers ?? []).filter { added in !before.contains { $0 === added } }
        companions = node.interactions.filter { added in added !== interaction && !held.contains { $0 === added } }
        for recognizer in recognizers {
            // None of them holds back the node's own touch: its press is a
            // touch down and up, and fires at the lift, once.
            recognizer.delaysTouchesEnded = false
        }
        syncTaps()
    }

    /// Selecting, or a finger of its own down: not to be taken away.
    var busy: Bool {
        selected.length > 0 || recognizers.contains { $0.numberOfTouches > 0 || $0.state == .began || $0.state == .changed }
    }
    func detach() {
        clear()
        if isFirstResponder { _ = resignFirstResponder() }
        guard let node else { return }
        node.removeInteraction(interaction)
        // Measured: two of UIKit's eighteen interactions stay behind.
        for left in node.interactions where companions.contains(where: { $0 === left }) { node.removeInteraction(left) }
        for left in node.gestureRecognizers ?? [] where recognizers.contains(where: { $0 === left }) { node.removeGestureRecognizer(left) }
        recognizers = []; companions = []
    }
    /// After a batch. False when the node no longer takes a selection.
    func settle() -> Bool {
        guard let node, host.admits(node) else { return false }
        let now = Self.shaped(node)
        if now != text {
            clear()
            text = now
        } else if selected.length > 0, node.bounds.size != laid {
            display?.setNeedsSelectionUpdate()
        }
        laid = node.bounds.size
        return true
    }

    /// UIKit's single taps swallow the touch they recognize. With nothing
    /// selected they have nothing to do, so the node's own press hears the
    /// tap; with a selection up a tap in the text clears it and is not a
    /// press (measured: a text view does the same).
    private func syncTaps() {
        let on = selected.length > 0
        for recognizer in recognizers {
            if let tap = recognizer as? UITapGestureRecognizer, tap.numberOfTapsRequired == 1, tap.isEnabled != on { tap.isEnabled = on }
        }
    }
    private var display: UITextSelectionDisplayInteraction? {
        node?.interactions.lazy.compactMap { $0 as? UITextSelectionDisplayInteraction }.first
    }

    // MARK: the selection

    /// The selected text as it is copied.
    var selectedText: String { selected.length > 0 ? text.substring(with: selected) : "" }
    /// The selection in the node's source text, as `selectionchange` has it.
    var reportable: (text: String, range: NSRange)? {
        guard let node, selected.length > 0 else { return nil }
        let source = node.paragraphText as NSString
        let lo = min(node.sourceOffset(selected.location), source.length), hi = min(node.sourceOffset(NSMaxRange(selected)), source.length)
        guard hi > lo else { return nil }
        let range = NSRange(location: lo, length: hi - lo)
        return (source.substring(with: range), range)
    }
    private func set(_ range: NSRange) {
        let lo = min(max(0, range.location), text.length), hi = min(max(lo, NSMaxRange(range)), text.length)
        let next = NSRange(location: lo, length: hi - lo)
        guard next != selected else { return }
        selected = next
        syncTaps()
        if next.length > 0 { host.selected(in: self) }
        report()
    }
    /// The web's `selectionchange` on a `text`: its part of the selection
    /// when that changes; nothing selected there is "" at 0, 0.
    private func report() {
        guard let node, node.handlers.contains("selectionchange") else { return }
        let found = reportable
        let range = found?.range ?? NSRange(location: 0, length: 0)
        guard range != reported else { return }
        reported = range
        host.report(node, found?.text ?? "", range)
    }
    /// Nothing is selected any more, said to UIKit so its handles go.
    func clear() {
        guard selected.length > 0 else { return }
        inputDelegate?.selectionWillChange(self)
        set(NSRange(location: 0, length: 0))
        inputDelegate?.selectionDidChange(self)
        node?.interactions.compactMap { $0 as? UIEditMenuInteraction }.forEach { $0.dismissMenu() }
    }
    /// The word at `point` (the node's space), by UIKit's own tokenizer, as
    /// its double tap takes it; shown as a gesture's selection is.
    func selectWord(at point: CGPoint) -> Bool {
        let at = SelectablePosition(index(at: point))
        guard let word = (tokenizer.rangeEnclosingPosition(at, with: .word, inDirection: .storage(.forward))
            ?? tokenizer.rangeEnclosingPosition(at, with: .word, inDirection: .storage(.backward))) as? SelectableRange, word.range.length > 0 else { return false }
        if !isFirstResponder { _ = becomeFirstResponder() }
        inputDelegate?.selectionWillChange(self)
        set(word.range)
        inputDelegate?.selectionDidChange(self)
        display?.isActivated = true
        display?.setNeedsSelectionUpdate()
        return true
    }

    // MARK: the responder

    override var next: UIResponder? { node }
    override var canBecomeFirstResponder: Bool { node?.window != nil }
    override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned { clear() }
        return resigned
    }
    /// Read only: no keyboard, and a long press selects a word.
    var isEditable: Bool { false }
    /// Before iOS 18 nothing asks `isEditable`; an empty input view keeps
    /// the keyboard away there.
    override var inputView: UIView? {
        if #available(iOS 18.0, *) { return super.inputView }
        return UIView()
    }
    func interactionShouldBegin(_ interaction: UITextInteraction, at point: CGPoint) -> Bool {
        guard let node, host.admits(node) else { return false }
        return true
    }

    // MARK: geometry, from the paragraph's lines

    private var layout: (paragraph: Paragraph, spec: Spec, box: CGRect)? {
        guard let node, let paragraph = node.paragraphLayout(), !paragraph.lines.isEmpty else { return nil }
        return (paragraph, node.paragraphSpec(), node.paragraphBox())
    }
    /// A rect of line `line` of the unfragmented paragraph, where its
    /// fragment shows it (LLP 1093 D8).
    private func shown(_ rect: CGRect, line: Int) -> CGRect {
        guard let fragment = node?.columnRecord?.fragments.first(where: { $0.lines.contains(line) }) else { return rect }
        return rect.offsetBy(dx: fragment.dx, dy: fragment.dy)
    }
    /// The line an offset is on: the last that starts at or before it.
    private func line(of offset: Int, in paragraph: Paragraph) -> Int {
        var found = 0
        for i in paragraph.lines.indices where CTLineGetStringRange(paragraph.lines[i]).location <= offset { found = i }
        return found
    }
    /// The caret nearest `point`, in the node's space.
    func index(at point: CGPoint) -> Int {
        guard let node, let (paragraph, spec, box) = layout else { return 0 }
        let mapped = node.caretPoint(point)
        if mapped.y < box.minY { return 0 }
        guard let line = paragraph.lineIndex(at: CGPoint(x: mapped.x - box.minX, y: mapped.y - box.minY), align: spec.align, width: box.width) else { return 0 }
        let found = paragraph.stringIndex(in: line, at: mapped.x - box.minX - paragraph.origin(line, align: spec.align, width: box.width))
        return found == kCFNotFound ? text.length : min(max(0, found), text.length)
    }
    private func rects(_ range: NSRange) -> [SelectableRect] {
        guard let (paragraph, spec, box) = layout else { return [] }
        let everywhere = CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7)
        let lines = paragraph.selectedLines(range, align: spec.align, in: box, dirty: everywhere)
        return lines.enumerated().map { i, found in
            // A line that reads right to left starts at its right edge.
            let line = paragraph.lines[found.line], within = CTLineGetStringRange(line)
            let lo = max(range.location, within.location), hi = min(NSMaxRange(range), within.location + within.length)
            let rtl = CTLineGetOffsetForStringIndex(line, lo, nil) > CTLineGetOffsetForStringIndex(line, hi, nil)
            return SelectableRect(shown(found.rect, line: found.line), first: i == 0, last: i == lines.count - 1, rtl: rtl)
        }
    }

    // MARK: UITextInput

    var hasText: Bool { text.length > 0 }
    func insertText(_ text: String) {}
    func deleteBackward() {}
    func text(in range: UITextRange) -> String? {
        guard let range = (range as? SelectableRange)?.range, NSMaxRange(range) <= text.length else { return nil }
        return text.substring(with: range)
    }
    func replace(_ range: UITextRange, withText text: String) {}
    var selectedTextRange: UITextRange? {
        get { selected.length > 0 ? SelectableRange(selected) : nil }
        set { set((newValue as? SelectableRange)?.range ?? NSRange(location: 0, length: 0)) }
    }
    var markedTextRange: UITextRange? { nil }
    var markedTextStyle: [NSAttributedString.Key: Any]? { get { nil } set {} }
    func setMarkedText(_ markedText: String?, selectedRange: NSRange) {}
    func unmarkText() {}
    var beginningOfDocument: UITextPosition { SelectablePosition(0) }
    var endOfDocument: UITextPosition { SelectablePosition(text.length) }
    func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        guard let a = (fromPosition as? SelectablePosition)?.offset, let b = (toPosition as? SelectablePosition)?.offset else { return nil }
        return SelectableRange(NSRange(location: min(a, b), length: abs(b - a)))
    }
    func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
        guard let from = (position as? SelectablePosition)?.offset else { return nil }
        let to = from + offset
        return to < 0 || to > text.length ? nil : SelectablePosition(to)
    }
    func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int) -> UITextPosition? {
        self.position(from: position, offset: direction == .left || direction == .up ? -offset : offset)
    }
    func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        let a = (position as? SelectablePosition)?.offset ?? 0, b = (other as? SelectablePosition)?.offset ?? 0
        return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
    }
    func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
        ((toPosition as? SelectablePosition)?.offset ?? 0) - ((from as? SelectablePosition)?.offset ?? 0)
    }
    weak var inputDelegate: UITextInputDelegate?
    lazy var tokenizer: UITextInputTokenizer = UITextInputStringTokenizer(textInput: self)
    func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        direction == .left || direction == .up ? range.start : range.end
    }
    func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection) -> UITextRange? {
        guard let at = (position as? SelectablePosition)?.offset else { return nil }
        let back = direction == .left || direction == .up
        return SelectableRange(back ? NSRange(location: max(0, at - 1), length: min(1, at)) : NSRange(location: at, length: min(1, text.length - at)))
    }
    func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection) -> NSWritingDirection {
        node?.style["direction"]?.string == "rtl" ? .rightToLeft : .leftToRight
    }
    func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {}
    func firstRect(for range: UITextRange) -> CGRect {
        (range as? SelectableRange).flatMap { rects($0.range).first?.rect } ?? caretRect(for: range.start)
    }
    func caretRect(for position: UITextPosition) -> CGRect {
        guard let offset = (position as? SelectablePosition)?.offset, let (paragraph, spec, box) = layout else { return .zero }
        let i = line(of: offset, in: paragraph), line = paragraph.lines[i]
        var above: CGFloat = 0, below: CGFloat = 0
        _ = CTLineGetTypographicBounds(line, &above, &below, nil)
        let x = box.minX + paragraph.origin(i, align: spec.align, width: box.width) + CTLineGetOffsetForStringIndex(line, offset, nil)
        return shown(CGRect(x: x - 1, y: box.minY + paragraph.baselines[i].rounded() - above, width: 2, height: above + below), line: i)
    }
    func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        (range as? SelectableRange).map { rects($0.range) } ?? []
    }
    func closestPosition(to point: CGPoint) -> UITextPosition? { SelectablePosition(index(at: point)) }
    func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
        guard let within = (range as? SelectableRange)?.range else { return nil }
        return SelectablePosition(min(max(index(at: point), within.location), NSMaxRange(within)))
    }
    func characterRange(at point: CGPoint) -> UITextRange? {
        let at = index(at: point)
        return at < text.length ? SelectableRange(text.rangeOfComposedCharacterSequence(at: at)) : nil
    }
    var textInputView: UIView { node ?? UIView() }
}

extension Presenter {
    /// The agent's `tap <id> dblclick` or `contextmenu` on selectable text,
    /// `point` in the window: the word there is selected, as a double tap
    /// or a long press selects it, and the reply's `selection` says what
    /// (`text`, and `start` and `end` in the node's own text, as
    /// `selectionchange` has them). Nil when `hit` is not selectable text,
    /// has no word there, or a long press belongs to its own or a row's menu.
    func agentSelect(_ hit: UIView, at point: CGPoint, event: String) -> [String: Any]? {
        guard let node = hit as? NodeView, textSelection.admits(node) else { return nil }
        if event == "contextmenu", node.contextMenuSource != nil { return nil }
        guard let found = textSelection.selectWord(in: node, at: node.convert(point, from: nil)) else { return nil }
        return ["text": found.text, "start": found.range.location, "end": NSMaxRange(found.range)]
    }
}

extension NodeView {
    /// A touch hit-tested here (`hitTest`): selectable text is made ready
    /// for it.
    func touchedText() { presenter?.textSelection.touched(self) }
    /// A row's menu wins a long press over selectable text; an authored
    /// menu on the paragraph itself is the nearest menu. Disabled controls
    /// or inert ancestry admit neither one.
    var contextMenuSource: NodeView? {
        guard let presenter, presenter.menus.eligible(self) else { return nil }
        var next: UIView? = self
        while let view = next {
            if let node = view as? NodeView,
               node.handlers.contains("contextmenu") || node.props["contextPopover"]?.isEmpty == false { return node }
            next = presenter.listenerParent(of: view)
        }
        return nil
    }
    /// Dynamic failure ordering follows the current row, including a menu
    /// added or removed after this paragraph took its text interaction.
    package func gestureRecognizer(_ gesture: UIGestureRecognizer, shouldBeRequiredToFailBy other: UIGestureRecognizer) -> Bool {
        guard gesture === contextRecognizer, let paragraph = other.view as? NodeView,
              paragraph.contextMenuSource === self,
              let text = presenter?.textSelection.text(for: paragraph) else { return false }
        return text.interaction.gesturesForFailureRequirements.contains { $0 === other }
    }
}
#endif
