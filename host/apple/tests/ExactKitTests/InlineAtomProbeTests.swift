// Stage 0 of LLP 1045.000 (inline atoms), the TextKit 2 gate: whether an
// `NSTextAttachment` that only the paragraph an `NSTextContentStorageDelegate`
// supplies carries lays out and draws, while the storage keeps the source.
// The paragraph has the source's length, as `MarkupHidden.swift`'s does: the
// atom's first code unit is U+FFFC carrying the attachment, the rest word
// joiners (U+2060). Each check runs for both ways an attachment draws: its
// view provider's view (§4's plan) and the picture it hands the layout
// fragment (`NSTextAttachmentLayout`). The views are plain TextKit 2 text
// views configured as the Markdown text area is, not ExactKit's: these tests
// record what TextKit does, which the stage 2 editor builds on, and stand in
// for no editor rule. No claim about a physical keyboard or VoiceOver itself.
// @ref LLP 1045.000 §4 "Stage 0 is a gate", §5 stage 0, §5.1, D4, D5, D8
import XCTest
#if canImport(UIKit)
import UIKit
private typealias ProbeView = UIView
private typealias ProbeFont = UIFont
private typealias ProbeImage = UIImage
#else
import AppKit
import ApplicationServices
private typealias ProbeView = NSView
private typealias ProbeFont = NSFont
private typealias ProbeImage = NSImage
#endif

private extension NSAttributedString.Key {
    /// An atom, as the styler would mark it: a `Mark`.
    static let probeAtom = NSAttributedString.Key("ExactProbeAtom")
}

/// One atom's mark, its source the face's key. Each is its own object, so
/// two adjacent atoms with equal source stay two runs.
private final class Mark: NSObject {
    let source: String
    init(_ source: String) { self.source = source }
}

private let font = ProbeFont.systemFont(ofSize: 16)
private let destination = "](t3-citation://v1?text=%E2%80%9CThe%20editor%20draws%20every%20source%20character%E2%80%9D&comment=Why)"
private let source = "Ask about [Assistant quote" + destination + " and more."
/// The atom's start and end.
private let a = ("Ask about " as NSString).length, b = a + ("[Assistant quote" + destination as NSString).length

/// The pill, in a colour no text or selection draws, so one pixel tells whether it drew.
private final class Pill: ProbeView {
    let label: String
    init(label: String) {
        self.label = label
        super.init(frame: .zero)
        #if canImport(UIKit)
        backgroundColor = UIColor(red: 1, green: 0, blue: 0, alpha: 1)
        layer.cornerRadius = 4
        #endif
    }
    required init?(coder: NSCoder) { fatalError() }
    #if os(macOS)
    override var isFlipped: Bool { true }
    override func draw(_ dirty: NSRect) {
        NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1).setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 4, yRadius: 4).fill()
    }
    #endif
}

private final class PillAttachment: NSTextAttachment {
    let label: String
    unowned let probe: Probe
    init(label: String, probe: Probe) {
        self.label = label
        self.probe = probe
        super.init(data: nil, ofType: nil)
        allowsTextAttachmentView = probe.drawing == .view
        probe.attachments.append(Weak(self))
    }
    required init?(coder: NSCoder) { fatalError() }
    override func viewProvider(for parentView: ProbeView?, location: NSTextLocation, textContainer: NSTextContainer?) -> NSTextAttachmentViewProvider? {
        guard probe.drawing == .view else { return nil }
        probe.providers += 1
        return PillProvider(textAttachment: self, parentView: parentView, textLayoutManager: textContainer?.textLayoutManager, location: location)
    }
    /// The pill as a picture the layout fragment draws, when it has no view.
    override func image(for bounds: CGRect, attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?) -> ProbeImage? {
        guard probe.drawing == .image else { return super.image(for: bounds, attributes: attributes, location: location, textContainer: textContainer) }
        probe.images += 1
        #if os(macOS)
        return NSImage(size: bounds.size, flipped: true) { rect in
            NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1).setFill()
            NSBezierPath(roundedRect: rect, xRadius: 4, yRadius: 4).fill()
            return true
        }
        #else
        return UIGraphicsImageRenderer(size: bounds.size).image { _ in
            UIColor(red: 1, green: 0, blue: 0, alpha: 1).setFill()
            UIBezierPath(roundedRect: CGRect(origin: .zero, size: bounds.size), cornerRadius: 4).fill()
        }
        #endif
    }
    /// The label's width and padding, no wider than the line (D4), and a
    /// point inside the font's ascender and descender, so the line keeps its
    /// height (the full ascender and descender make a 16 pt system font line
    /// 0.84 pt taller on macOS). TextKit 2 asks the attachment
    /// (`NSTextAttachmentLayout`), and its default answer is what makes the
    /// view provider: an override that skips `super` gets no view.
    override func attachmentBounds(for attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?,
                                   proposedLineFragment: CGRect, position: CGPoint) -> CGRect {
        _ = super.attachmentBounds(for: attributes, location: location, textContainer: textContainer, proposedLineFragment: proposedLineFragment, position: position)
        probe.proposed.append(proposedLineFragment)
        let face = attributes[.font] as? ProbeFont ?? font
        let wide = ceil((label as NSString).size(withAttributes: [.font: face]).width) + 12
        return CGRect(x: 0, y: face.descender + 1, width: max(1, min(wide, proposedLineFragment.width)), height: face.ascender - face.descender - 2)
    }
}

private final class PillProvider: NSTextAttachmentViewProvider {
    override func loadView() {
        let attachment = textAttachment as! PillAttachment
        let pill = Pill(label: attachment.label)
        attachment.probe.loaded.append(Weak(pill))
        view = pill
    }
}

private final class Weak {
    weak var object: AnyObject?
    init(_ object: AnyObject) { self.object = object }
}

/// The styler and the paragraph delegate in one: atoms are citation links,
/// marked with their source; faces map a source to a label. Each paragraph
/// TextKit asks for gets new attachments.
private final class Probe: NSObject, NSTextContentStorageDelegate {
    enum Drawing { case view, image }
    /// How the pill draws: its attachment's view provider (§4's plan), or a
    /// picture the attachment hands the layout fragment.
    var drawing: Drawing
    var labels: [String: String] = [:]
    var providers = 0, images = 0, paragraphs = 0
    var loaded: [Weak] = []
    var attachments: [Weak] = []
    var proposed: [CGRect] = []
    init(_ drawing: Drawing) { self.drawing = drawing }

