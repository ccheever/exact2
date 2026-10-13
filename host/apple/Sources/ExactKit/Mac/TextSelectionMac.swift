// @ref LLP 1033: selection is host state over the same CoreText lines that
// measure and paint. UTF-16 offsets match CoreText and NSString, including emoji.
#if os(macOS)
import AppKit
import CoreText

package final class TextSelection {
    weak var presenter: Presenter?
    private weak var anchor: NodeView?
    private weak var focus: NodeView?
    private var anchorIndex = 0
    private var focusIndex = 0
    private var dragged = false
    private var shown: [NodeView]?
    private var ordered: [NodeView]?
    private var indices: [UInt32: Int] = [:]
    private var painted: [UInt32: NSRange] = [:]
    /// What each `selectionchange` text last heard, in source offsets.
    private var reported: [UInt32: NSRange] = [:]
    // Collapsing whitespace is not invertible: retain scripted source boundaries.
    private var requested: (id: UInt32, text: String, range: NSRange)?
    private struct Position {
        var key: String
        var row: Int
        var paragraph: Int
        var offset: Int
        var tuple: (String, Int, Int) { (key, paragraph, offset) }
        var order: (Int, Int, Int) { (row, paragraph, offset) }
    }
    private weak var list: NodeView?
    private var logicalAnchor: Position?
    private var logicalFocus: Position?
    private var allListText = false
    private var positions: [UInt32: Position] = [:]
    private var gesture: UInt64 = 0
    private var motion: UInt64 = 0
    private var pendingBegin = false
    private var deferredDrag: NSEvent?
    private var deferredEnd: (NodeView, NSEvent)?
    /// The text view AppKit's menu for the selection is asked of (`systemMenu`).
    var menuText: SelectionMenuText?

    func structureChanged() {
        shown = nil; ordered = nil
        indices.removeAll(keepingCapacity: true)
        positions.removeAll(keepingCapacity: true)
        if let list, var a = logicalAnchor, var b = logicalFocus {
            if let ai = presenter?.onListIndex?(list.id, a.key), let bi = presenter?.onListIndex?(list.id, b.key) {
                a.row = ai; b.row = bi; logicalAnchor = a; logicalFocus = b
            } else {
                self.list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
                anchor = nil; focus = nil
            }
        }
    }

    init(_ presenter: Presenter) {
        self.presenter = presenter
        focusObservers = PageFacts.observe { [weak self] in self?.focusChanged() }
    }
    deinit { PageFacts.forget(focusObservers) }

    /// The fill of selected text (LLP 1115.000 D7.9): AppKit's selection
    /// colour at full strength, and its unemphasized grey while the window
    /// does not have the focus.
    static func fill(focused: Bool) -> NSColor {
        focused ? .selectedTextBackgroundColor : .unemphasizedSelectedTextBackgroundColor
    }
    /// The selected glyphs' colour: AppKit's own for text on that fill. Which
    /// glyphs take it is as an `NSTextView` has it, measured on macOS 27.0
    /// (LLP 1115.000 D7.9), with or without the window's focus:
    /// - dark appearance: every selected glyph, whatever its ink;
    /// - light appearance: text whose colour nobody wrote, and text in a
    ///   system colour (a role: a label tier, `AccentColor`, an inline
    ///   link's own blue). A colour its author fixed keeps its ink at its
    ///   own alpha: a literal (red stays red, and white stays white on the
    ///   pale fill), a `light-dark()` pair, as a dynamic `NSColor` does,
    ///   and a colour from the app's asset catalogue.
    /// A run says which it is (`Run.fixedInk`, from its colour's kind on the
    /// wire, `BatchValue.fixesInk`), and the painter is told the appearance
    /// (`keepsFixed`).
    static func ink(focused: Bool) -> NSColor {
        focused ? .selectedTextColor : .unemphasizedSelectedTextColor
    }
    /// Whether selected text in a colour its author fixed keeps it: in the
    /// light appearance, where AppKit's does (`ink`).
    static func keepsFixedInk(dark: Bool) -> Bool { !dark }
    /// `fill`, `ink` and `keepsFixedInk` as a raster request carries them
    /// (`RegionRasterRequest.selectionColor`): the three as `appearance` has
    /// them, the colours in `space`.
    static func rasterColors(focused: Bool, in appearance: NSAppearance, space: NSColorSpace) -> [CGFloat]? {
        var colors: [NSColor] = []
        appearance.performAsCurrentDrawingAppearance {
            colors = [fill(focused: focused), ink(focused: focused)].compactMap { $0.usingColorSpace(space) }
        }
        guard colors.count == 2 else { return nil }
        let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        return colors.flatMap { [$0.redComponent, $0.greenComponent, $0.blueComponent, $0.alphaComponent] } + [keepsFixedInk(dark: dark) ? 1 : 0]
    }
    /// The window's focus may have changed (`PageFacts.hasFocus`): what is
    /// selected is painted again, in the colour that goes with it.
    private var focusObservers: [NSObjectProtocol] = []
    func focusChanged() {
        for id in painted.keys { presenter?.views[id]?.needsDisplay = true }
    }

    /// Whether anything is selected at all: the cheap question a paragraph
    /// asks before the exact one (`range`).
    var isActive: Bool { allListText || logicalAnchor != nil || (anchor != nil && focus != nil) }

    /// Every shown paragraph in document order, whether or not it can be
    /// selected: the text pump owes each its pixels, a button's label as
    /// much as a paragraph (`refreshVisibleText`, X82).
    var shownParagraphs: [NodeView] {
        if let shown { return shown }
        guard let presenter else { return [] }
        var result: [NodeView] = []
        let tops: [NSView] = [presenter.root] + presenter.dialogs.presented + presenter.menus.presented
        for top in tops { texts(in: top) { result.append($0) } }
        shown = result
        return result
    }

    var paragraphs: [NodeView] {
        if let ordered { return ordered.filter(selectable) }
        guard let presenter else { return [] }
        var result: [NodeView] = []
        let tops: [NSView] = [presenter.root] + presenter.dialogs.presented + presenter.menus.presented
        // Which text is hidden or inert is asked each time: `inert` changes
        // with a prop or a dialog, which leave this order as it is.
        for top in tops { texts(in: top) { if $0.textSelectable { result.append($0) } } }
        ordered = result
        indices = Dictionary(uniqueKeysWithValues: result.enumerated().map { ($0.element.id, $0.offset) })
        return result.filter(selectable)
    }

    /// The paragraphs under `view` in order, selectable or not, as the
    /// runner's text projection walks a row: a hidden view and a region's
    /// own are left out, and an inline run is its paragraph's.
    private func texts(in view: NSView, _ found: (NodeView) -> Void) {
        if view.isHidden { return }
        if let node = view as? NodeView {
            if presenter?.session?.regions.owns(node) == true { return }
            if node.isParagraph { found(node); return }
        }
        for child in view.subviews { texts(in: child, found) }
    }

    private func invalidate() {
        var next: [UInt32: NSRange] = [:]
        for node in paragraphs {
            let selected = range(node).flatMap { $0.length > 0 ? $0 : nil }
            if let selected { next[node.id] = selected }
            if selected != painted[node.id] { node.needsDisplay = true }
        }
        if next != painted { forgetMenuText() }
        painted = next
        report(next)
    }

    /// The web's `selectionchange` on a `text` (the reader diary): its part
    /// of the selection, in its source text's UTF-16 offsets, when that part
    /// changes; nothing selected there is "" at 0, 0, as on the web.
    private func report(_ next: [UInt32: NSRange]) {
        guard let presenter else { return }
        var seen = Set<UInt32>()
        for node in paragraphs where node.handlers.contains("selectionchange") || node.inlineText.contains(where: { $0.handlers.contains("selectionchange") }) {
            let text = node.paragraphText as NSString
            var source = NSRange(location: 0, length: 0)
            if let requested, requested.id == node.id, anchor === node, requested.text == node.paragraphText {
                source = requested.range
            } else if let shaped = next[node.id] {
                let map = { (offset: Int) in min(node.readerParagraph == nil ? node.sourceOffset(offset) : offset, text.length) }
                let lo = map(shaped.location), hi = map(NSMaxRange(shaped))
                if hi > lo { source = NSRange(location: lo, length: hi - lo) }
            }
            var targets: [(UInt32, NSRange)] = []
            if node.handlers.contains("selectionchange") { targets.append((node.id, NSRange(location: 0, length: text.length))) }
            targets += node.inlineText.filter { $0.handlers.contains("selectionchange") }.map { ($0.id, $0.range) }
            for (id, extent) in targets {
                seen.insert(id)
                let part = NSIntersectionRange(source, extent)
                let local = part.length > 0 ? NSRange(location: part.location - extent.location, length: part.length) : NSRange(location: 0, length: 0)
                guard reported[id] ?? NSRange(location: 0, length: 0) != local else { continue }
                reported[id] = local
                let lo = node.readerParagraph == nil ? node.shapedOffset(part.location) : part.location
                let hi = node.readerParagraph == nil ? node.shapedOffset(NSMaxRange(part)) : NSMaxRange(part)
                let rects = part.length > 0 ? rectangles(node, range: NSRange(location: lo, length: hi - lo)) : []
                presenter.selectionChange(id, part.length > 0 ? text.substring(with: part) : "", local.location, NSMaxRange(local), rects: rects)
            }
        }
        reported = reported.filter { seen.contains($0.key) }
    }

    private func rectangles(_ node: NodeView, range: NSRange?) -> [CGRect] {
        guard let range, let presenter, let paragraph = node.paragraphLayout() else { return [] }
        let clip = presenter.viewport.contentView
        let selected = paragraph.selectedLines(range, align: node.paragraphSpec().align, in: node.paragraphBox(), dirty: .infinite)
        let local: [CGRect]
        if let fragments = node.columnRecord?.fragments, !fragments.isEmpty {
            local = fragments.flatMap { fragment in
                selected.filter { fragment.lines.contains($0.line) }.compactMap { line in
                    let rect = line.rect.offsetBy(dx: fragment.dx, dy: fragment.dy).intersection(fragment.rect)
                    return rect.isEmpty ? nil : rect
                }
            }
        } else { local = selected.map(\.rect) }
        return local.map { node.drawnRect($0, in: clip).offsetBy(dx: -clip.bounds.minX, dy: -clip.bounds.minY) }
    }

    func set(_ node: NodeView, _ selection: FieldSelection) {
        guard selectable(node) else { return }
        leaving = nil; gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = node; focus = node
        requested = (node.id, node.paragraphText, NSRange(location: selection.start, length: selection.end - selection.start))
        let shaped = { (offset: Int) in node.readerParagraph == nil ? node.shapedOffset(offset) : offset }
        let lo = shaped(selection.start), hi = shaped(selection.end)
        anchorIndex = selection.direction == "backward" ? hi : lo
        focusIndex = selection.direction == "backward" ? lo : hi
        unit = .character; anchorUnit = NSRange(location: lo, length: hi - lo); dragged = hi > lo
        if let (owner, start) = position(node, offset: anchorIndex), let (_, end) = position(node, offset: focusIndex) {
            var a = start, b = end
            a.offset = selection.direction == "backward" ? selection.end : selection.start
            b.offset = selection.direction == "backward" ? selection.start : selection.end
            list = owner; logicalAnchor = a; logicalFocus = b
        }
        invalidate()
    }

    package func clear() {
        requested = nil
        leaving = nil; gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        anchor = nil; focus = nil
        anchorIndex = 0; focusIndex = 0
        dragged = false
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        invalidate()
    }

    /// Where the focus goes decides whether the selection survives it. As on
    /// the web, a press on a `button` (or its label) leaves the selection and
    /// its highlight as they are, so its action reads it (#132); a link is
    /// text and clears it. AppKit moves the focus before the button hears
    /// `mouseDown`, so the node that takes it decides. When no node takes it,
    /// the next turn decides: the window or the page keeping it (the pressed
    /// button hid, a menu closed) leaves the selection, and a click on the
    /// ground clears it itself (`PageScrollView`); any other responder (a
    /// field's editor, as focus entering an editable takes the web's
    /// selection, a region, a native view) clears it.
    private var leaving: UInt64?
    func focusLeft() {
        guard isActive else { clear(); return }
        let current = gesture
        leaving = current
        DispatchQueue.main.async { [weak self] in
            guard let self, self.leaving == current else { return }
            self.leaving = nil
            let responder = self.presenter?.root.window?.firstResponder
            if self.gesture == current, !(responder is NSWindow || responder === self.presenter?.viewport) { self.clear() }
        }
    }
    /// A mouse-down on text a drag selects: the paragraph takes the focus
    /// and the selection begins there or, with Shift, grows to there. The
    /// focus moving between two paragraphs clears a selection; not one this
    /// click is about to extend.
    func press(_ node: NodeView, event: NSEvent) {
        holding = event.modifierFlags.contains(.shift) && anchor != nil
        node.window?.makeFirstResponder(node)
        holding = false
        begin(node, event: event)
    }
    private var holding = false
    func focusEntered(_ node: NodeView) {
        guard leaving != nil else { return }
        leaving = nil
        if holding { return }
        var at: NSView? = node
        while let view = at {
            if let node = view as? NodeView, node.isButton, node.props["href"] == nil { return }
            at = view.superview
        }
        clear()
    }

    /// Text takes part when the reader can select it (`textSelectable`),
    /// see it and reach it. As in Chrome, text that computes `visibility:
    /// hidden` (the stacked tab not shown, T3 F12-2) or is `inert` is
    /// neither highlighted nor copied, by ⌘A or a drag; a `display: none`
    /// subtree is `isHidden`, which `texts(in:)` leaves out.
    private func selectable(_ node: NodeView) -> Bool { !node.cssVisibilityHidden && node.textSelectable && !node.inert }
    func begin(_ node: NodeView, event: NSEvent) {
        requested = nil
        guard selectable(node) else { clear(); return }
        let clicks = event.clickCount
        // A Shift-click extends what is selected, as in an `NSTextView`
        // (LLP 1115.000 D7.9), where there is a selection it can reach.
        let extends = event.modifierFlags.contains(.shift) && anchor != nil && place(node, 0) != nil
        gesture += 1; motion = 0; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        let point = node.local(event.locationInWindow)
        let land = { [self] (index: Int) in
            if extends { shiftClick(node, clicks: clicks, point: point, index: index) } else { begin(node, clicks: clicks, point: point, index: index) }
        }
        if let reader = node.readerParagraph, reader.offset(at: point, node: node) == nil {
            pendingBegin = true
            let current = gesture
            reader.resolveOffset(at: point, node: node) { [weak self, weak node] index, _ in
                guard let self, node != nil, self.gesture == current else { return }
                self.pendingBegin = false
                guard let index else { self.clear(); return }
                land(index)
                let drag = self.deferredDrag, end = self.deferredEnd
                self.deferredDrag = nil; self.deferredEnd = nil
                if let drag { self.drag(drag) }
                if let end { self.end(end.0, event: end.1) }
            }
        } else { land(index(node, at: point)) }
    }

    /// A secondary click that no authored `contextmenu` takes (X76): on
    /// text outside the selection it selects the word under the pointer,
    /// the one a double click there selects (the text system's, MAC-16),
    /// and takes the focus as that click does, as Chrome and an
    /// `NSTextView` do on a Mac before their menu opens. Inside the
    /// selection, its ends included (Chrome's `FrameSelection::Contains`),
    /// it keeps it; text the reader cannot select keeps whatever is
    /// selected. Off the glyphs (past a line's end, below the last line) it
    /// takes the nearest word, as an `NSTextView` does; Chrome selects
    /// nothing there. Returns false when the point is not known yet: a
    /// reader paragraph whose caret cache misses it asks its raster worker,
    /// decides on the answer and then runs `answered`, for the menu the
    /// click could not open.
    func secondaryClick(_ node: NodeView, event: NSEvent, answered: @escaping () -> Void) -> Bool {
        guard selectable(node) else { return true }
        let point = node.local(event.locationInWindow)
        guard let reader = node.readerParagraph, reader.offset(at: point, node: node) == nil else {
            secondaryClick(node, at: index(node, at: point), point: point); return true
        }
        gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        let current = gesture
        reader.resolveOffset(at: point, node: node) { [weak self, weak node] index, _ in
            guard let self, let node, self.gesture == current, let index else { return }
            self.secondaryClick(node, at: index, point: point)
            answered()
        }
        return false
    }

    private func secondaryClick(_ node: NodeView, at index: Int, point: NSPoint) {
        if let selected = range(node), selected.length > 0, index >= selected.location, index <= NSMaxRange(selected) { return }
        node.window?.makeFirstResponder(node)
        gesture += 1; motion = 0; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        begin(node, clicks: 2, point: point, index: index)
    }

    /// `point` is the click's, in the node's space; `index` the caret
    /// nearest it. A double click and a secondary click take their word from
    /// both, by the one rule below.
    private func begin(_ node: NodeView, clicks: Int, point: NSPoint, index: Int) {
        requested = nil
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = node; focus = node
        anchorIndex = index
        focusIndex = anchorIndex
        dragged = false
        unit = clicks >= 3 ? .paragraph : clicks == 2 ? .word : .character
        defer { anchorUnit = NSRange(location: min(anchorIndex, focusIndex), length: abs(focusIndex - anchorIndex)) }
        if clicks >= 3 {
            anchorIndex = 0; focusIndex = length(node); dragged = true
        } else if clicks == 2 {
            let text = node.paragraphSpec().runs.map(\.text).joined() as NSString
            if text.length > 0 {
                // The word a double click takes in an `NSTextView`, by the
                // text system's own boundaries: "hello," without its comma,
                // a Chinese or Japanese word and not its whole run.
                let at = character(node, at: point, caret: index, in: text)
                let word = NSAttributedString(string: text as String).doubleClick(at: at)
                anchorIndex = word.location; focusIndex = NSMaxRange(word); dragged = true
            }
        }
        if let (owner, start) = position(node, offset: anchorIndex), let (_, end) = position(node, offset: focusIndex) {
            list = owner; logicalAnchor = start; logicalFocus = end
        }
        invalidate()
    }

    func drag(_ event: NSEvent) {
        if pendingBegin { deferredDrag = event; return }
        guard anchor != nil || list != nil else { return }
        dragged = true
        let point = event.locationInWindow
        let nodes = paragraphs.filter { list == nil || $0.isDescendant(of: list!) }
        guard let node = nodes.min(by: { distance($0, point) < distance($1, point) }) else { return }
        motion += 1
        let local = node.local(point)
        if let reader = node.readerParagraph, reader.offset(at: local, node: node) == nil {
            let current = gesture, sequence = motion
            reader.resolveOffset(at: local, node: node) { [weak self, weak node] index, _ in
                guard let self, let node, self.gesture == current, self.motion == sequence, let index else { return }
                self.extend(node, event: event, index: index)
            }
        } else { extend(node, event: event, index: index(node, at: local), point: local) }
    }

    /// The focus moves to `index` of `node`. After a double or a triple
    /// click the selection grows by that click's unit: the focus takes the
    /// whole word or paragraph it is in, and the anchor's own stays whole
    /// whichever way the focus goes, as an `NSTextView`'s does (LLP 1115.000
    /// D7.9). A giant paragraph's text is its worker's, and grows by character.
    private func extend(_ node: NodeView, event: NSEvent?, index: Int, point: NSPoint? = nil) {
        requested = nil
        var index = index
        if unit != .character, node.readerParagraph == nil, let anchor, let at = place(node, index),
           let low = place(anchor, anchorUnit.location) {
            let span = unitRange(node, index: index, point: point)
            let forward = at >= low
            anchorIndex = forward ? anchorUnit.location : NSMaxRange(anchorUnit)
            index = forward ? NSMaxRange(span) : span.location
            if node === anchor, forward { index = max(index, NSMaxRange(anchorUnit)) }
            if let (_, value) = position(anchor, offset: anchorIndex), list != nil { logicalAnchor = value }
        }
        focus = node
        focusIndex = index
        if let (_, value) = position(node, offset: focusIndex), list != nil { logicalFocus = value }
        if let event { node.autoscroll(with: event) }
        invalidate()
    }

    /// What a drag, a Shift-click or a key grows the selection by: a
    /// character, or the unit of the click that began it (a double click's
    /// word, a triple click's paragraph).
    private enum Unit { case character, word, paragraph }
    private var unit = Unit.character
    /// The anchor's own unit, in its paragraph's shaped offsets: what a drag
    /// after a double click keeps whole. A point after a Shift-click or a key.
    private var anchorUnit = NSRange(location: 0, length: 0)

    /// Where an offset of a paragraph stands in the selection's order: by row
    /// and paragraph in a list, whose rows come and go, and by the page's
    /// paragraphs elsewhere. Nil for text this selection cannot reach.
    private func place(_ node: NodeView, _ offset: Int) -> (Int, Int, Int)? {
        if let list {
            guard let (owner, p) = position(node, offset: offset), owner === list else { return nil }
            return p.order
        }
        _ = paragraphs
        return indices[node.id].map { (0, $0, offset) }
    }

    /// The word or the paragraph of `node` that a point, or else a caret, is
    /// in, by the text system's boundaries (`doubleClick(at:)`).
    private func unitRange(_ node: NodeView, index: Int, point: NSPoint?) -> NSRange {
        if unit == .paragraph { return NSRange(location: 0, length: length(node)) }
        let text = node.paragraphSpec().runs.map(\.text).joined() as NSString
        guard text.length > 0 else { return NSRange(location: 0, length: 0) }
        let at = point.map { character(node, at: $0, caret: index, in: text) } ?? min(max(0, index), text.length - 1)
        return NSAttributedString(string: text as String).doubleClick(at: at)
    }

    /// A Shift-click (LLP 1115.000 D7.9). As an `NSTextView` on macOS 27
    /// does it: the end of the selection nearer the click moves to it and
    /// the other stays, whichever way the selection was made (a click inside
    /// it shrinks it from the nearer end); after a double click it still
    /// grows by word, and a Shift double click makes it do so.
    private func shiftClick(_ node: NodeView, clicks: Int, point: NSPoint, index: Int) {
        let ends = list != nil ? (logicalAnchor?.order, logicalFocus?.order)
            : (anchor.flatMap { place($0, anchorIndex) }, focus.flatMap { place($0, focusIndex) })
        guard let at = place(node, index), let a = ends.0, let f = ends.1 else { begin(node, clicks: clicks, point: point, index: index); return }
        // Before the selection its start moves, after it its end; inside,
        // whichever is nearer. The end that stays is the anchor from here.
        func gap(_ x: (Int, Int, Int), _ y: (Int, Int, Int)) -> (Int, Int, Int) { (abs(x.0 - y.0), abs(x.1 - y.1), abs(x.2 - y.2)) }
        let (low, high) = a <= f ? (a, f) : (f, a)
        let startMoves = at <= low || (at < high && gap(at, low) <= gap(at, high))
        if (a == low) == startMoves {
            swap(&anchorIndex, &focusIndex); swap(&logicalAnchor, &logicalFocus)
            let other = anchor; anchor = focus; focus = other
        }
        if clicks >= 2 { unit = clicks >= 3 ? .paragraph : .word }
        anchorUnit = NSRange(location: anchorIndex, length: 0)
        allListText = false; dragged = true
        extend(node, event: nil, index: index, point: point)
    }

    /// Where a key command moves the selection's focus.
    private enum Move { case character(Int), word(Int), lineEdge(Int), line(Int), paragraphEdge(Int), documentEdge(Int) }
    private static let moves: [String: Move] = [
        "moveLeftAndModifySelection:": .character(-1), "moveBackwardAndModifySelection:": .character(-1),
        "moveRightAndModifySelection:": .character(1), "moveForwardAndModifySelection:": .character(1),
        "moveWordLeftAndModifySelection:": .word(-1), "moveWordBackwardAndModifySelection:": .word(-1),
        "moveWordRightAndModifySelection:": .word(1), "moveWordForwardAndModifySelection:": .word(1),
        "moveToLeftEndOfLineAndModifySelection:": .lineEdge(-1), "moveToBeginningOfLineAndModifySelection:": .lineEdge(-1),
        "moveToRightEndOfLineAndModifySelection:": .lineEdge(1), "moveToEndOfLineAndModifySelection:": .lineEdge(1),
        "moveUpAndModifySelection:": .line(-1), "moveDownAndModifySelection:": .line(1),
        "moveToBeginningOfParagraphAndModifySelection:": .paragraphEdge(-1), "moveParagraphBackwardAndModifySelection:": .paragraphEdge(-1),
        "moveToEndOfParagraphAndModifySelection:": .paragraphEdge(1), "moveParagraphForwardAndModifySelection:": .paragraphEdge(1),
        "moveToBeginningOfDocumentAndModifySelection:": .documentEdge(-1), "moveToEndOfDocumentAndModifySelection:": .documentEdge(1),
    ]

    /// A key command that moves an end of a selection (Shift with an arrow,
    /// by the system's key bindings: `NodeView.keyCommand`): the focus end
    /// moves and the anchor stays, as in an `NSTextView` (LLP 1115.000
    /// D7.9), from the click that placed it when nothing is selected yet,
    /// into the next paragraph past this one's end, and into view. True
    /// when the command is one of these, whether or not anything could move.
    func modify(_ command: Selector) -> Bool {
        guard let move = Self.moves[NSStringFromSelector(command)] else { return false }
        guard let from = focus, anchor != nil, from.window != nil, from.readerParagraph == nil, place(from, focusIndex) != nil else { return true }
        let (node, index) = target(move, from: from, focusIndex)
        gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        unit = .character; allListText = false; dragged = true
        extend(node, event: nil, index: index)
        if let paragraph = node.paragraphLayout() {
            let count = length(node), all = CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7)
            let caret = NSRange(location: min(index, max(0, count - 1)), length: min(1, count))
            if let rect = paragraph.selectionRects(caret, align: node.paragraphSpec().align, in: node.paragraphBox(), dirty: all).first {
                node.scrollToVisible(rect.insetBy(dx: -8, dy: -8))
            }
        }
        return true
    }

    private func target(_ move: Move, from node: NodeView, _ index: Int) -> (NodeView, Int) {
        let text = node.paragraphSpec().runs.map(\.text).joined() as NSString, count = text.length
        let index = min(max(0, index), count)
        let reach = paragraphs.filter { $0.readerParagraph == nil && (list == nil || $0.isDescendant(of: list!)) }
        let here = reach.firstIndex { $0 === node }
        func beside(_ step: Int) -> NodeView? { here.flatMap { reach.indices.contains($0 + step) ? reach[$0 + step] : nil } }
        /// Past this paragraph's end: the near end of the one beside it.
        func across(_ step: Int) -> (NodeView, Int) { beside(step).map { ($0, step < 0 ? length($0) : 0) } ?? (node, step < 0 ? 0 : count) }
        switch move {
        case .character(let step):
            if step < 0 { return index > 0 ? (node, text.rangeOfComposedCharacterSequence(at: index - 1).location) : across(step) }
            return index < count ? (node, NSMaxRange(text.rangeOfComposedCharacterSequence(at: index))) : across(step)
        case .word(let step):
            var found: Int?
            text.enumerateSubstrings(in: NSRange(location: 0, length: count), options: step < 0 ? [.byWords, .reverse] : .byWords) { _, word, _, stop in
                if step > 0, NSMaxRange(word) > index { found = NSMaxRange(word); stop.pointee = true }
                if step < 0, word.location < index { found = word.location; stop.pointee = true }
            }
            return found.map { (node, $0) } ?? across(step)
        case .paragraphEdge(let step): return (node, step < 0 ? 0 : count)
        case .documentEdge(let step): return step < 0 ? (reach.first ?? node, 0) : (reach.last.map { ($0, length($0)) } ?? (node, count))
        case .lineEdge(let step), .line(let step):
            guard let paragraph = node.paragraphLayout(), !paragraph.lines.isEmpty else { return (node, step < 0 ? 0 : count) }
            // The line the focus is on: the last that starts at or before it.
            var line = 0
            for i in paragraph.lines.indices where CTLineGetStringRange(paragraph.lines[i]).location <= index { line = i }
            let range = CTLineGetStringRange(paragraph.lines[line]), box = node.paragraphBox(), align = node.paragraphSpec().align
            if case .lineEdge = move {
                var end = range.location + range.length
                if end > range.location, end <= count, text.character(at: end - 1) == 0x0A { end -= 1 }
                return (node, step < 0 ? range.location : end)
            }
            let x = paragraph.origin(line, align: align, width: box.width) + CTLineGetOffsetForStringIndex(paragraph.lines[line], index, nil)
            if paragraph.lines.indices.contains(line + step) {
                let found = paragraph.stringIndex(in: line + step, at: x - paragraph.origin(line + step, align: align, width: box.width))
                return (node, found == kCFNotFound ? index : min(max(0, found), count))
            }
            // The first or last line: the same place across in the paragraph
            // beside it, or this one's own end where there is none.
            guard let other = beside(step), let next = other.paragraphLayout(), !next.lines.isEmpty else { return (node, step < 0 ? 0 : count) }
            let to = step < 0 ? next.lines.count - 1 : 0, room = other.paragraphBox()
            let across = node.convert(NSPoint(x: box.minX + x, y: 0), to: other).x - room.minX
            let found = next.stringIndex(in: to, at: across - next.origin(to, align: other.paragraphSpec().align, width: room.width))
            return (other, found == kCFNotFound ? (step < 0 ? length(other) : 0) : min(max(0, found), length(other)))
        }
    }

    func end(_ node: NodeView, event: NSEvent) {
        if pendingBegin { deferredEnd = (node, event); return }
        if !dragged, anchor === node, let reader = node.readerParagraph {
            let current = gesture
            reader.resolveOffset(at: node.local(event.locationInWindow), node: node) { [weak self] _, url in
                // A route of this app navigates in it, as a short paragraph's
                // link does (b6 review B5); the rest is the containing app's.
                guard let self, self.gesture == current, let url, let session = self.presenter?.session else { return }
                session.follow(url)
            }
            return
        }
        guard !dragged, anchor === node, let url = link(node, at: node.local(event.locationInWindow)) else { return }
        // A route of this app navigates in it; the containing app owns the
        // rest (local Markdown, anchors, browser URLs); no arbitrary URL
        // scheme is launched by the presenter.
        presenter?.session?.follow(url)
    }

    func selectAll() {
        requested = nil
        gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        let nodes = paragraphs
        if let first = nodes.first(where: { position($0, offset: 0) != nil }),
           let (owner, start) = position(first, offset: 0),
           let last = nodes.last(where: { $0.isDescendant(of: owner) }),
           let (_, end) = position(last, offset: length(last)) {
            list = owner; logicalAnchor = start; logicalFocus = end; allListText = true
            anchor = first; focus = last; dragged = true
            invalidate()
            return
        }
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = nodes.first; focus = nodes.last
        anchorIndex = 0; focusIndex = nodes.last.map(length) ?? 0
        dragged = true
        invalidate()
    }

    func copy() {
        let text = selectedText()
        guard !text.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    func selectedText() -> String {
        if let list {
            return presenter?.onListText?(list.id, allListText ? nil : logicalAnchor?.tuple, allListText ? nil : logicalFocus?.tuple) ?? ""
        }
        let parts = paragraphs.compactMap { node -> String? in
            guard let range = range(node), range.length > 0 else { return nil }
            return (node.paragraphSpec().runs.map(\.text).joined() as NSString).substring(with: range)
        }
        return parts.joined(separator: "\n\n")
    }

    private func length(_ node: NodeView) -> Int {
        if let source = node.readerParagraph?.accepted?.metadata.source { return source.utf16Count }
        return node.paragraphSpec().runs.reduce(0) { $0 + ($1.text as NSString).length }
    }

    func range(_ node: NodeView) -> NSRange? {
        guard selectable(node) else { return nil }
        if let list {
            guard let (owner, p) = position(node, offset: 0), owner === list else { return nil }
            let count = length(node)
            if allListText { return NSRange(location: 0, length: count) }
            guard let a = logicalAnchor, let b = logicalFocus else { return nil }
            let (start, end) = a.order <= b.order ? (a, b) : (b, a)
            guard (p.row, p.paragraph) >= (start.row, start.paragraph), (p.row, p.paragraph) <= (end.row, end.paragraph) else { return nil }
            let shaped = { (source: Int) in node.readerParagraph == nil ? node.shapedOffset(source) : source }
            let lo = (p.row, p.paragraph) == (start.row, start.paragraph) ? min(shaped(start.offset), count) : 0
            let hi = (p.row, p.paragraph) == (end.row, end.paragraph) ? min(shaped(end.offset), count) : count
            return NSRange(location: lo, length: max(0, hi - lo))
        }
        guard anchor != nil && focus != nil else { return nil }
        _ = paragraphs
        guard let anchor, let focus, let a = indices[anchor.id], let b = indices[focus.id],
              let n = indices[node.id], n >= min(a, b), n <= max(a, b) else { return nil }
        let forward = a < b || (a == b && anchorIndex <= focusIndex)
        let start = forward ? a : b, end = forward ? b : a
        let lo = n == start ? (forward ? anchorIndex : focusIndex) : 0
        let hi = n == end ? (forward ? focusIndex : anchorIndex) : length(node)
        let count = length(node)
        return NSRange(location: min(lo, count), length: max(0, min(hi, count) - min(lo, count)))
    }

    /// The wrapper's key survives retirement. Paragraph ordinals count every
    /// text of the row, selectable or not, as the runner's read-only text
    /// projection does; it leaves the unselectable ones out of a copy (X81).
    private func position(_ node: NodeView, offset: Int) -> (NodeView, Position)? {
        var ancestor: NSView? = node
        var wrapper: NodeView?
        while let view = ancestor {
            if let n = view as? NodeView, n.props["listItemKey"] != nil { wrapper = n; break }
            ancestor = view.superview
        }
        guard let wrapper, let key = wrapper.props["listItemKey"], let index = Int(wrapper.props["accessibilityPosInSet"] ?? "") else { return nil }
        ancestor = wrapper.superview
        while let view = ancestor {
            if let owner = view as? NodeView, owner.kind == "list" {
                if positions[node.id] == nil {
                    var ordinal = 0
                    texts(in: wrapper) {
                        positions[$0.id] = Position(key: key, row: index - 1, paragraph: ordinal, offset: 0)
                        ordinal += 1
                    }
                }
                guard var result = positions[node.id] else { return nil }
                // The runner's projection addresses source text; the shaped
                // text may have collapsed white space (LLP 1053 G5).
                result.offset = node.readerParagraph == nil ? node.sourceOffset(offset) : offset
                return (owner, result)
            }
            ancestor = view.superview
        }
        return nil
    }

    private func distance(_ node: NodeView, _ point: NSPoint) -> CGFloat {
        let p = node.local(point)
        let dy = max(0, max(-p.y, p.y - node.bounds.height))
        let dx = max(0, max(-p.x, p.x - node.bounds.width))
        return dy * 10000 + dx
    }

    private func line(_ node: NodeView, at point: NSPoint) -> (Paragraph, Spec, Int)? {
        // A fragmented paragraph maps the point into its fragment (LLP 1093 D8).
        let point = node.caretPoint(point)
        guard let paragraph = node.paragraphLayout() else { return nil }
        let spec = node.paragraphSpec()
        let box = node.paragraphBox()
        guard let i = paragraph.lineIndex(at: CGPoint(x: point.x - box.minX, y: point.y - box.minY),
                                          align: spec.align, width: box.width) else { return nil }
        return (paragraph, spec, i)
    }

    private func index(_ node: NodeView, at point: NSPoint) -> Int {
        if let reader = node.readerParagraph { return reader.offset(at: point, node: node) ?? 0 }
        let content = node.paragraphBox(), mapped = node.caretPoint(point)
        if mapped.y < content.minY { return 0 }
        if mapped.y > content.maxY { return length(node) }
        guard let (p, spec, i) = line(node, at: point) else { return 0 }
        let x = content.minX + p.origin(i, align: spec.align, width: content.width)
        let offset = p.stringIndex(in: i, at: mapped.x - x)
        return offset == kCFNotFound ? length(node) : min(max(0, offset), length(node))
    }

    /// The character under `point`, which a double click takes its word
    /// from. `caret` (`index`) is the caret nearest the point: the next
    /// character's from a glyph's trailing half, and the next line's first
    /// past a line's end. A reader paragraph's lines are its worker's; there
    /// the caret stands, stepped back off white space, so that a click on a
    /// word's last letter still takes the word.
    private func character(_ node: NodeView, at point: NSPoint, caret: Int, in text: NSString) -> Int {
        let caret = min(max(0, caret), text.length - 1)
        guard node.readerParagraph == nil, let (p, spec, i) = line(node, at: point) else {
            let blank = CharacterSet.whitespacesAndNewlines.contains(UnicodeScalar(text.character(at: caret)) ?? " ")
            return blank && caret > 0 ? caret - 1 : caret
        }
        let row = p.lines[i], range = CTLineGetStringRange(row)
        guard range.length > 0 else { return caret }
        let content = node.paragraphBox()
        let x = node.caretPoint(point).x - content.minX - p.origin(i, align: spec.align, width: content.width)
        var at = caret
        if caret > range.location {
            // Between the carets either side of the character before: on it,
            // whichever way its run reads.
            let before = text.rangeOfComposedCharacterSequence(at: caret - 1).location
            let a = CTLineGetOffsetForStringIndex(row, before, nil), b = CTLineGetOffsetForStringIndex(row, caret, nil)
            if x >= min(a, b), x < max(a, b) { at = before }
        }
        return min(max(at, range.location), range.location + range.length - 1)
    }

    private func link(_ node: NodeView, at point: NSPoint) -> String? { node.inlineLink(at: point) }

    /// Over the paragraph's own paint: the selection's fill, which covers
    /// the ink and any inline background inside it, then the selected glyphs
    /// in the selection's colour, or in their own where they keep it (`ink`),
    /// clipped to the fill.
    func draw(_ node: NodeView, paragraph: Paragraph, spec: Spec, in ctx: CGContext, dirty: NSRect) {
        guard let selection = range(node), selection.length > 0 else { return }
        let box = node.paragraphBox(), focused = PageFacts.hasFocus(node.window)
        let lines = paragraph.selectedLines(selection, align: spec.align, in: box, dirty: dirty)
        guard !lines.isEmpty else { return }
        let ink = Self.ink(focused: focused).cgColor, keepsFixed = Self.keepsFixedInk(dark: node.drawsDark)
        ctx.saveGState()
        ctx.clip(to: lines.map(\.rect))
        ctx.setFillColor(Self.fill(focused: focused).cgColor)
        ctx.fill(lines.map(\.rect))
        for index in Set(lines.map(\.line)) {
            let line = spec.ellipsis ? paragraph.ellipsized(index, spec: spec, width: box.width) : paragraph.lines[index]
            let origin = CGPoint(x: box.minX + paragraph.origin(index, align: spec.align, width: box.width),
                                 y: box.minY + paragraph.baselines[index].rounded())
            TextLinePaint.draw(line, at: origin, in: ctx, ink: ink, keepsFixed: keepsFixed, transparentInk: node.backgroundClip == "text")
        }
        ctx.restoreGState()
    }
}