    func textContentStorage(_ textContentStorage: NSTextContentStorage, textParagraphWith range: NSRange) -> NSTextParagraph? {
        guard let storage = textContentStorage.textStorage else { return nil }
        var runs: [(NSRange, String)] = []
        storage.enumerateAttribute(.probeAtom, in: range) { value, run, _ in
            if let mark = value as? Mark { runs.append((run, mark.source)) }
        }
        guard !runs.isEmpty else { return nil }
        paragraphs += 1
        let shown = NSMutableAttributedString(attributedString: storage.attributedSubstring(from: range))
        for (run, key) in runs.reversed() {
            let local = NSRange(location: run.location - range.location, length: run.length)
            var attributes = shown.attributes(at: local.location, effectiveRange: nil)
            attributes[.probeAtom] = nil
            let piece = NSMutableAttributedString(string: String(repeating: "\u{2060}", count: run.length), attributes: attributes)
            piece.replaceCharacters(in: NSRange(location: 0, length: 1), with: "\u{FFFC}")
            piece.addAttribute(.attachment, value: PillAttachment(label: labels[key] ?? Self.linkText(key), probe: self), range: NSRange(location: 0, length: 1))
            shown.replaceCharacters(in: local, with: piece)
        }
        return NSTextParagraph(attributedString: shown)
    }

    static func linkText(_ key: String) -> String {
        let text = key as NSString
        let close = text.range(of: "](")
        return close.location == NSNotFound ? key : text.substring(with: NSRange(location: 1, length: close.location - 1))
    }

    /// Marks every citation link, as a restyle would: recognized from the source each time.
    func restyle(_ storage: NSTextStorage) {
        let all = NSRange(location: 0, length: storage.length)
        let pattern = try! NSRegularExpression(pattern: #"!?\[[^\]\n]*\]\(t3-citation:[^)\s]*\)"#)
        storage.beginEditing()
        storage.removeAttribute(.probeAtom, range: all)
        for match in pattern.matches(in: storage.string, range: all) {
            storage.addAttribute(.probeAtom, value: Mark((storage.string as NSString).substring(with: match.range)), range: match.range)
        }
        storage.endEditing()
    }

    /// The pills in `view`'s hierarchy, in load order.
    func live(in view: ProbeView) -> [Pill] { loaded.compactMap { $0.object as? Pill }.filter { $0.isDescendant(of: view) } }
    /// The pills TextKit has loaded that are still allocated.
    var allocated: Int { loaded.filter { $0.object != nil }.count }
}

private extension NSTextLayoutManager {
    func location(_ p: Int) -> NSTextLocation {
        let content = textContentManager!
        return content.location(content.documentRange.location, offsetBy: p)!
    }
    func range(_ start: Int, _ end: Int) -> NSTextRange { NSTextRange(location: location(start), end: location(end))! }
    /// Each segment's rectangle over `start..<end`, in container coordinates.
    func segments(_ start: Int, _ end: Int, _ type: SegmentType = .standard) -> [CGRect] {
        var out: [CGRect] = []
        ensureLayout(for: documentRange)
        enumerateTextSegments(in: range(start, end), type: type, options: []) { _, rect, _, _ in out.append(rect); return true }
        return out
    }
    /// The insertion point at `p`: its x, and the top of its line.
    func caret(_ p: Int) -> CGPoint {
        let rect = segments(p, p, .selection).first ?? .null
        return CGPoint(x: rect.minX, y: rect.minY)
    }
    var lines: Int {
        var n = 0
        ensureLayout(for: documentRange)
        enumerateTextLayoutFragments(from: documentRange.location, options: [.ensuresLayout]) { n += $0.textLineFragments.count; return true }
        return n
    }
    /// Each laid-out attachment's frame, in container coordinates, found in
    /// the paragraphs TextKit laid out (the storage has none).
    var attachments: [CGRect] {
        var out: [CGRect] = []
        let content = textContentManager!
        ensureLayout(for: documentRange)
        enumerateTextLayoutFragments(from: documentRange.location, options: [.ensuresLayout]) { fragment in
            guard let paragraph = fragment.textElement as? NSTextParagraph, let start = paragraph.elementRange?.location else { return true }
            let text = paragraph.attributedString
            text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, run, _ in
                guard value != nil, let at = content.location(start, offsetBy: run.location) else { return }
                let origin = fragment.layoutFragmentFrame.origin
                out.append(fragment.frameForTextAttachment(at: at).offsetBy(dx: origin.x, dy: origin.y))
            }
            return true
        }
        return out
    }
}

private let drawings: [Probe.Drawing] = [.view, .image]

private func long(_ count: Int) -> String {
    "Cite [Assistant quote](t3-citation://v1?text=" + String(repeating: "%E2%80%9C", count: count / 9) + ") here."
}

#if os(macOS)
/// The text view, with atom elements for accessibility as D8 has them: the
/// value reads each atom's label, and each atom is a button child.
private final class ProbeTextView: NSTextView {
    weak var probe: Probe?
    var atomElements: [NSAccessibilityElement] = []
    var pressed: [String] = []

    override func accessibilityChildren() -> [Any]? { (super.accessibilityChildren() ?? []) + atomElements }
    override func accessibilityValue() -> String? {
        guard let storage = textStorage else { return super.accessibilityValue() }
        return shown(storage, NSRange(location: 0, length: storage.length))
    }
    override func accessibilityString(for range: NSRange) -> String? {
        guard let storage = textStorage, NSMaxRange(range) <= storage.length else { return super.accessibilityString(for: range) }
        return shown(storage, range)
    }
    private func shown(_ storage: NSTextStorage, _ range: NSRange) -> String {
        let out = NSMutableString(string: (storage.string as NSString).substring(with: range))
        storage.enumerateAttribute(.probeAtom, in: range, options: .reverse) { value, run, _ in
            guard let key = (value as? Mark)?.source else { return }
            out.replaceCharacters(in: NSRange(location: run.location - range.location, length: run.length), with: probe?.labels[key] ?? Probe.linkText(key))
        }
        return out as String
    }
}

private final class AtomElement: NSAccessibilityElement {
    var press: () -> Void = {}
    override func accessibilityPerformPress() -> Bool { press(); return true }
}

final class InlineAtomProbeMacTests: XCTestCase {
    nonisolated(unsafe) private static var launched = false
    private var windows: [NSWindow] = []

    override func tearDown() {
        for window in windows { window.orderOut(nil); window.close() }
        windows = []
        super.tearDown()
    }

    private func editor(_ drawing: Probe.Drawing, _ text: String = source, width: CGFloat = 500, rich: Bool = false) -> (ProbeTextView, Probe, NSTextLayoutManager) {
        _ = NSApplication.shared
        let probe = Probe(drawing)
        let view = ProbeTextView(usingTextLayoutManager: true)
        view.probe = probe
        view.frame = NSRect(x: 0, y: 0, width: width, height: 200)
        view.textContainer?.widthTracksTextView = true
        view.isRichText = rich
        view.importsGraphics = rich
        view.allowsUndo = true
        view.font = font
        view.textContentStorage!.delegate = probe
        view.string = text
        probe.restyle(view.textStorage!)
        let window = NSWindow(contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        window.makeFirstResponder(view)
        windows.append(window)
        settle(view)
        return (view, probe, view.textLayoutManager!)
    }

    private func settle(_ view: NSTextView) {
        let manager = view.textLayoutManager!
        manager.ensureLayout(for: manager.documentRange)
        view.layoutSubtreeIfNeeded()
        manager.textViewportLayoutController.layoutViewport()
    }

    /// Whether the pixel at `p` (container coordinates) is the pill's red.
    private func red(_ view: NSTextView, at p: CGPoint) -> Bool {
        settle(view)
        view.display()
        let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds)!
        view.cacheDisplay(in: view.bounds, to: rep)
        let scale = CGFloat(rep.pixelsWide) / view.bounds.width, o = view.textContainerOrigin
        guard let c = rep.colorAt(x: Int((p.x + o.x) * scale), y: Int((p.y + o.y) * scale))?.usingColorSpace(.sRGB) else { return false }
        return c.redComponent > 0.85 && c.greenComponent < 0.4 && c.blueComponent < 0.4
    }

    private func center(_ r: CGRect) -> CGPoint { CGPoint(x: r.midX, y: r.midY) }

    func testTheAttachmentLaysOutAndDrawsAndTheSourceStays() throws {
        for drawing in drawings {
            for rich in [false, true] {
                let (view, probe, manager) = editor(drawing, rich: rich)
                let what = "\(drawing), rich \(rich)"
                XCTAssertEqual(view.string, source, "the storage keeps the source")
                XCTAssertGreaterThan(probe.paragraphs, 0, "the delegate supplied the paragraph")
                XCTAssertEqual(manager.lines, 1, what)
                XCTAssertEqual(manager.attachments.count, 1, what)
                let frame = try XCTUnwrap(manager.attachments.first, "TextKit laid the attachment out")
                XCTAssertTrue(red(view, at: center(frame)), "the pill is drawn: \(what)")
                switch drawing {
                case .view:
                    XCTAssertEqual(probe.live(in: view).count, 1, "its view is in the text view")
                    let pill = try XCTUnwrap(probe.live(in: view).first)
                    XCTAssertEqual(view.convert(pill.bounds, from: pill).width, frame.width, accuracy: 0.5)
                case .image:
                    XCTAssertEqual(probe.providers, 0)
                    XCTAssertGreaterThan(probe.images, 0, "the fragment asked for its picture")
                }
                // Carets: the start at the pill's left edge; every offset after it at its right edge.
                XCTAssertEqual(manager.caret(a).x, frame.minX, accuracy: 0.5)
                for p in a + 1...b { XCTAssertEqual(manager.caret(p).x, frame.maxX, accuracy: 0.5, "offset \(p), \(what)") }
                XCTAssertGreaterThan(manager.caret(b + 1).x, frame.maxX)
                // The pill keeps the line's height.
                let plain = editor(drawing, "Ask about Assistant quote and more.").2
                XCTAssertEqual(manager.segments(0, 3).first!.height, plain.segments(0, 3).first!.height, accuracy: 0.01, what)
                print("probe(mac) \(what): pill \(frame)")
            }
        }
    }

    func testTheSelectionAndRangeFrameCoverThePill() throws {
        for drawing in drawings {
            let (view, _, manager) = editor(drawing)
            let frame = try XCTUnwrap(manager.attachments.first)
            // rangeFrame's segments (InlineGeometry.swift) over the atom: the pill's box.
            let box = manager.segments(a, b).reduce(CGRect.null) { $0.union($1) }
            XCTAssertEqual(box.minX, frame.minX, accuracy: 0.5)
            XCTAssertEqual(box.width, frame.width, accuracy: 0.5)
            let selected = manager.segments(a - 3, b + 3, .selection).reduce(CGRect.null) { $0.union($1) }
            XCTAssertLessThanOrEqual(selected.minX, frame.minX)
            XCTAssertGreaterThanOrEqual(selected.maxX, frame.maxX)
            view.setSelectedRange(NSRange(location: a, length: b - a))
            XCTAssertEqual(view.selectedRange(), NSRange(location: a, length: b - a))
            XCTAssertTrue(red(view, at: center(frame)), "the pill draws over the highlight, so its selected look is its own: \(drawing)")
        }
    }

    func testACitationOfThousandsOfCodeUnits() throws {
        let text = long(6000), start = ("Cite " as NSString).length, end = text.utf16.count - (" here." as NSString).length
        XCTAssertGreaterThan(end - start, 5000)
        for drawing in drawings {
            _ = editor(drawing) // TextKit's first attachment of this kind, out of the timing
            let began = Date()
            let (view, _, manager) = editor(drawing, text)
            let first = Date().timeIntervalSince(began)
            XCTAssertEqual(manager.lines, 1)
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertEqual(manager.caret(start + 1).x, frame.maxX, accuracy: 0.5)
            XCTAssertEqual(manager.caret(end).x, frame.maxX, accuracy: 0.5)
            XCTAssertTrue(red(view, at: center(frame)))
            let edited = Date()
            view.insertText("!", replacementRange: NSRange(location: end, length: 0))
            settle(view)
            print("probe(mac) \(drawing), \(end - start) code units: first layout \(Int(first * 1000)) ms, an edit beside it \(Int(Date().timeIntervalSince(edited) * 1000)) ms")
        }
    }