// Which text takes part (`NodeView.textSelectable`) is `TextSelectable.swift`'s.

/// A secondary click on selectable text with no authored `contextmenu` or
/// `contextPopover` here or above is the menu an `NSTextView` shows for
/// read-only text, AppKit's own (LLP 1115.000 D7.9, `TextMenuMac.swift`):
/// on a word outside the selection it first selects the word, as Chrome
/// and AppKit do on a Mac (X76); then Look Up and Translate the selection,
/// Search With, Copy (the `copy` event first, as ⌘C), Share, Speech, and
/// the Services that take text, which AppKit adds itself to a contextual
/// menu whose responder can send them (`validRequestor`). A reader
/// paragraph whose raster worker must place the click opens it when the
/// answer comes. An authored handler above, or on the run under the
/// pointer, hears the click instead and nothing is selected, as in Chrome,
/// which selects only for its own menu. Anywhere else there is no menu, as
/// before, and under the agent none: AppKit's tracking of it would hold
/// the agent's click.
extension NodeView: NSServicesMenuRequestor {
    package override func menu(for event: NSEvent) -> NSMenu? {
        guard isParagraph, let selection = presenter?.selection, !contextMenuAuthored,
              inlineContextTarget(at: local(event.locationInWindow)) == nil else { return super.menu(for: event) }
        if event.type == .rightMouseDown || (event.type == .leftMouseDown && event.modifierFlags.contains(.control)),
           !selection.secondaryClick(self, event: event, answered: {
               // The reader's worker placed the click: its menu opens on the
               // next turn, once the reader has taken the answer in.
               DispatchQueue.main.async { [weak self] in
                   guard let self, self.window != nil, let menu = self.textMenu(event) else { return }
                   NSMenu.popUpContextMenu(menu, with: event, for: self)
               }
           }) {
            return super.menu(for: event)
        }
        return textMenu(event) ?? super.menu(for: event)
    }
    /// AppKit's menu for the selection, with the Services that take its text.
    private func textMenu(_ event: NSEvent) -> NSMenu? {
        guard let menu = presenter?.selection.systemMenu(for: self, event: event) else { return nil }
        _ = Self.servicesRegistered
        return menu
    }
    private static let servicesRegistered: Void = {
        NSApp.registerServicesMenuSendTypes([.string], returnTypes: [])
    }()
    package override func validRequestor(forSendType sendType: NSPasteboard.PasteboardType?, returnType: NSPasteboard.PasteboardType?) -> Any? {
        if isParagraph, returnType == nil, sendType == .string, presenter?.selection.selectedText().isEmpty == false { return self }
        return super.validRequestor(forSendType: sendType, returnType: returnType)
    }
    package func writeSelection(to pboard: NSPasteboard, types: [NSPasteboard.PasteboardType]) -> Bool {
        guard types.contains(.string), let text = presenter?.selection.selectedText(), !text.isEmpty else { return false }
        pboard.clearContents()
        return pboard.setString(text, forType: .string)
    }
    /// Whether a node from here up runs an authored `contextmenu` or
    /// `contextPopover` for a secondary click, as `dispatchContextMenu` does
    /// on the event's way up the responder chain (a Control-click's too,
    /// `contextClick`).
    var contextMenuAuthored: Bool {
        guard let presenter else { return false }
        var next: NSView? = self
        while let view = next {
            next = view.superview
            guard let node = view as? NodeView, presenter.views[node.id] === node, !node.disabled, !node.inert else { continue }
            if node.handlers.contains("contextmenu") || node.props["contextPopover"]?.isEmpty == false { return true }
        }
        return false
    }
}

#endif