    func testThePillWrapsWholeAndALongLabelFitsTheLine() throws {
        let before = "one two three four five six "
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing, before + "[Assistant quote" + destination, width: 220)
            XCTAssertEqual(manager.lines, 2)
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertGreaterThan(frame.minY, manager.caret(0).y, "on the second line")
            XCTAssertEqual(frame.minX, manager.caret(0).x, accuracy: 0.5, "at its start")
            XCTAssertGreaterThan(manager.caret((before as NSString).length).y, manager.caret(0).y)
            XCTAssertTrue(red(view, at: center(frame)))
            // A label wider than the line: the attachment is offered the line, and the pill fits it.
            probe.labels["[Assistant quote" + destination] = String(repeating: "a very long file name ", count: 6)
            probe.restyle(view.textStorage!)
            settle(view)
            let wide = try XCTUnwrap(manager.attachments.first)
            let line = try XCTUnwrap(probe.proposed.last)
            XCTAssertEqual(wide.width, line.width, accuracy: 0.5)
            XCTAssertEqual(manager.lines, 2)
            XCTAssertTrue(red(view, at: center(wide)))
            print("probe(mac) \(drawing) wrapped pill \(frame), long label \(wide) in line \(line)")
        }
    }

    /// Two atoms side by side. Word joiners forbid a line break on either
    /// side of themselves (UAX #14 LB11), so the pair wraps as one, where CSS
    /// has a soft wrap opportunity between two inline-blocks (CSS Text 3 §5.1). A native
    /// arrow crosses each atom in one step: the joiners are no caret stops.
    func testAdjacentAtomsWrapTogetherAndArrowsCrossEach() throws {
        let atom = "[Assistant quote" + destination, n = (atom as NSString).length, start = ("one two three " as NSString).length
        for drawing in drawings {
            let (view, _, manager) = editor(drawing, "one two three " + atom + atom, width: 300)
            let frames = manager.attachments
            XCTAssertEqual(frames.count, 2)
            XCTAssertEqual(manager.lines, 2)
            XCTAssertEqual(frames.map(\.minY), [frames[1].minY, frames[1].minY], "both on the second line, though the first fits the first")
            XCTAssertLessThan(manager.caret(start - 1).x + frames[0].width, 290)
            view.setSelectedRange(NSRange(location: start, length: 0))
            var stops: [Int] = []
            for _ in 0..<2 { view.moveRight(nil); stops.append(view.selectedRange().location) }
            for _ in 0..<2 { view.moveLeft(nil); stops.append(view.selectedRange().location) }
            XCTAssertEqual(stops, [start + n, start + 2 * n, start + n, start], "\(drawing)")
        }
    }

    func testBidirectionalTextAroundIt() throws {
        for drawing in drawings {
            for (before, after) in [("שלום ", " עולם"), ("مرحبا ", " بالعالم")] {
                let text = before + "[Assistant quote" + destination + after
                let start = (before as NSString).length, end = start + ("[Assistant quote" + destination as NSString).length
                let (view, _, manager) = editor(drawing, text)
                XCTAssertEqual(manager.lines, 1)
                let frame = try XCTUnwrap(manager.attachments.first)
                XCTAssertTrue(red(view, at: center(frame)), before)
                // Right to left: the start at the pill's right edge, every later offset at its left.
                XCTAssertEqual(manager.caret(start).x, frame.maxX, accuracy: 0.5, before)
                for p in start + 1...end { XCTAssertEqual(manager.caret(p).x, frame.minX, accuracy: 0.5, before) }
                XCTAssertGreaterThan(manager.caret(1).x, frame.maxX, "the first word right of it")
                XCTAssertLessThan(manager.caret(text.utf16.count - 1).x, frame.minX, "the last left of it")
            }
        }
    }

    func testNativeUndoAndAnInputMethodBesideIt() throws {
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            view.setSelectedRange(NSRange(location: b, length: 0))
            view.insertText(" hi", replacementRange: view.selectedRange())
            probe.restyle(view.textStorage!)
            view.breakUndoCoalescing()
            XCTAssertEqual(view.string, "Ask about [Assistant quote" + destination + " hi and more.")
            view.undoManager!.undo()
            probe.restyle(view.textStorage!)
            XCTAssertEqual(view.string, source)
            // The whole atom removed in one native replacement, then undone.
            view.insertText("", replacementRange: NSRange(location: a, length: b - a))
            probe.restyle(view.textStorage!)
            view.breakUndoCoalescing()
            XCTAssertEqual(view.string, "Ask about  and more.")
            settle(view)
            XCTAssertEqual(manager.attachments.count, 0)
            if drawing == .view { XCTAssertEqual(probe.live(in: view).count, 0, "its view left with it") }
            view.undoManager!.undo()
            probe.restyle(view.textStorage!)
            XCTAssertEqual(view.string, source)
            let back = try XCTUnwrap(manager.attachments.first)
            XCTAssertTrue(red(view, at: center(back)), "undo brings the pill back: \(drawing)")
            // Composing beside it, at either edge: the pill stays laid out and drawn.
            for (at, moves) in [(b, false), (a, true)] {
                view.setSelectedRange(NSRange(location: at, length: 0))
                view.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
                XCTAssertTrue(view.hasMarkedText())
                settle(view)
                let composing = try XCTUnwrap(manager.attachments.first)
                XCTAssertEqual(composing.minX > back.minX + 1, moves, "composed text at \(at) is laid out: \(drawing)")
                XCTAssertTrue(red(view, at: center(composing)))
                view.unmarkText()
                probe.restyle(view.textStorage!)
            }
            XCTAssertEqual(view.string, "Ask about に[Assistant quote" + destination + "に and more.")
        }
    }

    func testAFaceUpdateRedrawsIt() throws {
        let key = "[Assistant quote" + destination
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            let before = try XCTUnwrap(manager.attachments.first), after = manager.caret(b + 1).x
            let first = probe.live(in: view).first
            probe.labels[key] = "Q"
            probe.restyle(view.textStorage!)
            settle(view)
            let narrow = try XCTUnwrap(manager.attachments.first)
            XCTAssertLessThan(narrow.width, before.width, "the new label is measured")
            XCTAssertEqual(manager.caret(b + 1).x, after - (before.width - narrow.width), accuracy: 0.5, "the text after it moved")
            XCTAssertTrue(red(view, at: center(narrow)))
            XCTAssertFalse(red(view, at: CGPoint(x: (narrow.maxX + before.maxX) / 2, y: narrow.midY)), "nothing is left where the wider pill was")
            if drawing == .view {
                XCTAssertEqual(probe.live(in: view).first?.label, "Q", "a new view draws it")
                XCTAssertFalse(first?.isDescendant(of: view) ?? true, "the old view left the text view")
            }
        }
    }

    /// A pill across restyles. AppKit's TextKit asks for a view provider each
    /// time it lays a paragraph out and keeps every provider it was handed,
    /// with its view, while the text view lives, though only the newest is in
    /// the hierarchy; an attachment in the storage, not the delegate's
    /// paragraph, does the same (the third case). A picture leaves nothing
    /// behind: the attachments restyles replaced are freed. Measured once and
    /// not kept as a case: a provider the attachment hands back again keeps
    /// its first location, so it cannot follow the text.
    func testWhatARestyleLeavesBehind() throws {
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            var allocated: [Int] = []
            for i in 0..<120 {
                autoreleasepool {
                    if i % 2 == 0 { view.insertText("x", replacementRange: NSRange(location: 0, length: 0)) }
                    probe.restyle(view.textStorage!)
                    settle(view)
                }
                if i % 30 == 29 { allocated.append(probe.attachments.filter { $0.object != nil }.count) }
            }
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertEqual(frame.minX, manager.caret(60 + a).x, accuracy: 0.5)
            XCTAssertTrue(red(view, at: center(frame)), "\(drawing)")
            switch drawing {
            case .view:
                XCTAssertEqual(probe.live(in: view).count, 1)
                // A tripwire: if AppKit stops keeping the providers, this fails and §5.1's record is out of date.
                XCTAssertGreaterThan(probe.allocated, 100, "AppKit kept the providers")
            case .image:
                XCTAssertLessThanOrEqual(allocated.last!, allocated.first! + 2, "the replaced attachments were freed: \(allocated)")
            }
            print("probe(mac) \(drawing) after 120 restyles: \(probe.paragraphs) paragraphs, \(probe.providers) providers, \(probe.allocated) views allocated (\(probe.live(in: view).count) in the text view), attachments allocated every 30 restyles \(allocated) of \(probe.attachments.count)")
        }
        // One attachment in the storage itself, no delegate paragraph: each re-layout
        // of its paragraph (an attribute change, an edit before it) asks for a new provider.
        let (view, probe, _) = editor(.view, "x", rich: true)
        let storage = view.textStorage!
        storage.append(NSAttributedString(attachment: PillAttachment(label: "Assistant quote", probe: probe)))
        for i in 0..<120 {
            autoreleasepool {
                storage.beginEditing()
                storage.addAttribute(.font, value: font, range: NSRange(location: 0, length: storage.length))
                storage.endEditing()
                if i % 2 == 0 { view.insertText("y", replacementRange: NSRange(location: 0, length: 0)) }
                settle(view)
            }
        }
        XCTAssertEqual(probe.live(in: view).count, 1)
        XCTAssertGreaterThan(probe.allocated, 100, "AppKit kept the providers of an attachment in the storage too (a tripwire, as above)")
        print("probe(mac) attachment in the storage after 120 re-layouts: \(probe.providers) providers, \(probe.allocated) views allocated (\(probe.live(in: view).count) in the text view)")
    }

    /// D8 through the AX API, from a thread of its own as VoiceOver reads it
    /// (AccessibilityClientMacTests): the atom is a button child; the text
    /// view's value reads its label; and the text view's own line and range
    /// answers are those of the same view without atom children.
    func testAtomElementsLeaveTheTextViewsOwnAnswers() throws {
        guard AXIsProcessTrusted() else { throw XCTSkip("this process is not trusted for accessibility") }
        if !Self.launched {
            Self.launched = true
            NSApplication.shared.setActivationPolicy(.accessory)
            NSApp.finishLaunching()
        }
        let text = source + "\nA second line."
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing, text)
            let (bare, _, _) = editor(drawing, text)
            let withAtoms = "with-atoms-\(drawing)", plain = "bare-\(drawing)"
            view.setAccessibilityIdentifier(withAtoms)
            bare.setAccessibilityIdentifier(plain)
            let frame = try XCTUnwrap(manager.attachments.first)
            let element = AtomElement()
            element.setAccessibilityRole(.button)
            element.setAccessibilityLabel("Assistant quote")
            element.setAccessibilityHelp("The editor draws every source character")
            element.setAccessibilityParent(view)
            let o = view.textContainerOrigin
            element.setAccessibilityFrameInParentSpace(frame.offsetBy(dx: o.x, dy: o.y))
            element.press = { view.pressed.append("press") }
            view.atomElements = [element]
            for window in windows { window.orderFront(nil) }

            let client = AXReader()
            let attributes = ["AXRole", "AXNumberOfCharacters", "AXSelectedTextRange", "AXInsertionPointLineNumber", "AXVisibleCharacterRange"]
            let lines: [(String, Int)] = [("AXLineForIndex", 60), ("AXRangeForLine", 1), ("AXRangeForIndex", 3), ("AXStyleRangeForIndex", 3)]
            let with = try XCTUnwrap(client.read(withAtoms, attributes, lines))
            let without = try XCTUnwrap(client.read(plain, attributes, lines))
            for name in attributes + lines.map(\.0) { XCTAssertEqual(with[name], without[name], "\(name), \(drawing)") }
            XCTAssertEqual(with["AXRole"], "AXTextArea")
            XCTAssertEqual(with["AXRangeForLine"], "{\(text.utf16.count - 14), 14}", "line reading is the source's")
            XCTAssertEqual(client.value(withAtoms, "AXValue"), "Ask about Assistant quote and more.\nA second line.")
            XCTAssertEqual(client.string(withAtoms, NSRange(location: a, length: b - a + 4)), "Assistant quote and")
            let names = ["AXValue", "AXSelectedTextRange", "AXFocused"]
            XCTAssertEqual(client.settable(withAtoms, names), client.settable(plain, names))
            // A client's selection is a source selection, and native editing goes on.
            XCTAssertTrue(client.select(withAtoms, NSRange(location: b, length: 0)))
            XCTAssertEqual(view.selectedRange(), NSRange(location: b, length: 0))
            view.insertText("!", replacementRange: view.selectedRange())
            probe.restyle(view.textStorage!)
            XCTAssertEqual(view.string, source.replacingOccurrences(of: destination, with: destination + "!") + "\nA second line.")
            // The atom, as a client reaches it: one button, the pill's view no element of its own.
            let children = client.children(withAtoms)
            XCTAssertEqual(children.filter { $0["AXRole"] == "AXButton" }.count, 1, "\(drawing): \(children)")
            let child = try XCTUnwrap(children.first { $0["AXRole"] == "AXButton" })
            XCTAssertEqual(child["AXDescription"], "Assistant quote")
            XCTAssertEqual(child["AXHelp"], "The editor draws every source character")
            XCTAssertNotNil(child["AXFrame"])
            XCTAssertTrue(client.pressButton(withAtoms))
            XCTAssertEqual(view.pressed, ["press"])
            print("probe(mac) \(drawing) ax: \(with); children \(children)")
            for window in windows { window.orderOut(nil) }
        }
    }
}

/// The AX API, read on a thread of its own while the main run loop serves.
private final class AXReader: @unchecked Sendable {
    private let app = AXUIElementCreateApplication(getpid())
    private let queue = DispatchQueue(label: "ax-probe")

    private func onClient<T>(_ body: @escaping @Sendable () -> T) -> T {
        var result: T?
        let done = DispatchSemaphore(value: 0)
        queue.async { result = body(); done.signal() }
        while done.wait(timeout: .now()) == .timedOut { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.005)) }
        return result!
    }
    private static func value(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
        var value: CFTypeRef?
        return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
    }
    private static func find(_ identifier: String, in element: AXUIElement, depth: Int = 0) -> AXUIElement? {
        if value(element, "AXIdentifier") as? String == identifier { return element }
        guard depth < 40 else { return nil }
        for child in value(element, "AXChildren") as? [AXUIElement] ?? [] {
            if let hit = find(identifier, in: child, depth: depth + 1) { return hit }
        }
        return nil
    }
    private static func text(_ v: CFTypeRef?) -> String? {
        guard let v else { return nil }
        if CFGetTypeID(v) == AXValueGetTypeID() {
            let ax = v as! AXValue
            var range = CFRange()
            if AXValueGetType(ax) == .cfRange, AXValueGetValue(ax, .cfRange, &range) { return "{\(range.location), \(range.length)}" }
        }
        return "\(v)"
    }
    func read(_ id: String, _ names: [String], _ pairs: [(String, Int)]) -> [String: String]? {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app) else { return nil }
            var out: [String: String] = [:]
            for name in names { out[name] = Self.text(Self.value(element, name)) }
            for (name, n) in pairs {
                var value: CFTypeRef?
                let parameter = n as CFNumber
                if AXUIElementCopyParameterizedAttributeValue(element, name as CFString, parameter, &value) == .success { out[name] = Self.text(value) }
            }
            return out
        }
    }
    func value(_ id: String, _ name: String) -> String? {
        let app = app
        return onClient { Self.find(id, in: app).flatMap { Self.value($0, name) as? String } }
    }
    func string(_ id: String, _ range: NSRange) -> String? {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app) else { return nil }
            var r = CFRange(location: range.location, length: range.length), value: CFTypeRef?
            let parameter = AXValueCreate(.cfRange, &r)!
            return AXUIElementCopyParameterizedAttributeValue(element, "AXStringForRange" as CFString, parameter, &value) == .success ? value as? String : nil
        }
    }
    func settable(_ id: String, _ names: [String]) -> [Bool] {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app) else { return [] }
            return names.map { var s: DarwinBoolean = false; return AXUIElementIsAttributeSettable(element, $0 as CFString, &s) == .success && s.boolValue }
        }
    }
    func select(_ id: String, _ range: NSRange) -> Bool {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app) else { return false }
            var r = CFRange(location: range.location, length: range.length)
            return AXUIElementSetAttributeValue(element, "AXSelectedTextRange" as CFString, AXValueCreate(.cfRange, &r)!) == .success
        }
    }
    func children(_ id: String) -> [[String: String]] {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app) else { return [] }
            return (Self.value(element, "AXChildren") as? [AXUIElement] ?? []).map { child in
                var out: [String: String] = [:]
                for name in ["AXRole", "AXDescription", "AXHelp", "AXFrame"] { out[name] = Self.text(Self.value(child, name)) }
                return out
            }
        }
    }
    func pressButton(_ id: String) -> Bool {
        let app = app
        return onClient {
            guard let element = Self.find(id, in: app),
                  let button = (Self.value(element, "AXChildren") as? [AXUIElement] ?? []).first(where: { Self.value($0, "AXRole") as? String == "AXButton" }) else { return false }
            return AXUIElementPerformAction(button, "AXPress" as CFString) == .success
        }
    }
}
#endif

#if os(iOS)
/// The text view, with D8's value: each atom reads as its label.
private final class ProbeTextViewIOS: UITextView {
    weak var probe: Probe?
    override var accessibilityValue: String? {
        get {
            let out = NSMutableString(string: textStorage.string)
            textStorage.enumerateAttribute(.probeAtom, in: NSRange(location: 0, length: textStorage.length), options: .reverse) { value, run, _ in
                guard let key = (value as? Mark)?.source else { return }
                out.replaceCharacters(in: run, with: probe?.labels[key] ?? Probe.linkText(key))
            }
            return out as String
        }
        set { super.accessibilityValue = newValue }
    }
}

/// The same on UIKit, in a simulator: bun host/apple/build.mjs --test --ios
final class InlineAtomProbeIOSTests: XCTestCase {
    private var windows: [UIWindow] = []

    override func tearDown() {
        for window in windows { window.isHidden = true }
        windows = []
        super.tearDown()
    }

    private func editor(_ drawing: Probe.Drawing, _ text: String = source, width: CGFloat = 500) -> (ProbeTextViewIOS, Probe, NSTextLayoutManager) {
        let probe = Probe(drawing)
        let view = ProbeTextViewIOS(usingTextLayoutManager: true)
        view.probe = probe
        view.frame = CGRect(x: 0, y: 0, width: width, height: 200)
        view.font = font
        view.isScrollEnabled = false
        (view.textLayoutManager!.textContentManager as! NSTextContentStorage).delegate = probe
        view.text = text
        probe.restyle(view.textStorage)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: max(width, 400), height: 300))
        window.addSubview(view)
        window.makeKeyAndVisible()
        windows.append(window)
        XCTAssertTrue(view.becomeFirstResponder())
        settle(view)
        return (view, probe, view.textLayoutManager!)
    }

    private func settle(_ view: UITextView) {
        let manager = view.textLayoutManager!
        manager.ensureLayout(for: manager.documentRange)
        view.layoutIfNeeded()
        manager.textViewportLayoutController.layoutViewport()
        view.layoutIfNeeded()
    }

    /// Whether the pixel at `p` (container coordinates) is the pill's red.
    private func red(_ view: UITextView, at p: CGPoint) -> Bool {
        settle(view)
        let point = CGPoint(x: p.x + view.textContainerInset.left, y: p.y + view.textContainerInset.top)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let image = UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1), format: format).image { context in
            context.cgContext.translateBy(x: -point.x.rounded(.down), y: -point.y.rounded(.down))
            view.layer.render(in: context.cgContext)
        }
        guard let cg = image.cgImage, let data = cg.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return false }
        // BGRA or RGBA, premultiplied: the pill's pure red is one channel high and the other two low.
        let info = cg.bitmapInfo.rawValue & CGBitmapInfo.byteOrderMask.rawValue
        let (r, g, bl) = info == CGBitmapInfo.byteOrder32Little.rawValue ? (bytes[2], bytes[1], bytes[0]) : (bytes[0], bytes[1], bytes[2])
        return r > 215 && g < 100 && bl < 100
    }

    private func center(_ r: CGRect) -> CGPoint { CGPoint(x: r.midX, y: r.midY) }

    func testTheAttachmentLaysOutAndDrawsAndTheSourceStays() throws {
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            XCTAssertEqual(view.text, source, "the storage keeps the source")
            XCTAssertGreaterThan(probe.paragraphs, 0)
            XCTAssertEqual(manager.lines, 1)
            XCTAssertEqual(manager.attachments.count, 1, "\(drawing)")
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertTrue(red(view, at: center(frame)), "the pill is drawn: \(drawing)")
            switch drawing {
            case .view:
                XCTAssertEqual(probe.live(in: view).count, 1, "its view is in the text view")
            case .image:
                XCTAssertEqual(probe.providers, 0)
                XCTAssertGreaterThan(probe.images, 0)
            }
            XCTAssertEqual(manager.caret(a).x, frame.minX, accuracy: 0.5)
            for p in a + 1...b { XCTAssertEqual(manager.caret(p).x, frame.maxX, accuracy: 0.5, "offset \(p), \(drawing)") }
            let caret = { (p: Int) in view.caretRect(for: view.position(from: view.beginningOfDocument, offset: p)!).midX - view.textContainerInset.left }
            XCTAssertEqual(caret(b), frame.maxX, accuracy: 0.5, "UIKit's caret, on the pixel grid")
            XCTAssertEqual(caret(a + 1), frame.maxX, accuracy: 0.5)
            let plain = editor(drawing, "Ask about Assistant quote and more.").2
            XCTAssertEqual(manager.segments(0, 3).first!.height, plain.segments(0, 3).first!.height, accuracy: 0.01)
            print("probe(ios) \(drawing): pill \(frame)")
        }
    }

    func testTheSelectionAndRangeFrameCoverThePill() throws {
        for drawing in drawings {
            let (view, _, manager) = editor(drawing)
            let frame = try XCTUnwrap(manager.attachments.first)
            let box = manager.segments(a, b).reduce(CGRect.null) { $0.union($1) }
            XCTAssertEqual(box.minX, frame.minX, accuracy: 0.5)
            XCTAssertEqual(box.width, frame.width, accuracy: 0.5)
            view.selectedRange = NSRange(location: a - 3, length: b - a + 6)
            let rects = view.selectionRects(for: view.selectedTextRange!).map(\.rect).filter { $0.width > 0 }
            let selected = rects.reduce(CGRect.null) { $0.union($1) }
            XCTAssertLessThanOrEqual(selected.minX - view.textContainerInset.left, frame.minX)
            XCTAssertGreaterThanOrEqual(selected.maxX - view.textContainerInset.left, frame.maxX)
            XCTAssertTrue(red(view, at: center(frame)), "\(drawing)")
        }
    }

    func testACitationOfThousandsOfCodeUnits() throws {
        let text = long(6000), start = ("Cite " as NSString).length, end = text.utf16.count - (" here." as NSString).length
        for drawing in drawings {
            _ = editor(drawing) // TextKit's first attachment of this kind, out of the timing
            let began = Date()
            let (view, _, manager) = editor(drawing, text)
            let first = Date().timeIntervalSince(began)
            XCTAssertEqual(manager.lines, 1)
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertEqual(manager.caret(start + 1).x, frame.maxX, accuracy: 0.5)
            XCTAssertEqual(manager.caret(end).x, frame.maxX, accuracy: 0.5)
            XCTAssertTrue(red(view, at: center(frame)))
            let edited = Date()
            view.selectedRange = NSRange(location: end, length: 0)
            view.insertText("!")
            settle(view)
            print("probe(ios) \(drawing), \(end - start) code units: first layout \(Int(first * 1000)) ms, an edit beside it \(Int(Date().timeIntervalSince(edited) * 1000)) ms")
        }
    }

    func testThePillWrapsWholeAndALongLabelFitsTheLine() throws {
        let before = "one two three four five six "
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing, before + "[Assistant quote" + destination, width: 220)
            XCTAssertEqual(manager.lines, 2)
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertGreaterThan(frame.minY, manager.caret(0).y, "on the second line")
            XCTAssertEqual(frame.minX, manager.caret(0).x, accuracy: 0.5, "at its start")
            XCTAssertTrue(red(view, at: center(frame)))
            probe.labels["[Assistant quote" + destination] = String(repeating: "a very long file name ", count: 6)
            probe.restyle(view.textStorage)
            settle(view)
            let wide = try XCTUnwrap(manager.attachments.first)
            let line = try XCTUnwrap(probe.proposed.last)
            XCTAssertEqual(wide.width, line.width, accuracy: 0.5)
            XCTAssertEqual(manager.lines, 2)
            XCTAssertTrue(red(view, at: center(wide)))
        }
    }

    func testBidirectionalTextAroundIt() throws {
        for drawing in drawings {
            for (before, after) in [("שלום ", " עולם"), ("مرحبا ", " بالعالم")] {
                let text = before + "[Assistant quote" + destination + after
                let start = (before as NSString).length, end = start + ("[Assistant quote" + destination as NSString).length
                let (view, _, manager) = editor(drawing, text)
                XCTAssertEqual(manager.lines, 1)
                let frame = try XCTUnwrap(manager.attachments.first)
                XCTAssertTrue(red(view, at: center(frame)), before)
                XCTAssertEqual(manager.caret(start).x, frame.maxX, accuracy: 0.5, before)
                for p in start + 1...end { XCTAssertEqual(manager.caret(p).x, frame.minX, accuracy: 0.5, before) }
            }
        }
    }

    /// As on the Mac: word joiners keep two adjacent atoms on one line.
    func testAdjacentAtomsWrapTogether() throws {
        let atom = "[Assistant quote" + destination
        for drawing in drawings {
            let (_, _, manager) = editor(drawing, "one two three " + atom + atom, width: 300)
            let frames = manager.attachments
            XCTAssertEqual(frames.count, 2)
            XCTAssertEqual(manager.lines, 2)
            XCTAssertEqual(frames[0].minY, frames[1].minY, accuracy: 0.5, "\(drawing)")
        }
    }

    func testNativeUndoAndAnInputMethodBesideIt() throws {
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            let undo = try XCTUnwrap(view.undoManager)
            let settled = {
                let deadline = Date().addingTimeInterval(2)
                while undo.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
            }
            // The whole atom removed in one native deletion, then undone.
            // UIKit's smart delete takes one of the two spaces with it.
            view.selectedRange = NSRange(location: a, length: b - a)
            view.deleteBackward()
            probe.restyle(view.textStorage)
            XCTAssertEqual(view.text, "Ask about and more.")
            settle(view)
            XCTAssertEqual(manager.attachments.count, 0)
            if drawing == .view { XCTAssertEqual(probe.live(in: view).count, 0) }
            settled()
            undo.undo()
            probe.restyle(view.textStorage)
            XCTAssertEqual(view.text, source, "\(drawing)")
            let back = try XCTUnwrap(manager.attachments.first)
            XCTAssertTrue(red(view, at: center(back)), "undo brings the pill back: \(drawing)")
            for (at, moves) in [(b, false), (a, true)] {
                view.selectedRange = NSRange(location: at, length: 0)
                view.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
                XCTAssertNotNil(view.markedTextRange)
                settle(view)
                let composing = try XCTUnwrap(manager.attachments.first)
                XCTAssertEqual(composing.minX > back.minX + 1, moves, "composed text at \(at): \(drawing)")
                XCTAssertTrue(red(view, at: center(composing)))
                view.unmarkText()
                probe.restyle(view.textStorage)
            }
            XCTAssertEqual(view.text, "Ask about に[Assistant quote" + destination + "に and more.")
        }
    }

    func testAFaceUpdateRedrawsIt() throws {
        let key = "[Assistant quote" + destination
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            let before = try XCTUnwrap(manager.attachments.first), after = manager.caret(b + 1).x
            probe.labels[key] = "Q"
            probe.restyle(view.textStorage)
            settle(view)
            let narrow = try XCTUnwrap(manager.attachments.first)
            XCTAssertLessThan(narrow.width, before.width)
            XCTAssertEqual(manager.caret(b + 1).x, after - (before.width - narrow.width), accuracy: 0.5, "the text after it moved")
            XCTAssertTrue(red(view, at: center(narrow)))
            XCTAssertFalse(red(view, at: CGPoint(x: (narrow.maxX + before.maxX) / 2, y: narrow.midY)), "nothing is left where the wider pill was")
            if drawing == .view { XCTAssertEqual(probe.live(in: view).map(\.label), ["Q"]) }
        }
    }

    /// UIKit, unlike AppKit, frees the view providers it no longer shows.
    func testWhatARestyleLeavesBehind() throws {
        for drawing in drawings {
            let (view, probe, manager) = editor(drawing)
            var allocated: [Int] = []
            for i in 0..<120 {
                autoreleasepool {
                    if i % 2 == 0 { view.selectedRange = NSRange(location: 0, length: 0); view.insertText("x") }
                    probe.restyle(view.textStorage)
                    settle(view)
                }
                if i % 30 == 29 { allocated.append(probe.attachments.filter { $0.object != nil }.count) }
            }
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertEqual(frame.minX, manager.caret(60 + a).x, accuracy: 0.5)
            XCTAssertTrue(red(view, at: center(frame)), "\(drawing)")
            if drawing == .view {
                XCTAssertEqual(probe.live(in: view).count, 1)
                XCTAssertLessThanOrEqual(probe.allocated, 10, "the replaced views were freed")
            }
            XCTAssertLessThanOrEqual(allocated.last!, allocated.first! + 2, "the replaced attachments were freed: \(allocated)")
            print("probe(ios) \(drawing) after 120 restyles: \(probe.providers) providers, \(probe.allocated) views allocated (\(probe.live(in: view).count) in the text view), attachments allocated every 30 restyles \(allocated) of \(probe.attachments.count)")
        }
    }

    /// D8 on UIKit, with its accessibility runtime (the hosted run). A text
    /// view is itself an element, and UIKit's documented rule is that an
    /// element's own `accessibilityElements` are not visited, so the atom
    /// element is placed beside the text view, in its container's elements.
    func testAtomElementsBesideTheTextView() throws {
        guard Self.axRuntimeLoaded else { throw missing("accessibility", "UIKit's accessibility runtime is not loaded") }
        for drawing in drawings {
            let (view, _, manager) = editor(drawing)
            let (bare, _, _) = editor(drawing)
            let frame = try XCTUnwrap(manager.attachments.first)
            XCTAssertTrue(view.isAccessibilityElement)
            let traits = bare.accessibilityTraits
            XCTAssertEqual(view.accessibilityValue, "Ask about Assistant quote and more.")
            let atom = UIAccessibilityElement(accessibilityContainer: view.superview!)
            atom.accessibilityLabel = "Assistant quote"
            atom.accessibilityTraits = .button
            atom.accessibilityFrameInContainerSpace = frame.offsetBy(dx: view.textContainerInset.left, dy: view.textContainerInset.top)
            view.superview!.accessibilityElements = [view, atom]
            XCTAssertTrue(view.isAccessibilityElement, "the text view stays an element")
            XCTAssertEqual(view.accessibilityTraits, traits, "with its own traits")
            XCTAssertEqual(view.superview!.accessibilityElementCount(), 2)
            XCTAssertEqual(atom.accessibilityFrame.width, frame.width, accuracy: 0.5)
            // Editing goes on with the atom beside it.
            view.selectedRange = NSRange(location: b, length: 0)
            view.insertText("!")
            XCTAssertEqual(view.text, source.replacingOccurrences(of: destination, with: destination + "!"))
            // Given elements of its own, the text view stays an element, which UIKit does not descend into.
            view.accessibilityElements = [atom]
            XCTAssertTrue(view.isAccessibilityElement, "\(drawing)")
            print("probe(ios) \(drawing) ax: text view element \(view.isAccessibilityElement), traits \(view.accessibilityTraits.rawValue), value \(view.accessibilityValue ?? "nil"), container elements \(view.superview!.accessibilityElementCount())")
        }
    }

    /// UIKit loads the code that answers accessibility only for an assistive
    /// technology or automation; a title-only button tells which this is.
    /// `Presenter.axRuntimeLoaded` says the same; the probe uses no ExactKit,
    /// so that it records TextKit alone.
    private static var axRuntimeLoaded: Bool {
        let probe = UIButton(type: .system)
        probe.setTitle("probe", for: .normal)
        return probe.accessibilityLabel == "probe"
    }
}
#endif
